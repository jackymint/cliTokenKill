use crate::core::filter::{FilterConfig, compact_output, signal_only};

const STACK_TRACE_SIGNAL_PATTERNS: &[&str] =
    &["panic", "exception", "traceback", "error", "at .+:[0-9]+"];

pub fn compact(output: &str, config: FilterConfig) -> String {
    let signal = signal_only(output, STACK_TRACE_SIGNAL_PATTERNS, config.max_lines);
    if signal.is_empty() {
        compact_output(output, config)
    } else {
        signal
    }
}
