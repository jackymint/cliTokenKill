use crate::core::filter::{FilterConfig, FilterLevel, compact_output};

pub fn compact(output: &str, config: FilterConfig) -> String {
    compact_output(
        output,
        FilterConfig {
            level: FilterLevel::Minimal,
            max_lines: config.max_lines,
            max_chars_per_line: config.max_chars_per_line,
        },
    )
}
