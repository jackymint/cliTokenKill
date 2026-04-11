# Contributing

Thanks for contributing to `cliTokenKill`.

## Before You Start

- Search existing issues and pull requests to avoid duplicate work.
- Open an issue first for larger changes so scope and approach can be aligned.

## Local Development

```bash
cargo fmt
cargo check
cargo build --release
```

Optional validation:

```bash
cargo test
./scripts/bench_tokens.sh
```

## Code Guidelines

- Keep changes focused and minimal.
- Preserve existing behavior unless the change explicitly intends to alter it.
- Prefer clear naming and small functions over complex branching.
- Keep user-facing CLI output stable unless a change is documented.

## Commit and Pull Request Guidelines

- Use clear commit messages describing intent.
- Include tests or verification steps for behavior changes.
- Update docs when commands, flags, or output behavior change.
- For PRs, include:
  - what changed
  - why it changed
  - how it was tested
  - any known limitations

## Reporting Bugs

When filing a bug, include:

- expected behavior
- actual behavior
- reproduction steps
- OS and shell
- command used and relevant output
- `ctk --version` output

## Feature Requests

Feature proposals should describe:

- use case and problem
- proposed behavior
- tradeoffs or alternatives considered
