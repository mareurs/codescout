# Default-Ignore Globs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the semantic code index honour `[ignored_paths]` (defaults included) so dependency/artifact trees are excluded by default, applied identically in the indexer and the preflight guard.

**Architecture:** Add one shared `build_ignore_matcher(root, patterns) -> ignore::gitignore::Gitignore` next to `lang_for_ext`. Both `stream_index` (indexer) and `check_index_scope` (guard) build a `WalkBuilder::filter_entry` from it, so a bare `node_modules` prunes that directory at any depth during the walk. Patterns are threaded from `config.ignored_paths.patterns` through `SyncOpts`.

**Tech Stack:** Rust, `ignore = "0.4"` (`gitignore::Gitignore`, `WalkBuilder::filter_entry`), tokio.

## Global Constraints

- `ignore = "0.4"` is already a dependency. Use `ignore::gitignore::{Gitignore, GitignoreBuilder}`.
- Lockstep: the indexer and guard MUST derive their walk filter from the same `build_ignore_matcher`. Never inline a second matcher (this is the 2026-06-02 divergence class).
- Serde semantics unchanged: key absent → `default_ignored_patterns()`; explicit `patterns = []` → ignore nothing.
- Gitignore matching prunes at the **directory** level in the walk; a file *under* an ignored dir is never visited (so `Gitignore::matched(file, false)` returning false is expected — assert the dir match in unit tests, prove end-to-end exclusion in integration tests).
- Pre-commit gate before every commit: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib`.

---

### Task 1: Shared `build_ignore_matcher`

**Files:**
- Modify: `src/embed/mod.rs` (add fn beside `lang_for_ext`)
- Test: `src/embed/mod.rs` (`#[cfg(test)] mod` in same file)

**Interfaces:**
- Produces: `pub fn build_ignore_matcher(root: &std::path::Path, patterns: &[String]) -> ignore::gitignore::Gitignore`

- [ ] **Step 1: Write the failing tests** (in `src/embed/mod.rs` tests module)

```rust
#[test]
fn ignore_matcher_prunes_bare_name_dir_at_any_depth() {
    use std::path::Path;
    let m = build_ignore_matcher(Path::new("/proj"), &["node_modules".into(), ".venv".into()]);
    // Pruning happens on the directory during the walk:
    assert!(m.matched("/proj/a/b/node_modules", true).is_ignore());
    assert!(m.matched("/proj/services/.venv", true).is_ignore());
    assert!(!m.matched("/proj/src/main.rs", false).is_ignore());
}

#[test]
fn ignore_matcher_supports_glob_patterns() {
    use std::path::Path;
    let m = build_ignore_matcher(Path::new("/proj"), &["**/*.gen.rs".into()]);
    assert!(m.matched("/proj/a/foo.gen.rs", false).is_ignore());
    assert!(!m.matched("/proj/a/foo.rs", false).is_ignore());
}

#[test]
fn ignore_matcher_empty_matches_nothing() {
    use std::path::Path;
    let m = build_ignore_matcher(Path::new("/proj"), &[]);
    assert!(!m.matched("/proj/node_modules", true).is_ignore());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib ignore_matcher 2>&1`
Expected: FAIL — `build_ignore_matcher` not found.

- [ ] **Step 3: Implement `build_ignore_matcher`**

```rust
/// Build a gitignore-style matcher from `[ignored_paths] patterns`, rooted at
/// `root`. Shared by the code indexer (`sync_project`/`stream_index`) and the
/// preflight guard (`check_index_scope`) so the two never disagree on what is
/// excluded (the 2026-06-02 walker-divergence class). Gitignore semantics: a bare
/// `node_modules` prunes any directory of that name at any depth during the walk.
/// Fail-soft: an invalid pattern is logged and skipped; a build failure yields an
/// empty matcher (ignores nothing) rather than aborting the index.
pub fn build_ignore_matcher(
    root: &std::path::Path,
    patterns: &[String],
) -> ignore::gitignore::Gitignore {
    let mut b = ignore::gitignore::GitignoreBuilder::new(root);
    for p in patterns {
        if let Err(e) = b.add_line(None, p) {
            tracing::warn!(pattern = %p, error = %e, "skipping invalid ignore pattern");
        }
    }
    b.build().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "ignore matcher build failed; ignoring nothing");
        ignore::gitignore::Gitignore::empty()
    })
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib ignore_matcher 2>&1`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings 2>&1
git add src/embed/mod.rs
git commit -m "feat(embed): shared build_ignore_matcher (gitignore-style)" \
  -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Thread `ignore_patterns` through `SyncOpts` (plumbing, no behavior yet)

