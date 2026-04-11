use crate::core::filter::{FilterConfig, FilterLevel, compact_output, signal_only};
use regex::Regex;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentKind {
    Json,
    Ndjson,
    Diff,
    GrepLike,
    StackTrace,
    TestOutput,
    TableText,
    LogStream,
    Plain,
}

impl ContentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ContentKind::Json => "json",
            ContentKind::Ndjson => "ndjson",
            ContentKind::Diff => "diff",
            ContentKind::GrepLike => "grep-like",
            ContentKind::StackTrace => "stack-trace",
            ContentKind::TestOutput => "test-output",
            ContentKind::TableText => "table-text",
            ContentKind::LogStream => "log-stream",
            ContentKind::Plain => "plain",
        }
    }
}

pub fn classify_content(output: &str) -> ContentKind {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return ContentKind::Plain;
    }

    if looks_like_ndjson(output) {
        return ContentKind::Ndjson;
    }
    if serde_json::from_str::<Value>(trimmed).is_ok() {
        return ContentKind::Json;
    }
    if looks_like_diff(output) {
        return ContentKind::Diff;
    }
    if looks_like_stack_trace(output) {
        return ContentKind::StackTrace;
    }
    if looks_like_test_output(output) {
        return ContentKind::TestOutput;
    }
    if looks_like_log(output) {
        return ContentKind::LogStream;
    }
    if looks_like_grep(output) {
        return ContentKind::GrepLike;
    }
    if looks_like_table(output) {
        return ContentKind::TableText;
    }

    ContentKind::Plain
}

pub fn compact_by_kind(output: &str, kind: ContentKind, config: FilterConfig) -> String {
    match kind {
        ContentKind::Json | ContentKind::Ndjson => compact_json_like(output, config),
        ContentKind::Diff => compact_diff(output, config),
        ContentKind::GrepLike => compact_grep(output, config),
        ContentKind::StackTrace => compact_stack_trace(output, config),
        ContentKind::TestOutput => compact_test_output(output, config),
        ContentKind::TableText => compact_output(
            output,
            FilterConfig {
                level: FilterLevel::Minimal,
                max_lines: config.max_lines,
                max_chars_per_line: config.max_chars_per_line,
            },
        ),
        ContentKind::LogStream => compact_logs(output, config),
        ContentKind::Plain => compact_output(output, config),
    }
}

fn compact_json_like(output: &str, config: FilterConfig) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(output.trim()) {
        let normalized =
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| output.to_string());
        return compact_output(&redact_long_json_strings(&normalized), config);
    }

    let lines: Vec<String> = output.lines().map(redact_long_json_strings).collect();
    compact_output(&lines.join("\n"), config)
}

fn compact_diff(output: &str, config: FilterConfig) -> String {
    let filtered: Vec<&str> = output
        .lines()
        .filter(|line| {
            line.starts_with("diff --git")
                || line.starts_with("index ")
                || line.starts_with("--- ")
                || line.starts_with("+++ ")
                || line.starts_with("@@")
                || line.starts_with('+')
                || line.starts_with('-')
        })
        .collect();
    let candidate = if filtered.is_empty() {
        output.to_string()
    } else {
        filtered.join("\n")
    };
    compact_output(&candidate, config)
}

fn compact_grep(output: &str, config: FilterConfig) -> String {
    let mut lines: Vec<String> = output
        .lines()
        .map(str::trim_end)
        .map(ToString::to_string)
        .collect();
    lines.sort();
    compact_output(&lines.join("\n"), config)
}

fn compact_stack_trace(output: &str, config: FilterConfig) -> String {
    let signal = signal_only(
        output,
        &["panic", "exception", "traceback", "error", "at .+:[0-9]+"],
        config.max_lines,
    );
    if signal.is_empty() {
        compact_output(output, config)
    } else {
        signal
    }
}

