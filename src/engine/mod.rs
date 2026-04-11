mod classify;
mod compact;
mod content_kind;

pub use classify::classify_content;
pub use compact::compact_by_kind;
pub use content_kind::ContentKind;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::filter::{FilterConfig, FilterLevel};

    fn golden_config() -> FilterConfig {
        FilterConfig {
            level: FilterLevel::Minimal,
            max_lines: 120,
            max_chars_per_line: 240,
        }
    }

    fn normalize(text: &str) -> String {
        text.trim_end().replace("\r\n", "\n")
    }

    fn assert_golden(input: &str, expected: &str, kind: ContentKind) {
        assert_eq!(classify_content(input), kind);
        let actual = compact_by_kind(input, kind, golden_config());
        assert_eq!(normalize(&actual), normalize(expected));
    }

    #[test]
    fn golden_json_compaction() {
        let input = include_str!("../../tests/golden/json.input.txt");
        let expected = include_str!("../../tests/golden/json.expected.txt");
        assert_golden(input, expected, ContentKind::Json);
    }

    #[test]
    fn golden_ndjson_compaction() {
        let input = include_str!("../../tests/golden/ndjson.input.txt");
        let expected = include_str!("../../tests/golden/ndjson.expected.txt");
        assert_golden(input, expected, ContentKind::Ndjson);
    }

    #[test]
    fn golden_diff_compaction() {
        let input = include_str!("../../tests/golden/diff.input.txt");
        let expected = include_str!("../../tests/golden/diff.expected.txt");
        assert_golden(input, expected, ContentKind::Diff);
    }

    #[test]
    fn golden_log_compaction() {
        let input = include_str!("../../tests/golden/log.input.txt");
        let expected = include_str!("../../tests/golden/log.expected.txt");
        assert_golden(input, expected, ContentKind::LogStream);
    }

    #[test]
    fn golden_stacktrace_compaction() {
        let input = include_str!("../../tests/golden/stacktrace.input.txt");
        let expected = include_str!("../../tests/golden/stacktrace.expected.txt");
        assert_golden(input, expected, ContentKind::StackTrace);
    }

    #[test]
    fn golden_grep_compaction() {
        let input = include_str!("../../tests/golden/grep.input.txt");
        let expected = include_str!("../../tests/golden/grep.expected.txt");
        assert_golden(input, expected, ContentKind::GrepLike);
    }

    #[test]
    fn golden_table_compaction() {
        let input = include_str!("../../tests/golden/table.input.txt");
        let expected = include_str!("../../tests/golden/table.expected.txt");
        assert_golden(input, expected, ContentKind::TableText);
    }

    #[test]
    fn golden_test_output_compaction() {
        let input = include_str!("../../tests/golden/test_output.input.txt");
        let expected = include_str!("../../tests/golden/test_output.expected.txt");
        assert_golden(input, expected, ContentKind::TestOutput);
    }
}
