use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

const SESSION_START_PY: &str = r#"#!/usr/bin/env python3
import json

print(json.dumps({
    "hookSpecificOutput": {
        "hookEventName": "SessionStart",
        "additionalContext": (
            "For Bash commands that may produce large output, run them via "
            "'CTK_AI_CLI_NAME=codex ctk proxy -- <command>' to compact output "
            "before returning. Examples: 'CTK_AI_CLI_NAME=codex ctk proxy -- "
            "git diff', 'CTK_AI_CLI_NAME=codex ctk proxy -- rg ERROR .', "
            "'CTK_AI_CLI_NAME=codex ctk proxy -- cargo test'."
        )
    }
}))
"#;

const PRE_BASH_PY: &str = r#"#!/usr/bin/env python3
import json
import sys

payload = json.load(sys.stdin)
cmd = payload.get("tool_input", {}).get("command", "").strip()
normalized_cmd = cmd

if normalized_cmd.startswith("CTK_AI_CLI_NAME="):
    parts = normalized_cmd.split(None, 1)
    normalized_cmd = parts[1].lstrip() if len(parts) == 2 else ""

# Don't block if already wrapped or is ctk command
if normalized_cmd.startswith("ctk"):
    sys.exit(0)

# Block and require ctk proxy for all other commands
ctk_cmd = f"CTK_AI_CLI_NAME=codex ctk proxy -- {cmd}"
print(json.dumps({
    "decision": "block",
    "reason": f"Please run via CTK: {ctk_cmd}"
}))
sys.exit(0)
"#;

const POST_BASH_PY: &str = r#"#!/usr/bin/env python3
import json
import sys

payload = json.load(sys.stdin)
cmd = payload.get("tool_input", {}).get("command", "")
tool_response = payload.get("tool_response", "")

text = tool_response if isinstance(tool_response, str) else json.dumps(tool_response)
too_big = len(text) > 12000

if too_big and not cmd.startswith("ctk proxy"):
    ctk_cmd = f"ctk proxy -- {cmd}"
    print(json.dumps({
        "decision": "block",
        "reason": "Output was large. Please rerun via CTK.",
        "hookSpecificOutput": {
            "hookEventName": "PostToolUse",
            "additionalContext": f"Rerun with: {ctk_cmd}"
        }
    }))
    sys.exit(0)

sys.exit(0)
"#;

pub fn install_hooks(home: &str) -> Result<Vec<PathBuf>> {
    let codex_dir = PathBuf::from(home).join(".codex");
    let hooks_dir = codex_dir.join("hooks");

    fs::create_dir_all(&hooks_dir)
        .with_context(|| format!("failed to create {}", hooks_dir.display()))?;

    let session_start = hooks_dir.join("ctk_session_start.py");
    let pre_bash = hooks_dir.join("ctk_pre_bash.py");
    let post_bash = hooks_dir.join("ctk_post_bash.py");

    fs::write(&session_start, SESSION_START_PY)?;
    fs::write(&pre_bash, PRE_BASH_PY)?;
    fs::write(&post_bash, POST_BASH_PY)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for file in [&session_start, &pre_bash, &post_bash] {
            let mut perms = fs::metadata(file)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(file, perms)?;
        }
    }

    let hooks_json = codex_dir.join("hooks.json");
    let config = format!(
        r#"{{
  "hooks": {{
    "SessionStart": [
      {{
        "matcher": "startup|resume",
        "hooks": [
          {{
            "type": "command",
            "command": "/usr/bin/python3 {}/ctk_session_start.py",
            "statusMessage": "Loading CTK policy"
          }}
        ]
      }}
    ],
    "PreToolUse": [
      {{
        "matcher": "Bash",
        "hooks": [
          {{
            "type": "command",
            "command": "/usr/bin/python3 {}/ctk_pre_bash.py",
            "statusMessage": "Checking Bash command for CTK"
          }}
        ]
      }}
    ],
    "PostToolUse": [
      {{
        "matcher": "Bash",
        "hooks": [
          {{
            "type": "command",
            "command": "/usr/bin/python3 {}/ctk_post_bash.py",
            "statusMessage": "Reviewing Bash output for CTK"
          }}
        ]
      }}
    ]
  }}
}}
"#,
        hooks_dir.display(),
        hooks_dir.display(),
        hooks_dir.display()
    );

    fs::write(&hooks_json, config)?;

    let config_toml = codex_dir.join("config.toml");
    let mut config_content = if config_toml.exists() {
        fs::read_to_string(&config_toml)?
    } else {
        String::new()
    };

    if !config_content.contains("codex_hooks") {
        if !config_content.contains("[features]") {
            config_content.push_str("\n[features]\n");
        }
        if !config_content.contains("codex_hooks = true") {
            let insert_pos = config_content.find("[features]").unwrap() + "[features]".len();
            config_content.insert_str(insert_pos, "\ncodex_hooks = true");
        }
        fs::write(&config_toml, config_content)?;
    }

    Ok(vec![hooks_json, config_toml])
}

pub fn uninstall_hooks(home: &str) -> Result<()> {
    let codex_dir = PathBuf::from(home).join(".codex");
    let hooks_dir = codex_dir.join("hooks");

    if hooks_dir.exists() {
        for file in [
            "ctk_session_start.py",
            "ctk_pre_bash.py",
            "ctk_post_bash.py",
        ] {
            let path = hooks_dir.join(file);
            if path.exists() {
                fs::remove_file(path)?;
            }
        }
    }

    let hooks_json = codex_dir.join("hooks.json");
    if hooks_json.exists() {
        fs::remove_file(hooks_json)?;
    }

    Ok(())
}
