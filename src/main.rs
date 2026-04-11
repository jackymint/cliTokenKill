mod core;
mod debug;
mod engine;
mod integration;
mod monitor;
mod report;
mod stats;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use core::budget::apply_token_budget;
use core::chunk::{maybe_auto_chunk, read_chunk};
use core::filter::{FilterConfig, FilterLevel};
use core::pipeline::{PipelineMode, run_pipeline};
use debug::explain::{explain_command, explain_file};
use engine::{classify_content, compact_by_kind};
use integration::claude::{doctor_claude, init_claude, uninstall_claude};
use integration::codex::{doctor_codex, init_codex, uninstall_codex};
use monitor::run_monitor;
use report::{print_doctor_result, print_init_result, print_pipeline_output, print_uninstall_result};
use stats::Stats;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(
    name = "ctk",
    version,
    about = "cliTokenKill - compact command output for LLM workflows"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run any command and compact its output
    Proxy {
        /// Command to run
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
        /// Filtering level
        #[arg(short, long, default_value = "minimal")]
        level: FilterLevel,
        /// Max lines after compaction
        #[arg(short, long, default_value_t = 120)]
        max_lines: usize,
        /// Max chars per line after compaction
        #[arg(long, default_value_t = 240)]
        max_chars_per_line: usize,
    },
    /// Read file and compact content
    Read {
        file: PathBuf,
        #[arg(short, long, default_value = "minimal")]
        level: FilterLevel,
        #[arg(short, long, default_value_t = 120)]
        max_lines: usize,
        #[arg(long, default_value_t = 240)]
        max_chars_per_line: usize,
    },
    /// Common git commands with compact output
    Git {
        #[command(subcommand)]
        command: GitCommands,
        #[arg(short, long, default_value = "minimal")]
        level: FilterLevel,
        #[arg(short, long, default_value_t = 120)]
        max_lines: usize,
        #[arg(long, default_value_t = 240)]
        max_chars_per_line: usize,
    },
    /// Run tests and output only signal lines (fail/error/panic)
    Test {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
        #[arg(short, long, default_value_t = 120)]
        max_lines: usize,
        #[arg(long, default_value_t = 240)]
        max_chars_per_line: usize,
    },
    /// Explain which classifier/filter/budget behavior was applied
    Explain {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
        #[arg(short, long, default_value = "minimal")]
        level: FilterLevel,
        #[arg(short, long, default_value_t = 120)]
        max_lines: usize,
        #[arg(long, default_value_t = 240)]
        max_chars_per_line: usize,
        /// Explain pipeline mode
        #[arg(long, default_value = "normal")]
        mode: ExplainMode,
    },
    /// Explain classification/compaction for a file input
    ExplainFile {
        file: PathBuf,
        #[arg(short, long, default_value = "minimal")]
        level: FilterLevel,
        #[arg(short, long, default_value_t = 120)]
        max_lines: usize,
        #[arg(long, default_value_t = 240)]
        max_chars_per_line: usize,
    },
    /// Run any command and output only error/warning lines
    Err {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
        #[arg(short, long, default_value_t = 120)]
        max_lines: usize,
        #[arg(long, default_value_t = 240)]
        max_chars_per_line: usize,
    },
    /// Install shell wrappers and launchers for AI CLI integrations
    Init {
        /// Install integration for Codex CLI
        #[arg(long)]
        codex: bool,
        /// Install integration for Claude CLI
        #[arg(long)]
        claude: bool,
    },
    /// Uninstall shell wrappers and launchers for AI CLI integrations
    Uninstall {
        /// Uninstall integration for Codex CLI
        #[arg(long)]
        codex: bool,
        /// Uninstall integration for Claude CLI
        #[arg(long)]
        claude: bool,
    },
    /// Show integration status diagnostics
    Doctor {
        /// Show diagnostics for Codex integration
        #[arg(long)]
        codex: bool,
        /// Show diagnostics for Claude integration
        #[arg(long)]
        claude: bool,
        /// Attempt to auto-repair integration before reporting status
        #[arg(long)]
        fix: bool,
    },
    /// Show stored auto chunk by id and index
    Chunk {
        id: String,
        #[arg(default_value_t = 1)]
        index: usize,
    },
    /// Show live token-saving stats dashboard
    Monitor,
}

