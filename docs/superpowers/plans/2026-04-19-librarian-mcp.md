# librarian-mcp v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship librarian-mcp v1 — a stdio MCP server that indexes markdown artifacts across a multi-repo workspace, stores metadata + a link graph in SQLite, and exposes 11 tools for find/get/link/status/archive. Round-trips writes through file frontmatter.

**Architecture:** New sibling binary crate `librarian-mcp` inside the codescout Cargo workspace. Extracts a shared `codescout-embed` crate from codescout's existing `src/embed/` so both binaries can share embedding infrastructure. SQLite catalog at `~/.local/share/librarian/catalog.db`; workspace config at `~/.config/librarian/workspace.toml`. Tools-only MCP surface for GitHub Copilot cloud-agent compatibility.

**Tech Stack:** Rust 2021, rmcp 1.3 (stdio transport), rusqlite 0.39 (bundled), sqlite-vec 0.1, serde_yaml for frontmatter, globset for classification rules, ignore/walkdir for workspace walking, sha2 for content hashing, anyhow + thiserror for errors, tokio for async.

**Spec:** [2026-04-19-librarian-mcp-design.md](../specs/2026-04-19-librarian-mcp-design.md)

---

## Phase 0 — Cargo workspace conversion

Convert the codescout repo root from a single-crate project into a Cargo workspace so we can add sibling crates.

### Task 0.1: Verify starting state

**Files:**
- Read: `Cargo.toml`

- [ ] **Step 1: Confirm single-crate layout**

Run: `head -15 Cargo.toml`
Expected: `[package]` section with `name = "codescout"`. No `[workspace]` section yet.

- [ ] **Step 2: Baseline build passes**

Run: `cargo build && cargo test --lib`
Expected: Clean build, all existing tests pass. Record the passing test count for Phase 0.3.

### Task 0.2: Add `[workspace]` table

**Files:**
- Modify: `Cargo.toml` (top of file)

- [ ] **Step 1: Add the workspace table at the top of `Cargo.toml`**

Insert before the `[package]` section:

```toml
[workspace]
members = [".", "crates/codescout-embed", "crates/librarian-mcp"]
resolver = "2"

[workspace.package]
edition = "2021"
license = "Apache-2.0"
authors = ["Marius Ailinca"]

[workspace.dependencies]
anyhow = "1"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
rusqlite = { version = "0.39", features = ["bundled"] }
sqlite-vec = "0.1"
```

- [ ] **Step 2: Build still passes with just the workspace table**

Run: `cargo build`
Expected: Succeeds — members don't exist yet but Cargo tolerates this until build of a nonexistent member is attempted. If Cargo complains, reduce `members` to `["."]` temporarily and re-add the entries in Tasks 1.1 and 2.1.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: convert to cargo workspace"
```

### Task 0.3: Verify no regression

- [ ] **Step 1: Full test suite passes**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: Formatter clean, zero clippy warnings, test count matches Phase 0.1 baseline.

---

## Phase 1 — Extract `codescout-embed` crate

Pull the minimal embedding surface (Embedder trait, local/remote clients, markdown chunker) out of `src/embed/` into a new workspace crate. **Leaves drift/preflight/index logic in codescout** — those are consumers, not primitives.

### Task 1.1: Scaffold `codescout-embed`

**Files:**
- Create: `crates/codescout-embed/Cargo.toml`
- Create: `crates/codescout-embed/src/lib.rs`

- [ ] **Step 1: Create crate directory and Cargo.toml**

```bash
mkdir -p crates/codescout-embed/src
```

Write `crates/codescout-embed/Cargo.toml`:

```toml
[package]
name = "codescout-embed"
version = "0.1.0"
edition.workspace = true
license.workspace = true
authors.workspace = true
description = "Shared embedding primitives for codescout + librarian-mcp"

[dependencies]
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 2: Write a placeholder `lib.rs`**

Create `crates/codescout-embed/src/lib.rs`:

```rust
//! codescout-embed — shared embedding primitives.
//!
//! Extracted from codescout/src/embed/ to let librarian-mcp reuse the
//! Embedder implementation without pulling in codescout's full semantic
//! index machinery.

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_builds() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 3: Build the new crate**

Run: `cargo build -p codescout-embed && cargo test -p codescout-embed`
Expected: Both succeed with the smoke test passing.

- [ ] **Step 4: Commit**

```bash
git add crates/codescout-embed
git commit -m "chore(embed): scaffold codescout-embed crate"
```

### Task 1.2: Inventory the extraction surface

**Files:**
- Read: `src/embed/mod.rs`, `src/embed/local.rs`, `src/embed/remote.rs`, `src/embed/chunker.rs`, `src/embed/schema.rs`

- [ ] **Step 1: Identify the minimal extraction set**

Use codescout to list exports:

```
list_symbols("src/embed/mod.rs")
list_symbols("src/embed/local.rs")
list_symbols("src/embed/remote.rs")
list_symbols("src/embed/chunker.rs")
list_symbols("src/embed/schema.rs")
```

**Extract (target crate):** Embedder trait, `Embedding` type, `EmbedConfig`, `create_embedder_with_config`, local/remote client impls, generic `chunker.rs` helpers, any `schema.rs` types referenced by those.

**Keep in codescout:** `ast_chunker.rs` (code-specific), `drift.rs`, `preflight.rs`, `index.rs` (the vec0 semantic index engine).

Write the inventory into a temporary note (`docs/superpowers/plans/_extraction-notes.md`) listing every symbol, target crate, and any cross-references. This note is deleted at the end of Phase 1.

- [ ] **Step 2: Commit the inventory**

```bash
git add docs/superpowers/plans/_extraction-notes.md
git commit -m "chore(embed): document extraction surface"
```

### Task 1.3: Move `Embedding` + Embedder trait

**Files:**
- Create: `crates/codescout-embed/src/embedder.rs`
- Modify: `crates/codescout-embed/src/lib.rs`
- Modify: `src/embed/mod.rs`

- [ ] **Step 1: Copy the Embedder trait and `Embedding` type into `crates/codescout-embed/src/embedder.rs`**

Copy the complete trait definition plus the `Embedding` struct verbatim from `src/embed/mod.rs`. Do not alter signatures.

- [ ] **Step 2: Re-export from the crate root**

Replace `crates/codescout-embed/src/lib.rs` body with:

```rust
//! codescout-embed — shared embedding primitives.

mod embedder;

pub use embedder::{Embedder, Embedding};
```

- [ ] **Step 3: In codescout, re-export from the new crate instead of defining locally**

At the top of `src/embed/mod.rs`, replace the local definitions of `Embedder` and `Embedding` with:

```rust
pub use codescout_embed::{Embedder, Embedding};
```

Add `codescout-embed = { path = "crates/codescout-embed" }` to codescout's root `Cargo.toml` `[dependencies]` section.

- [ ] **Step 4: Build + test**

Run: `cargo build && cargo test --lib`
Expected: Clean build, all tests pass. Fix any import paths that broke (intra-module `use crate::embed::Embedder` stays valid because of the `pub use`).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/codescout-embed src/embed/mod.rs
git commit -m "refactor(embed): move Embedder trait to codescout-embed"
```

### Task 1.4: Move local + remote clients

**Files:**
- Create: `crates/codescout-embed/src/local.rs`
- Create: `crates/codescout-embed/src/remote.rs`
- Modify: `crates/codescout-embed/src/lib.rs`
- Modify: `src/embed/mod.rs`, `src/embed/local.rs`, `src/embed/remote.rs`

- [ ] **Step 1: Copy `local.rs` and `remote.rs` into the new crate**

`cp src/embed/local.rs crates/codescout-embed/src/local.rs` then fix imports:
- `use crate::embed::Embedding` → `use crate::Embedding`
- Any references to codescout-internal helpers get moved with them or temporarily stubbed via a new `use anyhow::Result` alias.

Same for `remote.rs`.

- [ ] **Step 2: Expose them via `lib.rs`**

```rust
mod embedder;
mod local;
mod remote;

pub use embedder::{Embedder, Embedding};
pub use local::{LocalEmbedder, LocalEmbedConfig};   // adjust to actual type names
pub use remote::{RemoteEmbedder, RemoteEmbedConfig};

pub fn create_embedder_with_config(cfg: EmbedConfig) -> anyhow::Result<Box<dyn Embedder>> {
    // body moved from src/embed/mod.rs
}
```

- [ ] **Step 3: Add any new dependencies the moved code needs to `crates/codescout-embed/Cargo.toml`**

Inspect compile errors. Candidates: `reqwest`, `tokenizers`, `candle-core`, `hf-hub`, etc. Pin versions matching codescout's root `Cargo.toml` using workspace deps where possible.

- [ ] **Step 4: Delete the moved code from `src/embed/local.rs` and `src/embed/remote.rs`** (or make each a thin re-export file: `pub use codescout_embed::local::*;`) — prefer thin re-export so callers in `src/embed/mod.rs` don't all need rewriting in this task.

- [ ] **Step 5: Build + test**

Run: `cargo build && cargo test --lib`
Expected: Clean build, all tests pass.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(embed): move local+remote embedders to codescout-embed"
```

### Task 1.5: Move generic `chunker.rs`

**Files:**
- Create: `crates/codescout-embed/src/chunker.rs`
- Modify: `crates/codescout-embed/src/lib.rs`
- Modify: `src/embed/chunker.rs`

- [ ] **Step 1: Inspect what's in `chunker.rs` vs `ast_chunker.rs`**

Use `list_symbols("src/embed/chunker.rs")` and `list_symbols("src/embed/ast_chunker.rs")`. Generic text-chunking helpers go to the extracted crate; tree-sitter / AST chunking stays in codescout.

- [ ] **Step 2: Copy generic chunker code to the new crate**

Move ONLY items that don't depend on tree-sitter / AST types.

- [ ] **Step 3: Add `chunk_markdown()` export**

If a markdown-specific chunker doesn't already exist, add a thin wrapper in `crates/codescout-embed/src/chunker.rs`:

```rust
pub fn chunk_markdown(text: &str, max_tokens: usize) -> Vec<String> {
    // Use the same generic token chunker as code, but split on
    // blank lines and `\n## ` boundaries first for locality.
    let mut chunks = Vec::new();
    for section in split_on_headings(text) {
        chunks.extend(split_by_tokens(&section, max_tokens));
    }
    chunks
}

fn split_on_headings(text: &str) -> Vec<String> { /* split at ^#{1,6} */ }
fn split_by_tokens(text: &str, max_tokens: usize) -> Vec<String> { /* reuse existing token chunker */ }
```

- [ ] **Step 4: Write a unit test for `chunk_markdown`**

In `crates/codescout-embed/src/chunker.rs` `#[cfg(test)]`:

