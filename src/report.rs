use crate::core::chunk::ChunkedText;
use crate::integration::{DoctorResult, InitResult, UninstallResult};
use std::path::PathBuf;

/// Prints pipeline output. Returns `true` if the output was stored as chunks.
pub fn print_pipeline_output(chunk: ChunkedText) -> bool {
    match chunk {
        ChunkedText::Inline(text) => {
            println!("{text}");
            false
        }
        ChunkedText::Stored {
            id,
            total_chunks,
            first_chunk,
        } => {
            println!("[ctk auto-chunk id={id} chunks={total_chunks}] showing chunk 1/{total_chunks}");
            println!("{first_chunk}");
            if total_chunks > 1 {
                println!();
                println!("next: ctk chunk {id} 2");
            }
            true
        }
    }
}

pub fn print_init_result(target: &str, result: &InitResult) {
    println!("ctk {target} integration installed");
    println!("mode: ai-cli-only");
    println!("wrapper dir: {}", result.bin_dir.display());
    print_wrappers_summary(&result.wrappers_installed);
    if let Some(launcher) = &result.launcher_path {
        println!("launcher: {}", launcher.display());
        println!("use: {} <args>", launcher.display());
    } else {
        println!("launcher: {target} not found in PATH");
    }
    print_rc_update_summary(&result.rc_files_updated, "already clean");
    if result.launcher_path.is_some() {
        println!("shell alias: {target} -> ~/.ctk/launchers/{target}-ctk");
        println!("next: open a new shell, then run: {target}");
    } else {
        println!("next: run {target} via launcher once available");
    }
}

pub fn print_uninstall_result(target: &str, result: &UninstallResult) {
    println!("ctk {target} integration removed");
    println!("wrapper files removed: {}", result.removed_wrapper_files);
    println!("wrapper dir removed: {}", result.removed_dir);
    print_rc_update_summary(&result.rc_files_updated, "no changes");
    println!("next: open a new terminal/{target} session");
}

pub fn print_doctor_result(target: &str, d: &DoctorResult) {
    println!("ctk doctor ({target})");
    println!("repaired: {}", d.repaired);
    match &d.real_command_path {
        Some(path) => println!("real {target} binary: {}", path.display()),
        None => println!("real {target} binary: not found"),
    }
    println!("ctk wrapper dir in PATH: {}", d.ctk_in_path);
    if let Some(v) = d.ctk_in_login_shell_path {
        println!("ctk wrapper dir in login shell PATH: {v}");
    } else {
        println!("ctk wrapper dir in login shell PATH: unknown");
    }
    println!("launcher exists: {}", d.launcher_exists);
    println!("launcher path: {}", d.launcher_path.display());
    match &d.launcher_exec_path {
        Some(path) => println!("launcher exec target: {}", path.display()),
        None => println!("launcher exec target: unknown"),
    }
    match d.launcher_selected_first {
        Some(v) => println!("launcher selected first: {v}"),
        None => println!("launcher selected first: unknown"),
    }
    match &d.shell_selected {
        Some(selected) => println!("shell resolves first: {selected}"),
        None => println!("shell resolves first: unknown"),
    }
    match &d.ai_cli_env {
        Some(v) => println!("CTK_AI_CLI: set ({v})"),
        None => println!("CTK_AI_CLI: unset"),
    }
    match &d.bypass_env {
        Some(v) => println!("CTK_BYPASS: set ({v})"),
        None => println!("CTK_BYPASS: unset"),
    }
    println!("bypass enabled: {}", d.bypass_enabled);
    println!("which -a {target}:");
    if d.command_matches.is_empty() {
        println!(" - (no match)");
    } else {
        for path in &d.command_matches {
            println!(" - {path}");
        }
    }
    println!("type -a {target} (login shell):");
    if d.shell_type_chain.is_empty() {
        println!(" - (no output)");
    } else {
        for line in &d.shell_type_chain {
            println!(" - {line}");
        }
    }
    println!("wrapped commands ({}):", d.wrappers_count);
    if d.wrapped_commands.is_empty() {
        println!(" - (none)");
    } else {
        for cmd in &d.wrapped_commands {
            println!(" - {cmd}");
        }
    }
    println!("PATH head:");
    for p in &d.path_head {
        println!(" - {p}");
    }
    if d.launcher_exists && d.launcher_selected_first == Some(false) {
        println!("hint: launcher is not first in PATH resolution; reopen shell or fix alias order");
    }
    println!("hint: if your shell still uses cached command paths, run: hash -r");
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
