# cliTokenKill (`ctk`)

`ctk` is a Rust CLI that reduces terminal output before it reaches an AI assistant context.
It is designed for AI coding workflows where command output (logs, diffs, test output, large listings) can consume too many tokens.

## Quick Start

### 1) Install quickly

```bash
brew install jackymint/cliTokenKill/ctk
ctk --help
```

### 2) Enable auto mode for Codex/Claude

```bash
ctk init --codex --claude
exec $SHELL -l
```

After reloading shell, use AI CLI as usual:

```bash
codex
claude
```

### 3) View live token-saving stats

In a separate terminal:

```bash
ctk monitor
ctk monitor --clear
```

Output:

```
CTK Monitor
────────────────────────────────────
Active AI CLI   : claude
Commands/min    : 46
Saved tokens    : 4,688
Savings ratio   : 61%
Fallbacks       : 0
Chunks created  : 0

Top commands
  1. /usr/bin/git          7
  2. /bin/ps               12
  3. /usr/bin/security     12
  4. /usr/bin/dirname      6
  5. /usr/local/bin/code   4

Tokens saved/min  peak: 4,688 tok
  ┤            ██
  ┤      ██    ██  ██
  ┤  ██  ██    ██  ██
  ┤  ██  ██    ██  ██
  ┤  ██  ██    ██  ██
  └───────────────
   -7m          now

Latency ms  peak: 89 ms
  ┤      ██
  ┤  ██  ██  ██
  ┤  ██  ██  ██
  └───────────────
   -7m          now

watching ~/.ctk/stats.json  •  ctrl-c to exit
```

### 4) Use CTK commands directly when needed

```bash
ctk proxy -- git diff
ctk test -- cargo test
ctk err -- cargo check
ctk explain -- cargo test
```

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
- `explain`: inspect classifier/filter/budget decisions for a command
- `explain-file`: inspect classifier/filter/budget decisions for a file
- `read`: compact file content
- `git status|diff`: convenience Git subcommands
- `test`: signal-only test output (fail/error/panic)
- `err`: signal-only error/warning output
- `chunk`: fetch stored auto-chunks by id/index
- `monitor`: view live token-saving stats and top commands
- `monitor --clear`: reset stored monitor stats
- `init --codex/--claude`: install AI CLI integration (AI-CLI-only)
- `doctor --codex/--claude`: inspect integration state
- `uninstall --codex/--claude`: remove integration

## Command Cheatsheet

```bash
# compact any command
ctk proxy -- <command>

# focused views
ctk test -- <test command>
ctk err -- <build/lint command>

# inspect decisions
ctk explain -- <command>
ctk explain-file <path>

# get next chunk
ctk chunk <chunk_id> 2
```

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

## Content-Aware Compaction

The engine classifies output shape (`json`, `ndjson`, `diff`, `logs`, `stack traces`, `grep`, `tables`, `test output`) and applies a compact strategy per kind.

**Examples:**
- JSON: collapse long strings, remove whitespace
- NDJSON: deduplicate identical lines
- Diff: keep only changed lines
- Logs: collapse repeated log lines
- Stack traces: keep only error messages
- Grep: sort and deduplicate matches
- Tables: collapse duplicate rows
- Test output: keep only failures/errors

See [tests/golden/](tests/golden/) for detailed examples with input/output pairs.

## Install

### Homebrew (recommended)

```bash
brew install jackymint/cliTokenKill/ctk
```

### From source

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

# Explain classification + filtering decisions
ctk explain -- cargo test
ctk explain --mode err -- cargo check
ctk explain-file README.md
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

## Plugin Adapters (Org-Specific Parser/Filter)

`ctk` can load command-specific adapters from TOML files so your team can add
custom parser/filter rules without touching `ctk` core code.

Adapter lookup paths:

- project scope: `./.ctk/adapters/*.toml`
- user-global scope: `~/.ctk/adapters/*.toml`

Rules are loaded in this order:

1. project adapters
2. user-global adapters

Within the combined set, higher `priority` runs first.

### Adapter Schema

Each file can define one or more adapters using `[[adapter]]`.

```toml
[[adapter]]
name = "corp-cargo-test"
match_command = "^cargo test"
priority = 50
signal_patterns = ["failed", "panic", "error:"]
exclude_patterns = ["^note:"]
level = "minimal"             # none | minimal | aggressive
max_lines = 80
max_chars_per_line = 220
on_empty = "ok: no failures"
```