```rust
#[test]
fn chunk_markdown_splits_on_headings() {
    let text = "intro\n\n## Section A\ntext a\n\n## Section B\ntext b\n";
    let chunks = chunk_markdown(text, 1000);
    assert!(chunks.len() >= 2, "expected at least 2 chunks, got {:?}", chunks);
    assert!(chunks.iter().any(|c| c.contains("Section A")));
    assert!(chunks.iter().any(|c| c.contains("Section B")));
}

#[test]
fn chunk_markdown_respects_token_budget() {
    let long = "a ".repeat(5000);
    let chunks = chunk_markdown(&long, 100);
    assert!(chunks.len() > 1, "long text should be split");
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p codescout-embed`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(embed): extract chunker + add chunk_markdown"
```

### Task 1.6: Update codescout callsites

**Files:**
- Modify: `src/embed/mod.rs`, `src/embed/index.rs`, `src/tools/semantic.rs`, `src/dashboard/api/index.rs`

- [ ] **Step 1: Replace `use crate::embed::{Embedder, Embedding, ...}` with `use codescout_embed::{...}` where applicable**

Use `grep -rn "use crate::embed::" src/` to find all callsites. Update intra-module imports only where the symbol is now in the extracted crate. Symbols still living in codescout (`ast_chunker`, `drift`, `preflight`, `index`) keep their old path.

- [ ] **Step 2: Build + full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: Clean build, ZERO clippy warnings, test count matches Phase 0.1 baseline. This is the proof that extraction was behavior-preserving.

- [ ] **Step 3: Delete `docs/superpowers/plans/_extraction-notes.md`**

```bash
rm docs/superpowers/plans/_extraction-notes.md
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor(embed): migrate codescout callsites to codescout-embed"
```

---

## Phase 2 — `librarian-mcp` crate skeleton

### Task 2.1: Scaffold the binary crate

**Files:**
- Create: `crates/librarian-mcp/Cargo.toml`
- Create: `crates/librarian-mcp/src/main.rs`
- Create: `crates/librarian-mcp/src/lib.rs`

- [ ] **Step 1: Create directory and Cargo.toml**

```bash
mkdir -p crates/librarian-mcp/src crates/librarian-mcp/tests
```

Write `crates/librarian-mcp/Cargo.toml`:

```toml
[package]
name = "librarian-mcp"
version = "0.1.0"
edition.workspace = true
license.workspace = true
authors.workspace = true

[[bin]]
name = "librarian-mcp"
path = "src/main.rs"

[dependencies]
codescout-embed = { path = "../codescout-embed" }
anyhow = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
serde_yaml = "0.9"
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
rusqlite = { workspace = true }
sqlite-vec = { workspace = true }
rmcp = { version = "1.3", features = ["server", "macros", "transport-io", "schemars"] }
schemars = "0.8"
globset = "0.4"
walkdir = "2"
ignore = "0.4"
sha2 = "0.10"
chrono = { version = "0.4", features = ["serde"] }
toml = "0.8"
dirs = "5"
clap = { version = "4", features = ["derive"] }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Stub `main.rs`**

```rust
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "librarian-mcp", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// One-shot import of workspace roots from codescout's project registry.
    ImportCodescout,
    /// Reindex the workspace without starting the MCP server.
    Reindex {
        #[arg(long)]
        repo: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        None => librarian_mcp::run_stdio_server().await,
        Some(Cmd::ImportCodescout) => librarian_mcp::import_codescout(),
        Some(Cmd::Reindex { repo }) => librarian_mcp::reindex_cli(repo.as_deref()),
    }
}
```

- [ ] **Step 3: Stub `lib.rs`**

```rust
//! librarian-mcp — workspace artifact registry, stdio MCP server.

use anyhow::Result;

pub async fn run_stdio_server() -> Result<()> {
    anyhow::bail!("not yet implemented")
}

pub fn import_codescout() -> Result<()> {
    anyhow::bail!("not yet implemented")
}

pub fn reindex_cli(_repo: Option<&str>) -> Result<()> {
    anyhow::bail!("not yet implemented")
}
```

- [ ] **Step 4: Build**

Run: `cargo build -p librarian-mcp`
Expected: Succeeds.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore(librarian): scaffold librarian-mcp crate"
```

---

## Phase 3 — Frontmatter module (TDD)

### Task 3.1: Parse YAML frontmatter from markdown

**Files:**
- Create: `crates/librarian-mcp/src/frontmatter.rs`
- Modify: `crates/librarian-mcp/src/lib.rs`

- [ ] **Step 1: Write failing tests first**

Create `crates/librarian-mcp/src/frontmatter.rs` with only the test module:

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Frontmatter {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub owners: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub time_scope: Option<String>,
}

/// Splits a markdown document into `(frontmatter, body)`. Returns `None` for
/// `frontmatter` when the document has no YAML block.
pub fn parse(_doc: &str) -> Result<(Option<Frontmatter>, &str)> {
    anyhow::bail!("not yet implemented")
}

/// Serializes frontmatter back to a YAML block + body.
pub fn write(_fm: &Frontmatter, _body: &str) -> String {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_frontmatter() {
        let doc = "---\nkind: spec\nstatus: active\ntitle: Example\n---\n\nBody here\n";
        let (fm, body) = parse(doc).unwrap();
        let fm = fm.expect("frontmatter present");
        assert_eq!(fm.kind.as_deref(), Some("spec"));
        assert_eq!(fm.status.as_deref(), Some("active"));
        assert_eq!(fm.title.as_deref(), Some("Example"));
        assert_eq!(body, "\nBody here\n");
    }

    #[test]
    fn returns_none_for_no_frontmatter() {
        let doc = "# just a heading\n\nbody\n";
        let (fm, body) = parse(doc).unwrap();
        assert!(fm.is_none());
        assert_eq!(body, doc);
    }

    #[test]
    fn handles_trailing_crlf() {
        let doc = "---\r\nkind: plan\r\n---\r\n\r\nbody\r\n";
        let (fm, _) = parse(doc).unwrap();
        assert_eq!(fm.unwrap().kind.as_deref(), Some("plan"));
    }

    #[test]
    fn rejects_missing_closing_delimiter() {
        let doc = "---\nkind: spec\n\nbody without close\n";
        let err = parse(doc).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("closing"));
    }

    #[test]
    fn rejects_malformed_yaml() {
        let doc = "---\nkind: [unclosed\n---\nbody\n";
        assert!(parse(doc).is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        let doc = "---\nkind: spec\nbogus: nope\n---\nbody\n";
        assert!(parse(doc).is_err());
    }

    #[test]
    fn round_trip_preserves_body() {
        let fm = Frontmatter {
            kind: Some("spec".into()),
            status: Some("active".into()),
            title: Some("X".into()),
            ..Default::default()
        };
        let body = "\nBody text\n";
        let doc = write(&fm, body);
        let (parsed, parsed_body) = parse(&doc).unwrap();
        assert_eq!(parsed.unwrap(), fm);
        assert_eq!(parsed_body, body);
    }
}
```

Add `pub mod frontmatter;` to `crates/librarian-mcp/src/lib.rs`.

- [ ] **Step 2: Run the tests — expect ALL to fail**

Run: `cargo test -p librarian-mcp frontmatter::tests`
Expected: 7 failures with "not yet implemented" / `unimplemented!`.

- [ ] **Step 3: Implement `parse` and `write`**

Replace the body of `parse`:

```rust
pub fn parse(doc: &str) -> Result<(Option<Frontmatter>, &str)> {
    // Normalize CRLF → LF for delimiter detection but slice the ORIGINAL doc.
    let looks_like_fm = doc.starts_with("---\n") || doc.starts_with("---\r\n");
    if !looks_like_fm {
        return Ok((None, doc));
    }
    let after_open = if doc.starts_with("---\r\n") { 5 } else { 4 };
    let rest = &doc[after_open..];
    // Find closing `---` on its own line.
    let mut idx = 0usize;
    let mut close = None;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            close = Some((idx, idx + line.len()));
            break;
        }
        idx += line.len();
    }
    let (yaml_end, body_start) = close
        .ok_or_else(|| anyhow::anyhow!("frontmatter missing closing `---`"))?;
    let yaml_src = &rest[..yaml_end];
    let fm: Frontmatter = serde_yaml::from_str(yaml_src)
        .map_err(|e| anyhow::anyhow!("malformed frontmatter YAML: {e}"))?;
    Ok((Some(fm), &rest[body_start..]))
}

pub fn write(fm: &Frontmatter, body: &str) -> String {
    let yaml = serde_yaml::to_string(fm).expect("frontmatter serializes");
    format!("---\n{yaml}---\n{body}")
}
```

- [ ] **Step 4: Run tests until all 7 pass**

Run: `cargo test -p librarian-mcp frontmatter::tests`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(librarian): frontmatter parser with TDD-driven edge cases"
```

### Task 3.2: Round-trip `update_in_place`

**Files:**
- Modify: `crates/librarian-mcp/src/frontmatter.rs`

- [ ] **Step 1: Add a failing test**

```rust
#[test]
fn update_in_place_preserves_untouched_fields() {
    let doc = "---\nkind: spec\nstatus: draft\ntitle: Original\n---\n\nbody\n";
    let updated = update_in_place(doc, |fm| {
        fm.status = Some("active".into());
    }).unwrap();
    assert!(updated.contains("status: active"));
    assert!(updated.contains("title: Original"));
    assert!(updated.ends_with("\nbody\n"));
}

