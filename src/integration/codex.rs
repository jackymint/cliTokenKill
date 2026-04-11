use anyhow::{Context, Result};

use super::agent::{
    DoctorResult, InitResult, UninstallResult, doctor_agent, init_agent, uninstall_agent,
};
use super::hooks;

pub fn init_codex() -> Result<InitResult> {
    let mut result = init_agent("codex", "codex-ctk")?;
    install_codex_hooks(&mut result)?;
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