**Files:**
- Modify: `src/retrieval/sync.rs` (`SyncOpts` struct)
- Modify: `src/tools/semantic/index.rs:125`, `:299` (two `SyncOpts {…}`)
- Modify: `src/agent/mod.rs` (`maybe_auto_index_library`, the `SyncOpts::default()` site)
- Modify: `src/bin/sync_project.rs:19`, `src/main.rs:261`

**Interfaces:**
- Produces: `SyncOpts { …, pub ignore_patterns: Vec<String> }` (derives `Default` → `vec![]`)

- [ ] **Step 1: Add the field** to `SyncOpts` in `src/retrieval/sync.rs`:

```rust
    /// Glob/gitignore-style patterns to exclude from the index walk. Sourced from
    /// `config.ignored_paths.patterns`; an empty vec ignores nothing.
    pub ignore_patterns: Vec<String>,
```

- [ ] **Step 2: Populate each indexing-path constructor.** A `SyncOpts::default()` / `..Default::default()` gives an *empty* vec (ignore nothing), so each real indexing path must set it explicitly:
  - `src/tools/semantic/index.rs:299` (project sync): set `ignore_patterns: ignore_patterns.clone()` — `ignore_patterns` is fetched in Task 4 Step 2's closure; until then use `Vec::new()` and wire it in Task 4. To keep Task 2 self-contained, set `ignore_patterns: Vec::new()` here and update in Task 4.
  - `src/tools/semantic/index.rs:125` (library sync): set `ignore_patterns: crate::config::project::ProjectConfig::load_or_default(&lib_path).map(|c| c.ignored_paths.patterns).unwrap_or_default()` — the *library's own* `ignored_paths` (or defaults), so a vendored `node_modules`/`.venv` inside the library checkout is pruned too (`lib_path` is already in scope at the `sync_project` call).
  - `src/agent/mod.rs` `maybe_auto_index_library`: change `SyncOpts::default()` to `SyncOpts { ignore_patterns: ignore_patterns_for_lib, ..Default::default() }`, where `ignore_patterns_for_lib` is captured from `p.config.ignored_paths.patterns` in the same up-front `inner.read()` block that already extracts `max_index_bytes` (extend that tuple).
  - `src/bin/sync_project.rs:19` and `src/main.rs:261`: set `ignore_patterns: ProjectConfig::load_or_default(&root)?.ignored_paths.patterns` (import `codescout::config::project::ProjectConfig`). main.rs uses `..Default::default()` so add the field before it.

- [ ] **Step 3: Build to verify it compiles** (no test yet — pure plumbing)

Run: `cargo build 2>&1`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings 2>&1
git add src/retrieval/sync.rs src/tools/semantic/index.rs src/agent/mod.rs src/bin/sync_project.rs src/main.rs
git commit -m "feat(retrieval): add SyncOpts.ignore_patterns + populate from config" \
  -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: `stream_index` applies the matcher

**Files:**
- Modify: `src/retrieval/sync.rs` (`stream_index` signature + walk; `sync_project` passes it; existing tests; new test)

**Interfaces:**
- Consumes: `build_ignore_matcher` (Task 1), `SyncOpts.ignore_patterns` (Task 2)
- Produces: `stream_index(…, ignore_patterns: &[String])` — new final param

- [ ] **Step 1: Write the failing integration test** (in `src/retrieval/sync.rs` tests module). Extend the existing `RecordingStore`/`FakeEmbedder` harness:

