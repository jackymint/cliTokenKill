use crate::core::filter::{FilterConfig, signal_only};
use crate::core::runner::run_command;
use crate::engine::{classify_content, compact_by_kind};
use anyhow::Result;

pub enum PipelineMode {
    Normal,
    ErrorOnly,
    TestOnly,
}

pub struct PipelineResult {
    pub exit_code: i32,
    pub output: String,
    pub fallback_used: bool,
}

pub fn run_pipeline(
    command: &[String],
    config: FilterConfig,
    mode: PipelineMode,
) -> Result<PipelineResult> {
    let process_output = run_command(command)?;
    let merged = merge_output(&process_output.stdout, &process_output.stderr);

    let signal_patterns: &[&str] = match mode {
        PipelineMode::ErrorOnly => &["error", "warn", "warning", "panic", "exception", "failed"],
        PipelineMode::TestOnly => &["fail", "failed", "error", "panic", "exception", "traceback"],
        PipelineMode::Normal => &[],
    };

    if !signal_patterns.is_empty() {
        let signal = signal_only(&merged, signal_patterns, config.max_lines);
        if !signal.is_empty() {
            return Ok(PipelineResult {
                exit_code: process_output.code,
                output: signal,
                fallback_used: false,
            });
        }
    }

    let kind = classify_content(&merged);

    let compacted = std::panic::catch_unwind(|| compact_by_kind(&merged, kind, config));

    let (output, fallback_used) = match compacted {
        Ok(text) if !text.trim().is_empty() || merged.trim().is_empty() => (text, false),
        _ => (merged, true),
    };

    Ok(PipelineResult {
        exit_code: process_output.code,
        output,
        fallback_used,
    })
}

pub fn merge_output(stdout: &str, stderr: &str) -> String {
    match (stdout.trim().is_empty(), stderr.trim().is_empty()) {
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (false, false) => format!("{stdout}\n{stderr}"),
        (true, true) => String::new(),
    }
}
