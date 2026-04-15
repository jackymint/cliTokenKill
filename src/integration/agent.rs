use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod doctor;
mod rc;

use self::doctor::{
    dedupe_paths, detect_in_login_shell_path, is_truthy_env, launcher_exec_target,
    launcher_target_name, list_wrapped_commands, resolve_all_command_matches,
    resolve_login_shell_selected, resolve_login_shell_type_chain,
};
use self::rc::{
    remove_agent_alias_blocks, remove_legacy_rc_path_blocks, upsert_agent_alias_blocks,
};
const LEGACY_PATH_BLOCK_START: &str = "# >>> ctk codex init >>>";
const LEGACY_PATH_BLOCK_END: &str = "# <<< ctk codex init <<<";
const AI_ENV_FLAG: &str = "CTK_AI_CLI";
const LAUNCH_DEPTH_ENV: &str = "CTK_LAUNCH_DEPTH";
const MAX_LAUNCH_DEPTH: usize = 3;
const SKIP_WRAPPING: &[&str] = &[
    "ctk",
    "bash",
    "zsh",
    "sh",
    "fish",
    "sudo",
    "su",
    "ssh",
    "scp",
    "sftp",
    "login",
    "env",
    "which",
    "docker",
    "docker-compose",
    "cargo",
    "rustc",
    "rustup",
    "codex",
    "claude",
    "gemini",
];
pub struct InitResult {
    pub wrappers_installed: Vec<String>,
    pub rc_files_updated: Vec<PathBuf>,
    pub bin_dir: PathBuf,
    pub launcher_path: Option<PathBuf>,
}
pub struct UninstallResult {
    pub removed_wrapper_files: usize,
    pub removed_dir: bool,
    pub rc_files_updated: Vec<PathBuf>,
}
pub struct DoctorResult {
    pub ctk_in_path: bool,
    pub ctk_in_login_shell_path: Option<bool>,
    pub wrappers_count: usize,
    pub wrapped_commands: Vec<String>,
    pub path_head: Vec<String>,
    pub repaired: bool,
    pub launcher_exists: bool,
    pub launcher_path: PathBuf,
    pub launcher_exec_path: Option<PathBuf>,
    pub real_command_path: Option<PathBuf>,
    pub launcher_selected_first: Option<bool>,
    pub shell_selected: Option<String>,
    pub shell_type_chain: Vec<String>,
    pub command_matches: Vec<String>,
    pub ai_cli_env: Option<String>,
    pub bypass_env: Option<String>,
    pub bypass_enabled: bool,
}
struct AgentLayout {
    home: PathBuf,
    bin_dir: PathBuf,
    launchers_dir: PathBuf,
}
pub(crate) fn init_agent(agent_cmd: &str, launcher_file: &str) -> Result<InitResult> {
    let layout = AgentLayout::load()?;
    fs::create_dir_all(&layout.bin_dir)
        .with_context(|| format!("failed to create directory: {}", layout.bin_dir.display()))?;
    clear_wrapper_dir(&layout.bin_dir)?;
    link_ctk_binary(&layout.bin_dir)?;
    let ctk_bin = env::current_exe().context("failed to resolve current executable path")?;
    let discovered = discover_commands_from_path(&layout.bin_dir)?;
    let mut wrappers_installed = Vec::new();
    for (cmd, real_path) in discovered {
        let wrapper_path = layout.bin_dir.join(&cmd);
        let script = wrapper_script(&ctk_bin, &real_path);
        fs::write(&wrapper_path, script)
            .with_context(|| format!("failed to write wrapper: {}", wrapper_path.display()))?;
        set_executable(&wrapper_path)?;
        wrappers_installed.push(cmd);
    }
    let launcher_path = create_launcher(&layout, agent_cmd, launcher_file)?;
    let mut rc_files_updated = remove_legacy_rc_path_blocks(&layout.home)?;
    if launcher_path.is_some() {
        rc_files_updated.extend(upsert_agent_alias_blocks(
            &layout.home,
            agent_cmd,
            launcher_file,
        )?);
    }
    dedupe_paths(&mut rc_files_updated);
    Ok(InitResult {
        wrappers_installed,
        rc_files_updated,
        bin_dir: layout.bin_dir,
        launcher_path,
    })
}
pub(crate) fn uninstall_agent(launcher_file: &str) -> Result<UninstallResult> {
    let layout = AgentLayout::load()?;
    let agent_cmd = launcher_target_name(launcher_file);
    let launcher_path = layout.launchers_dir.join(launcher_file);
    if launcher_path.exists() {
        fs::remove_file(&launcher_path)
            .with_context(|| format!("failed to remove launcher: {}", launcher_path.display()))?;
    }
    let mut removed_dir = false;
    if layout.launchers_dir.exists() {
        let launchers_left = fs::read_dir(&layout.launchers_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .count();
        if launchers_left == 0 {
            fs::remove_dir_all(&layout.launchers_dir).ok();
        }
    }
    let removed_wrapper_files = if layout.launchers_dir.exists() {
        0usize
    } else if layout.bin_dir.exists() {
        clear_wrapper_dir(&layout.bin_dir)?
    } else {
        0usize
    };
    if !layout.launchers_dir.exists() && layout.bin_dir.exists() {
        removed_dir = fs::remove_dir(&layout.bin_dir).is_ok();
    }
    let mut rc_files_updated = remove_legacy_rc_path_blocks(&layout.home)?;
    rc_files_updated.extend(remove_agent_alias_blocks(&layout.home, &agent_cmd)?);
    dedupe_paths(&mut rc_files_updated);
    Ok(UninstallResult {
        removed_wrapper_files,
        removed_dir,
        rc_files_updated,
    })
}
pub(crate) fn doctor_agent(
    fix: bool,
    agent_cmd: &str,
    launcher_file: &str,
) -> Result<DoctorResult> {
    let mut repaired = false;
    if fix {
        init_agent(agent_cmd, launcher_file)?;
        repaired = true;
    }
    let layout = AgentLayout::load()?;
    let launcher_path = layout.launchers_dir.join(launcher_file);
    let launcher_exists = launcher_path.exists();
    let launcher_exec_path = launcher_exec_target(&launcher_path);
    let ignore_prefixes = [layout.bin_dir.clone(), layout.launchers_dir.clone()];
    let real_command_path = resolve_command_path(agent_cmd, &ignore_prefixes)?;
    let wrapped_commands = list_wrapped_commands(&layout.bin_dir)?;
    let wrappers_count = wrapped_commands.len();
    let path_var = env::var("PATH").unwrap_or_default();
    let path_parts: Vec<String> = path_var.split(':').map(|s| s.to_string()).collect();
    let ctk_in_path = path_parts
        .iter()
        .any(|p| p == &layout.bin_dir.display().to_string());
    let path_head = path_parts.into_iter().take(8).collect();
    let ctk_in_login_shell_path = detect_in_login_shell_path(&layout.bin_dir);
    let command_matches = resolve_all_command_matches(agent_cmd);
    let launcher_selected_first = command_matches
        .first()
        .map(|first| Path::new(first) == launcher_path.as_path());
    let shell_selected = resolve_login_shell_selected(agent_cmd);
    let shell_type_chain = resolve_login_shell_type_chain(agent_cmd);
    let ai_cli_env = env::var(AI_ENV_FLAG).ok();
    let bypass_env = env::var("CTK_BYPASS").ok();
    let bypass_enabled = bypass_env.as_deref().map(is_truthy_env).unwrap_or(false);
    Ok(DoctorResult {
        ctk_in_path,
        ctk_in_login_shell_path,
        wrappers_count,
        wrapped_commands,
        path_head,
        repaired,
        launcher_exists,
        launcher_path,
        launcher_exec_path,
        real_command_path,
        launcher_selected_first,
        shell_selected,
        shell_type_chain,
        command_matches,
        ai_cli_env,
        bypass_env,
        bypass_enabled,
    })
}
fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME environment variable is not set")
}
impl AgentLayout {
    fn load() -> Result<Self> {
        let home = home_dir()?;
        let ctk_root = home.join(".ctk");
        let bin_dir = ctk_root.join("bin");
        let launchers_dir = ctk_root.join("launchers");
        Ok(Self {
            home,
            bin_dir,
            launchers_dir,
        })
    }
}
fn discover_commands_from_path(wrapper_dir: &Path) -> Result<Vec<(String, PathBuf)>> {
    let path_var = env::var_os("PATH").context("PATH environment variable is not set")?;
    let mut found: HashMap<String, PathBuf> = HashMap::new();
    for dir in env::split_paths(&path_var) {
        if dir == wrapper_dir || !dir.exists() || !dir.is_dir() {
            continue;
        }
        let entries =
            fs::read_dir(&dir).with_context(|| format!("failed to read dir {}", dir.display()))?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || !is_executable(&path) {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if SKIP_WRAPPING.contains(&name) {
                continue;
            }
            found.entry(name.to_string()).or_insert(path);
        }
    }
    let mut items: Vec<(String, PathBuf)> = found.into_iter().collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(items)
}
fn clear_wrapper_dir(dir: &Path) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let entries = fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))?;
    let mut removed = 0usize;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            match fs::remove_file(&path) {
                Ok(_) => removed += 1,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(err)
                        .with_context(|| format!("failed to remove {}", path.display()));
                }
            }
        }
    }
    Ok(removed)
}
fn link_ctk_binary(bin_dir: &Path) -> Result<()> {
    let current_exe = env::current_exe().context("failed to resolve current executable path")?;
    let ctk_link = bin_dir.join("ctk");
    if ctk_link.exists() {
        fs::remove_file(&ctk_link).ok();
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&current_exe, &ctk_link).with_context(|| {
            format!(
                "failed to create symlink {} -> {}",
                ctk_link.display(),
                current_exe.display()
            )
        })?;
    }
    #[cfg(not(unix))]
    {
        fs::copy(&current_exe, &ctk_link).with_context(|| {
            format!(
                "failed to copy binary {} -> {}",
                current_exe.display(),
                ctk_link.display()
            )
        })?;
    }
    Ok(())
}
fn create_launcher(
    layout: &AgentLayout,
    agent_cmd: &str,
    launcher_file: &str,
) -> Result<Option<PathBuf>> {
    let ignore_prefixes = [layout.bin_dir.clone(), layout.launchers_dir.clone()];
    let Some(real_agent) = resolve_command_path(agent_cmd, &ignore_prefixes)? else {
        return Ok(None);
    };
    fs::create_dir_all(&layout.launchers_dir).with_context(|| {
        format!(
            "failed to create launchers dir: {}",
            layout.launchers_dir.display()
        )
    })?;
    let launcher_path = layout.launchers_dir.join(launcher_file);
    let script = format!(
        "#!/usr/bin/env bash\nset -euo pipefail\ndepth=\"${{{LAUNCH_DEPTH_ENV}:-0}}\"\nif (( depth >= {MAX_LAUNCH_DEPTH} )); then\n  echo \"ctk: launcher recursion guard triggered ({agent_cmd})\" >&2\n  exit 125\nfi\nexport {LAUNCH_DEPTH_ENV}=\"$((depth + 1))\"\nexport {AI_ENV_FLAG}=1\nexport CTK_AI_CLI_NAME={agent_cmd}\nexport PATH=\"{}:$PATH\"\nexec \"{}\" \"$@\"\n",
        layout.bin_dir.display(),
        real_agent.display(),
    );
    fs::write(&launcher_path, script)
        .with_context(|| format!("failed to write launcher: {}", launcher_path.display()))?;
    set_executable(&launcher_path)?;
    Ok(Some(launcher_path))
}
fn resolve_command_path(command: &str, ignore_prefixes: &[PathBuf]) -> Result<Option<PathBuf>> {
    let path_var = env::var_os("PATH").context("PATH environment variable is not set")?;
    for dir in env::split_paths(&path_var) {
        if !dir.exists() || !dir.is_dir() {
            continue;
        }
        let candidate = dir.join(command);
        if !candidate.is_file() || !is_executable(&candidate) {
            continue;
        }
        if ignore_prefixes.iter().any(|p| candidate.starts_with(p)) {
            continue;
        }
        return Ok(Some(candidate));
    }
    Ok(None)
}
fn wrapper_script(ctk_bin: &Path, real_cmd: &Path) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

