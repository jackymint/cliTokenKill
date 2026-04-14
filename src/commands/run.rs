use super::*;

pub(super) fn run_and_exit(
    command: &[String],
    path: Option<PathBuf>,
    config: FilterConfig,
    mode: PipelineMode,
) -> Result<()> {
    // Log proxy execution
    if std::env::var("CTK_DEBUG").is_ok() {
        let cmd_str = command.join(" ");
        let ai_cli = std::env::var("CTK_AI_CLI_NAME").unwrap_or_else(|_| "none".to_string());
        eprintln!("[ctk proxy] ai_cli={} cmd={}", ai_cli, cmd_str);
    }

    if let Some(ref dir) = path {
        std::env::set_current_dir(dir)
            .with_context(|| format!("failed to change directory to {}", dir.display()))?;
    }

    let start = Instant::now();
    let result = run_pipeline(command, config, mode)?;
    let latency_ms = start.elapsed().as_millis() as u64;

    let raw_chars = result.raw_chars;
    let filtered_chars = result.output.len();
    let fallback_used = result.fallback_used;
    let exit_code = result.exit_code;

    let budgeted = apply_token_budget(result.output);
    let chunk = maybe_auto_chunk(budgeted)?;
    let new_chunks = u64::from(print_pipeline_output(chunk));

    record_stats(
        command,
        raw_chars,
        filtered_chars,
        latency_ms,
        fallback_used,
        new_chunks,
    );

    // Log stats recording
    if std::env::var("CTK_DEBUG").is_ok() {
        let saved = raw_chars.saturating_sub(filtered_chars);
        eprintln!(
            "[ctk proxy] raw={} filtered={} saved={} fallback={}",
            raw_chars, filtered_chars, saved, fallback_used
        );
    }

    if fallback_used {
        eprintln!("ctk: filter fallback to raw output");
    }
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

fn record_stats(
    command: &[String],
    raw_chars: usize,
    filtered_chars: usize,
    latency_ms: u64,
    fallback: bool,
    new_chunks: u64,
) {
    let cmd = command.first().map(|s| s.as_str()).unwrap_or("unknown");
    let stats = match Stats::record_and_save(
        cmd,
        raw_chars,
        filtered_chars,
        latency_ms,
        fallback,
        new_chunks,
    ) {
        Ok(stats) => stats,
        Err(err) => {
            eprintln!("ctk: failed to update stats: {err:#}");
            return;
        }
    };

    // Send stats to remote endpoint if configured
    if let Ok(endpoint) = std::env::var("CTK_STATS_ENDPOINT") {
        send_stats_to_remote(&endpoint, &stats).ok();
    }
}

fn send_stats_to_remote(endpoint: &str, _stats: &Stats) -> Result<()> {
    // TODO: Implement HTTP POST to remote endpoint
    // For now, just log
    if std::env::var("CTK_DEBUG").is_ok() {
        eprintln!("[ctk] would send stats to: {}", endpoint);
    }
    Ok(())
}
