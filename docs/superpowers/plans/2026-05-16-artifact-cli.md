# Artifact CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `codescout artifact …`, `codescout artifact-event …`, `codescout artifact-refresh …`, and `codescout artifact-augment …` subcommands that mirror the librarian MCP tools 1:1, unblocking shell scripts and hooks (notably the goal-tracker Stop hook deferred from the goal-tracker plan's Phase 3).

**Architecture:** New `src/cli/` module that translates clap-parsed args into `serde_json::Value` tool args, calls the existing librarian-mcp tool functions, and pretty-prints (or JSONs) the result. Bootstrap reuses `librarian_mcp::build_tool_context()` so the CLI shares project/catalog resolution with the MCP server. Embedder stays opt-in via `LIBRARIAN_EMBED_MODEL` env.

**Tech Stack:** Rust, clap, tokio, serde_json, librarian-mcp, assert_cmd (integration tests).

**Spec:** `docs/superpowers/specs/2026-05-16-artifact-cli-design.md`

**Phase checkpoints:** Phase 1 ships `artifact find` end-to-end (unblocks Phase 3 of the goal-tracker plan). Phases 2/3/4 each end with `cargo fmt && cargo clippy -- -D warnings && cargo test` clean.

---

## Phase 1 — Foundation + `artifact find`

Adds the `src/cli/` module, the `Commands::Artifact` clap variant, and the `find` verb end-to-end. After Phase 1 the Stop hook from the goal-tracker plan becomes implementable.

### Task 1: Module scaffold + dev-deps

**Files:**
- Modify: `Cargo.toml` (add `[dev-dependencies]` entries)
- Modify: `src/lib.rs` (add `pub mod cli;`)
- Create: `src/cli/mod.rs` (placeholder)

- [ ] **Step 1: Add dev-dependencies**

`assert_cmd` and `predicates` are already in `crates/librarian-mcp/Cargo.toml`'s dev-deps. They need to live in the root crate too because the new integration test will live at `tests/cli_artifact.rs` (root crate).

Use `mcp__codescout__edit_file` to find this exact text in `Cargo.toml`:

```toml
[dev-dependencies]
```

