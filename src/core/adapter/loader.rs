use super::*;

pub(super) fn load_rules() -> Vec<AdapterRule> {
    let mut rules = Vec::new();
    let mut sequence = 0usize;

    for dir in adapter_dirs() {
        let mut loaded = load_rules_from_dir(&dir, sequence);
        sequence += loaded.len();
        rules.append(&mut loaded);
    }

    if !rules.is_empty() {
        debug_log(&format!("loaded {} adapter rules", rules.len()));
    }

    rules
}

fn load_rules_from_dir(dir: &Path, sequence_start: usize) -> Vec<AdapterRule> {
    if !dir.exists() {
        return Vec::new();
    }

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            debug_log(&format!(
                "failed to read adapter dir {}: {err}",
                dir.display()
            ));
            return Vec::new();
        }
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| is_toml_file(path))
        .collect();
    files.sort();

    let mut out = Vec::new();
    for (file_idx, file) in files.iter().enumerate() {
        let raw = match fs::read_to_string(file) {
            Ok(text) => text,
            Err(err) => {
                debug_log(&format!(
                    "failed to read adapter file {}: {err}",
                    file.display()
                ));
                continue;
            }
        };

        let parsed = match toml::from_str::<AdapterFile>(&raw) {
            Ok(spec) => spec,
            Err(err) => {
                debug_log(&format!(
                    "failed to parse adapter file {}: {err}",
                    file.display()
                ));
                continue;
            }
        };

        for (spec_idx, spec) in parsed.adapter.iter().enumerate() {
            let sequence = sequence_start + (file_idx * 1000) + spec_idx;
            if let Some(rule) = compile_rule(spec, file, sequence) {
                out.push(rule);
            }
        }
    }

    out
}

pub(super) fn compile_rule(
    spec: &AdapterSpec,
    file: &Path,
    sequence: usize,
) -> Option<AdapterRule> {
    let match_command = match Regex::new(&spec.match_command) {
        Ok(rx) => rx,
        Err(err) => {
            debug_log(&format!(
                "adapter '{}' skipped (invalid match_command regex in {}): {err}",
                spec.name,
                file.display()
            ));
            return None;
        }
    };

    let include_patterns = compile_regexes(&spec.include_patterns, file, &spec.name, "include");
    let exclude_patterns = compile_regexes(&spec.exclude_patterns, file, &spec.name, "exclude");

    let level = parse_level(spec.level.as_deref(), file, &spec.name);

    Some(AdapterRule {
        name: spec.name.clone(),
        match_command,
        signal_patterns: spec.signal_patterns.clone(),
        include_patterns,
        exclude_patterns,
        on_empty: spec.on_empty.clone(),
        level,
        max_lines: spec.max_lines,
        max_chars_per_line: spec.max_chars_per_line,
        priority: spec.priority.unwrap_or(0),
        sequence,
    })
}

fn compile_regexes(patterns: &[String], file: &Path, name: &str, label: &str) -> Vec<Regex> {
    let mut out = Vec::new();
    for raw in patterns {
        match Regex::new(raw) {
            Ok(rx) => out.push(rx),
            Err(err) => debug_log(&format!(
                "adapter '{}' skipped invalid {} regex '{}' in {}: {err}",
                name,
                label,
                raw,
                file.display()
            )),
        }
    }
    out
}

fn parse_level(raw: Option<&str>, file: &Path, name: &str) -> Option<FilterLevel> {
    let raw = raw?;

    match raw.trim().to_ascii_lowercase().as_str() {
        "none" => Some(FilterLevel::None),
        "minimal" => Some(FilterLevel::Minimal),
        "aggressive" => Some(FilterLevel::Aggressive),
        _ => {
            debug_log(&format!(
                "adapter '{}' ignored unknown level '{}' in {}",
                name,
                raw,
                file.display()
            ));
            None
        }
    }
}

fn adapter_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(cwd) = env::current_dir() {
        dirs.push(cwd.join(PROJECT_ADAPTER_DIR));
    }

    if let Some(home) = env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(GLOBAL_ADAPTER_DIR));
    }

    dirs
}

fn is_toml_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("toml"))
        .unwrap_or(false)
}