```rust
#[tokio::test]
async fn stream_index_excludes_ignored_dirs() {
    let dir = tempfile::tempdir().unwrap();
    write_sources(dir.path(), 3); // file_0.rs..file_2.rs at root
    std::fs::create_dir_all(dir.path().join("node_modules")).unwrap();
    std::fs::write(dir.path().join("node_modules/dep.js"), "function x(){ return 1; }\n").unwrap();
    std::fs::create_dir_all(dir.path().join("svc/.venv")).unwrap();
    std::fs::write(dir.path().join("svc/.venv/lib.py"), "def y():\n    return 2\n").unwrap();
    let store = RecordingStore::default();
    let emb = FakeEmbedder { dim: 4 };

    let patterns = vec!["node_modules".to_string(), ".venv".to_string()];
    let (added, _) = stream_index(dir.path(), "p", "coll", &[], &emb, &store, false, 1200, 256, &patterns)
        .await.unwrap();

    let ids: Vec<String> = store.upserted.lock().unwrap().iter().map(|r| r.chunk_id.clone()).collect();
    assert!(ids.iter().all(|id| !id.contains("node_modules") && !id.contains(".venv")),
        "ignored dirs must not be indexed: {ids:?}");
    assert!(added >= 3, "the 3 root .rs files should still index");

    // With no patterns, the dep files ARE indexed.
    let store2 = RecordingStore::default();
    let (added2, _) = stream_index(dir.path(), "p", "coll", &[], &emb, &store2, false, 1200, 256, &[])
        .await.unwrap();
    assert!(added2 > added, "empty patterns must index everything");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib stream_index_excludes_ignored_dirs 2>&1`
Expected: FAIL — `stream_index` takes 9 args, not 10 (compile error).

- [ ] **Step 3: Add the param + filter_entry** to `stream_index`. Add final param `ignore_patterns: &[String]`, and replace the walker construction:

```rust
    let matcher = crate::embed::build_ignore_matcher(root, ignore_patterns);
    for entry in ignore::WalkBuilder::new(root)
        .hidden(false) // index tracked dotfiles; gitignore handles exclusions
        .filter_entry(move |e| {
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            !matcher.matched(e.path(), is_dir).is_ignore()
        })
        .build()
        .filter_map(|e| e.ok())
    {
```

- [ ] **Step 4: Update `sync_project`** to pass the patterns — change the `stream_index(…)` call to add a final argument `&opts.ignore_patterns`.

- [ ] **Step 5: Update the existing `stream_index` test calls** — the three pre-existing tests (`stream_index_flushes_in_bounded_batches`, `…_incremental_…`, `…_force_…`) each call `stream_index(...)`; append `, &[]` as the final argument to every call (they assert no-ignore behavior).

- [ ] **Step 6: Run to verify pass**

Run: `cargo test --lib stream_index 2>&1`
Expected: PASS (4 tests).

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings 2>&1
git add src/retrieval/sync.rs
git commit -m "feat(retrieval): stream_index honours ignore_patterns (prunes dep dirs)" \
  -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: `check_index_scope` regains `patterns` + lockstep

**Files:**
- Modify: `src/embed/preflight.rs` (`check_index_scope` signature, walk, comment at :100)
- Modify: `src/tools/semantic/index.rs` (preflight call ~:200 + the `:299` `SyncOpts.ignore_patterns`)
- Test: `src/embed/preflight.rs` (lockstep + exclusion tests)

**Interfaces:**
- Consumes: `build_ignore_matcher` (Task 1)
- Produces: `check_index_scope(root: &Path, max_bytes: u64, patterns: &[String]) -> Result<PreflightVerdict>`

- [ ] **Step 1: Write the failing test** in `src/embed/preflight.rs` tests:

```rust
#[test]
fn check_index_scope_excludes_ignored_dirs() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    std::fs::create_dir_all(dir.path().join("node_modules")).unwrap();
    // Big ignored file that would trip a tiny threshold if counted.
    std::fs::write(dir.path().join("node_modules/big.js"), vec![b'x'; 4096]).unwrap();
    // 2 KB threshold; only a.rs (~10 B) counts once node_modules is pruned.
    let v = check_index_scope(dir.path(), 2048, &["node_modules".to_string()]).unwrap();
    assert!(matches!(v, PreflightVerdict::Clear), "node_modules must be pruned: {v:?}");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib check_index_scope_excludes_ignored_dirs 2>&1`
Expected: FAIL — `check_index_scope` takes 2 args, not 3 (compile error).