…and replace with (preserving the existing entries — open the file first via `read_file` to see what's already there). Add these two lines underneath the existing entries:

```toml
assert_cmd = "2"
predicates = "3"
```

- [ ] **Step 2: Verify the workspace builds with the new deps**

Run: `cargo build`

Expected: clean build. If `assert_cmd` or `predicates` complain about MSRV, pin to a known-compatible version (`assert_cmd = "2.0"`, `predicates = "3.0"`).

- [ ] **Step 3: Create `src/cli/mod.rs`**

Use `mcp__codescout__create_file`:

```rust
//! CLI dispatch layer for `codescout artifact*` subcommands.
//!
//! Each verb translates clap-parsed args into a `serde_json::Value` shaped
//! like the corresponding librarian-mcp tool's input, calls the tool, and
//! routes the response through `format::print`.

pub mod format;
```

- [ ] **Step 4: Create `src/cli/format.rs` placeholder**

```rust
//! Output formatter for the CLI. Pretty by default, JSON via `--json`.

use anyhow::Result;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default)]
pub struct OutputOpts {
    pub json: bool,
    pub no_color: bool,
}

pub fn print(value: &Value, opts: &OutputOpts) -> Result<()> {
    // Phase 1 placeholder — replaced in Task 3.
    let _ = (value, opts);
    Ok(())
}
```

- [ ] **Step 5: Wire `cli` module into the library**

Use `mcp__codescout__edit_file` to find this exact text in `src/lib.rs`:

```rust
pub mod agent;
```

Replace with:

```rust
pub mod agent;
pub mod cli;
```

(If the exact preceding line differs, use `mcp__codescout__symbols(path="src/lib.rs")` to find a stable anchor; the goal is one new top-level `pub mod cli;` declaration alongside other `pub mod` entries.)

- [ ] **Step 6: Build**

Run: `cargo build`

Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src/lib.rs src/cli/
git commit -m "feat(cli): scaffold cli module + add assert_cmd/predicates dev-deps

Empty cli::format placeholder. The cli module will host the
artifact / artifact-event / artifact-refresh / artifact-augment
subcommand dispatchers introduced in subsequent tasks.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 2: `cli/mod.rs` core — CommonOpts, open_ctx, exit_with, read_at_or_stdin

**Files:**
- Modify: `src/cli/mod.rs` (replace the placeholder)
- Modify: `src/cli/mod.rs` `tests` module (new)

- [ ] **Step 1: Write the failing tests**

Use `mcp__codescout__edit_file` with `insert: append` on `src/cli/mod.rs`:

```rust

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn read_at_or_stdin_reads_file_when_at_prefix() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "hello from file").unwrap();
        let arg = format!("@{}", tmp.path().display());
        let got = read_at_or_stdin(&arg).unwrap();
        assert_eq!(got.trim_end(), "hello from file");
    }

    #[test]
    fn read_at_or_stdin_rejects_missing_file() {
        let err = read_at_or_stdin("@/definitely/not/a/path").unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("/definitely/not/a/path"),
            "error should name the missing path; got: {msg}"
        );
    }

    #[test]
    fn read_at_or_stdin_returns_raw_text_without_at_prefix() {
        // Anything not starting with `@` (and not equal to `-`) is treated as the literal value.
        let got = read_at_or_stdin("plain value").unwrap();
        assert_eq!(got, "plain value");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout --lib cli::tests`

Expected: FAIL — `read_at_or_stdin` does not exist yet.

- [ ] **Step 3: Replace the placeholder body in `cli/mod.rs`**

Use `mcp__codescout__edit_code` `action="replace"` is awkward at module scope; use `edit_file` to replace the file's pre-test content. Replace the existing top of file (everything before `#[cfg(test)]`) with:

```rust
//! CLI dispatch layer for `codescout artifact*` subcommands.
//!
//! Each verb translates clap-parsed args into a `serde_json::Value` shaped
//! like the corresponding librarian-mcp tool's input, calls the tool, and
//! routes the response through `format::print`.

pub mod format;

use anyhow::{anyhow, Context, Result};
use std::io::Read;
use std::path::PathBuf;

/// Flags shared by every CLI subcommand.
#[derive(Debug, Clone, Default)]
pub struct CommonOpts {
    pub project: Option<PathBuf>,
    pub json: bool,
    pub no_color: bool,
}

impl CommonOpts {
    pub fn output(&self) -> format::OutputOpts {
        format::OutputOpts {
            json: self.json,
            no_color: self.no_color,
        }
    }
}

/// Build the librarian-mcp `ToolContext`. Honors `--project` by setting
/// `LIBRARIAN_CWD` before delegating to the shared bootstrap.
///
/// Thread-safety: `std::env::set_var` is not safe in the presence of other
/// threads. The codescout binary runs one command per process, so the racy
/// window does not exist in practice. If a future refactor moves CLI dispatch
/// into a long-running context (e.g. a REPL), this must change.
pub async fn open_ctx(opts: &CommonOpts) -> Result<librarian_mcp::tools::ToolContext> {
    if let Some(p) = opts.project.as_ref() {
        std::env::set_var("LIBRARIAN_CWD", p);
    }
    librarian_mcp::build_tool_context()
        .await
        .context("opening librarian tool context")
}

/// Print `result` and exit with the right code. JSON mode wraps errors so
/// hooks can parse them; pretty mode writes to stderr.
pub fn exit_with(result: Result<()>, opts: &format::OutputOpts) -> ! {
    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            if opts.json {
                let _ = serde_json::to_writer(
                    std::io::stdout(),
                    &serde_json::json!({"ok": false, "error": format!("{e:#}")}),
                );
                println!();
            } else {
                eprintln!("error: {e:#}");
            }
            std::process::exit(1);
        }
    }
}

/// Resolve a CLI string that may be `@<path>`, `-` (stdin), or a literal value.
pub fn read_at_or_stdin(value: &str) -> Result<String> {
    if value == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("reading stdin")?;
        return Ok(buf);
    }
    if let Some(path) = value.strip_prefix('@') {
        if path.is_empty() {
            return Err(anyhow!("`@` must be followed by a file path"));
        }
        return std::fs::read_to_string(path)
            .with_context(|| format!("reading {path}"));
    }
    Ok(value.to_string())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout --lib cli::tests`

Expected: PASS (3 tests).

- [ ] **Step 5: Clippy + fmt**

Run:
```
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
```

Both clean.

- [ ] **Step 6: Commit**

```bash
git add src/cli/mod.rs
git commit -m "feat(cli): bootstrap helpers — CommonOpts, open_ctx, read_at_or_stdin

Reuses librarian_mcp::build_tool_context() so CLI and MCP server share
project / catalog / workspace resolution. read_at_or_stdin supports
@<file>, -, or literal values, used by write verbs in later tasks.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 3: `cli/format.rs` — print() with JSON mode + shape inference scaffold

**Files:**
- Modify: `src/cli/format.rs` (replace the placeholder)
- Modify: `src/cli/format.rs` `tests` module (new)

- [ ] **Step 1: Write the failing tests**

Append to `src/cli/format.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_mode_emits_pretty_json() {
        let v = json!({"items": [{"id": "abc", "title": "t"}]});
        let mut buf = Vec::new();
        write_value(&v, &OutputOpts { json: true, no_color: true }, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("\"items\""));
        assert!(s.ends_with('\n'));
    }

    #[test]
    fn unknown_shape_falls_back_to_json() {
        // A bare string the shape inferrer doesn't recognise.
        let v = json!("ok");
        let mut buf = Vec::new();
        write_value(&v, &OutputOpts { json: false, no_color: true }, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        // Either "ok" recognised as a WriteAck or fallback JSON — both must include "ok".
        assert!(s.contains("ok"), "expected 'ok' in output; got: {s}");
    }

    #[test]
    fn infer_shape_recognises_find_result() {
        let v = json!({"items": [{"id":"a"}], "total": 1});
        assert!(matches!(infer_shape(&v), Shape::FindResult));
    }

    #[test]
    fn infer_shape_unknown_for_arbitrary_object() {
        let v = json!({"weird": "shape"});
        assert!(matches!(infer_shape(&v), Shape::Unknown));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout --lib cli::format::tests`

Expected: FAIL — `write_value`, `infer_shape`, and `Shape` do not exist.

- [ ] **Step 3: Replace the placeholder with the real formatter**

Replace the pre-test content in `src/cli/format.rs` with:

```rust
//! Output formatter for the CLI. Pretty by default, JSON via `--json`.

use anyhow::Result;
use serde_json::Value;
use std::io::{IsTerminal, Write};

#[derive(Debug, Clone, Copy, Default)]
pub struct OutputOpts {
    pub json: bool,
    pub no_color: bool,
}

/// Resolve `--no-color` based on stdout capability when not explicitly set.
fn effective_no_color(opts: &OutputOpts) -> bool {
    opts.no_color || !std::io::stdout().is_terminal()
}

pub(crate) enum Shape {
    FindResult,
    GetResult,
    GraphResult,
    StateAtResult,
    EventList,
    StaleList,
    WriteAck,
    Unknown,
}

pub(crate) fn infer_shape(v: &Value) -> Shape {
    if v.is_string() && v.as_str() == Some("ok") {
        return Shape::WriteAck;
    }
    if let Some(obj) = v.as_object() {
        if obj.contains_key("items") && obj.contains_key("total") {
            // could be FindResult or EventList — disambiguate on shape of items
            if let Some(first) = obj.get("items").and_then(|i| i.as_array()).and_then(|a| a.first()) {
                if first.get("kind").is_some() && first.get("artifact_id").is_some() {
                    return Shape::EventList;
                }
            }
            return Shape::FindResult;
        }
        if obj.contains_key("nodes") && obj.contains_key("edges") {
            return Shape::GraphResult;
        }
        if obj.contains_key("artifact") && obj.contains_key("status_at") {
            return Shape::StateAtResult;
        }
        if obj.contains_key("stale") && obj.contains_key("threshold_hours") {
            return Shape::StaleList;
        }
        if obj.contains_key("id") && obj.contains_key("body") {
            return Shape::GetResult;
        }
    }
    Shape::Unknown
}

/// Print to stdout — main entrypoint used by every verb after a tool call.
pub fn print(value: &Value, opts: &OutputOpts) -> Result<()> {
    let stdout = std::io::stdout();
    let mut h = stdout.lock();
    write_value(value, opts, &mut h)
}

pub(crate) fn write_value<W: Write>(value: &Value, opts: &OutputOpts, w: &mut W) -> Result<()> {
    let no_color = effective_no_color(opts);
    if opts.json {
        serde_json::to_writer_pretty(&mut *w, value)?;
        writeln!(w)?;
        return Ok(());
    }
    match infer_shape(value) {
        Shape::WriteAck => write_ack(value, no_color, w),
        // All pretty branches land in Phase 1 (Task 5) or later. Until then,
        // fall back to JSON for everything other than WriteAck so the user
        // always sees something useful.
        _ => fallback_json(value, w),
    }
}

fn write_ack<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    // "ok" string → "ok". Object with {"ok":true, "id":...} → "ok: created <id>".
    if let Some(obj) = value.as_object() {
        if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
            writeln!(w, "ok: {id}")?;
            return Ok(());
        }
    }
    writeln!(w, "ok")?;
    Ok(())
}

fn fallback_json<W: Write>(value: &Value, w: &mut W) -> Result<()> {
    serde_json::to_writer_pretty(&mut *w, value)?;
    writeln!(w)?;
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout --lib cli::format::tests`

Expected: PASS (4 tests).

- [ ] **Step 5: Clippy + fmt**

```
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
```

Both clean.

- [ ] **Step 6: Commit**

```bash
git add src/cli/format.rs
git commit -m "feat(cli): format::print with JSON mode + shape inference scaffold

JSON mode short-circuits to pretty JSON. Pretty mode delegates to
shape-specific branches; until those land in Task 5, non-Ack values
fall back to JSON so output is never silent.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 4: `artifact find` verb + `Commands::Artifact` wiring + integration smoke

**Files:**
- Create: `src/cli/artifact.rs`
- Modify: `src/cli/mod.rs` (`pub mod artifact;`)
- Modify: `src/main.rs` (new `Commands::Artifact` variant + match arm)
- Create: `tests/cli_artifact.rs` (integration test)

- [ ] **Step 1: Author the verb file**

Use `mcp__codescout__create_file` for `src/cli/artifact.rs`:

```rust
//! `codescout artifact <verb>` — find/get/graph/state-at/create/update/move/link.

use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand};
use serde_json::{json, Value};

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Subcommand)]
pub enum Verb {
    /// Find artifacts by filter / tag / kind / semantic query.
    Find(FindArgs),
}

#[derive(Debug, Args)]
pub struct FindArgs {
    /// kind=eq filter, e.g. "tracker"
    #[arg(long)]
    pub kind: Option<String>,
    /// repeatable; each → {"tags":{"contains":<tag>}}
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    /// status=eq filter; disables archived-hide default
    #[arg(long)]
    pub status: Option<String>,
    /// owner=eq filter (owners contains <owner>)
    #[arg(long)]
    pub owner: Option<String>,
    /// topic LIKE %<value>%
    #[arg(long = "has-topic")]
    pub has_topic: Option<String>,
    /// Raw FilterNode JSON; AND-merged with shortcuts.
    #[arg(long)]
    pub filter: Option<String>,
    /// Natural-language semantic search. Requires LIBRARIAN_EMBED_MODEL env.
    #[arg(long)]
    pub semantic: Option<String>,
    /// project|repo|umbrella|all
    #[arg(long, default_value = "project")]
    pub scope: String,
    /// Include archived/superseded by default.
    #[arg(long = "include-archived")]
    pub include_archived: bool,
    /// Filter to augmented (true) or non-augmented (false) artifacts.
    #[arg(long)]
    pub augmented: Option<bool>,
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
    /// Optional project root override (defaults to cwd).
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    /// Emit JSON to stdout.
    #[arg(long)]
    pub json: bool,
    /// Force no color (also implicit when stdout is not a TTY).
    #[arg(long = "no-color")]
    pub no_color: bool,
}

impl FindArgs {
    pub fn common(&self) -> CommonOpts {
        CommonOpts {
            project: self.project.clone(),
            json: self.json,
            no_color: self.no_color,
        }
    }
}

/// Compile shortcuts + raw --filter into a single FilterNode `Value`.
pub(crate) fn compile_filter(args: &FindArgs) -> Result<Option<Value>> {
    let mut leaves: Vec<Value> = Vec::new();
    if let Some(k) = &args.kind {
        leaves.push(json!({"kind": {"eq": k}}));
    }
    if let Some(s) = &args.status {
        leaves.push(json!({"status": {"eq": s}}));
    }
    if let Some(o) = &args.owner {
        leaves.push(json!({"owners": {"contains": o}}));
    }
    if let Some(t) = &args.has_topic {
        leaves.push(json!({"topic": {"contains": t}}));
    }
    for tag in &args.tag {
        leaves.push(json!({"tags": {"contains": tag}}));
    }
    if let Some(raw) = &args.filter {
        let parsed: Value =
            serde_json::from_str(raw).with_context(|| format!("--filter is not valid JSON: {raw}"))?;
        leaves.push(parsed);
    }
    Ok(match leaves.len() {
        0 => None,
        1 => Some(leaves.pop().unwrap()),
        _ => Some(json!({"and": leaves})),
    })
}

pub async fn dispatch(verb: Verb) -> Result<()> {
    match verb {
        Verb::Find(args) => run_find(args).await,
    }
}

pub(crate) async fn run_find(args: FindArgs) -> Result<()> {
    let common = args.common();
    let output = common.output();

    if args.semantic.is_some() && std::env::var("LIBRARIAN_EMBED_MODEL").is_err() {
        return Err(anyhow!(
            "--semantic requires the embedding service. Set LIBRARIAN_EMBED_MODEL \
             (and optionally LIBRARIAN_EMBED_URL, LIBRARIAN_EMBED_API_KEY) and re-run."
        ));
    }

    let ctx = open_ctx(&common).await?;

    let mut tool_args = serde_json::Map::new();
    if let Some(f) = compile_filter(&args)? {
        tool_args.insert("filter".into(), f);
    }
    if let Some(s) = &args.semantic {
        tool_args.insert("semantic".into(), Value::String(s.clone()));
    }
    tool_args.insert("scope".into(), Value::String(args.scope.clone()));
    tool_args.insert("include_archived".into(), Value::Bool(args.include_archived));
    if let Some(a) = args.augmented {
        tool_args.insert("augmented".into(), Value::Bool(a));
    }
    tool_args.insert("limit".into(), Value::Number(args.limit.into()));
    tool_args.insert("offset".into(), Value::Number(args.offset.into()));

    let v = librarian_mcp::tools::find::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    let _ = output;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_with_tag(tags: &[&str]) -> FindArgs {
        FindArgs {
            kind: None,
            tag: tags.iter().map(|s| s.to_string()).collect(),
            status: None,
            owner: None,
            has_topic: None,
            filter: None,
            semantic: None,
            scope: "project".into(),
            include_archived: false,
            augmented: None,
            limit: 50,
            offset: 0,
            project: None,
            json: false,
            no_color: false,
        }
    }

    #[test]
    fn compile_filter_single_tag_yields_leaf() {
        let a = args_with_tag(&["goal"]);
        let f = compile_filter(&a).unwrap().unwrap();
        assert_eq!(f, json!({"tags": {"contains": "goal"}}));
    }

    #[test]
    fn compile_filter_two_tags_and_joined() {
        let a = args_with_tag(&["goal", "p1"]);
        let f = compile_filter(&a).unwrap().unwrap();
        assert_eq!(
            f,
            json!({"and": [
                {"tags": {"contains": "goal"}},
                {"tags": {"contains": "p1"}}
            ]})
        );
    }

    #[test]
    fn compile_filter_kind_status_tag_combined() {
        let mut a = args_with_tag(&["goal"]);
        a.kind = Some("tracker".into());
        a.status = Some("active".into());
        let f = compile_filter(&a).unwrap().unwrap();
        assert_eq!(
            f,
            json!({"and": [
                {"kind": {"eq": "tracker"}},
                {"status": {"eq": "active"}},
                {"tags": {"contains": "goal"}}
            ]})
        );
    }

    #[test]
    fn compile_filter_raw_filter_parses_and_joins() {
        let mut a = args_with_tag(&[]);
        a.filter = Some(r#"{"kind":{"eq":"spec"}}"#.into());
        let f = compile_filter(&a).unwrap().unwrap();
        assert_eq!(f, json!({"kind": {"eq": "spec"}}));
    }

    #[test]
    fn compile_filter_raw_filter_bad_json_errors() {
        let mut a = args_with_tag(&[]);
        a.filter = Some("{not json".into());
        let err = compile_filter(&a).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("--filter is not valid JSON"));
    }

    #[test]
    fn compile_filter_none_when_no_shortcuts_or_filter() {
        let a = args_with_tag(&[]);
        assert!(compile_filter(&a).unwrap().is_none());
    }
}
```

- [ ] **Step 2: Expose the artifact submodule**

Use `mcp__codescout__edit_file` to find this exact text in `src/cli/mod.rs`:

```rust
pub mod format;
```

Replace with:

```rust
pub mod artifact;
pub mod format;
```

- [ ] **Step 3: Run unit tests**

Run: `cargo test -p codescout --lib cli::artifact::tests`

Expected: PASS (6 tests).

- [ ] **Step 4: Wire `Commands::Artifact` into `main.rs`**

Read `src/main.rs` first to find the `Commands` enum and the match in `main()`:

```
mcp__codescout__symbols(path="src/main.rs", include_body=true)
```

Append a new variant to the `Commands` enum (use `mcp__codescout__edit_code` `action="replace"` on `symbol="Commands"`, preserving every existing variant and adding the new one **last**). Add:

```rust
    /// Read and mutate artifacts (find, get, graph, state-at, create, …).
    Artifact {
        #[command(subcommand)]
        verb: codescout::cli::artifact::Verb,
    },
```

Then add the corresponding match arm in `main()`. Use `mcp__codescout__edit_file` to find this exact text:

```rust
        Commands::Mux {
```

…and insert the new arm before it (the order doesn't actually matter; the goal is correctness, not placement):

```rust
        Commands::Artifact { verb } => {
            codescout::cli::artifact::dispatch(verb).await?;
        }
```

- [ ] **Step 5: Verify it compiles + `--help` lists the new subcommand**

Run: `cargo build`

Expected: clean.

Run: `cargo run -- artifact --help`

Expected: clap prints `Usage: codescout artifact <COMMAND>` with `find` listed.

Run: `cargo run -- artifact find --help`

Expected: every flag from `FindArgs` documented.

- [ ] **Step 6: Author the integration test**

Create `tests/cli_artifact.rs`:

```rust
//! Integration smoke tests for `codescout artifact*` CLI verbs.
//!
//! Each test isolates state via tempdir + env overrides so they can run in
//! parallel without stepping on each other.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn run_cmd(tmp: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("codescout").unwrap();
    let db = tmp.path().join("cat.db");
    cmd.env("LIBRARIAN_DB", &db);
    // Workspace config path defaulting to data-local-dir is fine when unset;
    // build_tool_context() falls back to a default config gracefully.
    cmd.env_remove("LIBRARIAN_EMBED_MODEL");
    cmd
}

#[test]
fn artifact_find_on_empty_catalog_returns_empty_items_json() {
    let tmp = TempDir::new().unwrap();
    let assert = run_cmd(&tmp)
        .args(["artifact", "find", "--json"])
        .assert()
        .success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    // The exact key set depends on the tool; require at least an "items" array.
    assert!(out.contains("\"items\""), "expected items field; got: {out}");
}

#[test]
fn artifact_find_bad_filter_reports_error() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["artifact", "find", "--filter", "{not-json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--filter is not valid JSON"));
}

#[test]
fn artifact_find_semantic_without_embedder_reports_hint() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["artifact", "find", "--semantic", "anything"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("LIBRARIAN_EMBED_MODEL"));
}
```

- [ ] **Step 7: Run the integration smoke**

Run: `cargo test --test cli_artifact`

Expected: 3 tests pass. If `artifact_find_on_empty_catalog_returns_empty_items_json` fails because `find` returns a different envelope shape on an empty DB, inspect with `cargo run -- artifact find --json` and adjust the assertion to match the real shape (e.g. it may return `{"items":[],"total":0,...}` or a related envelope). Update the test to assert on whatever the live tool emits — the goal is "smoke passes, output looks well-formed".

- [ ] **Step 8: Workspace verification**

```
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib
cargo test --test cli_artifact
```

All clean. (Pre-existing failures in `tests/integration.rs::workflow_read_search_replace` and the parallel-execution flake in `codescout-embed::remote::tests` are not introduced by this task; ignore.)

- [ ] **Step 9: Commit**

```bash
git add src/cli/artifact.rs src/cli/mod.rs src/main.rs tests/cli_artifact.rs
git commit -m "feat(cli): codescout artifact find — shortcuts + --filter + --semantic

Compiles --kind/--tag/--status/--owner/--has-topic into a FilterNode AST,
AND-merging with raw --filter when both are given. --semantic fails fast
with a clear hint when LIBRARIAN_EMBED_MODEL is unset.

Integration smoke runs the built binary against a tempdir catalog and
asserts the JSON envelope shape on an empty DB.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 5: Pretty FindResult table branch

**Files:**
- Modify: `src/cli/format.rs` (replace the `Shape::FindResult` arm in `write_value`)
- Modify: `src/cli/format.rs` `tests` module (add a pretty-table test)

- [ ] **Step 1: Write the failing test**

Append to the `tests` module in `src/cli/format.rs`:

```rust
    #[test]
    fn pretty_find_result_renders_table_with_id_kind_status_title() {
        let v = json!({
            "items": [
                {"id":"abcd1234","kind":"tracker","status":"active","title":"Ship Feature X","rel_path":"docs/trackers/x.md"},
                {"id":"bbbb5678","kind":"spec","status":"draft","title":"Design Y","rel_path":"docs/specs/y.md"}
            ],
            "total": 2
        });
        let mut buf = Vec::new();
        write_value(&v, &OutputOpts { json: false, no_color: true }, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("abcd1234"), "row 1 id missing; got: {s}");
        assert!(s.contains("Ship Feature X"), "row 1 title missing; got: {s}");
        assert!(s.contains("docs/specs/y.md"), "row 2 rel_path missing; got: {s}");
        // The "kind | status" axis should be visible regardless of exact framing.
        assert!(s.contains("tracker"));
        assert!(s.contains("draft"));
        // The table must NOT be JSON — assert the column-header line is present
        // so a JSON fallback would visibly fail this test.
        assert!(
            s.lines().any(|line| line.contains("id") && line.contains("kind") && line.contains("status") && line.contains("title")),
            "expected a table header line; got: {s}"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout --lib cli::format::tests::pretty_find_result_renders_table_with_id_kind_status_title`

Expected: FAIL — the FindResult branch currently falls back to JSON. The new assertion on a header line (`id` `kind` `status` `title` all on one line, ASCII spaces between them) cannot be satisfied by `serde_json::to_writer_pretty` output, so the test must fail until Step 3 lands.

- [ ] **Step 3: Implement the table branch**

In `src/cli/format.rs`, replace the `match infer_shape(value)` block inside `write_value` to dispatch `Shape::FindResult` to a new `write_find_table` function. Use `mcp__codescout__edit_file` to find this exact text:

```rust
        Shape::WriteAck => write_ack(value, no_color, w),
        // All pretty branches land in Phase 1 (Task 5) or later. Until then,
        // fall back to JSON for everything other than WriteAck so the user
        // always sees something useful.
        _ => fallback_json(value, w),
```

Replace with:

```rust
        Shape::WriteAck => write_ack(value, no_color, w),
        Shape::FindResult => write_find_table(value, no_color, w),
        // Other pretty branches land in later tasks. Until then, fall back
        // to JSON for everything else so output is never silent.
        _ => fallback_json(value, w),
```

Then append `write_find_table` to the module (before the `#[cfg(test)]` block):

```rust
fn write_find_table<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    let items = value.get("items").and_then(|v| v.as_array());
    let Some(items) = items else {
        return fallback_json(value, w);
    };
    if items.is_empty() {
        writeln!(w, "(no results)")?;
        return Ok(());
    }
    // Compute column widths from the data so each row aligns.
    let mut widths = [8usize, 7, 7, 40]; // id, kind, status, title (rel_path follows wrapped)
    for it in items {
        let id = it.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let kind = it.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let status = it.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let title = it.get("title").and_then(|v| v.as_str()).unwrap_or("");
        widths[0] = widths[0].max(id.len());
        widths[1] = widths[1].max(kind.len());
        widths[2] = widths[2].max(status.len());
        widths[3] = widths[3].max(title.len()).min(60);
    }
    writeln!(
        w,
        "{:<w0$}  {:<w1$}  {:<w2$}  {:<w3$}  {}",
        "id", "kind", "status", "title", "rel_path",
        w0 = widths[0], w1 = widths[1], w2 = widths[2], w3 = widths[3]
    )?;
    for it in items {
        let id = it.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let kind = it.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let status = it.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let title = it.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let rel_path = it.get("rel_path").and_then(|v| v.as_str()).unwrap_or("");
        let title_trunc = if title.len() > widths[3] {
            format!("{}…", &title[..widths[3].saturating_sub(1)])
        } else {
            title.to_string()
        };
        writeln!(
            w,
            "{:<w0$}  {:<w1$}  {:<w2$}  {:<w3$}  {}",
            id, kind, status, title_trunc, rel_path,
            w0 = widths[0], w1 = widths[1], w2 = widths[2], w3 = widths[3]
        )?;
    }
    if let Some(total) = value.get("total").and_then(|v| v.as_u64()) {
        if (total as usize) > items.len() {
            writeln!(
                w,
                "\nShowing {} of {} — narrow with --kind / --tag / --filter, or paginate with --offset.",
                items.len(),
                total
            )?;
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p codescout --lib cli::format::tests`

Expected: all format tests pass including the new one.

- [ ] **Step 5: Verify CLI smoke still passes**

Run: `cargo test --test cli_artifact`

Expected: 3 tests still pass.

- [ ] **Step 6: Workspace verification + commit**

```
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
git add src/cli/format.rs
git commit -m "feat(cli): pretty table for artifact find results

Columns: id | kind | status | title | rel_path. Auto-widths from data.
Title truncates at 60 chars with ellipsis. Footer shows pagination hint
when total > items shown.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 6: Phase 1 verification + manual smoke

**Files:** none.

- [ ] **Step 1: Workspace-wide verification**

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib
cargo test --test cli_artifact
```

All clean. Pre-existing flakes ignored.

- [ ] **Step 2: Release build**

Run: `cargo build --release`

Expected: clean.

- [ ] **Step 3: Manual smoke against the live project (optional)**

In a real project with a seeded librarian catalog, run:

```
./target/release/codescout artifact find --kind tracker --tag goal --json
./target/release/codescout artifact find --kind tracker --tag goal
```

Expected: JSON envelope, then pretty table.

**Phase 1 ends here. `codescout artifact find` works. The Stop hook from the goal-tracker plan's Phase 3 is now implementable. To stop here, cherry-pick Phase 1 commits to master. To continue, proceed to Phase 2.**

---

## Phase 2 — Read verbs

Adds `get`, `graph`, `state-at`, `artifact-event list`, `artifact-refresh list-stale`. Each verb follows the same TDD shape as `find`: parse args → build tool args → call tool → pretty/JSON output.

### Task 7: `artifact get <id>` verb

**Files:**
- Modify: `src/cli/artifact.rs` (extend `Verb` enum, add `GetArgs`, `run_get`)
- Modify: `src/cli/format.rs` (`Shape::GetResult` branch in `write_value`)

- [ ] **Step 1: Add the variant + args struct**

Extend `Verb` and add `GetArgs` in `src/cli/artifact.rs`. Insert immediately after `FindArgs`'s impl:

```rust
#[derive(Debug, clap::Args)]
pub struct GetArgs {
    /// Artifact id.
    pub id: String,
    #[arg(long)]
    pub full: bool,
    #[arg(long)]
    pub heading: Option<String>,
    #[arg(long = "start-line")]
    pub start_line: Option<usize>,
    #[arg(long = "end-line")]
    pub end_line: Option<usize>,
    #[arg(long = "include-links")]
    pub include_links: bool,
    #[arg(long = "links-direction")]
    pub links_direction: Option<String>,
    #[arg(long = "links-rel")]
    pub links_rel: Option<String>,
    #[arg(long = "include-observations")]
    pub include_observations: bool,
    #[arg(long = "include-events")]
    pub include_events: bool,
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    #[arg(long)]
    pub json: bool,
    #[arg(long = "no-color")]
    pub no_color: bool,
}

impl GetArgs {
    pub fn common(&self) -> CommonOpts {
        CommonOpts {
            project: self.project.clone(),
            json: self.json,
            no_color: self.no_color,
        }
    }
}
```

Add a variant to `Verb`:

```rust
    /// Read one artifact by id.
    Get(GetArgs),
```

And extend `dispatch`:

```rust
        Verb::Get(args) => run_get(args).await,
```

Then add `run_get`:

```rust
pub(crate) async fn run_get(args: GetArgs) -> Result<()> {
    let common = args.common();
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let mut tool_args = serde_json::Map::new();
    tool_args.insert("id".into(), Value::String(args.id.clone()));
    tool_args.insert("full".into(), Value::Bool(args.full));
    if let Some(h) = &args.heading {
        tool_args.insert("heading".into(), Value::String(h.clone()));
    }
    if let Some(s) = args.start_line {
        tool_args.insert("start_line".into(), Value::Number(s.into()));
    }
    if let Some(e) = args.end_line {
        tool_args.insert("end_line".into(), Value::Number(e.into()));
    }
    if args.include_links {
        tool_args.insert("include_links".into(), Value::Bool(true));
    }
    if let Some(d) = &args.links_direction {
        tool_args.insert("links_direction".into(), Value::String(d.clone()));
    }
    if let Some(r) = &args.links_rel {
        tool_args.insert("links_rel".into(), Value::String(r.clone()));
    }
    if args.include_observations {
        tool_args.insert("include_observations".into(), Value::Bool(true));
    }
    if args.include_events {
        tool_args.insert("include_events".into(), Value::Bool(true));
    }

    let v = librarian_mcp::tools::get::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
```

- [ ] **Step 2: Write a unit test for arg → tool-args mapping**

Append to the `tests` module in `src/cli/artifact.rs`:

```rust
    #[test]
    fn get_args_common_carries_project_json_no_color() {
        let a = GetArgs {
            id: "abc".into(),
            full: false,
            heading: None,
            start_line: None,
            end_line: None,
            include_links: false,
            links_direction: None,
            links_rel: None,
            include_observations: false,
            include_events: false,
            project: Some(std::path::PathBuf::from("/tmp/proj")),
            json: true,
            no_color: true,
        };
        let c = a.common();
        assert_eq!(c.project, Some(std::path::PathBuf::from("/tmp/proj")));
        assert!(c.json);
        assert!(c.no_color);
    }
```

- [ ] **Step 3: Build + run unit tests**

```
cargo build
cargo test -p codescout --lib cli::artifact::tests
```

Expected: passes (existing + new).

- [ ] **Step 4: Implement `Shape::GetResult` pretty branch in `cli/format.rs`**

In `write_value`, change the `_ => fallback_json(...)` line to add a `Shape::GetResult` arm before it:

```rust
        Shape::GetResult => write_get_summary(value, no_color, w),
```

Add `write_get_summary` to the module:

```rust
fn write_get_summary<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("?");
    let title = value.get("title").and_then(|v| v.as_str()).unwrap_or("(untitled)");
    let kind = value.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
    let status = value.get("status").and_then(|v| v.as_str()).unwrap_or("?");
    writeln!(w, "{title}  [{kind}/{status}]  {id}")?;
    if let Some(path) = value.get("abs_path").and_then(|v| v.as_str()) {
        writeln!(w, "{path}")?;
    }
    writeln!(w)?;
    if let Some(body) = value.get("body").and_then(|v| v.as_str()) {
        writeln!(w, "{body}")?;
    } else {
        // No body field — print the whole JSON as a fallback so users still see the data.
        fallback_json(value, w)?;
    }
    Ok(())
}
```

- [ ] **Step 5: Run all format + artifact unit tests**

```
cargo test -p codescout --lib cli
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/cli/artifact.rs src/cli/format.rs
git commit -m "feat(cli): codescout artifact get <id> — body, headings, line slice, links

Mirrors the librarian artifact(action=get) surface. Pretty mode renders
a one-line header + abs_path + body; JSON mode emits the full envelope.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 8: `artifact graph <id>` verb + ASCII tree pretty

**Files:**
- Modify: `src/cli/artifact.rs` (`Verb::Graph`, `GraphArgs`, `run_graph`)
- Modify: `src/cli/format.rs` (`Shape::GraphResult` branch)

Implementation follows Task 7's pattern. Differences:

- `GraphArgs` fields: `id: String`, `depth: u8` (default 1), `rels: Option<String>` (CSV → `Vec<String>`), `include_events: bool`, plus the common `project/json/no_color`.
- `run_graph` builds tool args:
  ```json
  {
    "id": "<id>",
    "depth": <depth>,
    "rels": ["<r>","<r>"],
    "include_events": <bool>
  }
  ```
  …calls `librarian_mcp::tools::graph::call(&ctx, args).await?`, prints.
- Pretty branch: `write_graph_tree` walks the `nodes`+`edges` envelope and prints a depth-indented tree.

- [ ] **Step 1: Add `Verb::Graph(GraphArgs)` and `GraphArgs` struct** (mirrors Task 7 structure; copy and adapt)

```rust
#[derive(Debug, clap::Args)]
pub struct GraphArgs {
    pub id: String,
    #[arg(long, default_value_t = 1)]
    pub depth: u8,
    /// Comma-separated list of rel types to include (e.g. "supersedes,implements").
    #[arg(long)]
    pub rels: Option<String>,
    #[arg(long = "include-events")]
    pub include_events: bool,
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    #[arg(long)]
    pub json: bool,
    #[arg(long = "no-color")]
    pub no_color: bool,
}

impl GraphArgs {
    pub fn common(&self) -> CommonOpts {
        CommonOpts {
            project: self.project.clone(),
            json: self.json,
            no_color: self.no_color,
        }
    }
}
```

Wire into `Verb` and `dispatch`:

```rust
    /// BFS neighbourhood around an artifact.
    Graph(GraphArgs),
```

```rust
        Verb::Graph(args) => run_graph(args).await,
```

- [ ] **Step 2: Implement `run_graph`**

```rust
pub(crate) async fn run_graph(args: GraphArgs) -> Result<()> {
    if !(1..=3).contains(&args.depth) {
        return Err(anyhow!("--depth must be in 1..=3 (got {})", args.depth));
    }
    let common = args.common();
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let mut tool_args = serde_json::Map::new();
    tool_args.insert("id".into(), Value::String(args.id.clone()));
    tool_args.insert("depth".into(), Value::Number(args.depth.into()));
    if let Some(r) = &args.rels {
        let list: Vec<Value> = r
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| Value::String(s.trim().to_string()))
            .collect();
        tool_args.insert("rels".into(), Value::Array(list));
    }
    if args.include_events {
        tool_args.insert("include_events".into(), Value::Bool(true));
    }
    let v = librarian_mcp::tools::graph::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
```

- [ ] **Step 3: Implement `Shape::GraphResult` branch** (ASCII tree)

In `cli/format.rs`, add `Shape::GraphResult => write_graph_tree(value, no_color, w),` to `write_value`, then:

```rust
fn write_graph_tree<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    let nodes = value
        .get("nodes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let edges = value
        .get("edges")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if nodes.is_empty() {
        writeln!(w, "(empty graph)")?;
        return Ok(());
    }
    let id_to_title: std::collections::HashMap<String, String> = nodes
        .iter()
        .filter_map(|n| {
            let id = n.get("id")?.as_str()?.to_string();
            let title = n.get("title").and_then(|v| v.as_str()).unwrap_or("(untitled)").to_string();
            Some((id, title))
        })
        .collect();

    let root = nodes
        .first()
        .and_then(|n| n.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    writeln!(w, "{} — {}", root, id_to_title.get(root).cloned().unwrap_or_default())?;
    for e in &edges {
        let src = e.get("src_id").and_then(|v| v.as_str()).unwrap_or("");
        let dst = e.get("dst_id").and_then(|v| v.as_str()).unwrap_or("");
        let rel = e.get("rel").and_then(|v| v.as_str()).unwrap_or("?");
        let other = if src == root { dst } else { src };
        let title = id_to_title.get(other).cloned().unwrap_or_default();
        let arrow = if src == root { "→" } else { "←" };
        writeln!(w, "  {arrow} [{rel}] {other} — {title}")?;
    }
    Ok(())
}
```

- [ ] **Step 4: Tests + commit**

```
cargo test -p codescout --lib cli
cargo fmt && cargo clippy --workspace --all-targets -- -D warnings
```

```bash
git add src/cli/artifact.rs src/cli/format.rs
git commit -m "feat(cli): codescout artifact graph <id> — BFS neighbourhood + ASCII tree

--depth bounded to 1..=3. Pretty mode renders root + one indented line
per edge (rel + other-side id + title); JSON mode passes the raw envelope.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 9: `artifact state-at <id>` verb

**Files:**
- Modify: `src/cli/artifact.rs` (`Verb::StateAt`, `StateAtArgs`, `run_state_at`)
- Modify: `src/cli/format.rs` (`Shape::StateAtResult` branch)

Same shape as Task 8. Differences:

- `StateAtArgs`: `id: String`, `commit: Option<String>`, `timestamp: Option<i64>`. Validate exactly one of the two is set.
- Tool call: `librarian_mcp::tools::state_at::call(&ctx, args).await?`.
- Pretty branch: print `{title}\n  status: {status_at}\n  at: {commit or timestamp}`.

- [ ] **Step 1: Add args + validation**

```rust
#[derive(Debug, clap::Args)]
pub struct StateAtArgs {
    pub id: String,
    /// Git commit hash to time-travel to. Mutually exclusive with --timestamp.
    #[arg(long, conflicts_with = "timestamp")]
    pub commit: Option<String>,
    /// Unix epoch ms to time-travel to. Mutually exclusive with --commit.
    #[arg(long, conflicts_with = "commit")]
    pub timestamp: Option<i64>,
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    #[arg(long)]
    pub json: bool,
    #[arg(long = "no-color")]
    pub no_color: bool,
}
```

- [ ] **Step 2: Implement `run_state_at`**

```rust
pub(crate) async fn run_state_at(args: StateAtArgs) -> Result<()> {
    if args.commit.is_none() && args.timestamp.is_none() {
        return Err(anyhow!(
            "state-at requires exactly one of --commit <sha> or --timestamp <ms>"
        ));
    }
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let mut tool_args = serde_json::Map::new();
    tool_args.insert("artifact_id".into(), Value::String(args.id.clone()));
    if let Some(c) = &args.commit {
        tool_args.insert("commit".into(), Value::String(c.clone()));
    }
    if let Some(t) = args.timestamp {
        tool_args.insert("timestamp".into(), Value::Number(t.into()));
    }
    let v = librarian_mcp::tools::state_at::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
```

Add `Verb::StateAt(StateAtArgs)` and dispatch arm.

- [ ] **Step 3: `write_state_summary` branch in format.rs**

```rust
fn write_state_summary<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    let artifact = value.get("artifact");
    let title = artifact
        .and_then(|a| a.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("(untitled)");
    let status_at = value.get("status_at").and_then(|v| v.as_str()).unwrap_or("?");
    writeln!(w, "{title}")?;
    writeln!(w, "  status at cutoff: {status_at}")?;
    if let Some(c) = value.get("at_commit").and_then(|v| v.as_str()) {
        writeln!(w, "  commit:           {c}")?;
    }
    if let Some(t) = value.get("at_timestamp").and_then(|v| v.as_i64()) {
        writeln!(w, "  timestamp_ms:     {t}")?;
    }
    Ok(())
}
```

- [ ] **Step 4: Tests + commit**

```bash
git add src/cli/artifact.rs src/cli/format.rs
git commit -m "feat(cli): codescout artifact state-at <id> — time-travel snapshot

Exactly one of --commit <sha> or --timestamp <ms> is required.
Pretty mode prints a 3-line summary; JSON mode the raw envelope.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 10: `artifact-event list` verb + ArtifactEvent Commands variant

**Files:**
- Create: `src/cli/artifact_event.rs`
- Modify: `src/cli/mod.rs` (`pub mod artifact_event;`)
- Modify: `src/main.rs` (`Commands::ArtifactEvent`)
- Modify: `src/cli/format.rs` (`Shape::EventList` branch)

- [ ] **Step 1: Create the module**

```rust
//! `codescout artifact-event <verb>` — create / list.

use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::Value;

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Subcommand)]
pub enum Verb {
    /// List events for an artifact, newest-first.
    List(ListArgs),
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long = "artifact-id")]
    pub artifact_id: String,
    /// Comma-separated event kinds (note, reviewed, status_change, …).
    #[arg(long)]
    pub kinds: Option<String>,
    #[arg(long)]
    pub since: Option<i64>,
    #[arg(long)]
    pub until: Option<i64>,
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    #[arg(long)]
    pub json: bool,
    #[arg(long = "no-color")]
    pub no_color: bool,
}

pub async fn dispatch(verb: Verb) -> Result<()> {
    match verb {
        Verb::List(args) => run_list(args).await,
    }
}

async fn run_list(args: ListArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let mut tool_args = serde_json::Map::new();
    tool_args.insert("action".into(), Value::String("list".into()));
    tool_args.insert("artifact_id".into(), Value::String(args.artifact_id.clone()));
    if let Some(k) = &args.kinds {
        let list: Vec<Value> = k.split(',').map(|s| Value::String(s.trim().into())).collect();
        tool_args.insert("kinds".into(), Value::Array(list));
    }
    if let Some(s) = args.since {
        tool_args.insert("since".into(), Value::Number(s.into()));
    }
    if let Some(u) = args.until {
        tool_args.insert("until".into(), Value::Number(u.into()));
    }
    tool_args.insert("limit".into(), Value::Number(args.limit.into()));
    let v = librarian_mcp::tools::artifact_event::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
```

- [ ] **Step 2: Expose + wire**

`src/cli/mod.rs`:

```rust
pub mod artifact_event;
```

`src/main.rs` — add the `Commands::ArtifactEvent` variant and match arm (mirror the Artifact variant).

- [ ] **Step 3: `write_event_list` branch in format.rs**

```rust
fn write_event_list<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    let Some(items) = value.get("items").and_then(|v| v.as_array()) else {
        return fallback_json(value, w);
    };
    if items.is_empty() {
        writeln!(w, "(no events)")?;
        return Ok(());
    }
    writeln!(w, "{:<24}  {:<14}  {}", "when_ms", "kind", "payload")?;
    for it in items {
        let when = it.get("when_ms").and_then(|v| v.as_i64()).unwrap_or(0);
        let kind = it.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
        let payload = it
            .get("payload")
            .map(|v| {
                let s = serde_json::to_string(v).unwrap_or_default();
                if s.len() > 80 { format!("{}…", &s[..79]) } else { s }
            })
            .unwrap_or_default();
        writeln!(w, "{:<24}  {:<14}  {}", when, kind, payload)?;
    }
    Ok(())
}
```

Wire it in `write_value`: `Shape::EventList => write_event_list(value, no_color, w),`.

- [ ] **Step 4: Tests + commit**

```bash
git add src/cli/artifact_event.rs src/cli/mod.rs src/main.rs src/cli/format.rs
git commit -m "feat(cli): codescout artifact-event list — chronology + pretty table

Mirrors librarian artifact_event(action=list). Filters by artifact id,
kinds (CSV), since/until ms range, limit. Pretty mode renders a 3-column
table; JSON mode passes the envelope through.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 11: `artifact-refresh list-stale` verb + ArtifactRefresh Commands variant

**Files:**
- Create: `src/cli/artifact_refresh.rs`
- Modify: `src/cli/mod.rs` (`pub mod artifact_refresh;`)
- Modify: `src/main.rs` (`Commands::ArtifactRefresh`)
- Modify: `src/cli/format.rs` (`Shape::StaleList` branch)

Same shape as Task 10. Differences:

- `Verb::ListStale(ListStaleArgs)`. clap subcommand name: `list-stale` (clap auto-renames from `ListStale`).
- Args: `threshold_hours: Option<i64>`, `scope: Option<String>`, `limit: Option<usize>`, plus common.
- Tool call: `librarian_mcp::tools::artifact_refresh::call(&ctx, args).await?` with `action: "list_stale"`.
- Pretty branch: print a 3-column table (`id`, `last_refreshed_ms`, `hours_stale`).

- [ ] **Step 1: Implement the module** (follow Task 10 verbatim, swapping tool name + args)

- [ ] **Step 2: `write_stale_list` in format.rs**

```rust
fn write_stale_list<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    let Some(items) = value.get("stale").and_then(|v| v.as_array()) else {
        return fallback_json(value, w);
    };
    if items.is_empty() {
        writeln!(w, "(no stale artifacts)")?;
        return Ok(());
    }
    writeln!(w, "{:<18}  {:<24}  {}", "id", "last_refreshed_ms", "title")?;
    for it in items {
        let id = it.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let lr = it.get("last_refreshed_ms").and_then(|v| v.as_i64()).unwrap_or(0);
        let title = it.get("title").and_then(|v| v.as_str()).unwrap_or("(untitled)");
        writeln!(w, "{id:<18}  {lr:<24}  {title}")?;
    }
    Ok(())
}
```

- [ ] **Step 3: Tests + commit**

```bash
git add src/cli/artifact_refresh.rs src/cli/mod.rs src/main.rs src/cli/format.rs
git commit -m "feat(cli): codescout artifact-refresh list-stale — staleness scan

Mirrors librarian artifact_refresh(action=list_stale). Pretty mode renders
a 3-column table; JSON mode passes the envelope through.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 12: Phase 2 verification + integration tests

**Files:**
- Modify: `tests/cli_artifact.rs` (add smokes for get/graph/state-at/event list/refresh list-stale)

- [ ] **Step 1: Author one integration smoke per read verb**

Append to `tests/cli_artifact.rs`. Each test seeds the catalog by spawning `codescout artifact create …` (we don't have that verb yet — defer the seeded-state tests to Phase 3 Task 19; for Phase 2, smoke only the "empty catalog gracefully handles the verb" path).

```rust
#[test]
fn artifact_get_missing_id_fails_cleanly() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["artifact", "get", "definitely-not-a-real-id", "--json"])
        .assert()
        .failure();
}

