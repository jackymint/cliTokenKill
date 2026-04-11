pub fn detect(output: &str) -> bool {
    output.contains("diff --git") || output.contains("@@")
}
