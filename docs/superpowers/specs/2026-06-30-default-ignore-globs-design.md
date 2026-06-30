---
status: approved
created: 2026-06-30
owner: marius
topic: default-ignore-globs
related:
  - docs/issues/2026-06-19-mcp-server-oom-68gb.md
  - docs/issues/2026-06-02-preflight-sync-walker-divergence.md
  - docs/trackers/index-scope-default-ignores.md
---

# Design: wire `[ignored_paths]` into the code index (default-ignore globs)

## Context / problem

The semantic **code index** (`RetrievalClient::sync_project` → `stream_index`,
`src/retrieval/sync.rs`) walks the tree honouring only `.gitignore` (via
`ignore::WalkBuilder`) and the `lang_for_ext` extension allowlist. It does **not**
honour `config.ignored_paths.patterns` at all.

That config and a sensible default set (`default_ignored_patterns()` in
`src/config/project.rs`: `.git, node_modules, target, __pycache__, .venv, dist,
build, .codescout, .worktrees, .claude`) already exist — but `compile_ignore`
(`src/librarian/workspace.rs`) is consumed **only by the librarian** (markdown)
indexer. The `2026-06-02` preflight-divergence fix deliberately *dropped*
`ignored_paths` from the guard (`check_index_scope`), reasoning "the indexer never
honoured it, and gitignore + the extension allowlist already exclude
`.git`/binaries/`target`."

The `2026-06-19` 68 GB OOM disproved that reasoning: gitignore + `lang_for_ext`
does **not** exclude un-gitignored dependency *source* trees (the
`python-services/` `.py` files under a tree with `[ignored_paths] patterns = []`).
The streaming fix (shipped) makes such a walk memory-safe, but it still wastes
embed-server work and pollutes the code index with dependency source.

**Goal:** make the code index honour `[ignored_paths]` (defaults included), so
dependency/artifact trees are excluded by default — reusing the one mechanism
users already know, applied identically in the indexer **and** the preflight guard
so they cannot drift (the `2026-06-02` lesson).

## Non-goals

- Expanding `default_ignored_patterns()` (e.g. `site-packages`, `*.pt`,
  `**/models/`) — kept as a future tracker item.
- Changing the librarian's plain-globset matcher (it shares the pattern *list* but
  keeps its own matcher; its latent bare-name-vs-nested inconsistency is noted in
  the tracker, not fixed here).
- Any new config surface. `[ignored_paths]` and its serde defaults are unchanged.

## Design

### 1. Shared ignore matcher (lockstep guarantee)

Add one shared primitive next to `lang_for_ext` so the indexer and guard cannot
diverge on ignores (mirroring how `lang_for_ext` unified the allowlist):

```rust
// src/embed/mod.rs
pub fn build_ignore_matcher(root: &Path, patterns: &[String])
    -> ignore::gitignore::Gitignore
```

Builds a `Gitignore` via `GitignoreBuilder::new(root)` + one `add_line(None, p)`
per pattern, then `.build()`. Invalid patterns fail-soft (logged, skipped) so a bad
entry never aborts indexing. Both `stream_index` and `check_index_scope` build
their walk filter from **this one function**.

### 2. Matching + pruning (gitignore semantics)

Both walkers gain a `.filter_entry(move |entry| ...)` closure that consults the
shared matcher: a path whose match `.is_ignore()` is skipped. Because
`filter_entry` prunes **directories**, the walker never descends into an ignored
dir — a 50k-file dep tree costs ~one stat, not 50k walks. Bare names match at any
depth (gitignore semantics), so the existing defaults work unchanged; advanced
users get `*`/`**`.

Matcher paths: build the `Gitignore` rooted at the walk `root`; the `filter_entry`
passes the entry's absolute path + `is_dir`, which `Gitignore` resolves relative to
`root`.

### 3. Defaults, override, `= []`

- Reuse `default_ignored_patterns()` **as-is** (no expansion).
- Existing serde semantics, unchanged: **key absent → defaults apply; explicit
  `patterns = []` → ignore nothing** (index everything). backend-kotlin's `[]` keeps
  indexing everything, now memory-safe via the streaming fix. This is the documented
  per-project override path.

### 4. Plumbing (passing the patterns to `sync_project`)

`sync_project` holds only `RetrievalConfig`, not the project config, so patterns are
threaded in:

- Add `ignore_patterns: Vec<String>` to `SyncOpts` (`src/retrieval/sync.rs`).
  `stream_index` gains a corresponding param and builds the matcher.
- `check_index_scope` (`src/embed/preflight.rs`) gains a `patterns: &[String]` param
  — re-adding what `2026-06-02` dropped, but now applied in **both** the guard and
  the indexer, fed the same list.
- Callers populate from `p.config.ignored_paths.patterns`:
  - `src/tools/semantic/index.rs` — the `index` tool (both the preflight call and
    the `SyncOpts`).
  - `src/agent/mod.rs` — `maybe_auto_index_library` (already reads `p.config`).
- `src/bin/sync_project.rs` (CLI) — load `<root>/.codescout/project.toml`
  `ignored_paths` if present, else `default_ignored_patterns()`.

### 5. Self-healing migration

No forced reindex. On the next sync, excluded dirs aren't walked → their chunk-ids
drop out of `local_ids` → the existing streaming `to_delete` path prunes the
now-stale dependency chunks automatically. Polluted indexes clean themselves on the
next pass.

## Testing

- `build_ignore_matcher`: bare name matches a nested dir
  (`a/b/node_modules/x.py` ignored); a glob pattern (`**/*.gen.rs`) matches; empty
  list matches nothing.
- `stream_index` (extends the existing fake-store/fake-embedder harness): a fixture
  with `node_modules/` and `.venv/` subdirs → those files are neither embedded nor
  appear in `local`; with `ignore_patterns=[]` everything is indexed.
- Lockstep regression guard: `check_index_scope` and `stream_index` agree on the
  eligible-file set for the same tree + patterns (directly guards the `2026-06-02`
  divergence class).

## Incidental fixes (this work touches them)

- Correct the `2026-06-19` OOM issue's per-project mitigation note — setting
  `[ignored_paths]` was ineffective for the code index before this change.
- Update `src/embed/preflight.rs:100`'s comment claiming the indexer "never
  honoured" `ignored_paths` (it now does).

## Affected files

- `src/embed/mod.rs` — new `build_ignore_matcher`.
- `src/retrieval/sync.rs` — `SyncOpts.ignore_patterns`; `stream_index` builds +
  applies the matcher; tests.
- `src/embed/preflight.rs` — `check_index_scope` regains `patterns` param + applies
  matcher; comment fix.
- `src/tools/semantic/index.rs` — pass patterns to preflight + `SyncOpts`.
- `src/agent/mod.rs` — pass patterns in `maybe_auto_index_library`.
- `src/bin/sync_project.rs` — load project `ignored_paths` (or defaults).
- `docs/issues/2026-06-19-mcp-server-oom-68gb.md` — mitigation note fix.