#[test]
fn artifact_graph_missing_id_fails_cleanly() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["artifact", "graph", "definitely-not-a-real-id", "--json"])
        .assert()
        .failure();
}

#[test]
fn artifact_state_at_requires_commit_or_timestamp() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["artifact", "state-at", "x", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--commit").or(predicate::str::contains("--timestamp")));
}

#[test]
fn artifact_event_list_empty_catalog_returns_empty_or_error() {
    let tmp = TempDir::new().unwrap();
    // Either an error (artifact not found) or success with empty items is acceptable.
    let _ = run_cmd(&tmp)
        .args(["artifact-event", "list", "--artifact-id", "x", "--json"])
        .assert();
}

#[test]
fn artifact_refresh_list_stale_empty_catalog_runs() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["artifact-refresh", "list-stale", "--json"])
        .assert()
        .success();
}
```

- [ ] **Step 2: Verification**

```
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib
cargo test --test cli_artifact
```

All clean.

- [ ] **Step 3: Commit**

```bash
git add tests/cli_artifact.rs
git commit -m "test(cli): integration smokes for read verbs on empty catalogs

Each smoke exercises the verb's clap layer + tool dispatch path without
relying on seeded artifacts. Seeded smokes (round-trip through create →
get) land in Phase 3 once the create verb exists.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