Field behavior:

- `name`: display/debug name for the adapter
- `match_command`: regex matched against full command line (e.g. `cargo test --all`)
- `priority`: higher value wins (`0` default)
- `signal_patterns`: optional patterns to extract signal lines first
- `include_patterns`: optional allowlist regexes (keep matching lines only)
- `exclude_patterns`: optional denylist regexes (drop matching lines)
- `level`, `max_lines`, `max_chars_per_line`: optional overrides
- `on_empty`: fallback message if all lines are filtered out

If an adapter does not match (or produces no output), `ctk` falls back to the
built-in classifier/compactor behavior automatically.

Starter template: `examples/adapters/cargo-test.toml`

### Adapter Debug

Enable adapter load/match logs:

```bash
CTK_ADAPTER_DEBUG=1 ctk proxy -- cargo test
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

## Explain Mode

Use `ctk explain` to inspect what the pipeline did for a command:

- classifier result (`json`, `ndjson`, `diff`, `log-stream`, etc.)
- strategy used (`content-aware`, `signal-only`, `adapter`, or `fallback`)
- raw vs filtered line counts and removed lines
- budget token estimate before/after, plus trim marker line

Example:

```bash
ctk explain -- cargo test
ctk explain --mode err -- cargo check
ctk explain --mode test -- cargo test
ctk explain-file src/main.rs
```

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

## AI CLI Integration (AI-CLI-only)

`ctk` supports an integration mode designed to affect **only AI CLI sessions**.

### Supported AI CLIs

- **Codex (OpenAI)** - requires local execution mode ✅
- **Claude Desktop** - supports local execution ✅

### Install integration

```bash
ctk init --codex
ctk init --claude
ctk init --codex --claude
```

What it does:

- creates wrappers in `~/.ctk/bin`
- creates launchers:
  - `~/.ctk/launchers/claude-ctk`
- adds shell aliases in `~/.zshrc` and `~/.bashrc`:
  - `claude -> ~/.ctk/launchers/claude-ctk`
- removes old shell PATH injection blocks (keeps normal shell clean)
- wrappers compact output only when `CTK_AI_CLI=1`
- **For Codex**: configures `~/.codex/config.toml` to enable local execution:
  ```toml
  sandbox_mode = "danger-full-access"
  approval_policy = "never"
  
  [features]
  codex_hooks = true
  multi_agent = true
  ```

### Run AI CLI through CTK

```bash
codex
claude  # after opening a new shell

# direct launcher also works
~/.ctk/launchers/claude-ctk
```

Codex uses hooks to enforce:

- `CTK_AI_CLI_NAME=codex ctk proxy -- <command>`

Claude launcher sets:

- `CTK_AI_CLI=1`
- `PATH="$HOME/.ctk/bin:$PATH"`

So compaction is active in the AI CLI session, while normal terminal sessions remain unaffected.

### Check integration

```bash
ctk doctor --codex
ctk doctor --claude
ctk doctor --codex --claude
ctk doctor --codex --claude --fix
```

### Remove integration

```bash
ctk uninstall --codex
ctk uninstall --claude
ctk uninstall --codex --claude
```

## Benchmarking

Use the included script to compare raw vs compacted token estimates:

```bash
./scripts/bench_tokens.sh
./scripts/bench_tokens.sh scripts/commands.codex.txt
```

Example snapshot (run on this repo, 2026-04-11):

| Command | Raw Tokens | CTK Tokens | Saved |
| --- | ---: | ---: | ---: |
| `git diff` | 7205 | 900 | 87% |
| `cargo test` | 206 | 24 | 88% |
| `rg --line-number "fn " src` | 1989 | 892 | 55% |
| **Total** | **9400** | **1816** | **80%** |

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
- `CTK_AI_CLI_NAME` - AI CLI name for stats tracking (e.g., "codex", "claude")
- `CTK_DEBUG=1` - enable debug logging to stderr
- `CTK_ADAPTER_DEBUG=1` - print adapter loading/match diagnostics

## Typical Workflow

1. Build `ctk`
2. Run `ctk init --codex` and/or `ctk init --claude`
3. Start Codex normally or start Claude with the corresponding launcher (`claude-ctk`)
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
