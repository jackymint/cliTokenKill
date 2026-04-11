use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const PATH_BLOCK_START: &str = "# >>> ctk codex init >>>";
const PATH_BLOCK_END: &str = "# <<< ctk codex init <<<";
const AI_ENV_FLAG: &str = "CTK_AI_CLI";

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
    pub path_head: Vec<String>,
    pub repaired: bool,
    pub launcher_exists: bool,
}

struct CodexLayout {
    home: PathBuf,
    bin_dir: PathBuf,
    launchers_dir: PathBuf,
}

pub fn init_codex() -> Result<InitResult> {
    let layout = CodexLayout::load()?;
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

    let launcher_path = create_codex_launcher(&layout)?;
    let rc_files_updated = remove_all_rc_path_blocks(&layout.home)?;

    Ok(InitResult {
        wrappers_installed,
        rc_files_updated,
        bin_dir: layout.bin_dir,
        launcher_path,
    })
}

pub fn uninstall_codex() -> Result<UninstallResult> {
    let layout = CodexLayout::load()?;

    let removed_wrapper_files = if layout.bin_dir.exists() {
        clear_wrapper_dir(&layout.bin_dir)?
    } else {
        0usize
    };

    let mut removed_dir = false;
    if layout.launchers_dir.exists() {
        fs::remove_dir_all(&layout.launchers_dir).ok();
    }
    if layout.bin_dir.exists() {
        removed_dir = fs::remove_dir(&layout.bin_dir).is_ok();
    }

    let rc_files_updated = remove_all_rc_path_blocks(&layout.home)?;

    Ok(UninstallResult {
        removed_wrapper_files,
        removed_dir,
        rc_files_updated,
    })
}

pub fn doctor_codex(fix: bool) -> Result<DoctorResult> {
    let mut repaired = false;
    if fix {
        init_codex()?;
        repaired = true;
    }

    let layout = CodexLayout::load()?;
    let launcher_exists = layout.launchers_dir.join("codex-ctk").exists();

    let wrappers_count = if layout.bin_dir.exists() {
        fs::read_dir(&layout.bin_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .count()
    } else {
        0
    };

    let path_var = env::var("PATH").unwrap_or_default();
    let path_parts: Vec<String> = path_var.split(':').map(|s| s.to_string()).collect();
    let ctk_in_path = path_parts
        .iter()
        .any(|p| p == &layout.bin_dir.display().to_string());
    let path_head = path_parts.into_iter().take(8).collect();
    let ctk_in_login_shell_path = detect_in_login_shell_path(&layout.bin_dir);

    Ok(DoctorResult {
        ctk_in_path,
        ctk_in_login_shell_path,
        wrappers_count,
        path_head,
        repaired,
        launcher_exists,
    })
}

fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME environment variable is not set")
}

impl CodexLayout {
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

fn create_codex_launcher(layout: &CodexLayout) -> Result<Option<PathBuf>> {
    let Some(real_codex) = resolve_command_path("codex", &[layout.bin_dir.clone()])? else {
        return Ok(None);
    };

    fs::create_dir_all(&layout.launchers_dir).with_context(|| {
        format!(
            "failed to create launchers dir: {}",
            layout.launchers_dir.display()
        )
    })?;
    let launcher_path = layout.launchers_dir.join("codex-ctk");

    let script = format!(
        "#!/usr/bin/env bash\nset -euo pipefail\nexport {AI_ENV_FLAG}=1\nexport PATH=\"$HOME/.ctk/bin:$PATH\"\nexec \"{}\" \"$@\"\n",
        real_codex.display()
    );
    fs::write(&launcher_path, script)
        .with_context(|| format!("failed to write launcher: {}", launcher_path.display()))?;
    set_executable(&launcher_path)?;
    Ok(Some(launcher_path))
}

fn resolve_command_path(command: &str, ignore_prefixes: &[PathBuf]) -> Result<Option<PathBuf>> {
    let output = Command::new("which")
        .arg(command)
        .output()
        .with_context(|| format!("failed to resolve command path: {command}"))?;
    if !output.status.success() {
        return Ok(None);
    }
    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path_str.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(path_str);
    if ignore_prefixes.iter().any(|p| path.starts_with(p)) {
        return Ok(None);
    }
    Ok(Some(path))
}

fn wrapper_script(ctk_bin: &Path, real_cmd: &Path) -> String {
    format!(
        "#!/usr/bin/env bash\nset -euo pipefail\nif [[ \"${{{AI_ENV_FLAG}:-0}}\" != \"1\" ]]; then exec \"{}\" \"$@\"; fi\nif [[ \"${{CTK_BYPASS:-0}}\" == \"1\" ]]; then exec \"{}\" \"$@\"; fi\nexec \"{}\" proxy --level \"${{CTK_LEVEL:-aggressive}}\" --max-lines \"${{CTK_MAX_LINES:-80}}\" --max-chars-per-line \"${{CTK_MAX_CHARS_PER_LINE:-220}}\" -- \"{}\" \"$@\"\n",
        real_cmd.display(),
        real_cmd.display(),
        ctk_bin.display(),
        real_cmd.display()
    )
}

fn remove_all_rc_path_blocks(home: &Path) -> Result<Vec<PathBuf>> {
    let mut updated = Vec::new();
    for file in [home.join(".zshrc"), home.join(".bashrc")] {
        if remove_path_block(&file)? {
            updated.push(file);
        }
    }
    Ok(updated)
}

fn remove_path_block(rc_file: &Path) -> Result<bool> {
    if !rc_file.exists() {
        return Ok(false);
    }
    let content = fs::read_to_string(rc_file)
        .with_context(|| format!("failed to read {}", rc_file.display()))?;

    let start = content.find(PATH_BLOCK_START);
    let end = content.find(PATH_BLOCK_END);
    let (Some(start_idx), Some(end_idx)) = (start, end) else {
        return Ok(false);
    };

    let end_inclusive = end_idx + PATH_BLOCK_END.len();
    let mut updated = String::new();
    updated.push_str(&content[..start_idx]);
    if end_inclusive < content.len() {
        let rest = content[end_inclusive..].trim_start_matches('\n');
        updated.push_str(rest);
    }

    fs::write(rc_file, updated)
        .with_context(|| format!("failed to write {}", rc_file.display()))?;
    Ok(true)
}

fn detect_in_login_shell_path(bin_dir: &Path) -> Option<bool> {
    let output = Command::new("zsh")
        .args(["-lic", "printf %s \"$PATH\""])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).to_string();
    let target = bin_dir.display().to_string();
    let parts: Vec<&str> = path.split(':').collect();
    Some(parts.iter().any(|p| *p == target.as_str()))
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