fn compact_test_output(output: &str, config: FilterConfig) -> String {
    let signal = signal_only(
        output,
        &["fail", "failed", "error", "panic", "exception", "traceback"],
        config.max_lines,
    );
    if signal.is_empty() {
        compact_output(
            output,
            FilterConfig {
                level: FilterLevel::Minimal,
                max_lines: config.max_lines,
                max_chars_per_line: config.max_chars_per_line,
            },
        )
    } else {
        signal
    }
}

fn compact_logs(output: &str, config: FilterConfig) -> String {
    compact_output(
        output,
        FilterConfig {
            level: FilterLevel::Aggressive,
            max_lines: config.max_lines,
            max_chars_per_line: config.max_chars_per_line,
        },
    )
}

fn redact_long_json_strings(line: &str) -> String {
    let re = Regex::new(r#":\s*"([^"\\]|\\.){80,}""#).expect("valid json string regex");
    re.replace_all(line, ": \"<str:long>\"").to_string()
}

fn looks_like_ndjson(output: &str) -> bool {
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(5)
        .collect();
    lines.len() >= 2
        && lines
            .iter()
            .all(|l| serde_json::from_str::<Value>(l.trim()).is_ok())
}

fn looks_like_diff(output: &str) -> bool {
    output.contains("diff --git") || output.contains("@@")
}

fn looks_like_stack_trace(output: &str) -> bool {
    output.contains("Traceback")
        || output.contains("stack backtrace:")
        || output.contains("Exception")
        || output.contains(" at ")
}

fn looks_like_test_output(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.contains("test result")
        || lower.contains("passed")
        || lower.contains("failed")
        || lower.contains("collected ")
}

fn looks_like_grep(output: &str) -> bool {
    output
        .lines()
        .take(10)
        .filter(|line| line.contains(':'))
        .count()
        >= 3
}

fn looks_like_table(output: &str) -> bool {
    output.lines().take(6).any(|l| l.contains('|')) && output.contains("---")
}

fn looks_like_log(output: &str) -> bool {
    let ts = Regex::new(r"\d{4}-\d{2}-\d{2}").expect("valid timestamp regex");
    output.lines().take(10).filter(|l| ts.is_match(l)).count() >= 3
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden_config() -> FilterConfig {
        FilterConfig {
            level: FilterLevel::Minimal,
            max_lines: 120,
            max_chars_per_line: 240,
        }
    }

    fn normalize(text: &str) -> String {
        text.trim_end().replace("\r\n", "\n")
    }

    fn assert_golden(input: &str, expected: &str, kind: ContentKind) {
        assert_eq!(classify_content(input), kind);
        let actual = compact_by_kind(input, kind, golden_config());
        assert_eq!(normalize(&actual), normalize(expected));
    }

    #[test]
    fn golden_json_compaction() {
        let input = include_str!("../../tests/golden/json.input.txt");
        let expected = include_str!("../../tests/golden/json.expected.txt");
        assert_golden(input, expected, ContentKind::Json);
    }

    #[test]
    fn golden_ndjson_compaction() {
        let input = include_str!("../../tests/golden/ndjson.input.txt");
        let expected = include_str!("../../tests/golden/ndjson.expected.txt");
        assert_golden(input, expected, ContentKind::Ndjson);
    }

    #[test]
    fn golden_diff_compaction() {
        let input = include_str!("../../tests/golden/diff.input.txt");
        let expected = include_str!("../../tests/golden/diff.expected.txt");
        assert_golden(input, expected, ContentKind::Diff);
    }

    #[test]
    fn golden_log_compaction() {
        let input = include_str!("../../tests/golden/log.input.txt");
        let expected = include_str!("../../tests/golden/log.expected.txt");
        assert_golden(input, expected, ContentKind::LogStream);
    }

    #[test]
    fn golden_stacktrace_compaction() {
        let input = include_str!("../../tests/golden/stacktrace.input.txt");
        let expected = include_str!("../../tests/golden/stacktrace.expected.txt");
        assert_golden(input, expected, ContentKind::StackTrace);
    }
}
