use crate::core::adapter::apply_command_adapter;
use crate::core::filter::{FilterConfig, signal_only};
use crate::core::runner::run_command;
use crate::engine::{ContentKind, classify_content, compact_by_kind};
use anyhow::Result;
use std::env;

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
    pub reason: String,
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
    pub fallback_used: bool,
    pub details: PipelineDetails,
}

pub fn run_pipeline(
    command: &[String],
    config: FilterConfig,
    mode: PipelineMode,
) -> Result<PipelineResult> {
    let process_output = run_command(command)?;
    let merged = merge_output(&process_output.stdout, &process_output.stderr);
    let classifier_kind = classify_content(&merged);
    let raw_lines = line_count(&merged);

    let mut stage_reports = vec![PipelineStageReport {
        stage: "merge",
        selected: true,
        lines_before: raw_lines,
        lines_after: raw_lines,
        reason: "merged stdout/stderr".to_string(),
    }];

    if let Some(adapter_output) =
        run_adapter_stage(command, &merged, config, raw_lines, &mut stage_reports)
    {
        if is_adapter_debug_enabled() {
            eprintln!("ctk: adapter applied: {}", adapter_output.adapter_name);
        }
        return Ok(PipelineResult {
            exit_code: process_output.code,
            output: adapter_output.output,
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

    let (output, fallback_used) = run_content_aware_stage(&merged, classifier_kind, config);
    let filtered_lines = line_count(&output);

    stage_reports.push(PipelineStageReport {
        stage: "content-aware",
        selected: true,
        lines_before: raw_lines,
        lines_after: filtered_lines,
        reason: if fallback_used {
            format!("fallback to raw output ({})", classifier_kind.as_str())
        } else {
            format!("compacted as {}", classifier_kind.as_str())
        },
    });

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

struct AdapterStageOutput {
    adapter_name: String,
    output: String,
    filtered_lines: usize,
}

fn run_adapter_stage(
    command: &[String],
    merged: &str,
    config: FilterConfig,
    raw_lines: usize,
    stage_reports: &mut Vec<PipelineStageReport>,
) -> Option<AdapterStageOutput> {
    if let Some(applied) = apply_command_adapter(command, merged, config) {
        let filtered_lines = line_count(&applied.output);
        stage_reports.push(PipelineStageReport {
            stage: "adapter",
            selected: true,
            lines_before: raw_lines,
            lines_after: filtered_lines,
            reason: format!("matched adapter '{}'", applied.name),
        });
        return Some(AdapterStageOutput {
            adapter_name: applied.name,
            output: applied.output,
            filtered_lines,
        });
    }

    stage_reports.push(PipelineStageReport {
        stage: "adapter",
        selected: false,
        lines_before: raw_lines,
        lines_after: raw_lines,
        reason: "no adapter matched".to_string(),
    });
    None
}

struct SignalStageOutput {
    output: String,
    filtered_lines: usize,
}

fn run_signal_stage(
    mode: PipelineMode,
    merged: &str,
    config: FilterConfig,
    raw_lines: usize,
    stage_reports: &mut Vec<PipelineStageReport>,
) -> Option<SignalStageOutput> {
    let patterns = signal_patterns(mode);
    if patterns.is_empty() {
        stage_reports.push(PipelineStageReport {
            stage: "signal-only",
            selected: false,
            lines_before: raw_lines,
            lines_after: raw_lines,
            reason: "mode=normal".to_string(),
        });
        return None;
    }

    let signal = signal_only(merged, patterns, config.max_lines);
    if signal.trim().is_empty() {
        stage_reports.push(PipelineStageReport {
            stage: "signal-only",
            selected: false,
            lines_before: raw_lines,
            lines_after: raw_lines,
            reason: format!("mode={} no signal match", mode.as_str()),
        });
        return None;
    }

    let filtered_lines = line_count(&signal);
    stage_reports.push(PipelineStageReport {
        stage: "signal-only",
        selected: true,
        lines_before: raw_lines,
        lines_after: filtered_lines,
        reason: format!("mode={} signal patterns matched", mode.as_str()),
    });

    Some(SignalStageOutput {
        output: signal,
        filtered_lines,
    })
}

fn run_content_aware_stage(
    merged: &str,
    classifier_kind: ContentKind,
    config: FilterConfig,
) -> (String, bool) {
    let compacted = std::panic::catch_unwind(|| compact_by_kind(merged, classifier_kind, config));

    match compacted {
        Ok(text) if !text.trim().is_empty() || merged.trim().is_empty() => (text, false),
        _ => (merged.to_string(), true),
    }
}

fn signal_patterns(mode: PipelineMode) -> &'static [&'static str] {
    match mode {
        PipelineMode::ErrorOnly => ERROR_ONLY_PATTERNS,
        PipelineMode::TestOnly => TEST_ONLY_PATTERNS,
        PipelineMode::Normal => &[],
    }
}

fn is_adapter_debug_enabled() -> bool {
    let Ok(raw) = env::var("CTK_ADAPTER_DEBUG") else {
        return false;
    };

    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count()
    }
}
