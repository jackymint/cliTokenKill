use crate::stats::Stats;
use anyhow::Result;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

const GRAPH_BUCKETS: usize = 14;
const BLOCKS: &[char] = &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub fn run_monitor() -> Result<()> {
    loop {
        let stats = Stats::load();
        move_to_top();
        print_dashboard(&stats);
        io::stdout().flush().ok();
        thread::sleep(Duration::from_secs(1));
    }
}

fn move_to_top() {
    print!("\x1B[H\x1B[J");
}

fn print_dashboard(stats: &Stats) {
    let active = match &stats.last_ai_cli {
        Some(name) if stats.commands_per_min() > 0 => name.as_str(),
        Some(name) => name.as_str(),
        None => "unknown",
    };

    println!("CTK Monitor");
    println!("{}", "─".repeat(32));
    println!("Active AI CLI   : {active}");
    println!("Commands/min    : {}", stats.commands_per_min());
    println!("Saved tokens    : {}", fmt_number(stats.saved_tokens()));
    println!("Savings ratio   : {:.0}%", stats.savings_ratio());
    println!("Fallbacks       : {}", stats.total_fallbacks);
    println!("Chunks created  : {}", stats.total_chunks);
    println!();

    let top = stats.top_commands(5);
    if top.is_empty() {
        println!("Top commands");
        println!("  (no data yet)");
    } else {
        println!("Top commands");
        for (i, (cmd, count)) in top.iter().enumerate() {
            println!("{}. {:<15} {}", i + 1, cmd, count);
        }
    }
    println!();

    let saved = stats.graph_saved_tokens(GRAPH_BUCKETS);
    let latency = stats.graph_latency_ms(GRAPH_BUCKETS);

    println!("Live graph  (last 7 min)");
    println!("tokens saved/min: {}", render_graph(&saved));
    println!("latency ms      : {}", render_graph(&latency));
    println!();
    println!("refreshing every 1s  •  ctrl-c to exit");
}

fn render_graph(values: &[u64]) -> String {
    let max = *values.iter().max().unwrap_or(&0);
    values
        .iter()
        .map(|&v| {
            if max == 0 {
                ' '
            } else {
                let idx = ((v * (BLOCKS.len() as u64 - 1)) / max) as usize;
                BLOCKS[idx.min(BLOCKS.len() - 1)]
            }
        })
        .collect()
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
