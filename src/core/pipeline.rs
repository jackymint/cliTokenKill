use crate::core::adapter::apply_command_adapter;
use crate::core::filter::{FilterConfig, signal_only};
use crate::core::runner::run_command;
use crate::engine::{ContentKind, classify_content, compact_by_kind};
use anyhow::Result;
use std::env;
use std::time::Instant;

const ERROR_ONLY_PATTERNS: &[&str] = &["error", "warn", "warning", "panic", "exception", "failed"];
const TEST_ONLY_PATTERNS: &[&str] = &["fail", "failed", "error", "panic", "exception", "traceback"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PipelineMode {
    Normal,
    ErrorOnly,
    TestOnly,
}

impl PipelineMode {
    pub fn as_str(self) -> &'static str {
        match self {
            PipelineMode::Normal => "normal",
            PipelineMode::ErrorOnly => "error-only",
            PipelineMode::TestOnly => "test-only",
        }
    }
}

#[derive(Clone, Debug)]
pub enum PipelineStrategy {
    Adapter { name: String },
    SignalOnly { mode: PipelineMode },
    ContentAware { kind: ContentKind },
    RawFallback { kind: ContentKind },
}

impl PipelineStrategy {
    pub fn label(&self) -> String {
        match self {
            PipelineStrategy::Adapter { name } => format!("adapter:{name}"),
            PipelineStrategy::SignalOnly { mode } => format!("signal-only:{}", mode.as_str()),
            PipelineStrategy::ContentAware { kind } => format!("content-aware:{}", kind.as_str()),
            PipelineStrategy::RawFallback { kind } => {
                format!("fallback-raw:{}", kind.as_str())
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct PipelineStageReport {
    pub stage: &'static str,
    pub selected: bool,
    pub lines_before: usize,
    pub lines_after: usize,
    pub elapsed_ms: u64,
    pub reason: String,
}

impl PipelineStageReport {
    pub fn new(
        stage: &'static str,
        selected: bool,
        lines_before: usize,
        lines_after: usize,
        elapsed_ms: u64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            selected,
            lines_before,
            lines_after,
            elapsed_ms,
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PipelineDetails {
    pub mode: PipelineMode,
    pub classifier_kind: ContentKind,
    pub strategy: PipelineStrategy,
    pub raw_lines: usize,
    pub filtered_lines: usize,
    pub dropped_lines: usize,
    pub stage_reports: Vec<PipelineStageReport>,
}

pub struct PipelineResult {
    pub exit_code: i32,
    pub output: String,
    pub raw_chars: usize,
    pub fallback_used: bool,
    pub details: PipelineDetails,
}

pub fn run_pipeline(
    command: &[String],
    config: FilterConfig,
    mode: PipelineMode,
) -> Result<PipelineResult> {
    let merge_started = Instant::now();
    let process_output = run_command(command)?;
    let merged = merge_output(&process_output.stdout, &process_output.stderr);
    let classifier_kind = classify_content(&merged);
    let raw_lines = line_count(&merged);
    let raw_chars = merged.len();

    let mut stage_reports = vec![PipelineStageReport::new(
        "merge",
        true,
        raw_lines,
        raw_lines,
        merge_started.elapsed().as_millis() as u64,
        "merged stdout/stderr",
    )];

    if let Some(adapter_output) =
        run_adapter_stage(command, &merged, config, raw_lines, &mut stage_reports)
    {
        if is_adapter_debug_enabled() {
            eprintln!("ctk: adapter applied: {}", adapter_output.adapter_name);
        }
        return Ok(PipelineResult {
            exit_code: process_output.code,
            output: adapter_output.output,
            raw_chars,
            fallback_used: false,
            details: PipelineDetails {
                mode,
                classifier_kind,
                strategy: PipelineStrategy::Adapter {
                    name: adapter_output.adapter_name,
                },
                raw_lines,
                filtered_lines: adapter_output.filtered_lines,
                dropped_lines: raw_lines.saturating_sub(adapter_output.filtered_lines),
                stage_reports,
            },
        });
    }

    if let Some(signal_output) =
        run_signal_stage(mode, &merged, config, raw_lines, &mut stage_reports)
    {
        return Ok(PipelineResult {
            exit_code: process_output.code,
            output: signal_output.output,
            raw_chars,
            fallback_used: false,
            details: PipelineDetails {
                mode,
                classifier_kind,
                strategy: PipelineStrategy::SignalOnly { mode },
                raw_lines,
                filtered_lines: signal_output.filtered_lines,
                dropped_lines: raw_lines.saturating_sub(signal_output.filtered_lines),
                stage_reports,
            },
        });
    }

    let (output, fallback_used, content_aware_elapsed_ms) =
        run_content_aware_stage(&merged, classifier_kind, config);
    let filtered_lines = line_count(&output);

    stage_reports.push(PipelineStageReport::new(
        "content-aware",
        true,
        raw_lines,
        filtered_lines,
        content_aware_elapsed_ms,
        if fallback_used {
            format!("fallback to raw output ({})", classifier_kind.as_str())
        } else {
            format!("compacted as {}", classifier_kind.as_str())
        },
    ));

    let strategy = if fallback_used {
        PipelineStrategy::RawFallback {
            kind: classifier_kind,
        }
    } else {
        PipelineStrategy::ContentAware {
            kind: classifier_kind,
        }
    };

    Ok(PipelineResult {
        exit_code: process_output.code,
        output,
        raw_chars,
        fallback_used,
        details: PipelineDetails {
            mode,
            classifier_kind,
            strategy,
            raw_lines,
            filtered_lines,
            dropped_lines: raw_lines.saturating_sub(filtered_lines),
            stage_reports,
        },
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

mod stages;
#[cfg(test)]
mod tests;

use self::stages::{
    is_adapter_debug_enabled, line_count, run_adapter_stage, run_content_aware_stage,
    run_signal_stage,
};
