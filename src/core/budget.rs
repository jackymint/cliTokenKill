use std::env;

const DEFAULT_TOKEN_BUDGET: usize = 900;
pub const BUDGET_TRIM_MARKER_PREFIX: &str = "... [ctk budget-trim:";

pub struct BudgetResult {
    pub output: String,
    pub token_budget: usize,
    pub estimated_tokens_before: usize,
    pub estimated_tokens_after: usize,
    pub lines_before: usize,
    pub lines_after: usize,
    pub trimmed: bool,
    pub marker_line: Option<usize>,
}

pub fn apply_token_budget(text: String) -> String {
    apply_token_budget_with_report(text).output
}

pub fn apply_token_budget_with_report(text: String) -> BudgetResult {
    let budget = env::var("CTK_TOKEN_BUDGET")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_TOKEN_BUDGET);
    apply_budget_with_limit(text, budget)
}

fn apply_budget_with_limit(text: String, token_budget: usize) -> BudgetResult {
    let estimated_tokens_before = estimate_tokens(&text);
    let lines_before = line_count(&text);

    if token_budget == 0 {
        let output = String::new();
        return BudgetResult {
            output,
            token_budget,
            estimated_tokens_before,
            estimated_tokens_after: 0,
            lines_before,
            lines_after: 0,
            trimmed: !text.is_empty(),
            marker_line: None,
        };
    }

    let output = if estimated_tokens_before <= token_budget {
        text.clone()
    } else {
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            text.clone()
        } else {
            // Keep 60% head + 40% tail when over budget.
            let target_chars = token_budget.saturating_mul(4);
            let head_chars = (target_chars * 3) / 5;
            let tail_chars = target_chars.saturating_sub(head_chars);

            let head = take_chars_from_start(&lines, head_chars);
            let tail = take_chars_from_end(&lines, tail_chars);

            format!(
                "{head}\n... [ctk budget-trim: est_tokens={estimated_tokens_before} limit={token_budget}]\n{tail}"
            )
        }
    };

    let marker_line = output
        .lines()
        .position(|line| line.contains(BUDGET_TRIM_MARKER_PREFIX))
        .map(|idx| idx + 1);

    BudgetResult {
        token_budget,
        estimated_tokens_before,
        estimated_tokens_after: estimate_tokens(&output),
        lines_before,
        lines_after: line_count(&output),
        trimmed: output != text,
        marker_line,
        output,
    }
}

fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

fn take_chars_from_start(lines: &[&str], max_chars: usize) -> String {
    let mut out = String::new();
    for line in lines {
        if out.len() + line.len() + 1 > max_chars {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn take_chars_from_end(lines: &[&str], max_chars: usize) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut chars = 0usize;
    for line in lines.iter().rev() {
        let needed = line.len() + 1;
        if chars + needed > max_chars {
            break;
        }
        chars += needed;
        out.push(line);
    }
    out.reverse();
    out.join("\n")
}

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_report_not_trimmed_when_under_limit() {
        let report = apply_budget_with_limit("a\nb\nc".to_string(), 100);
        assert!(!report.trimmed);
        assert_eq!(report.marker_line, None);
        assert_eq!(report.lines_before, 3);
        assert_eq!(report.lines_after, 3);
    }

    #[test]
    fn budget_report_marks_trimmed_output() {
        let input = (1..=200)
            .map(|n| format!("line-{n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let report = apply_budget_with_limit(input, 30);
        assert!(report.trimmed);
        assert!(report.marker_line.is_some());
        assert!(report.output.contains(BUDGET_TRIM_MARKER_PREFIX));
    }
}
