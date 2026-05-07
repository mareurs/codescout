# Contributing to codescout

We welcome contributions! Whether it's a bug fix, new language support, or documentation improvement — we're happy to review it.

## Getting Started

```bash
git clone https://github.com/mareurs/codescout.git
cd codescout
cargo build
cargo test
```

## Retrieval Stack

`semantic_search` (and the rest of the retrieval surface) defaults to a Qdrant + TEI
hybrid stack. Required for development if you want to exercise the default code path:

```bash
cp .env.example .env
./scripts/retrieval-stack.sh up         # docker compose, ~5min first time
cargo run --release --bin sync_project -- . codescout   # build the per-project index
```

E2E retrieval tests are gated by `--features retrieval-e2e` and assume the stack is
reachable on `127.0.0.1`. For unit/integration tests that don't exercise retrieval,
either set `CODESCOUT_RETRIEVAL_BACKEND=legacy` or stay on the default — the stack
codepath returns a `RecoverableError` when offline rather than panicking.

Tuning knobs live in `.env.example` with matrix-validated defaults
(see `docs/research/2026-05-06-retrieval-stack-benchmark.md` for the empirical record).
## Before Submitting a PR

Run the same checks CI will run:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

## What to Contribute

**Good first contributions:**
- Add a tree-sitter grammar for a new language (see `src/ast/`)
- Add an LSP server config for a new language (see `src/lsp/servers/`)
- Fix a bug
- Improve documentation

**Please open an issue first for:**
- Large architectural changes
- New tool categories
- Changes to the progressive disclosure design

## Using Claude Code?

PRs generated with Claude Code are welcome. Just mention it in the PR description. If you're using codescout itself as an MCP server while contributing to codescout — that's the dream. Let us know how it went.

## Project Structure

See [CLAUDE.md](CLAUDE.md) for the full developer guide, including project structure, design principles, and key patterns. That file is also what Claude Code reads when working on this project.
