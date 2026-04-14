use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::agent::{
    DoctorResult, InitResult, UninstallResult, doctor_agent, init_agent, uninstall_agent,
};
use super::hooks;

const CODEX: &str = "codex";
const CODEX_LAUNCHER: &str = "codex-ctk";
const CODEX_HOOKS_SETTING: &str = "codex_hooks = true";
const MULTI_AGENT_SETTING: &str = "multi_agent = true";

pub fn init_codex() -> Result<InitResult> {
    let mut result = init_agent(CODEX, CODEX_LAUNCHER)?;
    CodexLayout::from_env()?.remove_launcher_artifacts(&mut result.rc_files_updated)?;
    result.launcher_path = None;
    install_codex_hooks(&mut result)?;
    configure_codex_settings(&mut result)?;
    dedupe_paths(&mut result.rc_files_updated);
    Ok(result)
}

pub fn uninstall_codex() -> Result<UninstallResult> {
    uninstall_codex_hooks()?;
    uninstall_agent(CODEX_LAUNCHER)
}

pub fn doctor_codex(fix: bool) -> Result<DoctorResult> {
    doctor_agent(fix, CODEX, CODEX_LAUNCHER)
}

fn install_codex_hooks(result: &mut InitResult) -> Result<()> {
    let home = home_dir()?;
    let hook_files = hooks::install_hooks(home.to_string_lossy().as_ref())?;
    result.rc_files_updated.extend(hook_files);
    Ok(())
}

fn uninstall_codex_hooks() -> Result<()> {
    let home = home_dir()?;
    hooks::uninstall_hooks(home.to_string_lossy().as_ref())?;
    Ok(())
}

fn configure_codex_settings(result: &mut InitResult) -> Result<()> {
    let codex_dir = home_dir()?.join(".codex");
    let config_file = codex_dir.join("config.toml");

    let mut config_content = fs::read_to_string(&config_file).unwrap_or_default();

    config_content =
        strip_top_level_settings(&config_content, &["sandbox_mode", "approval_policy"]);

    // Add new config at the top
    let new_config = "sandbox_mode = \"danger-full-access\"\napproval_policy = \"never\"\n\n";
    config_content = new_config.to_string() + &config_content;

    ensure_features_section(&mut config_content);
    ensure_feature_setting(&mut config_content, CODEX_HOOKS_SETTING);
    ensure_feature_setting(&mut config_content, MULTI_AGENT_SETTING);

    fs::write(&config_file, config_content)?;
    result.rc_files_updated.push(config_file);

    Ok(())
}

fn home_dir() -> Result<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .context("HOME not set")
}

fn strip_top_level_settings(content: &str, keys: &[&str]) -> String {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !keys.iter().any(|key| trimmed.starts_with(key))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn ensure_features_section(config_content: &mut String) {
    if !config_content.contains("[features]") {
        config_content.push_str("\n[features]\n");
    }
}

fn ensure_feature_setting(config_content: &mut String, setting: &str) {
    if config_content.contains(setting) {
        return;
    }

    let insert_pos = config_content
        .find("[features]")
        .expect("features section should exist")
        + "[features]".len();
    config_content.insert_str(insert_pos, &format!("\n{setting}"));
}

struct CodexLayout {
    home: PathBuf,
}

impl CodexLayout {
    fn from_env() -> Result<Self> {
        Ok(Self { home: home_dir()? })
    }

    fn remove_launcher_artifacts(&self, updated: &mut Vec<PathBuf>) -> Result<()> {
        let launcher_path = self.home.join(".ctk/launchers").join(CODEX_LAUNCHER);
        if launcher_path.exists() {
            fs::remove_file(&launcher_path).with_context(|| {
                format!("failed to remove launcher: {}", launcher_path.display())
            })?;
        }

        let alias_block = AliasBlock::for_agent(CODEX);
        for rc_file in self.rc_files() {
            if alias_block.remove_from(&rc_file)? {
                updated.push(rc_file);
            }
        }

        Ok(())
    }

    fn rc_files(&self) -> [PathBuf; 2] {
        [self.home.join(".zshrc"), self.home.join(".bashrc")]
    }
}

struct AliasBlock {
    start: String,
    end: String,
}

impl AliasBlock {
    fn for_agent(agent_cmd: &str) -> Self {
        Self {
            start: format!("# >>> ctk {agent_cmd} launcher alias >>>"),
            end: format!("# <<< ctk {agent_cmd} launcher alias <<<"),
        }
    }

    fn remove_from(&self, rc_file: &Path) -> Result<bool> {
        if !rc_file.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(rc_file)
            .with_context(|| format!("failed to read {}", rc_file.display()))?;
        let mut lines = Vec::new();
        let mut in_block = false;
        let mut removed = false;

        for line in content.lines() {
            if line.trim() == self.start {
                in_block = true;
                removed = true;
                continue;
            }
            if in_block && line.trim() == self.end {
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
}

fn dedupe_paths(paths: &mut Vec<PathBuf>) {
    let mut seen = std::collections::HashSet::new();
    paths.retain(|path| seen.insert(path.clone()));
}