#[derive(Subcommand)]
enum GitCommands {
    Status,
    Diff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ExplainMode {
    Normal,
    Test,
    Err,
}

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

fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Proxy {
            command,
            level,
            max_lines,
            max_chars_per_line,
        } => run_and_exit(
            &command,
            filter_config(level, max_lines, max_chars_per_line),
            PipelineMode::Normal,
        )?,
        Commands::Read {
            file,
            level,
            max_lines,
            max_chars_per_line,
        } => handle_read(file, filter_config(level, max_lines, max_chars_per_line))?,
        Commands::Git {
            command,
            level,
            max_lines,
            max_chars_per_line,
        } => run_and_exit(
            &git_args(command),
            filter_config(level, max_lines, max_chars_per_line),
            PipelineMode::Normal,
        )?,
        Commands::Test {
            command,
            max_lines,
            max_chars_per_line,
        } => run_and_exit(
            &command,
            filter_config(FilterLevel::Minimal, max_lines, max_chars_per_line),
            PipelineMode::TestOnly,
        )?,
        Commands::Err {
            command,
            max_lines,
            max_chars_per_line,
        } => run_and_exit(
            &command,
            filter_config(FilterLevel::Aggressive, max_lines, max_chars_per_line),
            PipelineMode::ErrorOnly,
        )?,
        Commands::Explain {
            command,
            level,
            max_lines,
            max_chars_per_line,
            mode,
        } => explain_command(
            &command,
            filter_config(level, max_lines, max_chars_per_line),
            pipeline_mode_from_explain(mode),
        )?,
        Commands::ExplainFile {
            file,
            level,
            max_lines,
            max_chars_per_line,
        } => explain_file(&file, filter_config(level, max_lines, max_chars_per_line))?,
        Commands::Init { codex, claude } => {
            handle_init(codex, claude)?;
        }
        Commands::Chunk { id, index } => handle_chunk(&id, index)?,
        Commands::Uninstall { codex, claude } => {
            handle_uninstall(codex, claude)?;
        }
        Commands::Doctor { codex, claude, fix } => {
            handle_doctor(codex, claude, fix)?;
        }
        Commands::Monitor => run_monitor()?,
    }
    Ok(())
}

fn handle_read(file: PathBuf, config: FilterConfig) -> Result<()> {
    let content = fs::read_to_string(&file)
        .with_context(|| format!("failed to read file: {}", file.display()))?;
    let kind = classify_content(&content);
    let compacted =
        std::panic::catch_unwind(|| compact_by_kind(&content, kind, config)).unwrap_or(content);
    println!("{compacted}");
    Ok(())
}

fn handle_init(codex: bool, claude: bool) -> Result<()> {
    require_target_selected(codex, claude, "init")?;
    if codex {
        let result = init_codex()?;
        print_init_result("codex", &result);
    }
    if claude {
        let result = init_claude()?;
        print_init_result("claude", &result);
    }
    Ok(())
}

fn handle_uninstall(codex: bool, claude: bool) -> Result<()> {
    require_target_selected(codex, claude, "uninstall")?;
    if codex {
        let result = uninstall_codex()?;
        print_uninstall_result("codex", &result);
    }
    if claude {
        let result = uninstall_claude()?;
        print_uninstall_result("claude", &result);
    }
    Ok(())
}

fn handle_doctor(codex: bool, claude: bool, fix: bool) -> Result<()> {
    require_target_selected(codex, claude, "doctor")?;
    if codex {
        let d = doctor_codex(fix)?;
        print_doctor_result("codex", &d);
    }
    if claude {
        let d = doctor_claude(fix)?;
        print_doctor_result("claude", &d);
    }
    Ok(())
}

fn handle_chunk(id: &str, index: usize) -> Result<()> {
    let (total, content) = read_chunk(id, index)?;
    println!("[ctk chunk {index}/{total} id={id}]");
    println!("{content}");
    Ok(())
}

fn git_args(command: GitCommands) -> Vec<String> {
    match command {
        GitCommands::Status => vec!["git".into(), "status".into(), "--short".into()],
        GitCommands::Diff => vec!["git".into(), "diff".into(), "--minimal".into()],
    }
}

fn pipeline_mode_from_explain(mode: ExplainMode) -> PipelineMode {
    match mode {
        ExplainMode::Normal => PipelineMode::Normal,
        ExplainMode::Test => PipelineMode::TestOnly,
        ExplainMode::Err => PipelineMode::ErrorOnly,
    }
}

fn filter_config(level: FilterLevel, max_lines: usize, max_chars_per_line: usize) -> FilterConfig {
    FilterConfig {
        level,
        max_lines,
        max_chars_per_line,
    }
}

fn require_target_selected(codex: bool, claude: bool, verb: &str) -> Result<()> {
    if codex || claude {
        return Ok(());
    }
    eprintln!("ctk: no target selected. try: ctk {verb} --codex and/or --claude");
    std::process::exit(1);
}

fn run_and_exit(command: &[String], config: FilterConfig, mode: PipelineMode) -> Result<()> {
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

    record_stats(command, raw_chars, filtered_chars, latency_ms, fallback_used, new_chunks);

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
    let mut stats = Stats::load();
    stats.record(cmd, raw_chars, filtered_chars, latency_ms, fallback, new_chunks);
    stats.save().ok();
}
