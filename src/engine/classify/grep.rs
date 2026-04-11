const GREP_SCAN_LIMIT: usize = 10;
const GREP_MIN_MATCHING_LINES: usize = 3;

pub fn detect(output: &str) -> bool {
    output
        .lines()
        .take(GREP_SCAN_LIMIT)
        .filter(|line| line.contains(':'))
        .count()
        >= GREP_MIN_MATCHING_LINES
}
