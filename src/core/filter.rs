use regex::Regex;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum FilterLevel {
    None,
    Minimal,
    Aggressive,
}

#[derive(Clone, Copy, Debug)]
pub struct FilterConfig {
    pub level: FilterLevel,
    pub max_lines: usize,
    pub max_chars_per_line: usize,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            level: FilterLevel::Minimal,
            max_lines: 120,
            max_chars_per_line: 240,
        }
    }
}

pub fn compact_output(input: &str, config: FilterConfig) -> String {
    let mut runs: Vec<(String, usize)> = Vec::new();
    let mut prev_was_empty = false;

    for raw in input.lines() {
        let line = truncate_line(raw.trim_end(), config.max_chars_per_line);
        let is_empty = line.trim().is_empty();

        if is_empty && prev_was_empty {
            continue;
        }
        prev_was_empty = is_empty;

        if let Some((last, count)) = runs.last_mut()
            && *last == line
        {
            *count += 1;
            continue;
        }
        runs.push((line, 1));
    }

    let mut lines: Vec<String> = runs
        .into_iter()
        .map(|(line, count)| {
            if count > 1 {
                format!("{line}  [x{count}]")
            } else {
                line
            }
        })
        .collect();

    lines = apply_level(lines, config.level);

    if lines.len() > config.max_lines {
        let omitted = lines.len() - config.max_lines;
        lines = truncate_head_tail(lines, config.max_lines);
        lines.push(format!("... truncated {omitted} lines"));
    }

    lines.join("\n")
}

pub fn signal_only(input: &str, patterns: &[&str], max_lines: usize) -> String {
    let regexes: Vec<Regex> = patterns
        .iter()
        .filter_map(|p| Regex::new(&format!("(?i){p}")).ok())
        .collect();

    let mut lines: Vec<String> = input
        .lines()
        .map(str::trim_end)
        .filter(|line| regexes.iter().any(|rx| rx.is_match(line)))
        .map(ToString::to_string)
        .collect();

    if lines.is_empty() {
        return String::new();
    }

    if lines.len() > max_lines {
        let omitted = lines.len() - max_lines;
        lines = truncate_head_tail(lines, max_lines);
        lines.push(format!("... truncated {omitted} lines"));
    }

    lines.join("\n")
}

fn apply_level(lines: Vec<String>, level: FilterLevel) -> Vec<String> {
    match level {
        FilterLevel::None => lines,
        FilterLevel::Minimal => lines,
        FilterLevel::Aggressive => lines
            .into_iter()
            .filter(|line| {
                let trimmed = line.trim();
                !(trimmed.starts_with("warning:")
                    || trimmed.starts_with("note:")
                    || trimmed.starts_with("hint:")
                    || trimmed.starts_with("info:"))
            })
            .collect(),
    }
}

fn truncate_line(line: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return line.to_string();
    }

    let count = line.chars().count();
    if count <= max_chars {
        return line.to_string();
    }

    let keep = max_chars.saturating_sub(20);
    let prefix: String = line.chars().take(keep).collect();
    format!("{prefix} ... [truncated {} chars]", count - keep)
}

fn truncate_head_tail(mut lines: Vec<String>, max_lines: usize) -> Vec<String> {
    if lines.len() <= max_lines || max_lines == 0 {
        return lines;
    }

    if max_lines == 1 {
        lines.truncate(1);
        return lines;
    }

    let head = (max_lines * 2) / 3;
    let tail = max_lines - head;

    let mut out = Vec::with_capacity(max_lines);
    out.extend(lines.iter().take(head).cloned());
    out.extend(lines.iter().rev().take(tail).cloned().rev());
    out
}
