use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use super::agent::{
    DoctorResult, InitResult, UninstallResult, doctor_agent, init_agent, uninstall_agent,
};

pub fn init_codex() -> Result<InitResult> {
    let mut result = init_agent("codex", "codex-ctk")?;
    install_mcp_config(&mut result)?;
    Ok(result)
}

pub fn uninstall_codex() -> Result<UninstallResult> {
    uninstall_mcp_config()?;
    uninstall_agent("codex-ctk")
}

pub fn doctor_codex(fix: bool) -> Result<DoctorResult> {
    doctor_agent(fix, "codex", "codex-ctk")
}

fn install_mcp_config(result: &mut InitResult) -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let config_dir = PathBuf::from(&home).join("Library/Application Support/Codex");
    fs::create_dir_all(&config_dir)
        .with_context(|| format!("failed to create {}", config_dir.display()))?;

    let config_path = config_dir.join("mcp_config.json");
    let ctk_bin = PathBuf::from(&home).join(".cargo/bin/ctk");

    let config = serde_json::json!({
        "mcpServers": {
            "ctk": {
                "command": ctk_bin.display().to_string(),
                "args": ["mcp"]
            }
        }
    });

    fs::write(&config_path, serde_json::to_string_pretty(&config)?)
        .with_context(|| format!("failed to write {}", config_path.display()))?;

    result.rc_files_updated.push(config_path);
    Ok(())
}

fn uninstall_mcp_config() -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let config_path = PathBuf::from(home)
        .join("Library/Application Support/Codex")
        .join("mcp_config.json");

    if config_path.exists() {
        fs::remove_file(&config_path)
            .with_context(|| format!("failed to remove {}", config_path.display()))?;
    }
    Ok(())
}
