use crate::engine::ContentKind;
use serde_json::Value;

const NDJSON_SAMPLE_LIMIT: usize = 5;
const NDJSON_MIN_LINES: usize = 2;

pub fn detect(output: &str, trimmed: &str) -> Option<ContentKind> {
    if looks_like_ndjson(output) {
        return Some(ContentKind::Ndjson);
    }

    if serde_json::from_str::<Value>(trimmed).is_ok() {
        return Some(ContentKind::Json);
    }

    None
}

fn looks_like_ndjson(output: &str) -> bool {
    let lines: Vec<&str> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(NDJSON_SAMPLE_LIMIT)
        .collect();

    lines.len() >= NDJSON_MIN_LINES
        && lines
            .iter()
            .all(|line| serde_json::from_str::<Value>(line.trim()).is_ok())
}
