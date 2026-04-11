use std::env;

const DEFAULT_TOKEN_BUDGET: usize = 900;

pub fn apply_token_budget(text: String) -> String {
    let budget = env::var("CTK_TOKEN_BUDGET")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_TOKEN_BUDGET);
    apply_budget_with_limit(text, budget)
}

fn apply_budget_with_limit(text: String, token_budget: usize) -> String {
    if token_budget == 0 {
        return String::new();
    }

    let est_tokens = estimate_tokens(&text);
    if est_tokens <= token_budget {
        return text;
    }

    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return text;
    }

    // Keep 60% head + 40% tail when over budget.
    let target_chars = token_budget.saturating_mul(4);
    let head_chars = (target_chars * 3) / 5;
    let tail_chars = target_chars.saturating_sub(head_chars);

    let head = take_chars_from_start(&lines, head_chars);
    let tail = take_chars_from_end(&lines, tail_chars);

    format!("{head}\n... [ctk budget-trim: est_tokens={est_tokens} limit={token_budget}]\n{tail}")
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