- [ ] **Step 3: Add `patterns` param + filter_entry** to `check_index_scope`. Add `patterns: &[String]`, build the matcher, and add the same `.filter_entry(...)` closure (identical to Task 3 Step 3) to its `WalkBuilder`. Update the docstring/comment at `:100` to state the indexer now honours `ignored_paths` via the shared `build_ignore_matcher`.

- [ ] **Step 4: Update the caller** in `src/tools/semantic/index.rs`: extend the existing `with_project_at` closure to return both values, then pass patterns to both sites:

```rust
            let (max_bytes, ignore_patterns) = ctx
                .agent
                .with_project_at(ctx.workspace_override.as_deref(), |p| {
                    Ok((p.config.security.max_index_bytes, p.config.ignored_paths.patterns.clone()))
                })
                .await
                .unwrap_or((500 * 1024 * 1024, Vec::new()));
            let preflight_root = root.clone();
            let pf_patterns = ignore_patterns.clone();
            let verdict = tokio::task::spawn_blocking(move || {
                check_index_scope(&preflight_root, max_bytes, &pf_patterns)
            })
            .await
            .map_err(|e| anyhow::anyhow!("preflight task join error: {e}"))??;
```

  Then at `:299` set `ignore_patterns: ignore_patterns` in the `SyncOpts` (replacing the `Vec::new()` placeholder from Task 2). Update the other preflight tests in `preflight.rs` that call `check_index_scope(dir, n)` to `check_index_scope(dir, n, &[])`.

- [ ] **Step 5: Write the lockstep test** in `preflight.rs` (proves guard estimate == indexer's eligible set under the same patterns):

```rust
#[test]
fn check_index_scope_and_lang_for_ext_agree_on_pruned_set() {
    // The guard counts only files the indexer would embed: indexable extension
    // AND not under an ignored dir. node_modules/*.js is indexable-but-ignored.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("keep.rs"), "fn k() {}\n").unwrap();
    std::fs::create_dir_all(dir.path().join("node_modules")).unwrap();
    std::fs::write(dir.path().join("node_modules/skip.js"), "var z=1;\n").unwrap();
    let v = check_index_scope(dir.path(), 1, &["node_modules".to_string()]).unwrap();
    match v {
        PreflightVerdict::RequiresConfirmation(info) => assert_eq!(info.file_count, 1),
        other => panic!("expected 1 eligible file (keep.rs), got {other:?}"),
    }
}
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test --lib check_index_scope 2>&1`
Expected: PASS (all preflight tests).

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings 2>&1
git add src/embed/preflight.rs src/tools/semantic/index.rs
git commit -m "feat(embed): check_index_scope honours ignore_patterns (lockstep w/ indexer)" \
  -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Incidental doc fixes + full gate

**Files:**
- Modify: `docs/issues/2026-06-19-mcp-server-oom-68gb.md` (mitigation note)
- Modify: `docs/trackers/index-scope-default-ignores.md` (mark the wiring done; note librarian bare-name inconsistency remains)

- [ ] **Step 1: Correct the OOM issue mitigation note.** In `docs/issues/2026-06-19-mcp-server-oom-68gb.md`, the "per-project mitigation" / "immediate user-side mitigation" referencing `[ignored_paths]` must note it was ineffective for the **code** index until this change; now it works. Use `edit_markdown` (action="edit").

- [ ] **Step 2: Update the tracker.** In `docs/trackers/index-scope-default-ignores.md`, mark "wire existing config into the code index" as DONE (link this plan), keep "expand defaults" + "librarian shares the list but keeps a plain-globset matcher (bare names don't match nested there)" as open follow-ups.

- [ ] **Step 3: Full gate**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --lib 2>&1`
Expected: clean; all tests pass.

- [ ] **Step 4: Commit**

```bash
git add docs/issues/2026-06-19-mcp-server-oom-68gb.md docs/trackers/index-scope-default-ignores.md
git commit -m "docs: ignored_paths now honoured by the code index (OOM follow-up)" \
  -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Manual verification (optional, after gate)

Re-run the capped sync from the OOM validation on a tree with a `node_modules`/`.venv`, with `[ignored_paths]` at defaults: confirm those dirs are skipped (lower chunk count than an un-ignored run) and RSS stays flat. The streaming fix already guarantees memory safety; this confirms the walk prunes.