_ctk_active=0
if [[ "${{{AI_ENV_FLAG}:-0}}" == "1" ]]; then
  _ctk_active=1
else
  _pid=$PPID
  for _i in 1 2 3 4 5; do
    _name=$(ps -p "$_pid" -o comm= 2>/dev/null) || break
    case "$_name" in
      codex|claude|gemini|*"Amazon Q"*|*amazonq*) _ctk_active=1; break ;;
    esac
    _pid=$(ps -p "$_pid" -o ppid= 2>/dev/null | tr -d ' ') || break
    [[ "${{_pid:-1}}" -le 1 ]] && break
  done
fi

if [[ "$_ctk_active" != "1" ]]; then
  exec "{real}" "$@"
fi

if [[ "${{CTK_BYPASS:-0}}" == "1" ]]; then
  exec "{real}" "$@"
fi

exec "{ctk}" proxy \
  --level "${{CTK_LEVEL:-aggressive}}" \
  --max-lines "${{CTK_MAX_LINES:-80}}" \
  --max-chars-per-line "${{CTK_MAX_CHARS_PER_LINE:-220}}" \
  -- "{real}" "$@"
"#,
        real = real_cmd.display(),
        ctk = ctk_bin.display(),
    )
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .with_context(|| format!("failed to stat {}", path.display()))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)
        .with_context(|| format!("failed to set executable bit on {}", path.display()))?;
    Ok(())
}
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}
#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}
#[cfg(test)]
mod tests;