**Phase 2 ends here. All read verbs are usable. To stop here, cherry-pick. To continue with write verbs, proceed to Phase 3.**

---

## Phase 3 — Write verbs

Adds `create`, `update`, `move`, `link`, `artifact-event create`, `artifact-refresh gather`, `artifact-augment`. Each write verb returns `"ok"` per the project's no-echo-in-writes rule; pretty mode prints `ok` (or `ok: <id>` for create).

### Task 13: `artifact create` verb

**Files:**
- Modify: `src/cli/artifact.rs` (`Verb::Create`, `CreateArgs`, `run_create`)

- [ ] **Step 1: Add args + dispatch**

```rust
#[derive(Debug, clap::Args)]
pub struct CreateArgs {
    #[arg(long)]
    pub kind: String,
    #[arg(long)]
    pub title: String,
    #[arg(long = "rel-path")]
    pub rel_path: String,
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long)]
    pub status: Option<String>,
    /// Comma-separated owner list.
    #[arg(long)]
    pub owners: Option<String>,
    /// Comma-separated tag list.
    #[arg(long)]
    pub tags: Option<String>,
    #[arg(long)]
    pub topic: Option<String>,
    /// Body content: `@<file>` reads from file, `-` reads stdin, else literal.
    #[arg(long)]
    pub body: Option<String>,
    /// Persistent augmentation prompt (or `@<file>` / `-`).
    #[arg(long = "augment-prompt")]
    pub augment_prompt: Option<String>,
    /// Augmentation params JSON (or `@<file>` / `-`).
    #[arg(long = "augment-params")]
    pub augment_params: Option<String>,
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    #[arg(long)]
    pub json: bool,
    #[arg(long = "no-color")]
    pub no_color: bool,
}
```

