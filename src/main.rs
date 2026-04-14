mod cli;
mod commands;
mod core;
mod debug;
mod engine;
mod integration;
mod monitor;
mod report;
mod stats;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands, ExplainMode, GitCommands};
use commands::handle_command;
use core::budget::apply_token_budget;
use core::chunk::{maybe_auto_chunk, read_chunk};
use core::filter::{FilterConfig, FilterLevel};
use core::pipeline::{PipelineMode, run_pipeline};
use debug::explain::{explain_command, explain_file};
use engine::{classify_content, compact_by_kind};
use integration::claude::{doctor_claude, init_claude, uninstall_claude};
use integration::codex::{doctor_codex, init_codex, uninstall_codex};
use monitor::run_monitor;
use report::{
    print_doctor_result, print_init_result, print_pipeline_output, print_uninstall_result,
};
use stats::Stats;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    if let Err(err) = run() {
        eprintln!("ctk: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    handle_command(cli.command)
}
