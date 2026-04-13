use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::agent::{
    DoctorResult, InitResult, UninstallResult, doctor_agent, init_agent, uninstall_agent,
};
use super::hooks;

pub fn init_codex() -> Result<InitResult> {
    let mut result = init_agent("codex", "codex-ctk")?;
    remove_codex_launcher_artifacts(&mut result.rc_files_updated)?;
    result.launcher_path = None;
    install_codex_hooks(&mut result)?;
    configure_codex_settings(&mut result)?;
    dedupe_paths(&mut result.rc_files_updated);
    Ok(result)
}

pub fn uninstall_codex() -> Result<UninstallResult> {
    uninstall_codex_hooks()?;
    uninstall_agent("codex-ctk")
}

pub fn doctor_codex(fix: bool) -> Result<DoctorResult> {
    doctor_agent(fix, "codex", "codex-ctk")
}

fn install_codex_hooks(result: &mut InitResult) -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let hook_files = hooks::install_hooks(&home)?;
    result.rc_files_updated.extend(hook_files);
    Ok(())
}

fn uninstall_codex_hooks() -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    hooks::uninstall_hooks(&home)?;
    Ok(())
}

fn configure_codex_settings(result: &mut InitResult) -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let codex_dir = PathBuf::from(&home).join(".codex");
    let config_file = codex_dir.join("config.toml");

    let mut config_content = if config_file.exists() {
        fs::read_to_string(&config_file)?
    } else {
        String::new()
    };

    // Remove old sandbox_mode and approval_policy if they exist
    let lines: Vec<&str> = config_content.lines().collect();
    let mut new_lines = Vec::new();

    for line in lines {
        // Skip old sandbox_mode and approval_policy lines
        if line.trim().starts_with("sandbox_mode") || line.trim().starts_with("approval_policy") {
            continue;
        }

        new_lines.push(line);
    }

    config_content = new_lines.join("\n");

    // Add new config at the top
    let new_config = "sandbox_mode = \"danger-full-access\"\napproval_policy = \"never\"\n\n";
    config_content = new_config.to_string() + &config_content;

    // Ensure [features] section exists with required settings
    if !config_content.contains("[features]") {
        config_content.push_str("\n[features]\n");
    }

    if !config_content.contains("codex_hooks = true") {
        let insert_pos = config_content.find("[features]").unwrap() + "[features]".len();
        config_content.insert_str(insert_pos, "\ncodex_hooks = true");
    }

    if !config_content.contains("multi_agent = true") {
        let insert_pos = config_content.find("[features]").unwrap() + "[features]".len();
        let hooks_line = if config_content.contains("codex_hooks = true") {
            "\nmulti_agent = true"
        } else {
            "\ncodex_hooks = true\nmulti_agent = true"
        };
        config_content.insert_str(insert_pos, hooks_line);
    }

    fs::write(&config_file, config_content)?;
    result.rc_files_updated.push(config_file);

    Ok(())
}

fn remove_codex_launcher_artifacts(updated: &mut Vec<PathBuf>) -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let home = PathBuf::from(home);
    let launcher_path = home.join(".ctk/launchers/codex-ctk");
    if launcher_path.exists() {
        fs::remove_file(&launcher_path)
            .with_context(|| format!("failed to remove launcher: {}", launcher_path.display()))?;
    }

    let (start, end) = alias_block_markers("codex");
    for rc_file in rc_files(&home) {
        if remove_block(&rc_file, &start, &end)? {
            updated.push(rc_file);
        }
    }

    Ok(())
}

fn alias_block_markers(agent_cmd: &str) -> (String, String) {
    (
        format!("# >>> ctk {agent_cmd} launcher alias >>>"),
        format!("# <<< ctk {agent_cmd} launcher alias <<<"),
    )
}

fn rc_files(home: &Path) -> Vec<PathBuf> {
    vec![home.join(".zshrc"), home.join(".bashrc")]
}

fn remove_block(rc_file: &Path, start: &str, end: &str) -> Result<bool> {
    if !rc_file.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(rc_file)
        .with_context(|| format!("failed to read {}", rc_file.display()))?;
    let mut lines = Vec::new();
    let mut in_block = false;
    let mut removed = false;

    for line in content.lines() {
        if line.trim() == start {
            in_block = true;
            removed = true;
            continue;
        }
        if in_block && line.trim() == end {
            in_block = false;
            continue;
        }
        if !in_block {
            lines.push(line);
        }
    }

    if !removed {
        return Ok(false);
    }

    let mut new_content = lines.join("\n");
    if content.ends_with('\n') {
        new_content.push('\n');
    }

    fs::write(rc_file, new_content)
        .with_context(|| format!("failed to write {}", rc_file.display()))?;
    Ok(true)
}

fn dedupe_paths(paths: &mut Vec<PathBuf>) {
    let mut seen = std::collections::HashSet::new();
    paths.retain(|path| seen.insert(path.clone()));
}
