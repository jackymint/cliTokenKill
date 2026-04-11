use anyhow::Result;

use super::agent::{
    DoctorResult, InitResult, UninstallResult, doctor_agent, init_agent, uninstall_agent,
};

pub fn init_claude() -> Result<InitResult> {
    init_agent("claude", "claude-ctk")
}

pub fn uninstall_claude() -> Result<UninstallResult> {
    uninstall_agent("claude-ctk")
}

pub fn doctor_claude(fix: bool) -> Result<DoctorResult> {
    doctor_agent(fix, "claude", "claude-ctk")
}
