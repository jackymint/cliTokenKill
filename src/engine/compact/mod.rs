mod diff;
mod grep;
mod json_like;
mod logs;
mod stack_trace;
mod table_text;
mod test_output;

use crate::core::filter::{FilterConfig, compact_output};
use crate::engine::ContentKind;

pub fn compact_by_kind(output: &str, kind: ContentKind, config: FilterConfig) -> String {
    match kind {
        ContentKind::Json | ContentKind::Ndjson => json_like::compact(output, config),
        ContentKind::Diff => diff::compact(output, config),
        ContentKind::GrepLike => grep::compact(output, config),
        ContentKind::StackTrace => stack_trace::compact(output, config),
        ContentKind::TestOutput => test_output::compact(output, config),
        ContentKind::TableText => table_text::compact(output, config),
        ContentKind::LogStream => logs::compact(output, config),
        ContentKind::Plain => compact_output(output, config),
    }
}
