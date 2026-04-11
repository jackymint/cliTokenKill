use crate::core::budget::{BudgetResult, apply_token_budget_with_report};
use crate::core::chunk::{ChunkPlan, plan_auto_chunk};
use crate::core::filter::{FilterConfig, FilterLevel};
use crate::core::pipeline::{PipelineMode, PipelineStrategy, run_pipeline};
use crate::engine::{ContentKind, classify_content, compact_by_kind};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn explain_command(command: &[String], config: FilterConfig, mode: PipelineMode) -> Result<()> {
    let result = run_pipeline(command, config, mode)?;
    let max_lines_trimmed = has_max_line_trim_marker(&result.output);
    let budget = apply_token_budget_with_report(result.output.clone());
    let chunk = plan_auto_chunk(&budget.output);

    println!("ctk explain");
    println!("target: command");
    println!("command: {}", command.join(" "));
    println!("mode: {}", result.details.mode.as_str());
    println!("classifier: {}", result.details.classifier_kind.as_str());
    println!("strategy: {}", result.details.strategy.label());
    println!("filter: {}", strategy_filter_name(&result.details.strategy));
    println!(
        "lines: raw={} filtered={} removed={}",
        result.details.raw_lines, result.details.filtered_lines, result.details.dropped_lines
    );
    println!(
        "trim cause: max-lines={} token-budget={}",
        max_lines_trimmed, budget.trimmed
    );
    print_budget_summary(&budget);
    print_chunk_summary(&chunk);
    println!("fallback: {}", result.fallback_used);
    println!("exit code: {}", result.exit_code);
    println!("filter level: {}", filter_level_name(config.level));
    println!();
    println!("--- compacted output ---");
    println!("{}", budget.output);

    if result.exit_code != 0 {
        std::process::exit(result.exit_code);
    }
    Ok(())
}

pub fn explain_file(file: &Path, config: FilterConfig) -> Result<()> {
    let content = fs::read_to_string(file)
        .with_context(|| format!("failed to read file: {}", file.display()))?;

    let kind = classify_content(&content);
    let compacted = std::panic::catch_unwind(|| compact_by_kind(&content, kind, config));

    let (output, fallback_used) = match compacted {
        Ok(text) if !text.trim().is_empty() || content.trim().is_empty() => (text, false),
        _ => (content.clone(), true),
    };

    let raw_lines = line_count(&content);
    let filtered_lines = line_count(&output);
    let dropped_lines = raw_lines.saturating_sub(filtered_lines);
    let max_lines_trimmed = has_max_line_trim_marker(&output);
    let budget = apply_token_budget_with_report(output);
    let chunk = plan_auto_chunk(&budget.output);

    println!("ctk explain");
    println!("target: file");
    println!("file: {}", file.display());
    println!("classifier: {}", kind.as_str());
    println!("strategy: {}", file_strategy_label(kind, fallback_used));
    println!("filter: {}", file_filter_name(kind, fallback_used));
    println!(
        "lines: raw={} filtered={} removed={}",
        raw_lines, filtered_lines, dropped_lines
    );
    println!(
        "trim cause: max-lines={} token-budget={}",
        max_lines_trimmed, budget.trimmed
    );
    print_budget_summary(&budget);
    print_chunk_summary(&chunk);
    println!("fallback: {}", fallback_used);
    println!("filter level: {}", filter_level_name(config.level));
    println!();
    println!("--- compacted output ---");
    println!("{}", budget.output);

    Ok(())
}

fn file_strategy_label(kind: ContentKind, fallback_used: bool) -> String {
    if fallback_used {
        format!("fallback-raw:{}", kind.as_str())
    } else {
        format!("content-aware:{}", kind.as_str())
    }
}

fn file_filter_name(kind: ContentKind, fallback_used: bool) -> String {
    if fallback_used {
        "raw_output(fallback)".to_string()
    } else {
        format!("compact_by_kind::<{}>", kind.as_str())
    }
}

fn strategy_filter_name(strategy: &PipelineStrategy) -> String {
    match strategy {
        PipelineStrategy::Adapter { name } => format!("adapter:{name}"),
        PipelineStrategy::SignalOnly { mode } => {
            format!("signal_only(pattern-set={})", mode.as_str())
        }
        PipelineStrategy::ContentAware { kind } => {
            format!("compact_by_kind::<{}>", kind.as_str())
        }
        PipelineStrategy::RawFallback { .. } => "raw_output(fallback)".to_string(),
    }
}

fn print_budget_summary(budget: &BudgetResult) {
    println!(
        "budget: limit={} est_before={} est_after={} trimmed={}",
        budget.token_budget,
        budget.estimated_tokens_before,
        budget.estimated_tokens_after,
        budget.trimmed
    );
    println!(
        "budget lines: before={} after={} removed={}",
        budget.lines_before,
        budget.lines_after,
        budget.lines_before.saturating_sub(budget.lines_after)
    );
    if let Some(line) = budget.marker_line {
        println!("budget trim marker line: {line}");
    } else {
        println!("budget trim marker line: none");
    }
}

fn print_chunk_summary(chunk: &ChunkPlan) {
    println!(
        "chunk: triggered={} chunks={} lines={}",
        chunk.triggered, chunk.total_chunks, chunk.total_lines
    );
}

fn has_max_line_trim_marker(text: &str) -> bool {
    text.lines()
        .any(|line| line.starts_with("... truncated ") && line.ends_with(" lines"))
}

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count()
    }
}

fn filter_level_name(level: FilterLevel) -> &'static str {
    match level {
        FilterLevel::None => "none",
        FilterLevel::Minimal => "minimal",
        FilterLevel::Aggressive => "aggressive",
    }
}
