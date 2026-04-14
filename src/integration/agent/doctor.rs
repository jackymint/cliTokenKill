use super::*;

pub(super) fn launcher_target_name(launcher_file: &str) -> String {
    launcher_file
        .strip_suffix("-ctk")
        .unwrap_or(launcher_file)
        .to_string()
}
pub(super) fn dedupe_paths(paths: &mut Vec<PathBuf>) {
    paths.sort();
    paths.dedup();
}
pub(super) fn detect_in_login_shell_path(bin_dir: &Path) -> Option<bool> {
    let path = run_login_shell_capture("printf %s \"$PATH\"")?;
    let target = bin_dir.display().to_string();
    let parts: Vec<&str> = path.split(':').collect();
    Some(parts.contains(&target.as_str()))
}
pub(super) fn resolve_all_command_matches(command: &str) -> Vec<String> {
    if let Ok(output) = Command::new("which").args(["-a", command]).output()
        && output.status.success()
    {
        let mut entries = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !entries.iter().any(|v| v == trimmed) {
                entries.push(trimmed.to_string());
            }
        }
        if !entries.is_empty() {
            return entries;
        }
    }
    let mut entries = Vec::new();
    if let Some(path_var) = env::var_os("PATH") {
        for dir in env::split_paths(&path_var) {
            let candidate = dir.join(command);
            if candidate.is_file() && is_executable(&candidate) {
                let value = candidate.display().to_string();
                if !entries.iter().any(|v| v == &value) {
                    entries.push(value);
                }
            }
        }
    }
    entries
}
pub(super) fn list_wrapped_commands(bin_dir: &Path) -> Result<Vec<String>> {
    if !bin_dir.exists() {
        return Ok(Vec::new());
    }
    let mut commands = Vec::new();
    for entry in fs::read_dir(bin_dir)
        .with_context(|| format!("failed to read wrapper dir {}", bin_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name == "ctk" {
            continue;
        }
        commands.push(name.to_string());
    }
    commands.sort();
    Ok(commands)
}
pub(super) fn launcher_exec_target(launcher_path: &Path) -> Option<PathBuf> {
    let content = fs::read_to_string(launcher_path).ok()?;
    for line in content.lines().rev() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("exec \"") else {
            continue;
        };
        let Some(end_quote_idx) = rest.find('"') else {
            continue;
        };
        let target = &rest[..end_quote_idx];
        if target.is_empty() {
            continue;
        }
        return Some(PathBuf::from(target));
    }
    None
}
pub(super) fn resolve_login_shell_selected(command: &str) -> Option<String> {
    let quoted = shell_quote_single(command);
    run_login_shell_capture(&format!("command -v {quoted}"))
}
pub(super) fn resolve_login_shell_type_chain(command: &str) -> Vec<String> {
    let quoted = shell_quote_single(command);
    run_login_shell_lines(&format!("type -a {quoted} 2>/dev/null || true"))
}
pub(super) fn run_login_shell_capture(script: &str) -> Option<String> {
    let output = run_login_shell(script)?;
    let value = output.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
pub(super) fn run_login_shell_lines(script: &str) -> Vec<String> {
    let Some(output) = run_login_shell(script) else {
        return Vec::new();
    };
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect()
}
fn run_login_shell(script: &str) -> Option<String> {
    let shell = env::var("SHELL").unwrap_or_else(|_| "zsh".to_string());
    let shell_name = Path::new(&shell)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("zsh");
    let mut cmd = Command::new(&shell);
    if shell_name == "fish" {
        cmd.args(["-lc", script]);
    } else {
        cmd.args(["-lic", script]);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}
pub(super) fn shell_quote_single(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
pub(super) fn is_truthy_env(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}
