use super::*;

pub(super) struct StatsLock {
    path: PathBuf,
}

impl StatsLock {
    pub(super) fn acquire() -> Result<Self> {
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

pub(super) fn bucket_sum<F>(events: &[StatEvent], buckets: usize, value: F) -> Vec<u64>
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

pub(super) fn bucket_max<F>(events: &[StatEvent], buckets: usize, value: F) -> Vec<u64>
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

pub(super) fn chars_to_tokens(chars: usize) -> u64 {
    (chars as u64 / 4).max(if chars > 0 { 1 } else { 0 })
}

pub(super) fn now_ms() -> u64 {
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

pub(super) fn lock_path() -> PathBuf {
    stats_path().with_extension("json.lock")
}
