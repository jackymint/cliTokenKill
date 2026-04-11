use regex::Regex;
use std::sync::OnceLock;

const LOG_SCAN_LIMIT: usize = 10;
const LOG_MIN_TIMESTAMP_MATCHES: usize = 3;
const TIMESTAMP_PATTERN: &str = r"\d{4}-\d{2}-\d{2}";

pub fn detect(output: &str) -> bool {
    output
        .lines()
        .take(LOG_SCAN_LIMIT)
        .filter(|line| timestamp_regex().is_match(line))
        .count()
        >= LOG_MIN_TIMESTAMP_MATCHES
}

fn timestamp_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(TIMESTAMP_PATTERN).expect("valid timestamp regex"))
}