Wire into `Verb`:

```rust
    /// Create a new artifact.
    Create(CreateArgs),
```

And dispatch:

```rust
        Verb::Create(args) => run_create(args).await,
```

- [ ] **Step 2: Implement `run_create`**

```rust
pub(crate) async fn run_create(args: CreateArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let mut tool_args = serde_json::Map::new();
    tool_args.insert("kind".into(), Value::String(args.kind.clone()));
    tool_args.insert("title".into(), Value::String(args.title.clone()));
    tool_args.insert("rel_path".into(), Value::String(args.rel_path.clone()));
    if let Some(r) = &args.repo {
        tool_args.insert("repo".into(), Value::String(r.clone()));
    }
    if let Some(s) = &args.status {
        tool_args.insert("status".into(), Value::String(s.clone()));
    }
    if let Some(o) = &args.owners {
        let list: Vec<Value> = o.split(',').map(|s| Value::String(s.trim().into())).collect();
        tool_args.insert("owners".into(), Value::Array(list));
    }
    if let Some(t) = &args.tags {
        let list: Vec<Value> = t.split(',').map(|s| Value::String(s.trim().into())).collect();
        tool_args.insert("tags".into(), Value::Array(list));
    }
    if let Some(t) = &args.topic {
        tool_args.insert("topic".into(), Value::String(t.clone()));
    }
    if let Some(b) = &args.body {
        tool_args.insert("body".into(), Value::String(crate::cli::read_at_or_stdin(b)?));
    }
    if args.augment_prompt.is_some() || args.augment_params.is_some() {
        let mut aug = serde_json::Map::new();
        if let Some(p) = &args.augment_prompt {
            aug.insert("prompt".into(), Value::String(crate::cli::read_at_or_stdin(p)?));
        }
        if let Some(params) = &args.augment_params {
            let raw = crate::cli::read_at_or_stdin(params)?;
            let parsed: Value = serde_json::from_str(&raw)
                .context("--augment-params is not valid JSON")?;
            aug.insert("params".into(), parsed);
        }
        tool_args.insert("augment".into(), Value::Object(aug));
    }

    let v = librarian_mcp::tools::create::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
```

