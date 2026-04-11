use crate::core::adapter::apply_command_adapter;
use crate::core::filter::{FilterConfig, signal_only};
use crate::core::runner::run_command;
use crate::engine::{ContentKind, classify_content, compact_by_kind};
use anyhow::Result;
use std::env;

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
pub struct PipelineDetails {
    pub mode: PipelineMode,
    pub classifier_kind: ContentKind,
    pub strategy: PipelineStrategy,
    pub raw_lines: usize,
    pub filtered_lines: usize,
    pub dropped_lines: usize,
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

    if let Some(applied) = apply_command_adapter(command, &merged, config) {
        if is_adapter_debug_enabled() {
            eprintln!("ctk: adapter applied: {}", applied.name);
        }
        let filtered_lines = line_count(&applied.output);
        return Ok(PipelineResult {
            exit_code: process_output.code,
            output: applied.output,
            fallback_used: false,
            details: PipelineDetails {
                mode,
                classifier_kind,
                strategy: PipelineStrategy::Adapter { name: applied.name },
                raw_lines,
                filtered_lines,
                dropped_lines: raw_lines.saturating_sub(filtered_lines),
            },
        });
    }

    let signal_patterns: &[&str] = match mode {
        PipelineMode::ErrorOnly => &["error", "warn", "warning", "panic", "exception", "failed"],
        PipelineMode::TestOnly => &["fail", "failed", "error", "panic", "exception", "traceback"],
        PipelineMode::Normal => &[],
    };

    if !signal_patterns.is_empty() {
        let signal = signal_only(&merged, signal_patterns, config.max_lines);
        if !signal.is_empty() {
            let filtered_lines = line_count(&signal);
            return Ok(PipelineResult {
                exit_code: process_output.code,
                output: signal,
                fallback_used: false,
                details: PipelineDetails {
                    mode,
                    classifier_kind,
                    strategy: PipelineStrategy::SignalOnly { mode },
                    raw_lines,
                    filtered_lines,
                    dropped_lines: raw_lines.saturating_sub(filtered_lines),
                },
            });
        }
    }

    let compacted = std::panic::catch_unwind(|| compact_by_kind(&merged, classifier_kind, config));

    let (output, fallback_used) = match compacted {
        Ok(text) if !text.trim().is_empty() || merged.trim().is_empty() => (text, false),
        _ => (merged, true),
    };
    let filtered_lines = line_count(&output);
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
