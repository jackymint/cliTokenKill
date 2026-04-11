use crate::core::filter::{FilterConfig, compact_output};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

pub fn compact(output: &str, config: FilterConfig) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(output.trim()) {
        let normalized =
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| output.to_string());
        return compact_output(&redact_long_json_strings(&normalized), config);
    }

    let lines: Vec<String> = output.lines().map(redact_long_json_strings).collect();
    compact_output(&lines.join("\n"), config)
}

fn redact_long_json_strings(line: &str) -> String {
    long_json_string_regex()
        .replace_all(line, ": \"<str:long>\"")
        .to_string()
}

fn long_json_string_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#":\s*\"([^\"\\]|\\.){80,}\""#).expect("valid json string regex"))
}
