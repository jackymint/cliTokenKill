use super::*;
use std::sync::Mutex;

// ── pure functions ────────────────────────────────────────────────────────

#[test]
fn shell_quote_plain_string() {
    assert_eq!(doctor::shell_quote_single("codex"), "'codex'");
}

#[test]
fn shell_quote_empty_string() {
    assert_eq!(doctor::shell_quote_single(""), "''");
}

#[test]
fn shell_quote_escapes_single_quotes() {
    assert_eq!(doctor::shell_quote_single("it's"), "'it'\"'\"'s'");
}

#[test]
fn is_truthy_env_recognises_truthy_values() {
    for v in &["1", "true", "yes", "on", "TRUE", "YES", "  on  "] {
        assert!(doctor::is_truthy_env(v), "{v} should be truthy");
    }
}

#[test]
fn is_truthy_env_rejects_falsy_values() {
    for v in &["0", "false", "no", "off", "", "random"] {
        assert!(!doctor::is_truthy_env(v), "{v} should not be truthy");
    }
}

#[test]
fn launcher_target_name_strips_ctk_suffix() {
    assert_eq!(doctor::launcher_target_name("codex-ctk"), "codex");
    assert_eq!(doctor::launcher_target_name("claude-ctk"), "claude");
}

#[test]
fn launcher_target_name_unchanged_without_suffix() {
    assert_eq!(doctor::launcher_target_name("codex"), "codex");
}

// ── rc file block manipulation ─────────────────────────────────────────────

fn temp_file(content: &str) -> PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("ctk_rc_{id}.txt"));
    fs::write(&path, content).unwrap();
    path
}

#[test]
fn upsert_block_inserts_into_empty_file() {
    let path = temp_file("");
    let changed = rc::upsert_block(&path, "# S", "# E", "# S\nline\n# E\n").unwrap();
    assert!(changed);
    assert!(fs::read_to_string(&path).unwrap().contains("line"));
    fs::remove_file(&path).ok();
}

#[test]
fn upsert_block_replaces_existing_block() {
    let path = temp_file("before\n# S\nold\n# E\nafter\n");
    rc::upsert_block(&path, "# S", "# E", "# S\nnew\n# E\n").unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("new") && !content.contains("old"));
    assert!(content.contains("before") && content.contains("after"));
    fs::remove_file(&path).ok();
}

#[test]
fn upsert_block_noop_when_content_unchanged() {
    let block = "# S\nline\n# E\n";
    let path = temp_file(block);
    let changed = rc::upsert_block(&path, "# S", "# E", block).unwrap();
    assert!(!changed);
    fs::remove_file(&path).ok();
}

#[test]
fn remove_block_removes_existing_block() {
    let path = temp_file("before\n# S\nline\n# E\nafter\n");
    let changed = rc::remove_block(&path, "# S", "# E").unwrap();
    assert!(changed);
    let content = fs::read_to_string(&path).unwrap();
    assert!(!content.contains("line") && content.contains("before"));
    fs::remove_file(&path).ok();
}

#[test]
fn remove_block_noop_when_absent() {
    let path = temp_file("no block here\n");
    assert!(!rc::remove_block(&path, "# S", "# E").unwrap());
    fs::remove_file(&path).ok();
}

#[test]
fn remove_block_noop_on_missing_file() {
    let path = PathBuf::from("/tmp/ctk_nonexistent_xyzzy.txt");
    assert!(!rc::remove_block(&path, "# S", "# E").unwrap());
}

// ── launcher_exec_target ───────────────────────────────────────────────────

#[test]
fn launcher_exec_target_extracts_path() {
    let path = temp_file("#!/usr/bin/env bash\nexec \"/usr/local/bin/codex\" \"$@\"\n");
    assert_eq!(
        launcher_exec_target(&path).unwrap(),
        PathBuf::from("/usr/local/bin/codex")
    );
    fs::remove_file(&path).ok();
}

#[test]
fn launcher_exec_target_returns_none_without_exec_line() {
    let path = temp_file("#!/usr/bin/env bash\necho hello\n");
    assert!(launcher_exec_target(&path).is_none());
    fs::remove_file(&path).ok();
}

// ── init / uninstall roundtrip ─────────────────────────────────────────────

static HOME_LOCK: Mutex<()> = Mutex::new(());

fn with_temp_home<F: FnOnce(&Path) -> R, R>(f: F) -> R {
    let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("ctk_home_{id}"));
    fs::create_dir_all(&tmp).unwrap();
    let old_home = env::var_os("HOME");
    // SAFETY: protected by HOME_LOCK; no other threads touch HOME concurrently
    unsafe { env::set_var("HOME", &tmp) };
    let result = f(&tmp);
    unsafe {
        match old_home {
            Some(h) => env::set_var("HOME", h),
            None => env::remove_var("HOME"),
        }
    }
    fs::remove_dir_all(&tmp).ok();
    result
}

#[test]
fn init_creates_bin_dir() {
    with_temp_home(|home| {
        init_agent("codex", "codex-ctk").unwrap();
        assert!(home.join(".ctk/bin").is_dir());
    });
}

#[test]
fn init_places_ctk_in_bin() {
    with_temp_home(|home| {
        init_agent("codex", "codex-ctk").unwrap();
        assert!(home.join(".ctk/bin/ctk").exists());
    });
}

#[test]
fn init_result_reports_correct_bin_dir() {
    with_temp_home(|home| {
        let result = init_agent("codex", "codex-ctk").unwrap();
        assert_eq!(result.bin_dir, home.join(".ctk/bin"));
    });
}

#[test]
fn uninstall_after_init_removes_bin_dir() {
    with_temp_home(|home| {
        init_agent("codex", "codex-ctk").unwrap();
        assert!(home.join(".ctk/bin").exists());
        uninstall_agent("codex-ctk").unwrap();
        assert!(!home.join(".ctk/bin").exists());
    });
}

#[test]
fn uninstall_on_clean_home_does_not_err() {
    with_temp_home(|_| {
        assert!(uninstall_agent("codex-ctk").is_ok());
    });
}
