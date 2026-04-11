mod diff;
mod grep;
mod json;
mod logs;
mod stacktrace;
mod table;
mod test_output;

use crate::engine::ContentKind;

pub fn classify_content(output: &str) -> ContentKind {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return ContentKind::Plain;
    }

    if let Some(kind) = json::detect(output, trimmed) {
        return kind;
    }
    if diff::detect(output) {
        return ContentKind::Diff;
    }
    if stacktrace::detect(output) {
        return ContentKind::StackTrace;
    }
    if test_output::detect(output) {
        return ContentKind::TestOutput;
    }
    if logs::detect(output) {
        return ContentKind::LogStream;
    }
    if grep::detect(output) {
        return ContentKind::GrepLike;
    }
    if table::detect(output) {
        return ContentKind::TableText;
    }

    ContentKind::Plain
}
