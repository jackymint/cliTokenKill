const TEST_OUTPUT_MARKERS: &[&str] = &["test result", "passed", "failed", "collected "];

pub fn detect(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    TEST_OUTPUT_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}