- [ ] **Step 3: Build + tests + commit**

```
cargo fmt && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p codescout --lib cli
```

```bash
git add src/cli/artifact.rs
git commit -m "feat(cli): codescout artifact create — kind/title/rel_path + body/augment

Body and augment_prompt/augment_params accept @<file>, -, or literal.
Owners/tags accept CSV strings, compiled to JSON arrays. Returns 'ok'
on success per the project's no-echo-in-writes rule.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 14: `artifact update <id>` verb

**Files:**
- Modify: `src/cli/artifact.rs` (`Verb::Update`, `UpdateArgs`, `run_update`)

- [ ] **Step 1: Add args + dispatch**

```rust
#[derive(Debug, clap::Args)]
pub struct UpdateArgs {
    pub id: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long)]
    pub owners: Option<String>,
    #[arg(long)]
    pub tags: Option<String>,
    #[arg(long)]
    pub topic: Option<String>,
    #[arg(long)]
    pub body: Option<String>,
    #[arg(long = "patch-params")]
    pub patch_params: Option<String>,
    #[arg(long = "commit-refresh")]
    pub commit_refresh: bool,
    #[arg(long = "add-blocks")]
    pub add_blocks: Option<String>,
    #[arg(long = "add-blocked-by")]
    pub add_blocked_by: Option<String>,
    #[arg(long)]
    pub owner: Option<String>,
    #[arg(long = "active-form")]
    pub active_form: Option<String>,
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    #[arg(long)]
    pub json: bool,
    #[arg(long = "no-color")]
    pub no_color: bool,
}
```

- [ ] **Step 2: Implement `run_update`**

(Mirrors `run_create` — build `patch` map for fields that map to the patch payload; pass `commit_refresh` separately; `add_blocks`/`add_blocked_by` as CSV → array.)

```rust
pub(crate) async fn run_update(args: UpdateArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let mut tool_args = serde_json::Map::new();
    tool_args.insert("id".into(), Value::String(args.id.clone()));
    let mut patch = serde_json::Map::new();
    if let Some(t) = &args.title { patch.insert("title".into(), Value::String(t.clone())); }
    if let Some(s) = &args.status { patch.insert("status".into(), Value::String(s.clone())); }
    if let Some(o) = &args.owners {
        patch.insert("owners".into(), Value::Array(o.split(',').map(|s| Value::String(s.trim().into())).collect()));
    }
    if let Some(t) = &args.tags {
        patch.insert("tags".into(), Value::Array(t.split(',').map(|s| Value::String(s.trim().into())).collect()));
    }
    if let Some(t) = &args.topic { patch.insert("topic".into(), Value::String(t.clone())); }
    if let Some(b) = &args.body {
        patch.insert("body".into(), Value::String(crate::cli::read_at_or_stdin(b)?));
    }
    if let Some(pp) = &args.patch_params {
        let raw = crate::cli::read_at_or_stdin(pp)?;
        let parsed: Value = serde_json::from_str(&raw).context("--patch-params is not valid JSON")?;
        patch.insert("params".into(), parsed);
    }
    if !patch.is_empty() {
        tool_args.insert("patch".into(), Value::Object(patch));
    }
    if args.commit_refresh {
        tool_args.insert("commit_refresh".into(), Value::Bool(true));
    }
    if let Some(b) = &args.add_blocks {
        tool_args.insert(
            "addBlocks".into(),
            Value::Array(b.split(',').map(|s| Value::String(s.trim().into())).collect()),
        );
    }
    if let Some(b) = &args.add_blocked_by {
        tool_args.insert(
            "addBlockedBy".into(),
            Value::Array(b.split(',').map(|s| Value::String(s.trim().into())).collect()),
        );
    }
    if let Some(o) = &args.owner {
        tool_args.insert("owner".into(), Value::String(o.clone()));
    }
    if let Some(af) = &args.active_form {
        tool_args.insert("activeForm".into(), Value::String(af.clone()));
    }

    let v = librarian_mcp::tools::update::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
```

Wire into `Verb` (`Update(UpdateArgs)`) and `dispatch`.

- [ ] **Step 3: Build + tests + commit**

```bash
git add src/cli/artifact.rs
git commit -m "feat(cli): codescout artifact update <id> — patch fields + commit-refresh

All optional flags compile into the tool's 'patch' object; addBlocks /
addBlockedBy / owner / activeForm pass through as top-level args.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 15: `artifact move <id>` + `artifact link` verbs

**Files:**
- Modify: `src/cli/artifact.rs` (`Verb::Move`, `Verb::Link`, args, run functions)

Both verbs are small — single tool call apiece.

- [ ] **Step 1: Add `MoveArgs` and `run_move`**

```rust
#[derive(Debug, clap::Args)]
pub struct MoveArgs {
    pub id: String,
    #[arg(long = "new-rel-path")]
    pub new_rel_path: String,
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    #[arg(long)]
    pub json: bool,
    #[arg(long = "no-color")]
    pub no_color: bool,
}

pub(crate) async fn run_move(args: MoveArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let tool_args = serde_json::json!({
        "id": args.id,
        "new_rel_path": args.new_rel_path,
    });
    let v = librarian_mcp::tools::mv::call(&ctx, tool_args).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
```

- [ ] **Step 2: Add `LinkArgs` and `run_link`**

```rust
#[derive(Debug, clap::Args)]
pub struct LinkArgs {
    #[arg(long)]
    pub src: String,
    #[arg(long)]
    pub dst: String,
    #[arg(long)]
    pub rel: String,
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    #[arg(long)]
    pub json: bool,
    #[arg(long = "no-color")]
    pub no_color: bool,
}

pub(crate) async fn run_link(args: LinkArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let tool_args = serde_json::json!({
        "src_id": args.src,
        "dst_id": args.dst,
        "rel": args.rel,
    });
    let v = librarian_mcp::tools::link::call(&ctx, tool_args).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
```

Wire both into `Verb` and `dispatch`.

- [ ] **Step 3: Build + commit**

```bash
git add src/cli/artifact.rs
git commit -m "feat(cli): codescout artifact move / link — path rename + edge create

move: rename rel_path of an existing artifact (creates parent dirs).
link: add a typed edge between two artifact ids.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 16: `artifact-event create` + `artifact-refresh gather` verbs

**Files:**
- Modify: `src/cli/artifact_event.rs` (`Verb::Create`, `CreateArgs`, `run_create`)
- Modify: `src/cli/artifact_refresh.rs` (`Verb::Gather`, `GatherArgs`, `run_gather`)

Both are small. Pattern is identical to the previous write verbs.

- [ ] **Step 1: `artifact-event create`**

```rust
#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long = "artifact-id")]
    pub artifact_id: String,
    #[arg(long)]
    pub kind: String,
    /// Event payload: `@<file>`, `-`, or literal JSON string. Optional.
    #[arg(long)]
    pub payload: Option<String>,
    #[arg(long)]
    pub author: Option<String>,
    #[arg(long = "anchor-commit")]
    pub anchor_commit: Option<String>,
    #[arg(long = "head-commit")]
    pub head_commit: Option<String>,
    #[arg(long = "parent-event-id")]
    pub parent_event_id: Option<String>,
    #[arg(long = "resolves-intent-event-id")]
    pub resolves_intent_event_id: Option<String>,
    #[arg(long = "also-mutates")]
    pub also_mutates: Option<String>,
    #[arg(long = "source-uri")]
    pub source_uri: Option<String>,
    #[arg(long = "source-kind")]
    pub source_kind: Option<String>,
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    #[arg(long)]
    pub json: bool,
    #[arg(long = "no-color")]
    pub no_color: bool,
}

