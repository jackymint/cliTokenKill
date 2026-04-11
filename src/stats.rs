use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_EVENTS: usize = 120;
const GRAPH_WINDOW_MS: u64 = 7 * 60 * 1000;

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

    pub fn save(&self) -> Result<()> {
        let path = stats_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create stats dir: {}", parent.display()))?;
        }
        let json = serde_json::to_string(self).context("failed to serialize stats")?;
        fs::write(&path, json)
            .with_context(|| format!("failed to write stats: {}", path.display()))
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

fn stats_path() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ctk/stats.json")
}
