use anyhow::{Context, Result, bail};
use std::process::{Command, Stdio};

#[derive(Debug)]
pub struct CmdOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_command(args: &[String]) -> Result<CmdOutput> {
    if args.is_empty() {
        bail!("no command supplied");
    }

    let mut cmd = Command::new(&args[0]);
    if args.len() > 1 {
        cmd.args(&args[1..]);
    }
    cmd.stdin(Stdio::inherit());

    let output = cmd.output().with_context(|| {
        format!(
            "failed to execute command: {}",
            args.first().unwrap_or(&"<unknown>".to_string())
        )
    })?;

    Ok(CmdOutput {
        code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}
