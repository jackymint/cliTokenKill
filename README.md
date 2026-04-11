# cliTokenKill (`ctk`)

`ctk` is a Rust CLI that reduces terminal output before it reaches an AI assistant context.
It is designed for AI coding workflows where command output (logs, diffs, test output, large listings) can consume too many tokens.

## Why `ctk`?

- **Output-first optimization**: it optimizes by output shape, not by hardcoding command behavior.
- **Token reduction by default**: truncation, dedupe, compact formatting, and budget gating.
- **Auto chunking**: large output is split automatically; you send only chunk 1 first.
- **Safe runtime behavior**:
  - preserves original process exit code
  - fallback to raw output if compaction fails
- **AI-CLI-only integration mode**: can be enabled only for your AI CLI launcher, not for all terminal users.

## Features

- `proxy`: run any command and compact output
- `read`: compact file content
- `git status|diff`: convenience Git subcommands
- `test`: signal-only test output (fail/error/panic)
- `err`: signal-only error/warning output
- `chunk`: fetch stored auto-chunks by id/index
- `init --codex`: install Codex integration (AI-CLI-only)
- `doctor --codex`: inspect integration state
- `uninstall --codex`: remove integration

## Architecture

`ctk` is built in 3 layers:

1. **Universal core**
   - run process
   - capture `stdout` + `stderr`
   - preserve exit code
   - fallback-safe behavior

2. **Content-aware engine**
   - classify output shape (`json`, `ndjson`, `diff`, `logs`, `stack traces`, etc.)
   - apply compact strategy per content kind

3. **AI delivery control**
   - token budget gating
   - automatic chunking + chunk retrieval

## Install

### Build locally

```bash
cargo build --release
./target/release/ctk --help
```

### Optional: install into Cargo bin

```bash
cargo install --path .
ctk --help
```

## Quick Usage

```bash
# Generic command
ctk proxy -- ls -la

# Read a file with compaction
ctk read src/main.rs

# Git shortcuts
ctk git status
ctk git diff

# Test/error-focused views
ctk test -- cargo test
ctk err -- cargo check
```

## Output Controls

Available controls on command paths (`proxy`, `read`, `git`, `test`, `err`):

- `--level`: `none | minimal | aggressive`
- `--max-lines`: max compacted lines
- `--max-chars-per-line`: per-line truncation cap

Example:

```bash
ctk proxy \
  --level aggressive \
  --max-lines 80 \
  --max-chars-per-line 220 \
  -- npm run build
```

## Token Budget Gate

`ctk` applies a post-compaction token budget gate.

- env var: `CTK_TOKEN_BUDGET`
- default: `900` (approx using chars/4 heuristic)

Example:

```bash
CTK_TOKEN_BUDGET=600 ctk proxy -- sh -lc 'seq 1 2000'
```

When budget is exceeded, `ctk` keeps a useful head/tail slice and inserts a `budget-trim` marker.

## Auto Chunking

For long output, `ctk` automatically:

1. stores output in chunk files under `~/.ctk/chunks/<chunk_id>/`
2. prints only chunk 1
3. shows a next command hint

Retrieve additional chunks:

```bash
ctk chunk <chunk_id> 2
ctk chunk <chunk_id> 3
```

## Codex Integration (AI-CLI-only)

`ctk` supports an integration mode designed to affect **only AI CLI sessions**.

### Install integration

```bash
ctk init --codex
```

What it does:

- creates wrappers in `~/.ctk/bin`
- creates launcher: `~/.ctk/launchers/codex-ctk`
- removes old shell PATH injection blocks (keeps normal shell clean)
- wrappers compact output only when `CTK_AI_CLI=1`

### Run Codex through CTK

```bash
~/.ctk/launchers/codex-ctk
```

This launcher sets:

- `CTK_AI_CLI=1`
- `PATH="$HOME/.ctk/bin:$PATH"`

So compaction is active in that AI CLI session, while normal terminal sessions remain unaffected.

### Check integration

```bash
ctk doctor --codex
ctk doctor --codex --fix
```

### Remove integration

```bash
ctk uninstall --codex
```

## Benchmarking

Use the included script to compare raw vs compacted token estimates:

```bash
./scripts/bench_tokens.sh
./scripts/bench_tokens.sh scripts/commands.codex.txt
```

Notes:

- If Python `tiktoken` is installed, the script uses it.
- Otherwise it falls back to `chars/4` estimation.

## Environment Variables

- `CTK_TOKEN_BUDGET` - approximate token budget for post-processing gate
- `CTK_LEVEL` - wrapper default level (launcher sessions)
- `CTK_MAX_LINES` - wrapper default max lines
- `CTK_MAX_CHARS_PER_LINE` - wrapper default per-line char cap
- `CTK_BYPASS=1` - bypass compaction and execute raw command
- `CTK_AI_CLI=1` - enable wrapper compaction mode (set by launcher)

## Typical Workflow

1. Build `ctk`
2. Run `ctk init --codex`
3. Start AI CLI with `~/.ctk/launchers/codex-ctk`
4. Work normally while command output is compacted/chunked
5. Use `ctk chunk <id> <n>` only when more context is needed

## Development

```bash
cargo fmt
cargo check
cargo build --release
```

## License

MIT. See the [LICENSE](LICENSE) file.
