use super::*;
use std::path::Path;
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

static HOME_LOCK: Mutex<()> = Mutex::new(());

fn with_temp_home<F: FnOnce(&Path) -> R, R>(f: F) -> R {
    let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("ctk_stats_home_{id}"));
    fs::create_dir_all(&tmp).unwrap();
    let old_home = env::var_os("HOME");
    // SAFETY: protected by HOME_LOCK; no other tests mutate HOME concurrently.
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
fn record_and_save_persists_stats() {
    with_temp_home(|_| {
        let stats = Stats::record_and_save("echo", 20, 12, 5, false, 0).unwrap();
        let saved = Stats::load();

        assert_eq!(saved.total_commands, 1);
        assert_eq!(saved.command_counts.get("echo"), Some(&1));
        assert_eq!(saved.total_raw_tokens, stats.total_raw_tokens);
        assert_eq!(saved.total_filtered_tokens, stats.total_filtered_tokens);
    });
}

#[test]
fn record_and_save_is_process_safe() {
    with_temp_home(|_| {
        let mut threads = Vec::new();
        for _ in 0..8 {
            threads.push(thread::spawn(|| {
                for _ in 0..10 {
                    Stats::record_and_save("echo", 20, 12, 5, false, 0).unwrap();
                }
            }));
        }
        for handle in threads {
            handle.join().unwrap();
        }

        let saved = Stats::load();
        assert_eq!(saved.total_commands, 80);
        assert_eq!(saved.command_counts.get("echo"), Some(&80));
    });
}

#[test]
fn clear_resets_saved_stats() {
    with_temp_home(|_| {
        Stats::record_and_save("echo", 20, 12, 5, false, 0).unwrap();

        Stats::clear().unwrap();

        let saved = Stats::load();
        assert_eq!(saved.total_commands, 0);
        assert_eq!(saved.total_raw_tokens, 0);
        assert_eq!(saved.total_filtered_tokens, 0);
        assert_eq!(saved.total_fallbacks, 0);
        assert_eq!(saved.total_chunks, 0);
        assert!(saved.command_counts.is_empty());
        assert!(saved.recent_events.is_empty());
        assert_eq!(saved.last_ai_cli, None);
    });
}