#[test]
fn update_in_place_inserts_frontmatter_if_absent() {
    let doc = "# Heading\n\nbody\n";
    let updated = update_in_place(doc, |fm| {
        fm.kind = Some("doc".into());
    }).unwrap();
    assert!(updated.starts_with("---\n"));
    assert!(updated.contains("kind: doc"));
    assert!(updated.ends_with("# Heading\n\nbody\n"));
}
```

- [ ] **Step 2: Add the function stub**

```rust
pub fn update_in_place(doc: &str, edit: impl FnOnce(&mut Frontmatter)) -> Result<String> {
    let (fm_opt, body) = parse(doc)?;
    let mut fm = fm_opt.unwrap_or_default();
    edit(&mut fm);
    Ok(write(&fm, body))
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p librarian-mcp frontmatter::tests`
Expected: 9 passed.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(librarian): frontmatter update_in_place round-trip"
```

---

## Phase 4 — Classification (TDD)

### Task 4.1: Load rules from TOML

**Files:**
- Create: `crates/librarian-mcp/src/classify.rs`
- Modify: `crates/librarian-mcp/src/lib.rs`

- [ ] **Step 1: Write failing tests for rule parsing**

Create `crates/librarian-mcp/src/classify.rs`:

```rust
use anyhow::{Context, Result};
use globset::{Glob, GlobMatcher};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleFile {
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub glob: String,
    pub kind: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub time_scope: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub matcher: GlobMatcher,
    pub kind: String,
    pub status: Option<String>,
    pub time_scope: Option<String>,
}

pub fn load_rules(toml_str: &str) -> Result<Vec<CompiledRule>> {
    anyhow::bail!("not yet implemented")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    pub kind: String,
    pub status: Option<String>,
    pub time_scope: Option<String>,
}

pub fn classify(rules: &[CompiledRule], rel_path: &str) -> Option<Classification> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> &'static str {
        r#"
[[rule]]
glob = "**/docs/superpowers/specs/*.md"
kind = "spec"
status = "active"

[[rule]]
glob = "**/docs/research/*.md"
kind = "memory"
time_scope = "dated_snapshot"

[[rule]]
glob = "**/ROADMAP.md"
kind = "roadmap"
"#
    }

    #[test]
    fn load_rules_parses_multiple() {
        let rules = load_rules(sample()).unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].kind, "spec");
    }

    #[test]
    fn classify_matches_spec() {
        let rules = load_rules(sample()).unwrap();
        let c = classify(&rules, "docs/superpowers/specs/foo.md").unwrap();
        assert_eq!(c.kind, "spec");
        assert_eq!(c.status.as_deref(), Some("active"));
    }

    #[test]
    fn classify_matches_memory_with_time_scope() {
        let rules = load_rules(sample()).unwrap();
        let c = classify(&rules, "docs/research/2026-01-01-foo.md").unwrap();
        assert_eq!(c.kind, "memory");
        assert_eq!(c.time_scope.as_deref(), Some("dated_snapshot"));
    }

    #[test]
    fn classify_first_match_wins() {
        let toml = r#"
[[rule]]
glob = "**/docs/*.md"
kind = "doc"

[[rule]]
glob = "**/docs/superpowers/specs/*.md"
kind = "spec"
"#;
        let rules = load_rules(toml).unwrap();
        let c = classify(&rules, "docs/superpowers/specs/x.md").unwrap();
        assert_eq!(c.kind, "doc", "earlier rule must win");
    }

    #[test]
    fn classify_returns_none_for_unknown() {
        let rules = load_rules(sample()).unwrap();
        assert!(classify(&rules, "random/path.md").is_none());
    }

    #[test]
    fn load_rules_rejects_bad_glob() {
        let toml = "[[rule]]\nglob = \"[\"\nkind = \"spec\"\n";
        assert!(load_rules(toml).is_err());
    }
}
```

Add `pub mod classify;` to `lib.rs`.

- [ ] **Step 2: Run tests — expect failures**

Run: `cargo test -p librarian-mcp classify::tests`
Expected: 6 failing tests.

- [ ] **Step 3: Implement `load_rules` and `classify`**

```rust
pub fn load_rules(toml_str: &str) -> Result<Vec<CompiledRule>> {
    let file: RuleFile = toml::from_str(toml_str).context("parsing classification rules")?;
    file.rules
        .into_iter()
        .map(|r| {
            let matcher = Glob::new(&r.glob)
                .with_context(|| format!("invalid glob: {}", r.glob))?
                .compile_matcher();
            Ok(CompiledRule {
                matcher,
                kind: r.kind,
                status: r.status,
                time_scope: r.time_scope,
            })
        })
        .collect()
}

pub fn classify(rules: &[CompiledRule], rel_path: &str) -> Option<Classification> {
    for r in rules {
        if r.matcher.is_match(rel_path) {
            return Some(Classification {
                kind: r.kind.clone(),
                status: r.status.clone(),
                time_scope: r.time_scope.clone(),
            });
        }
    }
    None
}
```

- [ ] **Step 4: Tests pass**

Run: `cargo test -p librarian-mcp classify::tests`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(librarian): classification rules (TOML → globset matcher)"
```

---

## Phase 5 — Filter AST + SQL compiler (TDD)

### Task 5.1: Define `FilterNode` enum

**Files:**
- Create: `crates/librarian-mcp/src/filter.rs`
- Modify: `crates/librarian-mcp/src/lib.rs`

- [ ] **Step 1: Write the type + tests**

Create `crates/librarian-mcp/src/filter.rs`:

```rust
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Recursive filter AST ported from redis/agent-memory-server filters.py
/// (Apache-2.0). See CREDITS.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FilterNode {
    And { and: Vec<FilterNode> },
    Or { or: Vec<FilterNode> },
    Not { not: Box<FilterNode> },
    Leaf(serde_json::Map<String, Value>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafOp { Eq, Ne, In, Nin, Gt, Lt, Gte, Lte, Contains }

impl LeafOp {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "eq" => Self::Eq, "ne" => Self::Ne,
            "in" => Self::In, "nin" => Self::Nin,
            "gt" => Self::Gt, "lt" => Self::Lt,
            "gte" => Self::Gte, "lte" => Self::Lte,
            "contains" => Self::Contains,
            _ => return None,
        })
    }
    fn sql(self) -> &'static str {
        match self {
            Self::Eq => "=", Self::Ne => "!=",
            Self::In => "IN", Self::Nin => "NOT IN",
            Self::Gt => ">", Self::Lt => "<",
            Self::Gte => ">=", Self::Lte => "<=",
            Self::Contains => "LIKE",  // JSON-array contains: handled separately
        }
    }
}

/// SQL fragment + positional parameters (as `rusqlite::types::Value`).
pub struct SqlFragment {
    pub sql: String,
    pub params: Vec<rusqlite::types::Value>,
}

