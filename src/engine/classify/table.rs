const TABLE_SCAN_LIMIT: usize = 6;

pub fn detect(output: &str) -> bool {
    output
        .lines()
        .take(TABLE_SCAN_LIMIT)
        .any(|line| line.contains('|'))
        && output.contains("---")
}
