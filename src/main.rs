mod core;
mod engine;
mod integration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use core::budget::apply_token_budget;
use core::chunk::{ChunkedText, maybe_auto_chunk, read_chunk};
use core::filter::{FilterConfig, FilterLevel};
use core::pipeline::{PipelineMode, run_pipeline};
use engine::{classify_content, compact_by_kind};
use integration::codex::{doctor_codex, init_codex, uninstall_codex};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "ctk",
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
    /// Run any command and output only error/warning lines
    Err {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
        #[arg(short, long, default_value_t = 120)]
        max_lines: usize,
        #[arg(long, default_value_t = 240)]
        max_chars_per_line: usize,
    },
    /// Install shell wrappers and PATH updates for Codex CLI
    Init {
        /// Install integration for Codex CLI
        #[arg(long)]
        codex: bool,
    },
    /// Uninstall shell wrappers and PATH updates for Codex CLI
    Uninstall {
        /// Uninstall integration for Codex CLI
        #[arg(long)]
        codex: bool,
    },
    /// Show integration status diagnostics
    Doctor {
        /// Show diagnostics for Codex integration
        #[arg(long)]
        codex: bool,
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
}

#[derive(Subcommand)]
enum GitCommands {
    Status,
    Diff,
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
        Commands::Init { codex } => {
            handle_init(codex)?;
        }
        Commands::Chunk { id, index } => handle_chunk(&id, index)?,
        Commands::Uninstall { codex } => {
            handle_uninstall(codex)?;
        }
        Commands::Doctor { codex, fix } => {
            handle_doctor(codex, fix)?;
        }
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

fn handle_init(codex: bool) -> Result<()> {
    require_codex(codex, "init")?;
    let result = init_codex()?;
    println!("ctk codex integration installed");
    println!("mode: ai-cli-only");
    println!("wrapper dir: {}", result.bin_dir.display());
    print_wrappers_summary(&result.wrappers_installed);
    if let Some(launcher) = result.launcher_path {
        println!("launcher: {}", launcher.display());
        println!("use: {} <args>", launcher.display());
    } else {
        println!("launcher: codex not found in PATH");
    }
    print_rc_update_summary(&result.rc_files_updated, "already clean");
    println!("next: run Codex via launcher");
    Ok(())
}

fn handle_uninstall(codex: bool) -> Result<()> {
    require_codex(codex, "uninstall")?;
    let result = uninstall_codex()?;
    println!("ctk codex integration removed");
    println!("wrapper files removed: {}", result.removed_wrapper_files);
    println!("wrapper dir removed: {}", result.removed_dir);
    print_rc_update_summary(&result.rc_files_updated, "no changes");
    println!("next: open a new terminal/Codex session");
    Ok(())
}

fn handle_doctor(codex: bool, fix: bool) -> Result<()> {
    require_codex(codex, "doctor")?;
    let d = doctor_codex(fix)?;
    println!("ctk doctor (codex)");
    println!("repaired: {}", d.repaired);
    println!("ctk wrapper dir in PATH: {}", d.ctk_in_path);
    if let Some(v) = d.ctk_in_login_shell_path {
        println!("ctk wrapper dir in login shell PATH: {v}");
    } else {
        println!("ctk wrapper dir in login shell PATH: unknown");
    }
    println!("launcher exists: {}", d.launcher_exists);
    println!("wrappers count: {}", d.wrappers_count);
    println!("PATH head:");
    for p in d.path_head {
        println!(" - {p}");
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

fn filter_config(level: FilterLevel, max_lines: usize, max_chars_per_line: usize) -> FilterConfig {
    FilterConfig {
        level,
        max_lines,
        max_chars_per_line,
    }
}

fn require_codex(codex: bool, verb: &str) -> Result<()> {
    if codex {
        return Ok(());
    }
    eprintln!("ctk: no target selected. try: ctk {verb} --codex");
    std::process::exit(1);
}

fn print_wrappers_summary(wrappers: &[String]) {
    if wrappers.is_empty() {
        println!("wrappers: none (no matching commands found)");
        return;
    }
    let sample = wrappers
        .iter()
        .take(20)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    println!("wrappers: {} commands (sample: {})", wrappers.len(), sample);
}

fn print_rc_update_summary(files: &[PathBuf], empty_message: &str) {
    if files.is_empty() {
        println!("shell rc: {empty_message}");
        return;
    }
    let joined = files
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    println!("shell rc updated: {joined}");
}

fn run_and_exit(command: &[String], config: FilterConfig, mode: PipelineMode) -> Result<()> {
    let result = run_pipeline(command, config, mode)?;
    let budgeted = apply_token_budget(result.output);
    match maybe_auto_chunk(budgeted)? {
        ChunkedText::Inline(text) => println!("{text}"),
        ChunkedText::Stored {
            id,
            total_chunks,
            first_chunk,
        } => {
            println!(
                "[ctk auto-chunk id={id} chunks={total_chunks}] showing chunk 1/{total_chunks}"
            );
            println!("{first_chunk}");
            if total_chunks > 1 {
                println!();
                println!("next: ctk chunk {id} 2");
            }
        }
    }
    if result.fallback_used {
        eprintln!("ctk: filter fallback to raw output");
    }
    if result.exit_code != 0 {
        std::process::exit(result.exit_code);
    }
    Ok(())
}