pub fn compile(node: &FilterNode) -> Result<SqlFragment> {
    anyhow::bail!("not yet implemented")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(v: Value) -> FilterNode {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn compiles_simple_eq() {
        let node = parse(json!({"kind": {"eq": "spec"}}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "kind = ?");
        assert_eq!(f.params.len(), 1);
    }

    #[test]
    fn compiles_in_list() {
        let node = parse(json!({"status": {"in": ["active", "blocked"]}}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "status IN (?, ?)");
        assert_eq!(f.params.len(), 2);
    }

    #[test]
    fn compiles_and_composition() {
        let node = parse(json!({"and": [
            {"kind": {"eq": "spec"}},
            {"status": {"eq": "active"}}
        ]}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "(kind = ? AND status = ?)");
        assert_eq!(f.params.len(), 2);
    }

    #[test]
    fn compiles_or() {
        let node = parse(json!({"or": [
            {"kind": {"eq": "spec"}},
            {"kind": {"eq": "plan"}}
        ]}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "(kind = ? OR kind = ?)");
    }

    #[test]
    fn compiles_not() {
        let node = parse(json!({"not": {"status": {"eq": "archived"}}}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "NOT (status = ?)");
    }

    #[test]
    fn compiles_tags_contains_via_json_each() {
        let node = parse(json!({"tags": {"contains": "embedding"}}));
        let f = compile(&node).unwrap();
        // tags is a JSON array column; `contains` must translate to an EXISTS on json_each
        assert!(f.sql.contains("json_each(tags)"));
        assert_eq!(f.params.len(), 1);
    }

    #[test]
    fn compiles_gt_integer() {
        let node = parse(json!({"updated_at": {"gt": 1700000000}}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "updated_at > ?");
    }

    #[test]
    fn rejects_unknown_op() {
        let node = parse(json!({"kind": {"bogus": "x"}}));
        assert!(compile(&node).is_err());
    }
}
```

Add `pub mod filter;` to `lib.rs`.

- [ ] **Step 2: Tests fail as expected**

Run: `cargo test -p librarian-mcp filter::tests`
Expected: 8 failures.

- [ ] **Step 3: Implement `compile`**

```rust
pub fn compile(node: &FilterNode) -> Result<SqlFragment> {
    match node {
        FilterNode::And { and } => compile_composition("AND", and),
        FilterNode::Or { or } => compile_composition("OR", or),
        FilterNode::Not { not } => {
            let inner = compile(not)?;
            Ok(SqlFragment {
                sql: format!("NOT ({})", inner.sql),
                params: inner.params,
            })
        }
        FilterNode::Leaf(map) => compile_leaf(map),
    }
}

fn compile_composition(op: &str, children: &[FilterNode]) -> Result<SqlFragment> {
    if children.is_empty() {
        bail!("empty composition `{op}`");
    }
    let mut parts = Vec::new();
    let mut params = Vec::new();
    for c in children {
        let f = compile(c)?;
        parts.push(f.sql);
        params.extend(f.params);
    }
    Ok(SqlFragment {
        sql: format!("({})", parts.join(&format!(" {op} "))),
        params,
    })
}

fn compile_leaf(map: &serde_json::Map<String, Value>) -> Result<SqlFragment> {
    if map.len() != 1 {
        bail!("leaf must have exactly one field, got {}", map.len());
    }
    let (field, ops) = map.iter().next().unwrap();
    let ops = ops.as_object().ok_or_else(|| anyhow::anyhow!("ops must be object"))?;
    if ops.len() != 1 {
        bail!("exactly one op per leaf, got {}", ops.len());
    }
    let (op_name, value) = ops.iter().next().unwrap();
    let op = LeafOp::parse(op_name).ok_or_else(|| anyhow::anyhow!("unknown op `{op_name}`"))?;

    // JSON-array columns use `json_each` scans for membership.
    let is_array_col = matches!(field.as_str(), "tags" | "owners");
    if op == LeafOp::Contains && is_array_col {
        let lit = json_value_to_sql(value)?;
        return Ok(SqlFragment {
            sql: format!("EXISTS (SELECT 1 FROM json_each({field}) WHERE value = ?)"),
            params: vec![lit],
        });
    }

    match op {
        LeafOp::In | LeafOp::Nin => {
            let arr = value.as_array().ok_or_else(|| anyhow::anyhow!("IN expects array"))?;
            if arr.is_empty() { bail!("IN expects non-empty array"); }
            let placeholders = std::iter::repeat("?").take(arr.len()).collect::<Vec<_>>().join(", ");
            let params = arr.iter().map(json_value_to_sql).collect::<Result<Vec<_>>>()?;
            Ok(SqlFragment {
                sql: format!("{field} {} ({})", op.sql(), placeholders),
                params,
            })
        }
        LeafOp::Contains => {
            // non-array column: substring LIKE
            let s = value.as_str().ok_or_else(|| anyhow::anyhow!("contains expects string"))?;
            Ok(SqlFragment {
                sql: format!("{field} LIKE ?"),
                params: vec![rusqlite::types::Value::Text(format!("%{s}%"))],
            })
        }
        _ => Ok(SqlFragment {
            sql: format!("{field} {} ?", op.sql()),
            params: vec![json_value_to_sql(value)?],
        }),
    }
}

fn json_value_to_sql(v: &Value) -> Result<rusqlite::types::Value> {
    Ok(match v {
        Value::Null => rusqlite::types::Value::Null,
        Value::Bool(b) => rusqlite::types::Value::Integer(i64::from(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() { rusqlite::types::Value::Integer(i) }
            else if let Some(f) = n.as_f64() { rusqlite::types::Value::Real(f) }
            else { bail!("unrepresentable number: {n}") }
        }
        Value::String(s) => rusqlite::types::Value::Text(s.clone()),
        _ => bail!("arrays/objects not allowed in leaf op"),
    })
}
```

- [ ] **Step 4: Tests pass**

Run: `cargo test -p librarian-mcp filter::tests`
Expected: 8 passed.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(librarian): filter AST + SQL compiler"
```

---

## Phase 6 — SQLite catalog

### Task 6.1: Migrations + open

**Files:**
- Create: `crates/librarian-mcp/src/catalog/mod.rs`
- Create: `crates/librarian-mcp/src/catalog/schema.sql`
- Modify: `crates/librarian-mcp/src/lib.rs`

- [ ] **Step 1: Write schema**

Create `crates/librarian-mcp/src/catalog/schema.sql`:

```sql
-- v1 schema
CREATE TABLE IF NOT EXISTS artifact (
  id            TEXT PRIMARY KEY,
  repo          TEXT NOT NULL,
  rel_path      TEXT NOT NULL,
  kind          TEXT NOT NULL,
  status        TEXT NOT NULL,
  title         TEXT,
  owners        TEXT NOT NULL DEFAULT '[]',
  tags          TEXT NOT NULL DEFAULT '[]',
  topic         TEXT,
  time_scope    TEXT,
  source        TEXT,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  file_mtime    INTEGER NOT NULL,
  file_sha256   TEXT NOT NULL,
  confidence    REAL NOT NULL DEFAULT 1.0,
  UNIQUE(repo, rel_path)
);

CREATE TABLE IF NOT EXISTS artifact_link (
  src_id        TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  dst_id        TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  rel           TEXT NOT NULL,
  created_at    INTEGER NOT NULL,
  PRIMARY KEY (src_id, dst_id, rel)
);

CREATE TABLE IF NOT EXISTS artifact_observation (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  artifact_id   TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  text          TEXT NOT NULL,
  source        TEXT,
  created_at    INTEGER NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS artifact_vec USING vec0(
  id            TEXT PRIMARY KEY,
  embedding     FLOAT[768]
);

CREATE INDEX IF NOT EXISTS idx_artifact_kind_status ON artifact(kind, status);
CREATE INDEX IF NOT EXISTS idx_artifact_repo ON artifact(repo);
CREATE INDEX IF NOT EXISTS idx_link_dst ON artifact_link(dst_id, rel);

CREATE TABLE IF NOT EXISTS schema_version (
  version INTEGER PRIMARY KEY
);
INSERT OR IGNORE INTO schema_version (version) VALUES (1);
```

- [ ] **Step 2: Catalog open + migration module**

Create `crates/librarian-mcp/src/catalog/mod.rs`:

```rust
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

pub mod artifact;

pub struct Catalog {
    pub conn: Connection,
}

const SCHEMA_SQL: &str = include_str!("schema.sql");

impl Catalog {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating catalog dir {}", parent.display()))?;
        }
        let conn = Connection::open(db_path)
            .with_context(|| format!("opening {}", db_path.display()))?;
        unsafe {
            sqlite_vec::load(&conn).context("loading sqlite-vec")?;
        }
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        conn.execute_batch(SCHEMA_SQL).context("applying schema")?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        unsafe {
            sqlite_vec::load(&conn).context("loading sqlite-vec")?;
        }
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA_SQL).context("applying schema")?;
        Ok(Self { conn })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_in_memory_and_applies_schema() {
        let cat = Catalog::open_in_memory().unwrap();
        let tables: Vec<String> = cat.conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(tables.iter().any(|t| t == "artifact"));
        assert!(tables.iter().any(|t| t == "artifact_link"));
        assert!(tables.iter().any(|t| t == "artifact_observation"));
    }
}
```

Add `pub mod catalog;` to `lib.rs`.

- [ ] **Step 3: Empty stub for `artifact.rs`**

Create `crates/librarian-mcp/src/catalog/artifact.rs` with just `// will be filled by Task 6.2`.

- [ ] **Step 4: Build + test**

Run: `cargo test -p librarian-mcp catalog`
Expected: 1 test passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(librarian): SQLite catalog open + schema migration"
```

### Task 6.2: `ArtifactRow` CRUD

**Files:**
- Modify: `crates/librarian-mcp/src/catalog/artifact.rs`

- [ ] **Step 1: Write failing tests**

```rust
use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::Catalog;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRow {
    pub id: String,
    pub repo: String,
    pub rel_path: String,
    pub kind: String,
    pub status: String,
    pub title: Option<String>,
    pub owners: Vec<String>,
    pub tags: Vec<String>,
    pub topic: Option<String>,
    pub time_scope: Option<String>,
    pub source: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub file_mtime: i64,
    pub file_sha256: String,
    pub confidence: f64,
}

pub fn upsert(cat: &Catalog, row: &ArtifactRow) -> Result<()> {
    cat.conn.execute(
        "INSERT INTO artifact (id, repo, rel_path, kind, status, title, owners, tags,
            topic, time_scope, source, created_at, updated_at, file_mtime, file_sha256, confidence)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
         ON CONFLICT(id) DO UPDATE SET
            repo=excluded.repo, rel_path=excluded.rel_path,
            kind=excluded.kind, status=excluded.status,
            title=excluded.title, owners=excluded.owners, tags=excluded.tags,
            topic=excluded.topic, time_scope=excluded.time_scope,
            source=excluded.source, updated_at=excluded.updated_at,
            file_mtime=excluded.file_mtime, file_sha256=excluded.file_sha256,
            confidence=excluded.confidence",
        params![
            row.id, row.repo, row.rel_path, row.kind, row.status,
            row.title,
            serde_json::to_string(&row.owners)?,
            serde_json::to_string(&row.tags)?,
            row.topic, row.time_scope, row.source,
            row.created_at, row.updated_at, row.file_mtime, row.file_sha256, row.confidence,
        ],
    )?;
    Ok(())
}

pub fn get(cat: &Catalog, id: &str) -> Result<Option<ArtifactRow>> {
    cat.conn
        .prepare("SELECT id, repo, rel_path, kind, status, title, owners, tags,
                  topic, time_scope, source, created_at, updated_at, file_mtime, file_sha256, confidence
                  FROM artifact WHERE id = ?1")?
        .query_row(params![id], row_from_sql)
        .optional()
        .map_err(Into::into)
}

pub fn delete(cat: &Catalog, id: &str) -> Result<bool> {
    Ok(cat.conn.execute("DELETE FROM artifact WHERE id = ?1", params![id])? > 0)
}

fn row_from_sql(r: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactRow> {
    let owners_s: String = r.get(6)?;
    let tags_s: String = r.get(7)?;
    Ok(ArtifactRow {
        id: r.get(0)?, repo: r.get(1)?, rel_path: r.get(2)?, kind: r.get(3)?, status: r.get(4)?,
        title: r.get(5)?,
        owners: serde_json::from_str(&owners_s).unwrap_or_default(),
        tags: serde_json::from_str(&tags_s).unwrap_or_default(),
        topic: r.get(8)?, time_scope: r.get(9)?, source: r.get(10)?,
        created_at: r.get(11)?, updated_at: r.get(12)?,
        file_mtime: r.get(13)?, file_sha256: r.get(14)?, confidence: r.get(15)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(), repo: "r".into(), rel_path: "p.md".into(),
            kind: "spec".into(), status: "active".into(),
            title: Some("T".into()), owners: vec!["marius".into()],
            tags: vec!["a".into(), "b".into()],
            topic: None, time_scope: None, source: Some("repo".into()),
            created_at: 1, updated_at: 2, file_mtime: 3,
            file_sha256: "abc".into(), confidence: 1.0,
        }
    }

    #[test]
    fn upsert_and_get_roundtrip() {
        let cat = Catalog::open_in_memory().unwrap();
        let row = sample("id1");
        upsert(&cat, &row).unwrap();
        let fetched = get(&cat, "id1").unwrap().unwrap();
        assert_eq!(fetched, row);
    }

    #[test]
    fn upsert_updates_on_conflict() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut row = sample("id1");
        upsert(&cat, &row).unwrap();
        row.status = "archived".into();
        row.updated_at = 99;
        upsert(&cat, &row).unwrap();
        let fetched = get(&cat, "id1").unwrap().unwrap();
        assert_eq!(fetched.status, "archived");
        assert_eq!(fetched.updated_at, 99);
    }

    #[test]
    fn delete_removes_row() {
        let cat = Catalog::open_in_memory().unwrap();
        upsert(&cat, &sample("id1")).unwrap();
        assert!(delete(&cat, "id1").unwrap());
        assert!(get(&cat, "id1").unwrap().is_none());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p librarian-mcp catalog::artifact`
Expected: 3 pass.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(librarian): artifact CRUD (upsert/get/delete)"
```

### Task 6.3: Links + observations CRUD

**Files:**
- Create: `crates/librarian-mcp/src/catalog/links.rs`
- Create: `crates/librarian-mcp/src/catalog/observations.rs`
- Modify: `crates/librarian-mcp/src/catalog/mod.rs`

- [ ] **Step 1: Write links + observations modules with tests**

Create `crates/librarian-mcp/src/catalog/links.rs`:

```rust
use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Catalog;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRow {
    pub src_id: String,
    pub dst_id: String,
    pub rel: String,
    pub created_at: i64,
}

pub fn insert(cat: &Catalog, link: &LinkRow) -> Result<()> {
    cat.conn.execute(
        "INSERT OR IGNORE INTO artifact_link (src_id, dst_id, rel, created_at) VALUES (?, ?, ?, ?)",
        params![link.src_id, link.dst_id, link.rel, link.created_at],
    )?;
    Ok(())
}

pub fn outgoing(cat: &Catalog, src_id: &str) -> Result<Vec<LinkRow>> {
    collect(cat, "WHERE src_id = ?1", params![src_id])
}

pub fn incoming(cat: &Catalog, dst_id: &str) -> Result<Vec<LinkRow>> {
    collect(cat, "WHERE dst_id = ?1", params![dst_id])
}

fn collect(cat: &Catalog, where_clause: &str, p: impl rusqlite::Params) -> Result<Vec<LinkRow>> {
    let sql = format!(
        "SELECT src_id, dst_id, rel, created_at FROM artifact_link {where_clause}"
    );
    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt.query_map(p, |r| Ok(LinkRow {
        src_id: r.get(0)?, dst_id: r.get(1)?, rel: r.get(2)?, created_at: r.get(3)?,
    }))?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact;
    use crate::catalog::artifact::ArtifactRow;

    fn art(id: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(), repo: "r".into(), rel_path: format!("{id}.md"),
            kind: "spec".into(), status: "active".into(),
            title: None, owners: vec![], tags: vec![],
            topic: None, time_scope: None, source: None,
            created_at: 0, updated_at: 0, file_mtime: 0, file_sha256: "".into(),
            confidence: 1.0,
        }
    }

    #[test]
    fn insert_and_query_links() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a")).unwrap();
        artifact::upsert(&cat, &art("b")).unwrap();
        insert(&cat, &LinkRow {
            src_id: "a".into(), dst_id: "b".into(),
            rel: "supersedes".into(), created_at: 1,
        }).unwrap();
        let out = outgoing(&cat, "a").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].dst_id, "b");
        let inc = incoming(&cat, "b").unwrap();
        assert_eq!(inc.len(), 1);
    }

    #[test]
    fn cascade_delete_removes_links() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a")).unwrap();
        artifact::upsert(&cat, &art("b")).unwrap();
        insert(&cat, &LinkRow {
            src_id: "a".into(), dst_id: "b".into(),
            rel: "implements".into(), created_at: 1,
        }).unwrap();
        artifact::delete(&cat, "a").unwrap();
        assert!(outgoing(&cat, "a").unwrap().is_empty());
        assert!(incoming(&cat, "b").unwrap().is_empty());
    }
}
```

Create `crates/librarian-mcp/src/catalog/observations.rs` (analogous, with `insert` + `list_for_artifact`).

Add to `crates/librarian-mcp/src/catalog/mod.rs`:
```rust
pub mod links;
pub mod observations;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p librarian-mcp catalog`
Expected: All catalog tests pass.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(librarian): link + observation CRUD with cascade tests"
```

### Task 6.4: `artifact_find` query executor

**Files:**
- Create: `crates/librarian-mcp/src/catalog/find.rs`
- Modify: `crates/librarian-mcp/src/catalog/mod.rs`

- [ ] **Step 1: Write the function with tests**

```rust
use anyhow::Result;
use rusqlite::types::ToSqlOutput;

use super::Catalog;
use super::artifact::ArtifactRow;
use crate::filter::{compile, FilterNode};

pub struct FindOpts {
    pub filter: Option<FilterNode>,
    pub limit: usize,
    pub offset: usize,
}

pub fn find(cat: &Catalog, opts: &FindOpts) -> Result<Vec<ArtifactRow>> {
    let mut sql = String::from(
        "SELECT id, repo, rel_path, kind, status, title, owners, tags,
         topic, time_scope, source, created_at, updated_at, file_mtime,
         file_sha256, confidence FROM artifact"
    );
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(f) = &opts.filter {
        let frag = compile(f)?;
        sql.push_str(" WHERE ");
        sql.push_str(&frag.sql);
        params.extend(frag.params);
    }
    sql.push_str(&format!(" ORDER BY updated_at DESC LIMIT {} OFFSET {}", opts.limit, opts.offset));

    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(params.iter()),
        super::artifact::row_from_sql_public,
    )?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}
```

Expose `row_from_sql_public` by making the private function in `artifact.rs` `pub(crate)` (rename mental model — the existing `row_from_sql` stays private; add a `pub(crate)` wrapper or change visibility).

Tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use crate::catalog::artifact::{self, ArtifactRow};

    fn art(id: &str, kind: &str, status: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(), repo: "r".into(), rel_path: format!("{id}.md"),
            kind: kind.into(), status: status.into(),
            title: None, owners: vec![], tags: vec!["t".into()],
            topic: None, time_scope: None, source: None,
            created_at: 0, updated_at: id.chars().last().map(|c| c as i64).unwrap_or(0),
            file_mtime: 0, file_sha256: "x".into(), confidence: 1.0,
        }
    }

    #[test]
    fn find_by_kind() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "spec", "active")).unwrap();
        artifact::upsert(&cat, &art("b", "plan", "active")).unwrap();
        let rows = find(&cat, &FindOpts {
            filter: Some(serde_json::from_value(json!({"kind": {"eq": "spec"}})).unwrap()),
            limit: 10, offset: 0,
        }).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "a");
    }

    #[test]
    fn find_with_and_composition() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "spec", "active")).unwrap();
        artifact::upsert(&cat, &art("b", "spec", "archived")).unwrap();
        let rows = find(&cat, &FindOpts {
            filter: Some(serde_json::from_value(json!({"and": [
                {"kind": {"eq": "spec"}},
                {"status": {"eq": "active"}}
            ]})).unwrap()),
            limit: 10, offset: 0,
        }).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "a");
    }
}
```

- [ ] **Step 2: Build + test**

Run: `cargo test -p librarian-mcp catalog::find`
Expected: 2 pass.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(librarian): artifact_find query executor with filter AST"
```

---

## Phase 7 — Workspace walker / indexer

### Task 7.1: Workspace config loader

**Files:**
- Create: `crates/librarian-mcp/src/workspace.rs`
- Modify: `crates/librarian-mcp/src/lib.rs`

- [ ] **Step 1: Type + loader with tests**

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::classify::Rule;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub roots: Vec<Root>,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    pub name: String,
    pub path: PathBuf,
}

pub fn default_config_path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("no config dir")?;
    Ok(base.join("librarian").join("workspace.toml"))
}

pub fn load(path: &Path) -> Result<WorkspaceConfig> {
    let s = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let cfg: WorkspaceConfig = toml::from_str(&s).context("parsing workspace.toml")?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn loads_minimal_config() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"
[[roots]]
name = "backend-kotlin"
path = "/home/x/work/backend-kotlin"

[[roots]]
name = "eduplanner-ui"
path = "/home/x/work/eduplanner-ui"

[[rule]]
glob = "**/docs/specs/*.md"
kind = "spec"
"#).unwrap();
        let cfg = load(f.path()).unwrap();
        assert_eq!(cfg.roots.len(), 2);
        assert_eq!(cfg.rules.len(), 1);
    }
}
```

Add `pub mod workspace;` to `lib.rs`.

- [ ] **Step 2: Test passes**

Run: `cargo test -p librarian-mcp workspace`
Expected: 1 pass.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(librarian): workspace.toml loader"
```

### Task 7.2: ID derivation

**Files:**
- Create: `crates/librarian-mcp/src/ids.rs`
- Modify: `crates/librarian-mcp/src/lib.rs`

- [ ] **Step 1: Deterministic id function + tests**

```rust
use sha2::{Digest, Sha256};

/// Stable artifact id: sha256("{repo}\n{rel_path}") hex, truncated to 16 chars.
pub fn artifact_id(repo: &str, rel_path: &str) -> String {
    let mut h = Sha256::new();
    h.update(repo.as_bytes());
    h.update(b"\n");
    h.update(rel_path.as_bytes());
    let hex = format!("{:x}", h.finalize());
    hex[..16].into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        assert_eq!(artifact_id("r", "p.md"), artifact_id("r", "p.md"));
    }

    #[test]
    fn different_inputs_different_ids() {
        assert_ne!(artifact_id("r1", "p.md"), artifact_id("r2", "p.md"));
        assert_ne!(artifact_id("r", "a.md"), artifact_id("r", "b.md"));
    }

    #[test]
    fn sixteen_hex_chars() {
        let id = artifact_id("r", "p.md");
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
```

Add `pub mod ids;` to `lib.rs`.

- [ ] **Step 2: Tests pass**

Run: `cargo test -p librarian-mcp ids`
Expected: 3 pass.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(librarian): deterministic artifact id derivation"
```

### Task 7.3: Indexer — walk, classify, upsert

**Files:**
- Create: `crates/librarian-mcp/src/indexer.rs`
- Modify: `crates/librarian-mcp/src/lib.rs`

- [ ] **Step 1: Indexer skeleton with fixture test**

Create a fixture in `crates/librarian-mcp/tests/fixtures/repo_a/`:

```
tests/fixtures/repo_a/docs/superpowers/specs/2026-04-19-foo.md
tests/fixtures/repo_a/docs/research/2026-04-10-bar.md
tests/fixtures/repo_a/README.md          (no rule, should be unknown)
```

Contents of `foo.md`:

```markdown
---
title: Foo spec
---

# Foo

Body.
```

Contents of `bar.md`:

```markdown
# Bar memory

Text
```

Contents of `README.md`:

```markdown
# repo_a

Plain readme.
```

Create `crates/librarian-mcp/src/indexer.rs`:

```rust
use anyhow::{Context, Result};
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::catalog::{artifact, Catalog};
use crate::catalog::artifact::ArtifactRow;
use crate::classify::{classify, CompiledRule};
use crate::frontmatter;
use crate::ids::artifact_id;

#[derive(Debug, Default)]
pub struct IndexReport {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub unknown_ids: Vec<String>,
}

pub fn index_repo(
    cat: &Catalog,
    rules: &[CompiledRule],
    repo_name: &str,
    repo_root: &Path,
) -> Result<IndexReport> {
    let mut report = IndexReport::default();
    let walker = WalkBuilder::new(repo_root)
        .standard_filters(true)
        .build();
    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let rel = path.strip_prefix(repo_root)?.to_string_lossy().to_string();
        let id = artifact_id(repo_name, &rel);
        let bytes = std::fs::read(path)?;
        let content = String::from_utf8_lossy(&bytes);
        let sha = {
            let mut h = Sha256::new(); h.update(&bytes); format!("{:x}", h.finalize())
        };
        let mtime = path.metadata()?.modified()?
            .duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_millis() as i64;

        // Existing row?
        let existing = artifact::get(cat, &id)?;
        if let Some(ref ex) = existing {
            if ex.file_sha256 == sha { report.unchanged += 1; continue; }
        }

        let (fm, _body) = frontmatter::parse(&content).unwrap_or((None, ""));
        let rule_match = classify(rules, &rel);

        let kind = fm.as_ref().and_then(|f| f.kind.clone())
            .or_else(|| rule_match.as_ref().map(|r| r.kind.clone()))
            .unwrap_or_else(|| "unknown".into());
        let status = fm.as_ref().and_then(|f| f.status.clone())
            .or_else(|| rule_match.as_ref().and_then(|r| r.status.clone()))
            .unwrap_or_else(|| {
                if kind == "unknown" { "unknown".into() } else { "draft".into() }
            });
        let time_scope = fm.as_ref().and_then(|f| f.time_scope.clone())
            .or_else(|| rule_match.as_ref().and_then(|r| r.time_scope.clone()));
        let confidence = if fm.as_ref().and_then(|f| f.kind.as_ref()).is_some() { 1.0 } else { 0.5 };

        let now = chrono::Utc::now().timestamp_millis();
        let row = ArtifactRow {
            id: id.clone(),
            repo: repo_name.into(),
            rel_path: rel.clone(),
            kind: kind.clone(),
            status,
            title: fm.as_ref().and_then(|f| f.title.clone()),
            owners: fm.as_ref().map(|f| f.owners.clone()).unwrap_or_default(),
            tags: fm.as_ref().map(|f| f.tags.clone()).unwrap_or_default(),
            topic: fm.as_ref().and_then(|f| f.topic.clone()),
            time_scope,
            source: Some("repo".into()),
            created_at: existing.as_ref().map(|ex| ex.created_at).unwrap_or(now),
            updated_at: now,
            file_mtime: mtime,
            file_sha256: sha,
            confidence,
        };
        artifact::upsert(cat, &row)?;
        if existing.is_some() { report.updated += 1; } else { report.added += 1; }
        if kind == "unknown" { report.unknown_ids.push(id); }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::load_rules;

    #[test]
    fn indexes_fixture_repo_with_mixed_classifications() {
        let cat = Catalog::open_in_memory().unwrap();
        let rules = load_rules(r#"
[[rule]]
glob = "**/docs/superpowers/specs/*.md"
kind = "spec"
status = "active"

[[rule]]
glob = "**/docs/research/*.md"
kind = "memory"
"#).unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/repo_a");
        let report = index_repo(&cat, &rules, "repo_a", &fixture).unwrap();
        assert_eq!(report.added, 3, "should index 3 .md files");
        assert_eq!(report.unknown_ids.len(), 1, "README.md is unknown");

        // Re-index → all unchanged.
        let r2 = index_repo(&cat, &rules, "repo_a", &fixture).unwrap();
        assert_eq!(r2.unchanged, 3);
        assert_eq!(r2.added, 0);
    }
}
```

Add `pub mod indexer;` to `lib.rs`.

- [ ] **Step 2: Run test**

Run: `cargo test -p librarian-mcp indexer`
Expected: 1 pass.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(librarian): indexer walks repo + classifies + upserts"
```

### Task 7.4: Removed-file cleanup

**Files:**
- Modify: `crates/librarian-mcp/src/indexer.rs`

- [ ] **Step 1: Add failing test**

```rust
#[test]
fn index_removes_deleted_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("docs/specs")).unwrap();
    std::fs::write(root.join("docs/specs/a.md"), "# a\n").unwrap();
    std::fs::write(root.join("docs/specs/b.md"), "# b\n").unwrap();

    let cat = Catalog::open_in_memory().unwrap();
    let rules = crate::classify::load_rules(
        "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n"
    ).unwrap();

    let r1 = index_repo(&cat, &rules, "r", root).unwrap();
    assert_eq!(r1.added, 2);

    std::fs::remove_file(root.join("docs/specs/b.md")).unwrap();
    let r2 = index_repo(&cat, &rules, "r", root).unwrap();
    assert_eq!(r2.removed, 1);
}
```

- [ ] **Step 2: Implement — track seen ids, delete rows not in the set**

Modify `index_repo` to collect `seen_ids` during walk; after the walk, `DELETE FROM artifact WHERE repo = ?1 AND id NOT IN (...)`. Add `removed: usize` to `IndexReport`.

- [ ] **Step 3: Tests pass**

Run: `cargo test -p librarian-mcp indexer`
Expected: 2 pass.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(librarian): indexer deletes stale rows for removed files"
```

### Task 7.5: Three-query stale test

**Files:**
- Modify: `crates/librarian-mcp/src/indexer.rs`

- [ ] **Step 1: Add stale-assertion test**

```rust
#[test]
fn reindex_refreshes_stale_metadata() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("docs/specs")).unwrap();
    let path = root.join("docs/specs/a.md");
    std::fs::write(&path, "---\ntitle: Original\n---\nbody\n").unwrap();
    let cat = Catalog::open_in_memory().unwrap();
    let rules = crate::classify::load_rules(
        "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n"
    ).unwrap();
    index_repo(&cat, &rules, "r", root).unwrap();
    let id = crate::ids::artifact_id("r", "docs/specs/a.md");

    // 1. Baseline
    let before = crate::catalog::artifact::get(&cat, &id).unwrap().unwrap();
    assert_eq!(before.title.as_deref(), Some("Original"));

    // 2. Mutate file on disk (NOT via our API).
    std::fs::write(&path, "---\ntitle: Updated\n---\nbody\n").unwrap();

    // 3. Assert stale (still "Original" because no reindex yet).
    let stale = crate::catalog::artifact::get(&cat, &id).unwrap().unwrap();
    assert_eq!(stale.title.as_deref(), Some("Original"), "must be stale before reindex");

    // 4. Reindex.
    index_repo(&cat, &rules, "r", root).unwrap();

    // 5. Fresh.
    let fresh = crate::catalog::artifact::get(&cat, &id).unwrap().unwrap();
    assert_eq!(fresh.title.as_deref(), Some("Updated"));
}
```

- [ ] **Step 2: Run**

Run: `cargo test -p librarian-mcp indexer::tests::reindex_refreshes_stale_metadata`
Expected: PASS (no implementation change needed — proves current invalidation behaviour).

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "test(librarian): three-query stale-fresh regression test for reindex"
```

---

## Phase 8 — MCP server skeleton + read tools

### Task 8.1: Server shell with empty tool list

**Files:**
- Create: `crates/librarian-mcp/src/server.rs`
- Create: `crates/librarian-mcp/src/tools/mod.rs`
- Modify: `crates/librarian-mcp/src/lib.rs`

- [ ] **Step 1: Define `Tool` trait + registry**

`crates/librarian-mcp/src/tools/mod.rs`:

```rust
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

use crate::catalog::Catalog;
use crate::classify::CompiledRule;
use crate::workspace::WorkspaceConfig;

pub struct ToolContext {
    pub catalog: Arc<parking_lot::Mutex<Catalog>>,
    pub workspace: Arc<WorkspaceConfig>,
    pub rules: Arc<Vec<CompiledRule>>,
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value>;
}

pub fn all_tools() -> Vec<Arc<dyn Tool>> {
    Vec::new()  // populated in later tasks
}
```

Add deps to `Cargo.toml`: `async-trait = "0.1"`, `parking_lot = "0.12"`.

Create `crates/librarian-mcp/src/server.rs` with a minimal rmcp handler delegating to `all_tools()`. Mirror codescout's `src/server.rs` structure but stripped down.

```rust
use anyhow::Result;
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt};
use rmcp::model::{CallToolRequestParam, CallToolResult, Content, ListToolsResult, Tool as McpTool};
use std::sync::Arc;

use crate::tools::{ToolContext, all_tools};

#[derive(Clone)]
pub struct LibrarianServer {
    ctx: Arc<ToolContext>,
    tools: Arc<Vec<Arc<dyn crate::tools::Tool>>>,
}

impl LibrarianServer {
    pub fn new(ctx: ToolContext) -> Self {
        Self { ctx: Arc::new(ctx), tools: Arc::new(all_tools()) }
    }

    pub async fn serve_stdio(self) -> Result<()> {
        let (stdin, stdout) = rmcp::transport::stdio();
        self.serve((stdin, stdout)).await?.waiting().await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ServerHandler for LibrarianServer {
    async fn list_tools(&self, _req: Option<rmcp::model::PaginatedRequestParam>, _: rmcp::service::RequestContext<rmcp::RoleServer>) -> std::result::Result<ListToolsResult, McpError> {
        let tools = self.tools.iter().map(|t| McpTool {
            name: t.name().into(),
            description: Some(t.description().into()),
            input_schema: Arc::new(serde_json::from_value(t.input_schema()).unwrap()),
            annotations: None,
        }).collect();
        Ok(ListToolsResult { tools, next_cursor: None })
    }

    async fn call_tool(&self, req: CallToolRequestParam, _: rmcp::service::RequestContext<rmcp::RoleServer>) -> std::result::Result<CallToolResult, McpError> {
        let tool = self.tools.iter().find(|t| t.name() == req.name.as_ref())
            .ok_or_else(|| McpError::invalid_params(format!("unknown tool `{}`", req.name), None))?;
        let args = serde_json::Value::Object(req.arguments.unwrap_or_default());
        match tool.call(&self.ctx, args).await {
            Ok(v) => Ok(CallToolResult {
                content: vec![Content::text(serde_json::to_string(&v).unwrap())],
                is_error: Some(false),
                structured_content: Some(v),
                meta: None,
            }),
            Err(e) => Ok(CallToolResult {
                content: vec![Content::text(format!("error: {e:#}"))],
                is_error: Some(true),
                structured_content: None,
                meta: None,
            }),
        }
    }
}
```

- [ ] **Step 2: Wire `run_stdio_server` in `lib.rs`**

```rust
pub async fn run_stdio_server() -> Result<()> {
    let cfg_path = workspace::default_config_path()?;
    let ws = workspace::load(&cfg_path)
        .with_context(|| format!("Load workspace from {}. Run `librarian-mcp import-codescout` to seed.", cfg_path.display()))?;
    let rules = classify::load_rules(/* serialize ws.rules back */)?;
    let db_path = dirs::data_local_dir().context("no data dir")?.join("librarian/catalog.db");
    let catalog = catalog::Catalog::open(&db_path)?;
    let ctx = tools::ToolContext {
        catalog: Arc::new(parking_lot::Mutex::new(catalog)),
        workspace: Arc::new(ws),
        rules: Arc::new(rules),
    };
    server::LibrarianServer::new(ctx).serve_stdio().await
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p librarian-mcp`
Expected: Succeeds. Server starts but has zero tools — that's fine for now.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(librarian): MCP server skeleton (rmcp stdio, empty tool list)"
```

### Task 8.2: `artifact_find` tool

**Files:**
- Create: `crates/librarian-mcp/src/tools/find.rs`
- Modify: `crates/librarian-mcp/src/tools/mod.rs`

- [ ] **Step 1: Tool implementation**

```rust
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::catalog::find::{find, FindOpts};
use crate::filter::FilterNode;
use super::{Tool, ToolContext};

pub struct ArtifactFind;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    filter: Option<FilterNode>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}
fn default_limit() -> usize { 50 }

#[async_trait]
impl Tool for ArtifactFind {
    fn name(&self) -> &'static str { "artifact_find" }
    fn description(&self) -> &'static str {
        "Search artifacts by filter AST (kind/status/tags/updated_at etc). \
         Composition: and/or/not. Leaf ops: eq/ne/in/nin/gt/lt/gte/lte/contains."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filter": {"type": "object"},
                "limit": {"type": "integer", "default": 50},
                "offset": {"type": "integer", "default": 0}
            }
        })
    }
    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let cat = ctx.catalog.lock();
        let rows = find(&cat, &FindOpts { filter: a.filter, limit: a.limit, offset: a.offset })?;
        let items: Vec<Value> = rows.into_iter().map(|r| json!({
            "id": r.id, "kind": r.kind, "status": r.status,
            "title": r.title, "repo": r.repo, "rel_path": r.rel_path,
            "updated_at": r.updated_at,
        })).collect();
        Ok(json!({"items": items, "count": items.len()}))
    }
}
```

Register in `tools/mod.rs`:

```rust
pub mod find;
pub fn all_tools() -> Vec<Arc<dyn Tool>> {
    vec![Arc::new(find::ArtifactFind)]
}
```

- [ ] **Step 2: Unit test**

In `tools/find.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Catalog, artifact};
    use crate::catalog::artifact::ArtifactRow;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig { roots: vec![], ignore: vec![], rules: vec![] }),
            rules: Arc::new(vec![]),
        }
    }

    #[tokio::test]
    async fn returns_rows_matching_filter() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &ArtifactRow {
            id: "a".into(), repo: "r".into(), rel_path: "a.md".into(),
            kind: "spec".into(), status: "active".into(),
            title: Some("A".into()), owners: vec![], tags: vec![],
            topic: None, time_scope: None, source: None,
            created_at: 0, updated_at: 1, file_mtime: 0,
            file_sha256: "".into(), confidence: 1.0,
        }).unwrap();
        let ctx = mk_ctx(cat);
        let v = ArtifactFind.call(&ctx, json!({
            "filter": {"kind": {"eq": "spec"}},
            "limit": 10
        })).await.unwrap();
        assert_eq!(v["count"].as_u64(), Some(1));
    }
}
```

- [ ] **Step 3: Run**

Run: `cargo test -p librarian-mcp tools::find`
Expected: 1 pass.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(librarian): artifact_find tool"
```

### Task 8.3: `artifact_get`, `artifact_list_by_kind`, `artifact_links`, `artifact_graph`

Each tool mirrors the `artifact_find` shape: a struct, `Args`, schema, `call`, and a `#[tokio::test]` exercising happy-path + at least one edge case.

**Files:**
- Create: `crates/librarian-mcp/src/tools/get.rs`
- Create: `crates/librarian-mcp/src/tools/list_by_kind.rs`
- Create: `crates/librarian-mcp/src/tools/links.rs`
- Create: `crates/librarian-mcp/src/tools/graph.rs`
- Modify: `crates/librarian-mcp/src/tools/mod.rs`

- [ ] **Step 1: Implement `artifact_get`**

Args: `{id, include_observations?: bool, include_links?: bool}`. Fetch `ArtifactRow`, optionally attach observations and outgoing+incoming links. Test: insert 1 artifact + 1 link + 1 observation, call with both flags true, assert all three present.

- [ ] **Step 2: Implement `artifact_list_by_kind`**

Args: `{kind, status?, limit, offset}`. Thin wrapper over `find` that builds a FilterNode internally. Test: insert 2 specs + 1 plan, list by kind=spec, assert count=2.

- [ ] **Step 3: Implement `artifact_links`**

Args: `{id, rel?: string, direction: "out"|"in"|"both"}`. Test: 3 artifacts A→B, B→C. Calling with id=B direction=both returns 2 edges.

- [ ] **Step 4: Implement `artifact_graph`**

Args: `{id, depth: 1..=3, rels?: [string]}`. Breadth-first walk from seed id up to `depth`, collecting nodes + edges. Test: linear chain A→B→C→D, seed=A depth=2 returns 3 nodes {A, B, C} and 2 edges.

- [ ] **Step 5: Register all four in `all_tools()`**

```rust
vec![
    Arc::new(find::ArtifactFind),
    Arc::new(get::ArtifactGet),
    Arc::new(list_by_kind::ArtifactListByKind),
    Arc::new(links::ArtifactLinks),
    Arc::new(graph::ArtifactGraph),
]
```

- [ ] **Step 6: Run all tests**

Run: `cargo test -p librarian-mcp tools`
Expected: All tool tests pass.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(librarian): read tools (get, list_by_kind, links, graph)"
```

---

## Phase 9 — Write tools

### Task 9.1: `artifact_create`

**Files:**
- Create: `crates/librarian-mcp/src/tools/create.rs`
- Modify: `crates/librarian-mcp/src/tools/mod.rs`

- [ ] **Step 1: Implement**

Args: `{repo, rel_path, kind, title, body, owners?, tags?}`. Resolve repo root from workspace config by name. Refuse if `repo_root.join(rel_path)` already exists. Build `Frontmatter`, call `frontmatter::write()`, write file, upsert row. Return `{id, repo, rel_path}`.

```rust
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::frontmatter::Frontmatter;
use crate::ids::artifact_id;
use crate::catalog::artifact::{self, ArtifactRow};
use super::{Tool, ToolContext};

pub struct ArtifactCreate;

#[derive(Deserialize)]
struct Args {
    repo: String, rel_path: String,
    kind: String, title: String, body: String,
    #[serde(default)] owners: Vec<String>,
    #[serde(default)] tags: Vec<String>,
}

#[async_trait]
impl Tool for ArtifactCreate {
    fn name(&self) -> &'static str { "artifact_create" }
    fn description(&self) -> &'static str {
        "Create a new artifact. Writes frontmatter + body to the file. Fails if path exists."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["repo", "rel_path", "kind", "title", "body"],
            "properties": {
                "repo": {"type": "string"}, "rel_path": {"type": "string"},
                "kind": {"type": "string"}, "title": {"type": "string"}, "body": {"type": "string"},
                "owners": {"type": "array", "items": {"type": "string"}},
                "tags": {"type": "array", "items": {"type": "string"}}
            }
        })
    }
    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let root = ctx.workspace.roots.iter().find(|r| r.name == a.repo)
            .ok_or_else(|| anyhow::anyhow!("unknown repo `{}`", a.repo))?;
        let full = root.path.join(&a.rel_path);
        if full.exists() { bail!("path exists: {}", full.display()); }
        if let Some(parent) = full.parent() { std::fs::create_dir_all(parent)?; }
        let id = artifact_id(&a.repo, &a.rel_path);
        let fm = Frontmatter {
            id: Some(id.clone()),
            kind: Some(a.kind.clone()),
            status: Some("draft".into()),
            title: Some(a.title.clone()),
            owners: a.owners.clone(),
            tags: a.tags.clone(),
            topic: None,
            time_scope: None,
        };
        let content = crate::frontmatter::write(&fm, &format!("\n{}\n", a.body));
        std::fs::write(&full, &content)?;
        let now = chrono::Utc::now().timestamp_millis();
        let row = ArtifactRow {
            id: id.clone(), repo: a.repo, rel_path: a.rel_path,
            kind: a.kind, status: "draft".into(), title: Some(a.title),
            owners: a.owners, tags: a.tags, topic: None, time_scope: None,
            source: Some("generated".into()),
            created_at: now, updated_at: now,
            file_mtime: now, file_sha256: crate::util::sha_of_bytes(content.as_bytes()),
            confidence: 1.0,
        };
        artifact::upsert(&ctx.catalog.lock(), &row)?;
        Ok(json!({"id": id, "repo": row.repo, "rel_path": row.rel_path}))
    }
}
```

Create `crates/librarian-mcp/src/util.rs`:

```rust
use sha2::{Digest, Sha256};
pub fn sha_of_bytes(b: &[u8]) -> String {
    let mut h = Sha256::new(); h.update(b); format!("{:x}", h.finalize())
}
```

Add `pub mod util;` to `lib.rs`.

- [ ] **Step 2: Test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{Root, WorkspaceConfig};
    use tempfile::TempDir;

    #[tokio::test]
    async fn creates_file_and_row() {
        let tmp = TempDir::new().unwrap();
        let ctx = ToolContext {
            catalog: std::sync::Arc::new(parking_lot::Mutex::new(crate::catalog::Catalog::open_in_memory().unwrap())),
            workspace: std::sync::Arc::new(WorkspaceConfig {
                roots: vec![Root { name: "r".into(), path: tmp.path().into() }],
                ignore: vec![], rules: vec![],
            }),
            rules: std::sync::Arc::new(vec![]),
        };
        let v = ArtifactCreate.call(&ctx, json!({
            "repo": "r", "rel_path": "docs/specs/x.md",
            "kind": "spec", "title": "X", "body": "hello"
        })).await.unwrap();
        let path = tmp.path().join("docs/specs/x.md");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("---\n"));
        assert!(content.contains("title: X"));
        let id = v["id"].as_str().unwrap();
        assert!(crate::catalog::artifact::get(&ctx.catalog.lock(), id).unwrap().is_some());
    }

    #[tokio::test]
    async fn refuses_if_file_exists() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/x.md"), "").unwrap();
        let ctx = /* same builder as above */;
        let err = ArtifactCreate.call(&ctx, json!({
            "repo": "r", "rel_path": "docs/x.md",
            "kind": "doc", "title": "X", "body": "hi"
        })).await.unwrap_err();
        assert!(err.to_string().contains("path exists"));
    }
}
```

- [ ] **Step 3: Run**

Run: `cargo test -p librarian-mcp tools::create`
Expected: 2 pass.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(librarian): artifact_create tool (write + index)"
```

### Task 9.2: `artifact_update`

**Files:**
- Create: `crates/librarian-mcp/src/tools/update.rs`

- [ ] **Step 1: Implement**

Args: `{id, patch: {status?, title?, owners?, tags?, topic?, body?}}`. Lookup row → read file → `frontmatter::update_in_place` → write file → re-upsert row with merged fields. If `body` is in patch, replace the body portion (everything after the closing `---`).

- [ ] **Step 2: Tests**

- update title round-trips through file
- update status=archived is persisted
- missing id returns error
- body patch preserves frontmatter

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(librarian): artifact_update tool with frontmatter round-trip"
```

### Task 9.3: `artifact_link`

**Files:**
- Create: `crates/librarian-mcp/src/tools/link.rs`

- [ ] **Step 1: Implement**

Args: `{src_id, dst_id, rel}`. Insert `LinkRow`. If `rel == "supersedes"`, set dst.status = "superseded" via `artifact::upsert`.

- [ ] **Step 2: Tests**

- basic link insert
- `supersedes` transitions dst status
- unknown src/dst returns error (FK violation)

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(librarian): artifact_link tool + supersedes status transition"
```

### Task 9.4: `artifact_observe`

**Files:**
- Create: `crates/librarian-mcp/src/tools/observe.rs`

- [ ] **Step 1: Implement**

Args: `{id, text, source?}`. Insert into `artifact_observation`.

- [ ] **Step 2: Test**

Insert → list_for_artifact returns 1 row.

- [ ] **Step 3: Register all four write tools in `all_tools()`**

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(librarian): artifact_observe tool + register write tools"
```

---

## Phase 10 — `librarian_context` + admin tools

### Task 10.1: `librarian_reindex`

**Files:**
- Create: `crates/librarian-mcp/src/tools/reindex.rs`

- [ ] **Step 1: Implement**

Args: `{repo?, force?}`. If `repo` given, index just that root; else iterate all roots. Sum reports. If `force`, delete all rows for that repo first. Return `{added, updated, removed, unchanged, unknown_count, unknown_ids}`.

- [ ] **Step 2: Test**

Two fixture repos, full reindex reports non-zero adds + correct unknown ids.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(librarian): librarian_reindex tool"
```

### Task 10.2: `librarian_context` (packed bundle)

**Files:**
- Create: `crates/librarian-mcp/src/tools/context.rs`

- [ ] **Step 1: Implement**

Args: `{topic?, anchor_id?, max_tokens?: usize}` (default `max_tokens=4000`). Strategy:
1. If `anchor_id` given, start from it + expand via `artifact_graph` depth=1.
2. Else if `topic` given, run semantic search (stubbed to string LIKE on title+topic for v1 until Phase 11 wires embeddings).
3. For each candidate, fetch title + read first 30 lines of body + render as:
   ```markdown
   ## {title}  — {kind}/{status}  ({repo}/{rel_path})
   {first 30 lines}
   ```
4. Accumulate until token budget (approximate as `chars / 4`).

Return `{markdown: string, included_ids: [string]}`.

- [ ] **Step 2: Test**

Three artifacts, topic matches two. Context includes exactly the two titles and respects token budget.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(librarian): librarian_context packed-markdown tool (text fallback)"
```

---

## Phase 11 — Semantic search integration

### Task 11.1: Embed artifacts on upsert

**Files:**
- Modify: `crates/librarian-mcp/src/indexer.rs`
- Create: `crates/librarian-mcp/src/embedding.rs`

- [ ] **Step 1: Thin embedder holder**

Create `crates/librarian-mcp/src/embedding.rs`:

```rust
use anyhow::Result;
use codescout_embed::Embedder;
use std::sync::Arc;

pub struct EmbeddingService {
    pub embedder: Arc<dyn Embedder>,
}

impl EmbeddingService {
    pub fn new(e: Arc<dyn Embedder>) -> Self { Self { embedder: e } }

    pub async fn embed_artifact(&self, title: Option<&str>, body: &str) -> Result<Vec<f32>> {
        let text = format!("{}\n\n{}", title.unwrap_or(""), body);
        let v = self.embedder.embed(&text).await?;
        Ok(v)
    }
}
```

Add `embedding: Arc<EmbeddingService>` to `ToolContext`.

- [ ] **Step 2: Upsert into `artifact_vec`**

In `indexer.rs`, after `artifact::upsert`, if embedding is configured, compute the vector and `INSERT OR REPLACE INTO artifact_vec (id, embedding) VALUES (?, ?)`. Use `codescout-embed::chunk_markdown` to split long bodies; embed the first chunk only for the artifact-level vector (v1 simplification — chunk-level vectors are a future feature).

- [ ] **Step 3: Test**

Index 1 file, `SELECT count(*) FROM artifact_vec` returns 1.

Run: `cargo test -p librarian-mcp indexer`
Expected: All existing tests pass + new one.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(librarian): embed artifacts into sqlite-vec on upsert"
```

### Task 11.2: Wire semantic search into `artifact_find` + `librarian_context`

**Files:**
- Modify: `crates/librarian-mcp/src/catalog/find.rs`
- Modify: `crates/librarian-mcp/src/tools/find.rs`
- Modify: `crates/librarian-mcp/src/tools/context.rs`

- [ ] **Step 1: Add `semantic: Option<String>` to `FindOpts`**

If set, run `SELECT id FROM artifact_vec WHERE embedding MATCH ? AND k = ?` to get top-K candidate ids, then apply the filter + paginate. Return rows with an optional `score`.

- [ ] **Step 2: Surface in tool**

`artifact_find` accepts `semantic` string. `librarian_context` prefers semantic search when topic is given.

- [ ] **Step 3: Integration test**

Two artifacts with distinct topics, semantic query biased toward one, assert it ranks first.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(librarian): semantic search via sqlite-vec in find + context"
```

---

## Phase 12 — CLI subcommands

### Task 12.1: `import-codescout`

**Files:**
- Modify: `crates/librarian-mcp/src/lib.rs`

- [ ] **Step 1: Implement**

Read codescout's project registry (location: `~/.local/share/codescout/projects.toml` or equivalent — check codescout's `src/agent.rs` for the actual path). Convert each project into a `Root { name, path }`. Write a new `~/.config/librarian/workspace.toml` with the default classification rules from the spec.

If `workspace.toml` exists already, refuse with a message explaining how to merge by hand (v1 keeps it simple).

- [ ] **Step 2: Integration test**

Point codescout registry at a temp file with 2 fake projects. Run `import_codescout()`. Assert `workspace.toml` contains both roots + 9 default rules.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(librarian): import-codescout CLI subcommand"
```

### Task 12.2: `reindex` CLI subcommand

**Files:**
- Modify: `crates/librarian-mcp/src/lib.rs`

- [ ] **Step 1: Implement**

`reindex_cli(repo)` opens catalog + workspace, calls the indexer for one or all roots, prints a summary to stdout.

- [ ] **Step 2: Test**

From Rust test, set `XDG_CONFIG_HOME` + `XDG_DATA_HOME` to a tempdir containing a seeded workspace.toml and one fake repo. Call `reindex_cli(None)`. Assert it prints `added: N`.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(librarian): reindex CLI subcommand"
```

---

## Phase 13 — End-to-end validation

### Task 13.1: MCP subprocess integration test

**Files:**
- Create: `crates/librarian-mcp/tests/mcp_integration.rs`

- [ ] **Step 1: Spawn `librarian-mcp` as subprocess with tempdir config/data**

Use `assert_cmd` (add to `[dev-dependencies]`: `assert_cmd = "2"`, `predicates = "3"`). Write a minimal `workspace.toml` + fixture repo. Spawn the binary with `--` args, pipe JSON-RPC through stdin, assert `list_tools` returns 11 tools and `call_tool("artifact_find", {})` returns JSON with `count >= 1`.

- [ ] **Step 2: Run**

Run: `cargo test -p librarian-mcp --test mcp_integration`
Expected: Passes.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "test(librarian): MCP subprocess integration test"
```

### Task 13.2: Full project gate

- [ ] **Step 1: `cargo fmt --check`**

Run: `cargo fmt --all -- --check`
Expected: Clean.

- [ ] **Step 2: `cargo clippy -- -D warnings`**

Run: `cargo clippy --workspace -- -D warnings`
Expected: Zero warnings.

- [ ] **Step 3: Full test suite**

Run: `cargo test --workspace`
Expected: All passing. Record counts for codescout (must match Phase 0.1 baseline) and librarian (target: at least 40 tests across all modules).

- [ ] **Step 4: Build release**

Run: `cargo build --release -p librarian-mcp`
Expected: `target/release/librarian-mcp` exists.

### Task 13.3: Manual validation — real workspace

- [ ] **Step 1: Seed config**

```bash
./target/release/librarian-mcp import-codescout
```

Verify `~/.config/librarian/workspace.toml` lists the codescout project root (and any others codescout knows).

- [ ] **Step 2: Reindex**

```bash
./target/release/librarian-mcp reindex
```

Expect non-zero `added` and some `unknown_ids`.

- [ ] **Step 3: Wire as MCP server in Claude Code for codescout's own workspace**

Add to `~/.claude/settings.json` (or project `.mcp.json`):

```json
"librarian": {"command": "/absolute/path/to/target/release/librarian-mcp"}
```

Restart with `/mcp`. Confirm `librarian-mcp` reports 11 tools. Call `artifact_find` with `{"filter": {"kind": {"eq": "spec"}}}` from the agent. Verify specs in `docs/superpowers/specs/` appear.

- [ ] **Step 4: Bug log**

If anything unexpected happens, append an entry to `docs/TODO-tool-misbehaviors.md` before continuing.

### Task 13.4: Credits + ship

**Files:**
- Create: `crates/librarian-mcp/CREDITS.md`

- [ ] **Step 1: Write credits**

```markdown
# Credits

librarian-mcp borrows design from:

- **Redis agent-memory-server** (Apache-2.0) — https://github.com/redis/agent-memory-server
  - Filter AST shape (`filter.rs`)
  - Artifact row field layout (derived from `MemoryRecord`)
  - Working-vs-long-term split applied to draft-vs-indexed artifacts

- **Model Context Protocol reference memory server** (MIT) — https://github.com/modelcontextprotocol/servers
  - Entity / Relation / Observation conceptual model
```

- [ ] **Step 2: Commit**

```bash
git add -A
git commit -m "docs(librarian): credits + attribution"
```

- [ ] **Step 3: Sanity check the spec → plan mapping one more time**

Open `docs/superpowers/specs/2026-04-19-librarian-mcp-design.md` alongside this plan. Confirm every spec section has a corresponding task. Any gap → add a task before declaring done.

- [ ] **Step 4: Merge to `experiments`**

```bash
git checkout experiments
git merge <feature-branch> --no-ff
```

---

## Self-Review notes

**Spec coverage (cross-checked):**

| Spec section | Tasks |
|---|---|
| Architecture (two processes + shared crate) | 0, 1, 2 |
| Metadata authority split (file / DB) | 3 (frontmatter), 6 (catalog), 9 (round-trip writes) |
| SQLite schema | 6.1 |
| Frontmatter schema | 3.1–3.2 |
| Kind taxonomy + status lifecycle | 4, 7.3, 9.3 (supersedes transition) |
| Tool API (11 tools) | 8.2–8.3, 9.1–9.4, 10.1–10.2 |
| Filter AST | 5 |
| Classification (rules + unknown workflow) | 4, 7.3 |
| Discovery (on-demand reindex) | 7.3–7.4, 10.1 |
| Deployment (crate layout, CLI, config paths) | 0–2, 8.1, 12 |
| Testing (unit, integration, MCP-level, three-query stale) | every task + 7.5 + 13.1 |

**Type / signature cross-check:**
- `Frontmatter` struct defined in Task 3.1 is the same shape used in Tasks 7.3 + 9.1 + 9.2.
- `ArtifactRow` defined in Task 6.2 matches columns in the schema (Task 6.1).
- `FilterNode` defined in Task 5.1 is the same type consumed by Task 6.4's `FindOpts`.
- `Catalog::open_in_memory` vs `Catalog::open` split is consistent — only `open_in_memory` is used in tests.

**Known simplifications (explicit non-goals):**
- Chunk-level embeddings (one vector per artifact, not per chunk) — acceptable for v1 (spec's "semantic search" doesn't require chunking).
- No file watcher — spec's roadmap section already defers this.
- `artifact_update` body replacement is whole-body — no diff/patch semantics — matches spec.
