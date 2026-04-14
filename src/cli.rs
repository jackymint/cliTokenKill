use crate::core::filter::FilterLevel;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "ctk",
    version,
    about = "cliTokenKill - compact command output for LLM workflows"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Run any command and compact its output
    Proxy {
        /// Command to run
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
        /// Working directory for command execution
        #[arg(short, long)]
        path: Option<PathBuf>,
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
    Monitor {
        /// Clear stored monitor stats and exit
        #[arg(long)]
        clear: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum GitCommands {
    Status,
    Diff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ExplainMode {
    Normal,
    Test,
    Err,
}
