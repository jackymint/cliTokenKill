use crate::core::filter::{FilterConfig, FilterLevel, compact_output, signal_only};

const TEST_SIGNAL_PATTERNS: &[&str] =
    &["fail", "failed", "error", "panic", "exception", "traceback"];

pub fn compact(output: &str, config: FilterConfig) -> String {
    let signal = signal_only(output, TEST_SIGNAL_PATTERNS, config.max_lines);
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