async fn run_create(args: CreateArgs) -> Result<()> {
    use anyhow::Context;
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let mut tool_args = serde_json::Map::new();
    tool_args.insert("action".into(), Value::String("create".into()));
    tool_args.insert("artifact_id".into(), Value::String(args.artifact_id.clone()));
    tool_args.insert("kind".into(), Value::String(args.kind.clone()));
    if let Some(p) = &args.payload {
        let raw = crate::cli::read_at_or_stdin(p)?;
        let parsed: Value = serde_json::from_str(&raw)
            .context("--payload is not valid JSON")?;
        tool_args.insert("payload".into(), parsed);
    }
    if let Some(a) = &args.author { tool_args.insert("author".into(), Value::String(a.clone())); }
    if let Some(c) = &args.anchor_commit { tool_args.insert("anchor_commit".into(), Value::String(c.clone())); }
    if let Some(c) = &args.head_commit { tool_args.insert("head_commit".into(), Value::String(c.clone())); }
    if let Some(p) = &args.parent_event_id { tool_args.insert("parent_event_id".into(), Value::String(p.clone())); }
    if let Some(p) = &args.resolves_intent_event_id { tool_args.insert("resolves_intent_event_id".into(), Value::String(p.clone())); }
    if let Some(m) = &args.also_mutates {
        tool_args.insert(
            "also_mutates".into(),
            Value::Array(m.split(',').map(|s| Value::String(s.trim().into())).collect()),
        );
    }
    if let (Some(uri), Some(kind)) = (&args.source_uri, &args.source_kind) {
        tool_args.insert("source".into(), serde_json::json!({
            "uri": uri,
            "kind": kind,
        }));
    } else if args.source_uri.is_some() ^ args.source_kind.is_some() {
        return Err(anyhow::anyhow!("--source-uri and --source-kind must be passed together"));
    }
    let v = librarian_mcp::tools::artifact_event::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
```

Extend `Verb`:

```rust
    /// Append an event to an artifact's log.
    Create(CreateArgs),
```

Extend `dispatch`:

```rust
        Verb::Create(args) => run_create(args).await,
```

- [ ] **Step 2: `artifact-refresh gather <id>`**

In `src/cli/artifact_refresh.rs` add:

```rust
#[derive(Debug, Args)]
pub struct GatherArgs {
    pub id: String,
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    #[arg(long)]
    pub json: bool,
    #[arg(long = "no-color")]
    pub no_color: bool,
}

async fn run_gather(args: GatherArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let tool_args = serde_json::json!({
        "action": "gather",
        "id": args.id,
    });
    let v = librarian_mcp::tools::artifact_refresh::call(&ctx, tool_args).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
```

Extend `Verb`:

```rust
    /// Gather augmentation context for an artifact (collect — does NOT write).
    Gather(GatherArgs),
```

Extend `dispatch`:

```rust
        Verb::Gather(args) => run_gather(args).await,
```

- [ ] **Step 3: Build + commit**

```bash
git add src/cli/artifact_event.rs src/cli/artifact_refresh.rs
git commit -m "feat(cli): artifact-event create + artifact-refresh gather

Event create: full surface incl. source {uri,kind} pair. Refresh
gather is read-only (collects context for the caller; does not write).

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 17: `artifact-augment <id>` verb

**Files:**
- Create: `src/cli/artifact_augment.rs`
- Modify: `src/cli/mod.rs` (`pub mod artifact_augment;`)
- Modify: `src/main.rs` (`Commands::ArtifactAugment`)

- [ ] **Step 1: Create the module**

```rust
//! `codescout artifact-augment <id>` — attach or merge augmentation params/prompt.

use anyhow::{Context, Result};
use clap::Args;
use serde_json::Value;

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Args)]
pub struct AugmentArgs {
    pub id: String,
    /// Persistent prompt (or `@<file>` / `-`). Required unless `--merge` is passed.
    #[arg(long)]
    pub prompt: Option<String>,
    #[arg(long = "prompt-file")]
    pub prompt_file: Option<std::path::PathBuf>,
    /// Params JSON (or `@<file>` / `-`).
    #[arg(long)]
    pub params: Option<String>,
    #[arg(long = "params-schema")]
    pub params_schema: Option<String>,
    #[arg(long = "render-template")]
    pub render_template: Option<String>,
    /// RFC 7396 merge-patch on params only. Requires prior augmentation.
    #[arg(long)]
    pub merge: bool,
    #[arg(long = "append-mode")]
    pub append_mode: bool,
    #[arg(long = "history-cap")]
    pub history_cap: Option<usize>,
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    #[arg(long)]
    pub json: bool,
    #[arg(long = "no-color")]
    pub no_color: bool,
}

pub async fn run(args: AugmentArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let prompt = match (args.prompt.as_ref(), args.prompt_file.as_ref()) {
        (Some(p), None) => Some(crate::cli::read_at_or_stdin(p)?),
        (None, Some(path)) => Some(
            std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?,
        ),
        (Some(_), Some(_)) => {
            return Err(anyhow::anyhow!("pass at most one of --prompt or --prompt-file"));
        }
        (None, None) => None,
    };

    if !args.merge && prompt.is_none() {
        return Err(anyhow::anyhow!(
            "--prompt (or --prompt-file) is required unless --merge is passed"
        ));
    }

    let mut tool_args = serde_json::Map::new();
    tool_args.insert("id".into(), Value::String(args.id.clone()));
    if let Some(p) = prompt {
        tool_args.insert("prompt".into(), Value::String(p));
    }
    if let Some(params) = &args.params {
        let raw = crate::cli::read_at_or_stdin(params)?;
        let parsed: Value = serde_json::from_str(&raw).context("--params is not valid JSON")?;
        tool_args.insert("params".into(), parsed);
    }
    if let Some(s) = &args.params_schema {
        let raw = crate::cli::read_at_or_stdin(s)?;
        let parsed: Value = serde_json::from_str(&raw).context("--params-schema is not valid JSON")?;
        tool_args.insert("params_schema".into(), parsed);
    }
    if let Some(t) = &args.render_template {
        tool_args.insert("render_template".into(), Value::String(crate::cli::read_at_or_stdin(t)?));
    }
    if args.merge {
        tool_args.insert("merge".into(), Value::Bool(true));
    }
    if args.append_mode {
        tool_args.insert("append_mode".into(), Value::Bool(true));
    }
    if let Some(cap) = args.history_cap {
        tool_args.insert("history_cap".into(), Value::Number(cap.into()));
    }

    let v = librarian_mcp::tools::augment::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
```

- [ ] **Step 2: Wire `Commands::ArtifactAugment`**

In `src/main.rs`, add:

```rust
    /// Attach or merge augmentation (prompt + params) on an artifact.
    ArtifactAugment(codescout::cli::artifact_augment::AugmentArgs),
```

…and match arm:

```rust
        Commands::ArtifactAugment(args) => {
            codescout::cli::artifact_augment::run(args).await?;
        }
```

In `src/cli/mod.rs`:

```rust
pub mod artifact_augment;
```

- [ ] **Step 3: Build + commit**

```bash
git add src/cli/artifact_augment.rs src/cli/mod.rs src/main.rs
git commit -m "feat(cli): codescout artifact-augment <id> — prompt/params/merge

--prompt and --prompt-file are mutually exclusive. --merge applies RFC
7396 merge-patch on params only; requires prior augmentation. --append-mode
+ --history-cap control the dated-section append behaviour.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 18: Integration smokes for write verbs (round-trip create → get → update)

**Files:**
- Modify: `tests/cli_artifact.rs` (add seeded round-trip tests)

- [ ] **Step 1: Add three round-trip smokes**

Append:

```rust
fn make_workspace(tmp: &TempDir) -> std::path::PathBuf {
    // Minimal workspace config; library bootstrap is robust to default.
    let ws_path = tmp.path().join("workspace.toml");
    std::fs::write(&ws_path, "").unwrap();
    ws_path
}

fn run_cmd_with_workspace(tmp: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("codescout").unwrap();
    cmd.env("LIBRARIAN_DB", tmp.path().join("cat.db"));
    cmd.env("LIBRARIAN_WORKSPACE", make_workspace(tmp));
    cmd.env_remove("LIBRARIAN_EMBED_MODEL");
    cmd
}

#[test]
fn artifact_create_then_get_round_trip() {
    let tmp = TempDir::new().unwrap();
    let work = tmp.path().join("project");
    std::fs::create_dir_all(work.join("docs")).unwrap();

    // create
    let create = run_cmd_with_workspace(&tmp)
        .current_dir(&work)
        .args([
            "artifact", "create",
            "--kind", "spec",
            "--title", "Test Spec",
            "--rel-path", "docs/test-spec.md",
            "--json",
        ])
        .assert()
        .success();
    let create_out = String::from_utf8(create.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&create_out).expect("create returns JSON");
    let id = parsed.get("id").and_then(|v| v.as_str()).expect("create returns id").to_string();

    // get
    run_cmd_with_workspace(&tmp)
        .current_dir(&work)
        .args(["artifact", "get", &id, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Spec"));
}

#[test]
fn artifact_update_status_archived_then_find_excludes() {
    let tmp = TempDir::new().unwrap();
    let work = tmp.path().join("project");
    std::fs::create_dir_all(work.join("docs")).unwrap();

    let create = run_cmd_with_workspace(&tmp)
        .current_dir(&work)
        .args([
            "artifact", "create",
            "--kind", "spec",
            "--title", "Soon Archived",
            "--rel-path", "docs/soon-archived.md",
            "--json",
        ])
        .assert()
        .success();
    let create_out = String::from_utf8(create.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&create_out).unwrap();
    let id = parsed.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    // update to archived
    run_cmd_with_workspace(&tmp)
        .current_dir(&work)
        .args(["artifact", "update", &id, "--status", "archived", "--json"])
        .assert()
        .success();

    // find without --include-archived must not list it
    let find = run_cmd_with_workspace(&tmp)
        .current_dir(&work)
        .args(["artifact", "find", "--kind", "spec", "--json"])
        .assert()
        .success();
    let find_out = String::from_utf8(find.get_output().stdout.clone()).unwrap();
    assert!(!find_out.contains(&id), "archived artifact should not appear in default find; got: {find_out}");
}

#[test]
fn artifact_link_then_graph_shows_edge() {
    let tmp = TempDir::new().unwrap();
    let work = tmp.path().join("project");
    std::fs::create_dir_all(work.join("docs")).unwrap();

    let a = serde_json::from_str::<serde_json::Value>(
        &String::from_utf8(
            run_cmd_with_workspace(&tmp)
                .current_dir(&work)
                .args([
                    "artifact","create","--kind","spec","--title","A",
                    "--rel-path","docs/a.md","--json",
                ])
                .assert().success().get_output().stdout.clone()
        ).unwrap()
    ).unwrap();
    let b = serde_json::from_str::<serde_json::Value>(
        &String::from_utf8(
            run_cmd_with_workspace(&tmp)
                .current_dir(&work)
                .args([
                    "artifact","create","--kind","spec","--title","B",
                    "--rel-path","docs/b.md","--json",
                ])
                .assert().success().get_output().stdout.clone()
        ).unwrap()
    ).unwrap();
    let a_id = a["id"].as_str().unwrap();
    let b_id = b["id"].as_str().unwrap();

    run_cmd_with_workspace(&tmp)
        .current_dir(&work)
        .args(["artifact","link","--src",a_id,"--dst",b_id,"--rel","implements","--json"])
        .assert().success();

    let graph = run_cmd_with_workspace(&tmp)
        .current_dir(&work)
        .args(["artifact","graph",a_id,"--depth","1","--json"])
        .assert().success();
    let graph_out = String::from_utf8(graph.get_output().stdout.clone()).unwrap();
    assert!(graph_out.contains(b_id), "graph should mention B's id; got: {graph_out}");
}
```

Important caveats:
- These tests assume `librarian_mcp::tools::create::call` returns a `{"id": "..."}` envelope or at least includes the new id in the response. If it instead returns `"ok"`, the test must first call `find` to discover the id by title. **Adjust the assertion based on what `create` actually returns** when you run Step 1 — do not modify the tool to fit the test.
- These tests may be slow because each `Command::cargo_bin` spawns the built binary. Allow 30s+ per test on cold builds; subsequent runs are faster.

- [ ] **Step 2: Run + adjust if needed**

Run: `cargo test --test cli_artifact`

If `artifact_create_then_get_round_trip` fails because `create` returns `"ok"` instead of `{"id":...}`, adjust the test to find the id via a follow-up `artifact find --kind spec --json`. Document the adjustment in the commit message.

- [ ] **Step 3: Workspace verification + commit**

```
cargo fmt && cargo clippy --workspace --all-targets -- -D warnings
```

```bash
git add tests/cli_artifact.rs
git commit -m "test(cli): integration smokes for write verbs (create→get→update, link→graph)

Each smoke spawns the built binary with isolated LIBRARIAN_DB and
LIBRARIAN_WORKSPACE env. The link→graph smoke catches a regression
in edge persistence; the update→find smoke catches archived-hide
default-flag bugs.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 19: Phase 3 verification

**Files:** none.

- [ ] **Step 1: Workspace-wide verification**

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib
cargo test --test cli_artifact
cargo build --release
```

All clean.

- [ ] **Step 2: Optional manual smoke**

In a real project with seeded artifacts, run:

```
./target/release/codescout artifact find --kind tracker --tag goal --json | jq
./target/release/codescout artifact get <real-id>
./target/release/codescout artifact graph <real-id> --depth 2
```

Each should produce output matching the corresponding MCP tool's response.

**Phase 3 ends here. The full artifact CLI surface is shippable. To stop here, cherry-pick. To continue with the prompt-surface + docs update, proceed to Phase 4.**

---

## Phase 4 — Docs + ship

### Task 20: Update `src/prompts/source.md` with a CLI mention

**Files:**
- Modify: `src/prompts/source.md` (append a short paragraph under `### Artifact & Tracker Routing`)

- [ ] **Step 1: Read the current section**

Run: `mcp__codescout__read_markdown(path="src/prompts/source.md", heading="### Artifact & Tracker Routing")`

Locate the end of the section (just before `## Output System`).

- [ ] **Step 2: Append a short CLI mention**

Use `mcp__codescout__edit_markdown(path="src/prompts/source.md", action="insert_before", heading="### Goal-trackers", content=…)`:

```markdown
### Artifact CLI

For shell scripts and hooks that need to read or mutate the catalog without speaking MCP, the codescout binary exposes the artifact surface as subcommands: `codescout artifact find/get/graph/state-at/create/update/move/link`, `codescout artifact-event create/list`, `codescout artifact-refresh gather/list-stale`, `codescout artifact-augment <id>`. Each subcommand defaults to pretty output and adds `--json` for machine consumers. Names mirror MCP tool names 1:1, so any MCP example translates trivially.

```

- [ ] **Step 3: Run the prompt-surface drift test**

```
cargo test --lib server::tests::prompt_surfaces_reference_only_real_tools
```

Expected: PASS. The paragraph uses no bare-backticked snake_case tokens (subcommand names are hyphenated CLI strings, not in backticks alone).

- [ ] **Step 4: Regenerate the prompt-surface snapshot**

```
UPDATE_PROMPT_SNAPSHOTS=1 cargo test --lib prompts::tests::prompt_surfaces_server_instructions_snapshot
```

Expected: snapshot updated and the test passes.

- [ ] **Step 5: Commit**

```bash
git add src/prompts/source.md tests/fixtures/prompt_surfaces/server_instructions.md
git commit -m "docs(prompts): document codescout artifact CLI in server_instructions

Teaches every connecting LLM that an artifact CLI surface exists.
Loaded fresh per MCP connect; no ONBOARDING_VERSION bump required.

Spec: docs/superpowers/specs/2026-05-16-artifact-cli-design.md"
```

### Task 21: Final verification + ship sequence

**Files:** none initially; possible cherry-pick to `master`.

- [ ] **Step 1: Run full project verification**

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib
cargo test --test cli_artifact
cargo build --release
```

All clean.

- [ ] **Step 2: Manual MCP smoke**

Restart MCP via `/mcp` in Claude Code; verify the new section appears in the injected instructions.

- [ ] **Step 3: Cherry-pick to master (when ready)**

Per CLAUDE.md § Standard Ship Sequence, when the user is ready to ship:

```bash
git log --oneline experiments...master | grep -E "(cli|artifact|prompts)" | head -25
# Identify the SHAs of Tasks 1-20's commits.

git checkout master
git cherry-pick <sha>... # in order
git push
git checkout experiments
git rebase master
```

- [ ] **Step 4: Cut a release if warranted**

Per CLAUDE.md § Release Cycle. A new CLI surface warrants a minor version bump (`0.12.1 → 0.13.0`). Decide whether to bundle with other pending changes or ship the CLI alone.

---

## Summary of files touched

**Created:**
- `src/cli/mod.rs`
- `src/cli/format.rs`
- `src/cli/artifact.rs`
- `src/cli/artifact_event.rs`
- `src/cli/artifact_refresh.rs`
- `src/cli/artifact_augment.rs`
- `tests/cli_artifact.rs`

**Modified:**
- `Cargo.toml` — `[dev-dependencies]` adds `assert_cmd`, `predicates`
- `src/lib.rs` — `pub mod cli;`
- `src/main.rs` — 4 new `Commands` variants + match arms
- `src/prompts/source.md` — new `### Artifact CLI` subsection
- `tests/fixtures/prompt_surfaces/server_instructions.md` — regenerated snapshot

**Untouched:** `librarian-mcp` source. The CLI consumes the public tool functions as-is — no production-code changes inside the librarian crate.
