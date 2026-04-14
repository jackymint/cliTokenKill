use super::*;
use std::time::Instant;

fn push_stage_report(
    stage_reports: &mut Vec<PipelineStageReport>,
    stage: &'static str,
    selected: bool,
    lines_before: usize,
    lines_after: usize,
    started: Instant,
    reason: impl Into<String>,
) {
    stage_reports.push(PipelineStageReport::new(
        stage,
        selected,
        lines_before,
        lines_after,
        started.elapsed().as_millis() as u64,
        reason,
    ));
}

pub(super) struct AdapterStageOutput {
    pub(super) adapter_name: String,
    pub(super) output: String,
    pub(super) filtered_lines: usize,
}

pub(super) fn run_adapter_stage(
    command: &[String],
    merged: &str,
    config: FilterConfig,
    raw_lines: usize,
    stage_reports: &mut Vec<PipelineStageReport>,
) -> Option<AdapterStageOutput> {
    let started = Instant::now();
    if let Some(applied) = apply_command_adapter(command, merged, config) {
        let filtered_lines = line_count(&applied.output);
        push_stage_report(
            stage_reports,
            "adapter",
            true,
            raw_lines,
            filtered_lines,
            started,
            format!("matched adapter '{}'", applied.name),
        );
        return Some(AdapterStageOutput {
            adapter_name: applied.name,
            output: applied.output,
            filtered_lines,
        });
    }

    push_stage_report(
        stage_reports,
        "adapter",
        false,
        raw_lines,
        raw_lines,
        started,
        "no adapter matched",
    );
    None
}

pub(super) struct SignalStageOutput {
    pub(super) output: String,
    pub(super) filtered_lines: usize,
}

pub(super) fn run_signal_stage(
    mode: PipelineMode,
    merged: &str,
    config: FilterConfig,
    raw_lines: usize,
    stage_reports: &mut Vec<PipelineStageReport>,
) -> Option<SignalStageOutput> {
    let started = Instant::now();
    let patterns = signal_patterns(mode);
    if patterns.is_empty() {
        push_stage_report(
            stage_reports,
            "signal-only",
            false,
            raw_lines,
            raw_lines,
            started,
            "mode=normal",
        );
        return None;
    }

    let signal = signal_only(merged, patterns, config.max_lines);
    if signal.trim().is_empty() {
        push_stage_report(
            stage_reports,
            "signal-only",
            false,
            raw_lines,
            raw_lines,
            started,
            format!("mode={} no signal match", mode.as_str()),
        );
        return None;
    }

    let filtered_lines = line_count(&signal);
    push_stage_report(
        stage_reports,
        "signal-only",
        true,
        raw_lines,
        filtered_lines,
        started,
        format!("mode={} signal patterns matched", mode.as_str()),
    );

    Some(SignalStageOutput {
        output: signal,
        filtered_lines,
    })
}

pub(super) fn run_content_aware_stage(
    merged: &str,
    classifier_kind: ContentKind,
    config: FilterConfig,
) -> (String, bool, u64) {
    let started = Instant::now();
    let compacted = std::panic::catch_unwind(|| compact_by_kind(merged, classifier_kind, config));

    match compacted {
        Ok(text) if !text.trim().is_empty() || merged.trim().is_empty() => {
            (text, false, started.elapsed().as_millis() as u64)
        }
        _ => (
            merged.to_string(),
            true,
            started.elapsed().as_millis() as u64,
        ),
    }
}

fn signal_patterns(mode: PipelineMode) -> &'static [&'static str] {
    match mode {
        PipelineMode::ErrorOnly => ERROR_ONLY_PATTERNS,
        PipelineMode::TestOnly => TEST_ONLY_PATTERNS,
        PipelineMode::Normal => &[],
    }
}

pub(super) fn is_adapter_debug_enabled() -> bool {
    let Ok(raw) = env::var("CTK_ADAPTER_DEBUG") else {
        return false;
    };

    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

pub(super) fn line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count()
    }
}
