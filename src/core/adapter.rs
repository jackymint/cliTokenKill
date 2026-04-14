use crate::core::filter::{FilterConfig, FilterLevel, compact_output, signal_only};
use regex::Regex;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const ADAPTER_DEBUG_ENV: &str = "CTK_ADAPTER_DEBUG";
const PROJECT_ADAPTER_DIR: &str = ".ctk/adapters";
const GLOBAL_ADAPTER_DIR: &str = ".ctk/adapters";

pub struct AdapterResult {
    pub name: String,
    pub output: String,
}

#[derive(Debug, Deserialize)]
struct AdapterFile {
    #[serde(default)]
    adapter: Vec<AdapterSpec>,
}

#[derive(Debug, Deserialize)]
struct AdapterSpec {
    name: String,
    match_command: String,
    #[serde(default)]
    signal_patterns: Vec<String>,
    #[serde(default)]
    include_patterns: Vec<String>,
    #[serde(default)]
    exclude_patterns: Vec<String>,
    on_empty: Option<String>,
    level: Option<String>,
    max_lines: Option<usize>,
    max_chars_per_line: Option<usize>,
    priority: Option<i32>,
}

#[derive(Clone)]
struct AdapterRule {
    name: String,
    match_command: Regex,
    signal_patterns: Vec<String>,
    include_patterns: Vec<Regex>,
    exclude_patterns: Vec<Regex>,
    on_empty: Option<String>,
    level: Option<FilterLevel>,
    max_lines: Option<usize>,
    max_chars_per_line: Option<usize>,
    priority: i32,
    sequence: usize,
}

impl AdapterRule {
    fn apply(&self, output: &str, base_config: FilterConfig) -> Option<String> {
        let max_lines = self.max_lines.unwrap_or(base_config.max_lines);

        let mut candidate = if self.signal_patterns.is_empty() {
            output.to_string()
        } else {
            let patterns: Vec<&str> = self.signal_patterns.iter().map(String::as_str).collect();
            signal_only(output, &patterns, max_lines)
        };

        if !self.include_patterns.is_empty() {
            candidate = candidate
                .lines()
                .filter(|line| self.include_patterns.iter().any(|rx| rx.is_match(line)))
                .collect::<Vec<_>>()
                .join("\n");
        }

        if !self.exclude_patterns.is_empty() {
            candidate = candidate
                .lines()
                .filter(|line| !self.exclude_patterns.iter().any(|rx| rx.is_match(line)))
                .collect::<Vec<_>>()
                .join("\n");
        }

        if candidate.trim().is_empty() {
            return self.on_empty.clone();
        }

        let compacted = compact_output(
            &candidate,
            FilterConfig {
                level: self.level.unwrap_or(base_config.level),
                max_lines,
                max_chars_per_line: self
                    .max_chars_per_line
                    .unwrap_or(base_config.max_chars_per_line),
            },
        );

        if compacted.trim().is_empty() {
            self.on_empty.clone()
        } else {
            Some(compacted)
        }
    }
}

pub fn apply_command_adapter(
    command: &[String],
    output: &str,
    base_config: FilterConfig,
) -> Option<AdapterResult> {
    let cmdline = command_line(command);
    let mut rules = load_rules();
    if rules.is_empty() {
        return None;
    }

    rules.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.sequence.cmp(&b.sequence))
    });

    for rule in rules {
        if !rule.match_command.is_match(&cmdline) {
            continue;
        }
        if let Some(rendered) = rule.apply(output, base_config) {
            debug_log(&format!(
                "adapter matched: {} (priority={})",
                rule.name, rule.priority
            ));
            return Some(AdapterResult {
                name: rule.name,
                output: rendered,
            });
        }
    }

    None
}

mod loader;
#[cfg(test)]
mod tests;

use self::loader::load_rules;

fn command_line(command: &[String]) -> String {
    command.join(" ")
}

fn debug_enabled() -> bool {
    let Ok(raw) = env::var(ADAPTER_DEBUG_ENV) else {
        return false;
    };

    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn debug_log(message: &str) {
    if debug_enabled() {
        eprintln!("ctk adapter: {message}");
    }
}
