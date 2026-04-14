use super::*;

pub(crate) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Proxy {
            command,
            path,
            level,
            max_lines,
            max_chars_per_line,
        } => run_and_exit(
            &command,
            path,
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
            None,
            filter_config(level, max_lines, max_chars_per_line),
            PipelineMode::Normal,
        )?,
        Commands::Test {
            command,
            max_lines,
            max_chars_per_line,
        } => run_and_exit(
            &command,
            None,
            filter_config(FilterLevel::Minimal, max_lines, max_chars_per_line),
            PipelineMode::TestOnly,
        )?,
        Commands::Err {
            command,
            max_lines,
            max_chars_per_line,
        } => run_and_exit(
            &command,
            None,
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
        Commands::Monitor { clear } => {
            if clear {
                Stats::clear()?;
                println!("Cleared monitor stats at {}", stats::stats_path().display());
            } else {
                run_monitor()?;
            }
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

mod run;

use self::run::run_and_exit;
