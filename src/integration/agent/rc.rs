use super::*;

pub(super) fn remove_legacy_rc_path_blocks(home: &Path) -> Result<Vec<PathBuf>> {
    let mut updated = Vec::new();
    for file in rc_files(home) {
        if remove_legacy_path_block(&file)? {
            updated.push(file);
        }
    }
    Ok(updated)
}

fn remove_legacy_path_block(rc_file: &Path) -> Result<bool> {
    remove_block(rc_file, LEGACY_PATH_BLOCK_START, LEGACY_PATH_BLOCK_END)
}

pub(super) fn upsert_agent_alias_blocks(
    home: &Path,
    agent_cmd: &str,
    launcher_file: &str,
) -> Result<Vec<PathBuf>> {
    let mut updated = Vec::new();
    let (start, end) = alias_block_markers(agent_cmd);
    let block =
        format!("{start}\nalias {agent_cmd}=\"$HOME/.ctk/launchers/{launcher_file}\"\n{end}\n");

    for file in rc_files(home) {
        if upsert_block(&file, &start, &end, &block)? {
            updated.push(file);
        }
    }

    Ok(updated)
}

pub(super) fn remove_agent_alias_blocks(home: &Path, agent_cmd: &str) -> Result<Vec<PathBuf>> {
    let mut updated = Vec::new();
    let (start, end) = alias_block_markers(agent_cmd);
    for file in rc_files(home) {
        if remove_block(&file, &start, &end)? {
            updated.push(file);
        }
    }
    Ok(updated)
}

fn alias_block_markers(agent_cmd: &str) -> (String, String) {
    (
        format!("# >>> ctk {agent_cmd} launcher alias >>>"),
        format!("# <<< ctk {agent_cmd} launcher alias <<<"),
    )
}

fn rc_files(home: &Path) -> Vec<PathBuf> {
    vec![home.join(".zshrc"), home.join(".bashrc")]
}

pub(super) fn remove_block(rc_file: &Path, start: &str, end: &str) -> Result<bool> {
    if !rc_file.exists() {
        return Ok(false);
    }
    let content = fs::read_to_string(rc_file)
        .with_context(|| format!("failed to read {}", rc_file.display()))?;

    let start_idx = content.find(start);
    let end_idx = content.find(end);
    let (Some(start_idx), Some(end_idx)) = (start_idx, end_idx) else {
        return Ok(false);
    };

    let end_inclusive = end_idx + end.len();
    let mut updated = String::new();
    updated.push_str(&content[..start_idx]);
    if end_inclusive < content.len() {
        let rest = content[end_inclusive..].trim_start_matches('\n');
        updated.push_str(rest);
    }

    fs::write(rc_file, updated)
        .with_context(|| format!("failed to write {}", rc_file.display()))?;
    Ok(true)
}

pub(super) fn upsert_block(rc_file: &Path, start: &str, end: &str, block: &str) -> Result<bool> {
    let current = if rc_file.exists() {
        fs::read_to_string(rc_file)
            .with_context(|| format!("failed to read {}", rc_file.display()))?
    } else {
        String::new()
    };
    let mut original = current.clone();

    let replaced =
        if let (Some(start_idx), Some(end_idx)) = (original.find(start), original.find(end)) {
            let end_inclusive = end_idx + end.len();
            let mut out = String::new();
            out.push_str(&original[..start_idx]);
            if !out.ends_with('\n') && !out.is_empty() {
                out.push('\n');
            }
            out.push_str(block);
            let tail = original[end_inclusive..].trim_start_matches('\n');
            if !tail.is_empty() {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(tail);
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            out
        } else {
            if !original.is_empty() && !original.ends_with('\n') {
                original.push('\n');
            }
            if !original.is_empty() {
                original.push('\n');
            }
            original.push_str(block);
            original
        };

    let changed = replaced != current;
    if changed {
        fs::write(rc_file, replaced)
            .with_context(|| format!("failed to write {}", rc_file.display()))?;
    }
    Ok(changed)
}
