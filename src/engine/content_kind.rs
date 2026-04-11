#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentKind {
    Json,
    Ndjson,
    Diff,
    GrepLike,
    StackTrace,
    TestOutput,
    TableText,
    LogStream,
    Plain,
}

impl ContentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ContentKind::Json => "json",
            ContentKind::Ndjson => "ndjson",
            ContentKind::Diff => "diff",
            ContentKind::GrepLike => "grep-like",
            ContentKind::StackTrace => "stack-trace",
            ContentKind::TestOutput => "test-output",
            ContentKind::TableText => "table-text",
            ContentKind::LogStream => "log-stream",
            ContentKind::Plain => "plain",
        }
    }
}
