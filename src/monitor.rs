use crate::stats::{Stats, stats_path};
use anyhow::Result;
use notify::{EventKind, RecursiveMode, Watcher, recommended_watcher};
use std::io::{self, Write};
use std::sync::mpsc;

const GRAPH_BUCKETS: usize = 14;
const GRAPH_HEIGHT: usize = 5;

const GREEN: &str = "\x1B[32m";
const YELLOW: &str = "\x1B[33m";
const DIM: &str = "\x1B[2m";
const BOLD: &str = "\x1B[1m";
const RESET: &str = "\x1B[0m";

pub fn run_monitor() -> Result<()> {
    // Load existing stats or create new if not exists
    let stats = Stats::load();
    stats.save()?;
    render()?;

    let watch_path = {
        let p = stats_path();
        p.parent()
            .map(|d| d.to_path_buf())
            .unwrap_or_else(|| p.clone())
    };

    std::fs::create_dir_all(&watch_path).ok();

    let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = recommended_watcher(tx)?;
    watcher.watch(&watch_path, RecursiveMode::NonRecursive)?;

    for event in rx {
        let Ok(ev) = event else { continue };
        let is_stats_write = matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_))
            && ev.paths.iter().any(|p| p == &stats_path());
        if is_stats_write {
            render()?;
        }
    }

    Ok(())
}

fn render() -> Result<()> {
    let stats = Stats::load();
    move_to_top();
    print_dashboard(&stats);
    io::stdout().flush().ok();
    Ok(())
}

fn move_to_top() {
    print!("\x1B[H\x1B[J");
}

fn print_dashboard(stats: &Stats) {
    let active = match &stats.last_ai_cli {
        Some(name) => name.as_str(),
        None => "unknown",
    };

    println!("{BOLD}CTK Monitor{RESET}");
    println!("{DIM}{}{RESET}", "─".repeat(36));
    println!("Active AI CLI   : {GREEN}{BOLD}{active}{RESET}");
    println!("Commands/min    : {}", stats.commands_per_min());
    println!(
        "Saved tokens    : {GREEN}{}{RESET}",
        fmt_number(stats.saved_tokens())
    );
    println!("Savings ratio   : {:.0}%", stats.savings_ratio());
    println!("Fallbacks       : {}", stats.total_fallbacks);
    println!("Chunks created  : {}", stats.total_chunks);
    println!();

    let top = stats.top_commands(5);
    println!("{BOLD}Top commands{RESET}");
    if top.is_empty() {
        println!("  {DIM}(no data yet){RESET}");
    } else {
        for (i, (cmd, count)) in top.iter().enumerate() {
            println!("  {}. {DIM}{:<18}{RESET} {count}", i + 1, cmd);
        }
    }
    println!();

    let saved = stats.graph_saved_tokens(GRAPH_BUCKETS);
    let latency = stats.graph_latency_ms(GRAPH_BUCKETS);

    print!("{}", bar_graph("Tokens saved/min", "tok", &saved, GREEN));
    println!();
    print!("{}", bar_graph("Latency ms", "ms", &latency, YELLOW));
    println!();
    println!("{DIM}watching ~/.ctk/stats.json  •  ctrl-c to exit{RESET}");
}

fn bar_graph(label: &str, unit: &str, values: &[u64], color: &str) -> String {
    let max = *values.iter().max().unwrap_or(&0);
    let buckets = values.len();
    let mut out = String::new();

    let peak = if max == 0 {
        format!("{DIM}no data{RESET}")
    } else {
        format!("peak: {color}{}{RESET} {unit}", fmt_number(max))
    };
    out.push_str(&format!("{BOLD}{label}{RESET}  {peak}\n"));

    for row in (0..GRAPH_HEIGHT).rev() {
        out.push_str(&format!("  {DIM}┤{RESET}"));
        for &v in values {
            let filled = if max == 0 {
                0
            } else {
                (v * GRAPH_HEIGHT as u64 / max) as usize
            };
            if row < filled {
                out.push_str(&format!("{color}█{RESET}"));
            } else {
                out.push(' ');
            }
        }
        out.push('\n');
    }

    out.push_str(&format!("  {DIM}└{}─{RESET}\n", "─".repeat(buckets)));

    let pad = buckets.saturating_sub(6);
    out.push_str(&format!("   {DIM}-7m{:>pad$}now{RESET}\n", "", pad = pad));

    out
}

fn fmt_number(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}
