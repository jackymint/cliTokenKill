use crate::core::filter::{FilterConfig, compact_output};

pub fn compact(output: &str, config: FilterConfig) -> String {
    let mut lines: Vec<String> = output
        .lines()
        .map(str::trim_end)
        .map(ToString::to_string)
        .collect();
    lines.sort();
    compact_output(&lines.join("\n"), config)
}
