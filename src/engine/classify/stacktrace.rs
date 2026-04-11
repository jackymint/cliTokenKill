const STACK_TRACE_MARKERS: &[&str] = &["Traceback", "stack backtrace:", "Exception", " at "];

pub fn detect(output: &str) -> bool {
    STACK_TRACE_MARKERS
        .iter()
        .any(|marker| output.contains(marker))
}
