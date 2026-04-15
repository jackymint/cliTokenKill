use super::*;
use anyhow::{Context, Result};
use std::fs;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn stats_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ctk/stats.json")
}

pub(crate) fn events_path() -> PathBuf {
    stats_path().with_extension("events.ndjson")
}

pub(super) fn append_stats_event(stats: &Stats) -> Result<()> {
    let path = events_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create stats dir: {}", parent.display()))?;
    }

    let mut json = serde_json::to_string(stats).context("failed to serialize stats event")?;
    json.push('\n');

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open stats event log: {}", path.display()))?;
    file.write_all(json.as_bytes())
        .with_context(|| format!("failed to append stats event: {}", path.display()))
}

pub(super) fn clear_event_log() -> Result<()> {
    let path = events_path();
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => {
            Err(err).with_context(|| format!("failed to clear stats event log: {}", path.display()))
        }
    }
}

pub(super) fn load_event_stats() -> Option<Stats> {
    let path = events_path();
    if !path.exists() {
        return None;
    }

    let content = fs::read_to_string(path).ok()?;
    let mut stats = Stats::default();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(delta) = serde_json::from_str::<Stats>(line) else {
            continue;
        };
        merge_stats(&mut stats, &delta);
    }
    prune_recent_events(&mut stats);
    Some(stats)
}

pub(super) fn bucket_sum<F>(events: &[StatEvent], buckets: usize, value: F) -> Vec<u64>
where
    F: Fn(&StatEvent) -> u64,
{
    bucket_events(events, buckets, |bucket, event| {
        *bucket += value(event);
    })
}

pub(super) fn bucket_max<F>(events: &[StatEvent], buckets: usize, value: F) -> Vec<u64>
where
    F: Fn(&StatEvent) -> u64,
{
    bucket_events(events, buckets, |bucket, event| {
        *bucket = (*bucket).max(value(event));
    })
}

pub(super) fn chars_to_tokens(chars: usize) -> u64 {
    chars.div_ceil(4) as u64
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn bucket_events<F>(events: &[StatEvent], buckets: usize, mut apply: F) -> Vec<u64>
where
    F: FnMut(&mut u64, &StatEvent),
{
    let mut values = vec![0; buckets];
    if buckets == 0 {
        return values;
    }

    let now = now_ms();
    let bucket_ms = (GRAPH_WINDOW_MS / buckets as u64).max(1);
    for event in events {
        let age = now.saturating_sub(event.timestamp_ms);
        if age > GRAPH_WINDOW_MS {
            continue;
        }
        let index = buckets.saturating_sub(1 + (age / bucket_ms) as usize);
        if let Some(bucket) = values.get_mut(index) {
            apply(bucket, event);
        }
    }
    values
}

fn merge_stats(local: &mut Stats, remote: &Stats) {
    local.total_commands += remote.total_commands;
    local.total_raw_tokens += remote.total_raw_tokens;
    local.total_filtered_tokens += remote.total_filtered_tokens;
    local.total_fallbacks += remote.total_fallbacks;
    local.total_chunks += remote.total_chunks;
    for (cmd, count) in &remote.command_counts {
        *local.command_counts.entry(cmd.clone()).or_insert(0) += count;
    }
    local.recent_events.extend(remote.recent_events.clone());
    if remote.last_ai_cli.is_some() {
        local.last_ai_cli = remote.last_ai_cli.clone();
    }
}

fn prune_recent_events(stats: &mut Stats) {
    let cutoff = now_ms().saturating_sub(GRAPH_WINDOW_MS);
    stats.recent_events.retain(|e| e.timestamp_ms >= cutoff);
    if stats.recent_events.len() > MAX_EVENTS {
        let drain = stats.recent_events.len() - MAX_EVENTS;
        stats.recent_events.drain(..drain);
    }
}
