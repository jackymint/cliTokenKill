use anyhow::Result;

use super::agent::{
    DoctorResult, InitResult, UninstallResult, doctor_agent, init_agent, uninstall_agent,
};

pub fn init_codex() -> Result<InitResult> {
    init_agent("codex", "codex-ctk")
}

pub fn uninstall_codex() -> Result<UninstallResult> {
    uninstall_agent("codex-ctk")
}

pub fn doctor_codex(fix: bool) -> Result<DoctorResult> {
    doctor_agent(fix, "codex", "codex-ctk")
}
