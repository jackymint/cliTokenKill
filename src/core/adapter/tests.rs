use super::*;

fn default_config() -> FilterConfig {
    FilterConfig {
        level: FilterLevel::Minimal,
        max_lines: 50,
        max_chars_per_line: 200,
    }
}

#[test]
fn adapter_applies_signal_and_exclude_rules() {
    let spec = AdapterSpec {
        name: "cargo-test".to_string(),
        match_command: "^cargo test".to_string(),
        signal_patterns: vec!["failed".to_string(), "panic".to_string()],
        include_patterns: vec![],
        exclude_patterns: vec!["note:".to_string()],
        on_empty: Some("ok: no failures".to_string()),
        level: Some("minimal".to_string()),
        max_lines: Some(20),
        max_chars_per_line: Some(160),
        priority: Some(10),
    };

    let rule = loader::compile_rule(&spec, Path::new("test.toml"), 0).expect("rule compiles");

    let output = "running 2 tests\ntest a ... ok\nthread 'x' panicked at src/lib.rs:9\nnote: run with RUST_BACKTRACE=1\n";

    let rendered = rule
        .apply(output, default_config())
        .expect("rendered output");

    assert!(rendered.contains("panicked"));
    assert!(!rendered.contains("note:"));
}

#[test]
fn adapter_uses_on_empty_when_filters_drop_everything() {
    let spec = AdapterSpec {
        name: "quiet".to_string(),
        match_command: ".*".to_string(),
        signal_patterns: vec![],
        include_patterns: vec!["^ERROR".to_string()],
        exclude_patterns: vec![],
        on_empty: Some("ok".to_string()),
        level: None,
        max_lines: None,
        max_chars_per_line: None,
        priority: None,
    };

    let rule = loader::compile_rule(&spec, Path::new("test.toml"), 0).expect("rule compiles");
    let rendered = rule
        .apply("all good", default_config())
        .expect("on_empty value");

    assert_eq!(rendered, "ok");
}
