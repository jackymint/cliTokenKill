use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const AUTO_CHUNK_TRIGGER_LINES: usize = 140;
const CHUNK_SIZE_LINES: usize = 80;

pub struct ChunkPlan {
    pub triggered: bool,
    pub total_chunks: usize,
    pub total_lines: usize,
}

pub enum ChunkedText {
    Inline(String),
    Stored {
        id: String,
        total_chunks: usize,
        first_chunk: String,
    },
}

pub fn plan_auto_chunk(text: &str) -> ChunkPlan {
    let total_lines = text.lines().count();
    if total_lines <= AUTO_CHUNK_TRIGGER_LINES {
        return ChunkPlan {
            triggered: false,
            total_chunks: 1,
            total_lines,
        };
    }

    let total_chunks = total_lines.div_ceil(CHUNK_SIZE_LINES);
    ChunkPlan {
        triggered: true,
        total_chunks: total_chunks.max(1),
        total_lines,
    }
}

pub fn maybe_auto_chunk(text: String) -> Result<ChunkedText> {
    let plan = plan_auto_chunk(&text);
    if !plan.triggered {
        return Ok(ChunkedText::Inline(text));
    }
    let lines: Vec<&str> = text.lines().collect();

    let id = generate_id();
    let chunks: Vec<String> = lines
        .chunks(CHUNK_SIZE_LINES)
        .map(|block| block.join("\n"))
        .collect();

    if chunks.is_empty() {
        return Ok(ChunkedText::Inline(text));
    }

    let dir = chunk_dir().join(&id);
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create chunk dir: {}", dir.display()))?;

    for (i, chunk) in chunks.iter().enumerate() {
        let file = dir.join(format!("{:04}.txt", i + 1));
        fs::write(&file, chunk)
            .with_context(|| format!("failed to write chunk file: {}", file.display()))?;
    }

    Ok(ChunkedText::Stored {
        id,
        total_chunks: chunks.len(),
        first_chunk: chunks[0].clone(),
    })
}

pub fn read_chunk(id: &str, index: usize) -> Result<(usize, String)> {
    if index == 0 {
        bail!("chunk index must start from 1");
    }

    let dir = chunk_dir().join(id);
    let total = count_chunks(&dir)?;
    if total == 0 {
        bail!("no chunk data found for id={id}");
    }
    if index > total {
        bail!("chunk index out of range: {index} > {total}");
    }

    let file = dir.join(format!("{:04}.txt", index));
    let content =
        fs::read_to_string(&file).with_context(|| format!("failed to read {}", file.display()))?;
    Ok((total, content))
}

fn count_chunks(dir: &Path) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let entries = fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))?;
    let mut count = 0usize;
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            count += 1;
        }
    }
    Ok(count)
}

fn chunk_dir() -> PathBuf {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".ctk/chunks")
}

fn generate_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("ck{ts}")
}
