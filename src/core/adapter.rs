use crate::core::filter::{FilterConfig, FilterLevel, compact_output, signal_only};
use regex::Regex;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const ADAPTER_DEBUG_ENV: &str = "CTK_ADAPTER_DEBUG";
const PROJECT_ADAPTER_DIR: &str = ".ctk/adapters";
const GLOBAL_ADAPTER_DIR: &str = ".ctk/adapters";

test
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

fn load_rules() -> Vec<AdapterRule> {
    let mut rules = Vec::new();
    let mut sequence = 0usize;

    for dir in adapter_dirs() {
        let mut loaded = load_rules_from_dir(&dir, sequence);
        sequence += loaded.len();
        rules.append(&mut loaded);
    }

    if !rules.is_empty() {
        debug_log(&format!("loaded {} adapter rules", rules.len()));
    }

    rules
}

fn load_rules_from_dir(dir: &Path, sequence_start: usize) -> Vec<AdapterRule> {
    if !dir.exists() {
        return Vec::new();
    }

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            debug_log(&format!(
                "failed to read adapter dir {}: {err}",
                dir.display()
            ));
            return Vec::new();
        }
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| is_toml_file(path))
        .collect();
    files.sort();

    let mut out = Vec::new();
    for (file_idx, file) in files.iter().enumerate() {
        let raw = match fs::read_to_string(file) {
            Ok(text) => text,
            Err(err) => {
                debug_log(&format!(
                    "failed to read adapter file {}: {err}",
                    file.display()
                ));
                continue;
            }
        };

        let parsed = match toml::from_str::<AdapterFile>(&raw) {
            Ok(spec) => spec,
            Err(err) => {
                debug_log(&format!(
                    "failed to parse adapter file {}: {err}",
                    file.display()
                ));
                continue;
            }
        };

        for (spec_idx, spec) in parsed.adapter.iter().enumerate() {
            let sequence = sequence_start + (file_idx * 1000) + spec_idx;
            if let Some(rule) = compile_rule(spec, file, sequence) {
                out.push(rule);
            }
        }
    }

    out
}

fn compile_rule(spec: &AdapterSpec, file: &Path, sequence: usize) -> Option<AdapterRule> {
    let match_command = match Regex::new(&spec.match_command) {
        Ok(rx) => rx,
        Err(err) => {
            debug_log(&format!(
                "adapter '{}' skipped (invalid match_command regex in {}): {err}",
                spec.name,
                file.display()
            ));
            return None;
        }
    };

    let include_patterns = compile_regexes(&spec.include_patterns, file, &spec.name, "include");
    let exclude_patterns = compile_regexes(&spec.exclude_patterns, file, &spec.name, "exclude");

    let level = parse_level(spec.level.as_deref(), file, &spec.name);

    Some(AdapterRule {
        name: spec.name.clone(),
        match_command,
        signal_patterns: spec.signal_patterns.clone(),
        include_patterns,
        exclude_patterns,
        on_empty: spec.on_empty.clone(),
        level,
        max_lines: spec.max_lines,
        max_chars_per_line: spec.max_chars_per_line,
        priority: spec.priority.unwrap_or(0),
        sequence,
    })
}

fn compile_regexes(patterns: &[String], file: &Path, name: &str, label: &str) -> Vec<Regex> {
    let mut out = Vec::new();
    for raw in patterns {
        match Regex::new(raw) {
            Ok(rx) => out.push(rx),
            Err(err) => debug_log(&format!(
                "adapter '{}' skipped invalid {} regex '{}' in {}: {err}",
                name,
                label,
                raw,
                file.display()
            )),
        }
    }
    out
}

fn parse_level(raw: Option<&str>, file: &Path, name: &str) -> Option<FilterLevel> {
    let raw = raw?;

    match raw.trim().to_ascii_lowercase().as_str() {
        "none" => Some(FilterLevel::None),
        "minimal" => Some(FilterLevel::Minimal),
        "aggressive" => Some(FilterLevel::Aggressive),
        _ => {
            debug_log(&format!(
                "adapter '{}' ignored unknown level '{}' in {}",
                name,
                raw,
                file.display()
            ));
            None
        }
    }
}

fn adapter_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(cwd) = env::current_dir() {
        dirs.push(cwd.join(PROJECT_ADAPTER_DIR));
    }

    if let Some(home) = env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(GLOBAL_ADAPTER_DIR));
    }

    dirs
}

fn is_toml_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("toml"))
        .unwrap_or(false)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> FilterConfig {
        FilterConfig {
            level: FilterLevel::Minimal,
            max_lines: 50,
            max_chars_per_line: 200,
        }
    }

    #[test]
    fn adapter_applies_signal_and_exclude_rules() {
        let spec = AdapterSpec {
            name: "cargo-test".to_string(),
            match_command: "^cargo test".to_string(),
            signal_patterns: vec!["failed".to_string(), "panic".to_string()],
            include_patterns: vec![],
            exclude_patterns: vec!["note:".to_string()],
            on_empty: Some("ok: no failures".to_string()),
            level: Some("minimal".to_string()),
            max_lines: Some(20),
            max_chars_per_line: Some(160),
            priority: Some(10),
        };

        let rule = compile_rule(&spec, Path::new("test.toml"), 0).expect("rule compiles");

        let output = "running 2 tests\ntest a ... ok\nthread 'x' panicked at src/lib.rs:9\nnote: run with RUST_BACKTRACE=1\n";

        let rendered = rule
            .apply(output, default_config())
            .expect("rendered output");

        assert!(rendered.contains("panicked"));
        assert!(!rendered.contains("note:"));
    }

    #[test]
    fn adapter_uses_on_empty_when_filters_drop_everything() {
        let spec = AdapterSpec {
            name: "quiet".to_string(),
            match_command: ".*".to_string(),
            signal_patterns: vec![],
            include_patterns: vec!["^ERROR".to_string()],
            exclude_patterns: vec![],
            on_empty: Some("ok".to_string()),
            level: None,
            max_lines: None,
            max_chars_per_line: None,
            priority: None,
        };

        let rule = compile_rule(&spec, Path::new("test.toml"), 0).expect("rule compiles");
        let rendered = rule
            .apply("all good", default_config())
            .expect("on_empty value");

        assert_eq!(rendered, "ok");
    }
}
