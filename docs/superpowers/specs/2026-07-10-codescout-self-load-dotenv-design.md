# Design — codescout self-loads a startup dotenv (retire the MCP launch wrapper)

**Date:** 2026-07-10
**Status:** approved (brainstorming)
**Author:** Marius + Claude (Opus 4.8)

## Problem

The codescout MCP server is registered in Claude Code with an empty environment
block and launched as a bare binary. codescout reads every embedding/retrieval
endpoint from `std::env::var()` at use time (`src/retrieval/config.rs:from_env()`,
and the librarian embedder at `src/librarian/mod.rs:56`), so with an empty launch
env the embedder is never built and all semantic paths go dark — the outage
diagnosed on 2026-07-10.

The current mitigation is a hand-maintained launch wrapper
(`~/.local/bin/codescout-mcp`) that `source`s `.env.amd` and exports the two
`LIBRARIAN_EMBED_*` vars before exec'ing the binary. It works but is an external
script outside the repo that must be maintained and mirrored across three Claude
Code profiles — exactly the kind of glue that rots.

Underlying smell: the librarian embedder reads a **separate** env surface
(`LIBRARIAN_EMBED_MODEL`/`_URL`/`_API_KEY`) from the code-retrieval stack
(`CODESCOUT_EMBEDDER_*`), and neither is populated by the `.env.<profile>` files —
so wiring `.env.amd` alone still left the librarian dark.

## Goal

Make codescout self-configure by loading a dotenv file at startup, so the MCP
launcher needs **no** env injection and the wrapper can be deleted. Real
environment variables still win over the file, and the feature is opt-in (no
file → no behavior change), so production/other users are unaffected.

## Non-goals

- Moving endpoint config into the `GlobalConfig` TOML (`~/.config/codescout/config.toml`).
  Rejected in brainstorming as heavier (~13 fields + plumbing at two read sites)
  for no benefit over dotenv self-load, given all consumers already read env.
- A code-level fallback deriving `LIBRARIAN_EMBED_*` from `CODESCOUT_EMBEDDER_*`.
  Rejected: the two clients speak different protocols to the same server
  (retrieval hits `…:48081` llama-native; librarian needs `…:48081/v1`
  OpenAI-compatible, and a different model string), so it is not a clean copy.
  The unification is done at the config-file level instead (see below).

## Design

### 1. Startup loader

New function `load_startup_env()` in `src/config/` (beside `global_config_path`),
called in `main()` **after logging init and the crypto-provider install, before
`Cli::parse()`** (`src/main.rs`) — early enough that no command handler has read an
embedding env var yet, and applied to every subcommand (notably CLI
`artifact --semantic`). Running after logging init lets the confirmation/warning
below go through `tracing` instead of a raw `eprintln!`. (The `.env.<profile>`
files carry only retrieval/embedder vars, no `RUST_LOG`, so loading after logging
init costs nothing.)

Path resolution:
- If `CODESCOUT_ENV_FILE` is set → use that path (explicit override).
- Else → `<config-dir>/.env`, where `<config-dir>` is the same base
  `global_config_path()` uses (`$XDG_CONFIG_HOME/codescout` or
  `$HOME/.config/codescout`). Introduce a sibling `global_env_path()` (or extract
  a shared `global_config_dir()` so both derive from one place — DRY).

Load semantics:
- `dotenvy::from_path(&path)` — which **does not override already-set env vars**.
  This preserves precedence: explicit process env > dotenv file > built-in defaults.
- If the resolved file **exists** and loads → `tracing::debug!` a one-line
  confirmation naming the path (never values).
- If `CODESCOUT_ENV_FILE` was **explicitly set** but the file is missing/unreadable
  → `tracing::warn!` (explicit intent deserves a surfaced error).
- If the **default** path is simply absent → silent no-op (opt-in).
- **Never** loads from the current working directory. Only the fixed global path or
  the explicit `CODESCOUT_ENV_FILE`. Deliberate security choice: a user-scoped
  server must not absorb an arbitrary repo's `.env` (and its secrets) based on cwd.
### 2. Config-file unification

Add the librarian embedder vars to each retrieval-profile file so one loaded file
configures both stacks:

- `.env.amd` → `LIBRARIAN_EMBED_MODEL=CodeRankEmbed`, `LIBRARIAN_EMBED_URL=http://127.0.0.1:48081/v1`
- `.env.cpu` / `.env.gpu` / `.env.lite` → the profile's equivalent endpoint (mirror each file's existing `CODESCOUT_EMBEDDER_URL`, suffixed `/v1`; model per profile).
- `.env.example` → document both vars with comments.

### 3. Dependency

Add `dotenvy` (maintained fork of `dotenv`) to `Cargo.toml` `[dependencies]`.

### 4. Docs + memory

- `docs/manual/src/concepts/librarian-embedded.md` § Migration — replace the
  `env`-block example with the self-load story: symlink a profile to
  `~/.config/codescout/.env` (or set `CODESCOUT_ENV_FILE`), then a bare
  `codescout start --debug` registration.
- codescout memory `development-commands` — note the self-load + symlink setup.

### 5. Cutover (post-ship, after `cargo rb`)

1. `ln -s /home/marius/work/claude/codescout/.env.amd ~/.config/codescout/.env`
2. Re-register bare: `claude mcp remove codescout -s user` then
   `claude mcp add codescout -s user -- /home/marius/.cargo/bin/codescout start --debug`
3. `/mcp` reconnect, verify env loaded + semantic works.
4. `rm ~/.local/bin/codescout-mcp`.
5. Mirror step 2 across `~/.claude-sdd` and `~/.claude-kat` (per global CLAUDE.md).

The symlink means `.env.amd` stays the single source of truth; switching profiles
is re-pointing the symlink.

## Tests

`src/config/` unit tests (env-mutating → `#[serial]` + `lock_env_for_tests()` +
save/restore, matching the existing `global_config_path` tests):

1. `CODESCOUT_ENV_FILE` pointing at a temp file loads its vars into the process env.
2. A pre-set process env var is **not** overridden by a conflicting file entry.
3. `CODESCOUT_ENV_FILE` unset and default path absent → no-op (no panic, nothing set).
4. `global_env_path()` derives `<base>/codescout/.env` from `XDG_CONFIG_HOME`, and
   falls back to `$HOME/.config/codescout/.env` — mirroring the two existing
   `global_config_path` path tests.

## Risks / notes

- `main()` runs the loader; lib unit tests don't call `main()`, so existing tests
  are unaffected except the new ones.
- `dotenvy::from_path` non-override behavior is load-bearing for precedence — the
  test in item 2 pins it.
- Windows: `HOME` may be unset; `global_config_path()` already returns `Option`
  and callers treat `None` as "no global config" — the loader mirrors that
  (`None` default path → no-op).
