use super::*;
use crate::core::filter::{FilterConfig, FilterLevel};

fn config() -> FilterConfig {
    FilterConfig {
        level: FilterLevel::Minimal,
        max_lines: 120,
        max_chars_per_line: 240,
    }
}

fn sh(script: &str) -> Vec<String> {
    vec!["sh".to_string(), "-c".to_string(), script.to_string()]
}

// --- merge_output ---

#[test]
fn merge_stdout_only() {
    assert_eq!(merge_output("hello", ""), "hello");
}

#[test]
fn merge_stderr_only() {
    assert_eq!(merge_output("", "error"), "error");
}

#[test]
fn merge_both_nonempty() {
    let merged = merge_output("out", "err");
    assert!(merged.contains("out"));
    assert!(merged.contains("err"));
}

#[test]
fn merge_both_empty() {
    assert_eq!(merge_output("", ""), "");
}

// --- run_pipeline: exit code ---

#[test]
fn pipeline_preserves_zero_exit() {
    let result = run_pipeline(&sh("echo ok"), config(), PipelineMode::Normal).unwrap();
    assert_eq!(result.exit_code, 0);
}

#[test]
fn pipeline_preserves_nonzero_exit() {
    let result = run_pipeline(&sh("exit 42"), config(), PipelineMode::Normal).unwrap();
    assert_eq!(result.exit_code, 42);
}

// --- run_pipeline: strategy selection ---

#[test]
fn pipeline_normal_uses_content_aware_strategy() {
    let result = run_pipeline(&sh("echo hello"), config(), PipelineMode::Normal).unwrap();
    assert!(matches!(
        result.details.strategy,
        PipelineStrategy::ContentAware { .. } | PipelineStrategy::RawFallback { .. }
    ));
    assert!(!result.output.is_empty());
}

#[test]
fn pipeline_error_only_selects_signal_when_matched() {
    let result = run_pipeline(
        &sh("echo 'error: build failed'"),
        config(),
        PipelineMode::ErrorOnly,
    )
    .unwrap();
    assert!(matches!(
        result.details.strategy,
        PipelineStrategy::SignalOnly { .. }
    ));
    assert!(result.output.contains("error"));
    assert!(!result.fallback_used);
}

#[test]
fn pipeline_error_only_falls_through_when_no_signal() {
    let result = run_pipeline(
        &sh("echo 'everything is fine'"),
        config(),
        PipelineMode::ErrorOnly,
    )
    .unwrap();
    assert!(!matches!(
        result.details.strategy,
        PipelineStrategy::SignalOnly { .. }
    ));
}

#[test]
fn pipeline_test_only_selects_signal_when_matched() {
    let result = run_pipeline(
        &sh("echo 'test result: FAILED. 1 failed'"),
        config(),
        PipelineMode::TestOnly,
    )
    .unwrap();
    assert!(matches!(
        result.details.strategy,
        PipelineStrategy::SignalOnly { .. }
    ));
}

// --- run_pipeline: details ---

#[test]
fn pipeline_details_raw_lines_correct() {
    let result = run_pipeline(&sh("printf 'a\\nb\\nc'"), config(), PipelineMode::Normal).unwrap();
    assert_eq!(result.details.raw_lines, 3);
}

#[test]
fn pipeline_details_dropped_lines_consistent() {
    let result = run_pipeline(&sh("echo hello"), config(), PipelineMode::Normal).unwrap();
    let d = &result.details;
    assert_eq!(
        d.dropped_lines,
        d.raw_lines.saturating_sub(d.filtered_lines)
    );
}

#[test]
fn pipeline_stage_reports_always_include_merge() {
    let result = run_pipeline(&sh("echo hello"), config(), PipelineMode::Normal).unwrap();
    assert_eq!(result.details.stage_reports[0].stage, "merge");
    assert!(result.details.stage_reports[0].selected);
}
