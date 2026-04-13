use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_EVENTS: usize = 120;
const GRAPH_WINDOW_MS: u64 = 7 * 60 * 1000;
const STATS_LOCK_RETRY_MS: u64 = 10;
const STATS_LOCK_TIMEOUT_MS: u64 = 2_000;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Stats {
    pub total_commands: u64,
    pub total_raw_tokens: u64,
    pub total_filtered_tokens: u64,
    pub total_fallbacks: u64,
    pub total_chunks: u64,
    pub command_counts: HashMap<String, u64>,
    pub recent_events: Vec<StatEvent>,
    pub last_ai_cli: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct StatEvent {
    pub timestamp_ms: u64,
    pub raw_tokens: u64,
    pub filtered_tokens: u64,
    pub latency_ms: u64,
}

impl Stats {
    pub fn load() -> Self {
        fs::read_to_string(stats_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn clear() -> Result<()> {
        Self::default().save()
    }

    pub fn save(&self) -> Result<()> {
        let path = stats_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create stats dir: {}", parent.display()))?;
        }
        let json = serde_json::to_string(self).context("failed to serialize stats")?;
        let tmp = path.with_extension(format!("json.tmp-{}", std::process::id()));
        fs::write(&tmp, json)
            .with_context(|| format!("failed to write temp stats: {}", tmp.display()))?;
        fs::rename(&tmp, &path)
            .with_context(|| format!("failed to write stats: {}", path.display()))
    }

    pub fn record_and_save(
        command: &str,
        raw_chars: usize,
        filtered_chars: usize,
        latency_ms: u64,
        fallback: bool,
        new_chunks: u64,
    ) -> Result<Self> {
        let _lock = StatsLock::acquire()?;
        let mut stats = Self::load();
        stats.record(
            command,
            raw_chars,
            filtered_chars,
            latency_ms,
            fallback,
            new_chunks,
        );
        stats.save()?;
        Ok(stats)
    }

    pub fn record(
        &mut self,
        command: &str,
        raw_chars: usize,
        filtered_chars: usize,
        latency_ms: u64,
        fallback: bool,
        new_chunks: u64,
    ) {
        let raw_tokens = chars_to_tokens(raw_chars);
        let filtered_tokens = chars_to_tokens(filtered_chars);

        self.total_commands += 1;
        self.total_raw_tokens += raw_tokens;
        self.total_filtered_tokens += filtered_tokens;
        if fallback {
            self.total_fallbacks += 1;
        }
        self.total_chunks += new_chunks;

        let cmd = command.split_whitespace().next().unwrap_or(command);
        *self.command_counts.entry(cmd.to_string()).or_insert(0) += 1;

        self.last_ai_cli = env::var("CTK_AI_CLI_NAME").ok();

        let now_ms = now_ms();
        self.recent_events.push(StatEvent {
            timestamp_ms: now_ms,
            raw_tokens,
            filtered_tokens,
            latency_ms,
        });

        let cutoff = now_ms.saturating_sub(GRAPH_WINDOW_MS);
        self.recent_events.retain(|e| e.timestamp_ms >= cutoff);
        if self.recent_events.len() > MAX_EVENTS {
            let drain = self.recent_events.len() - MAX_EVENTS;
            self.recent_events.drain(..drain);
        }
    }

    pub fn saved_tokens(&self) -> u64 {
        self.total_raw_tokens
            .saturating_sub(self.total_filtered_tokens)
    }

    pub fn savings_ratio(&self) -> f64 {
        if self.total_raw_tokens == 0 {
            return 0.0;
        }
        (self.saved_tokens() as f64 / self.total_raw_tokens as f64) * 100.0
    }

    pub fn commands_per_min(&self) -> u64 {
        let cutoff = now_ms().saturating_sub(60_000);
        self.recent_events
            .iter()
            .filter(|e| e.timestamp_ms >= cutoff)
            .count() as u64
    }

    pub fn top_commands(&self, n: usize) -> Vec<(String, u64)> {
        let mut pairs: Vec<(String, u64)> = self.command_counts.clone().into_iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1));
        pairs.truncate(n);
        pairs
    }

    pub fn graph_saved_tokens(&self, buckets: usize) -> Vec<u64> {
        bucket_sum(&self.recent_events, buckets, |e| {
            e.raw_tokens.saturating_sub(e.filtered_tokens)
        })
    }

    pub fn graph_latency_ms(&self, buckets: usize) -> Vec<u64> {
        bucket_max(&self.recent_events, buckets, |e| e.latency_ms)
    }
}

struct StatsLock {
    path: PathBuf,
}

impl StatsLock {
    fn acquire() -> Result<Self> {
        let path = lock_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create stats dir: {}", parent.display()))?;
        }
        let start = now_ms();
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Self { path }),
                Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                    if now_ms().saturating_sub(start) >= STATS_LOCK_TIMEOUT_MS {
                        anyhow::bail!("timed out waiting for stats lock: {}", path.display());
                    }
                    thread::sleep(std::time::Duration::from_millis(STATS_LOCK_RETRY_MS));
                }
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("failed to acquire stats lock: {}", path.display())
                    });
                }
            }
        }
    }
}

impl Drop for StatsLock {
    fn drop(&mut self) {
        fs::remove_file(&self.path).ok();
    }
}

fn bucket_sum<F>(events: &[StatEvent], buckets: usize, value: F) -> Vec<u64>
where
    F: Fn(&StatEvent) -> u64,
{
    let now = now_ms();
    let bucket_ms = GRAPH_WINDOW_MS / buckets as u64;
    let mut totals = vec![0u64; buckets];
    for event in events {
        let age_ms = now.saturating_sub(event.timestamp_ms);
        if age_ms >= GRAPH_WINDOW_MS {
            continue;
        }
        let idx = buckets - 1 - (age_ms / bucket_ms).min(buckets as u64 - 1) as usize;
        totals[idx] += value(event);
    }
    totals
}

fn bucket_max<F>(events: &[StatEvent], buckets: usize, value: F) -> Vec<u64>
where
    F: Fn(&StatEvent) -> u64,
{
    let now = now_ms();
    let bucket_ms = GRAPH_WINDOW_MS / buckets as u64;
    let mut maxes = vec![0u64; buckets];
    for event in events {
        let age_ms = now.saturating_sub(event.timestamp_ms);
        if age_ms >= GRAPH_WINDOW_MS {
            continue;
        }
        let idx = buckets - 1 - (age_ms / bucket_ms).min(buckets as u64 - 1) as usize;
        maxes[idx] = maxes[idx].max(value(event));
    }
    maxes
}

fn chars_to_tokens(chars: usize) -> u64 {
    (chars as u64 / 4).max(if chars > 0 { 1 } else { 0 })
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn stats_path() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ctk/stats.json")
}

fn lock_path() -> PathBuf {
    stats_path().with_extension("json.lock")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;

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
}
