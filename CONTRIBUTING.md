# Contributing to codescout

We welcome contributions! Whether it's a bug fix, new language support, or documentation improvement — we're happy to review it.

## Getting Started

```bash
git clone https://github.com/mareurs/codescout.git
cd codescout
cargo build
cargo test
```

The Rust version is pinned in [`rust-toolchain.toml`](rust-toolchain.toml), so the first
`cargo` command may spend a minute letting rustup fetch that toolchain and its components
— that is expected, and it is why the checks below reproduce CI exactly. The file's own
comment explains why it is pinned and how to bump it. Note it is **not** the MSRV; that is
`rust-version` in `Cargo.toml`, checked separately by CI.

## Git Hooks

```bash
scripts/install-hooks.sh            # install / repair
scripts/install-hooks.sh --check    # report only
```

Three hooks, installed by two different routes because one of them cannot go through
the framework at all:

| hook | route | what it does |
|---|---|---|
| `pre-commit` | pre-commit.com | `cargo fmt --check`, plus two shared-checkout guards |
| `prepare-commit-msg` | direct shim | stamps `Session-Id: <uuid>` into the commit |
| `post-index-change` | direct shim | records which session staged each blob |

The last two exist because several agent sessions routinely share this working tree
and all commit as the same git author, so `%an`, commit adjacency and the dirty-file
list carry **zero** ownership signal — they are constant across sessions by
construction. The trailer makes authorship recoverable from the commit itself; the
stage log lets the `foreign-index` guard refuse a bare `git commit` that would sweep
a peer's staged paths into your commit. Both are no-ops outside an agent session:
with no `CLAUDE_CODE_SESSION_ID` in the environment, nothing is stamped and nothing
is refused.

`post-index-change` is a direct shim because pre-commit 4.6.2 cannot host it — it is
absent from `HOOK_TYPES` in its `clientlib.py`. `prepare-commit-msg` is a direct shim
by choice: the framework stashes every unstaged change in the checkout while its
hooks run, including other sessions', so each installed stage adds a window in which
a peer's in-flight work transiently reverts to HEAD and `git status` reports it clean.

**Verify positively.** A successful install is compatible with the hook never
running — that is exactly how the last hooks defect stayed invisible for a day, with
`core.hooksPath` pointing at a pre-rename path that no longer existed and git
declining to warn. `install-hooks.sh` refuses to run while that config is set, and
prints the probe to confirm the hooks actually fire.

If you work this checkout alongside other sessions, tell them before installing:
these hooks change every session's `git commit`, not just yours.
## Retrieval Stack

`semantic_search` (and the rest of the retrieval surface) defaults to a Qdrant + TEI
hybrid stack. Required for development if you want to exercise the default code path:

```bash
cp .env.example .env
./scripts/retrieval-stack.sh up         # docker compose, ~5min first time
cargo run --release --bin sync_project -- . codescout   # build the per-project index
```

E2E retrieval tests are gated by `--features retrieval-e2e` and assume the stack is
reachable on `127.0.0.1`. Unit/integration tests that don't exercise retrieval pass
without the stack — `semantic_search` returns a `RecoverableError` when the stack
is offline rather than panicking.

Tuning knobs live in `.env.example` with matrix-validated defaults
(see `docs/research/2026-05-06-retrieval-stack-benchmark.md` for the empirical record).

## Local Embedding (ONNX) Tests

No pre-commit hook compiles anything, so none of them compiles these tests in and
this section does not apply to one. That is deliberate rather than incidental:
anything that builds takes the shared `target/` lock and serialises every concurrent
session's commits, which `.pre-commit-config.yaml` records as measured. This section
applies when *you* run a `--features local-embed` verification
pass — `cargo test --features local-embed` — which then exercises a real ONNX
model against real weights
(`crates/codescout-embed/src/local.rs`: `from_dir_produces_a_stable_384d_vector` and
`from_dir_matches_the_hub_path_for_the_same_model`). They skip only on an explicit
opt-out, never on a merely-missing weights directory — a presence-keyed skip would be
indistinguishable from a pass in CI, so absent weights fail loudly by design. On your
first `cargo test --features local-embed` this reads as "the branch is broken"; it
is not — it means neither escape below is set yet.

Pick one:

```bash
# Point at a real five-file weights directory (see "Embedding without a server"
# in the manual for the exact layout) and the tests actually run:
export CODESCOUT_TEST_ONNX_DIR=/path/to/all-MiniLM-L6-v2

# Or opt out deliberately — CI sets this; it is not the default:
export CODESCOUT_SKIP_ONNX_TESTS=1
```

Without `local-embed`, neither variable matters — the two tests do not compile in.

## Dev Loop — Faster Live MCP Iteration

The default workflow documented in `CLAUDE.md` is `cargo build --release` + `/mcp`
restart. That's the right choice for users (release performance, stable artifact)
but it costs ~30s per iteration during active development.

For tighter iteration on tools, prompts, or hook surfaces:

1. Build the dev binary once: `cargo build` (no `--release`).
2. Point your MCP config at the debug binary instead of the release one. In
   `~/.claude/settings.json`:

   ```json
   {
     "mcpServers": {
       "codescout": {
         "command": "/absolute/path/to/codescout/target/debug/codescout",
         "args": ["start", "--project", "."]
       }
     }
   }
   ```

3. After each edit, just `cargo build` (~3s incremental) and `/mcp` reconnect.
   No `--release`.

**Trade-off:** debug builds are ~5–10× slower per call than release, but the
inner-loop savings dominate when you're iterating on tool descriptions, prompt
surfaces, or test scenarios where compile time matters more than runtime.

**Switch back before merging:** verify the change against `cargo build --release`
+ `/mcp` once before committing — the release build catches optimizations and
LTO-time issues that debug skips.
## Before Submitting a PR

Run the same checks CI will run — and since the toolchain is pinned, "the same" is literal
rather than approximate. A clippy lint you do not see locally is not a lint CI will find,
which is why **both** clippy invocations are listed: the bare one lints only the root
package's non-test targets with default features, and the second is the only one that
reaches `#[test]` code and `codescout-embed`'s feature-gated `local` module. CI runs both
(`.github/workflows/ci.yml:50` and `:61`), so the first passing proves nothing about the
second.

```bash
cargo fmt
cargo clippy -- -D warnings
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features
cargo test --workspace
```

The two `cargo test` lanes follow the same logic as the two clippy ones. CI's test matrix is
three configs — `default`, `local-embed`, `no-features` (`.github/workflows/ci.yml:70`) — and
the lean lane is what mirrors `no-features`. A test that reaches a `#[cfg(feature = "librarian")]`
item without gating itself compiles and passes under default features, so the default lane
cannot see it by construction.

Run them in that order. Both lanes share one `target/`, and `tests/cli_artifact.rs` resolves
`target/debug/codescout` by path at run time, so the lean lane leaves a librarian-less binary
sitting there; ending on the default lane puts it back.

This block is a deliberate second copy, kept because a contributor should not have to read an
internal agent contract to find out how to verify a patch. **[`CLAUDE.md`](CLAUDE.md) §
Development Commands is authoritative** and carries the rationale for every command above — if
the two ever disagree, this block is the bug.

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
