use crate::core::filter::{FilterConfig, compact_output};

const DIFF_PREFIXES: &[&str] = &["diff --git", "index ", "--- ", "+++ ", "@@"];

pub fn compact(output: &str, config: FilterConfig) -> String {
    let filtered: Vec<&str> = output
        .lines()
        .filter(|line| {
            DIFF_PREFIXES.iter().any(|prefix| line.starts_with(prefix))
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
