---
status: fixed
opened: 2026-06-02
closed: 2026-06-02
severity: low
owner: marius
related:
  - docs/issues/2026-06-02-markdown-chunks-collection-vestigial.md
tags:
  - embeddings
  - indexing
  - preflight
  - scanning
kind: bug
---

# BUG: index preflight scope-guard walks a different tree than the indexer

## Summary
The semantic-index preflight guard (`check_index_scope`) — which gates indexing
behind a user-confirmation prompt for oversized / system-path roots — walked the
tree with **different rules** than the actual indexer (`sync_project`). Its
file-count and byte estimate therefore did not reflect what actually gets
indexed, so the guard could wave through a root the indexer then over-indexes.
Fixed by extracting a single shared extension allowlist (`lang_for_ext`) and
aligning the guard's walker to the indexer's.

## Symptom (Effect)
Not a crash — a correctness/accuracy defect in the guard. Four concrete
divergences between `check_index_scope` (`src/embed/preflight.rs`) and
`sync_project` (`src/retrieval/sync.rs`):

1. **Hidden files:** guard used `.hidden(true)` (skips dotfiles/dotdirs); indexer
   uses `.hidden(false)` (indexes tracked dotfiles, gitignore handles exclusions).
   → guard *under-counts* tracked dotfiles the indexer embeds.
2. **`ignored_paths`:** guard applied `p.config.ignored_paths.patterns` as a
   filename filter; the indexer applies **none** of it. → guard subtracts paths
   the indexer keeps.
3. **Extension allowlist:** guard counted **every** file's bytes; the indexer
   only embeds an extension allowlist (`rs, py, ts, …, md, toml`). → guard
   *over-counts* binary blobs / non-source files.
4. **Stale docstring:** the guard's docstring claimed it walked "matching
   `build_index`'s walker", but `build_index` no longer exists (renamed to
   `sync_project` in `edeaa96c`); the two were never reconciled.

Net effect: the guard's "Eligible files / Approx source content" estimate (the
numbers shown in the confirmation dialog, and the `max_index_bytes` threshold
comparison) measured a different file set than the indexer acts on.

## Reproduction
Code inspection at `git rev-parse HEAD` on branch
`feat/per-request-workspace-pinning`. Compare the two walkers:

```
symbols(name="check_index_scope", path="src/embed/preflight.rs", include_body=true)
symbols(name="impl crate::retrieval::client::RetrievalClient", path="src/retrieval/sync.rs", include_body=true)
```

and the call site that passes `ignored_paths` only to the guard:

```
grep -n "ignored_paths" src/tools/semantic/index.rs   # → preflight only, never sync_project
```

## Environment
Linux, Rust, codescout v0.14.0, MCP stdio. Project: code-explorer. Branch:
`feat/per-request-workspace-pinning`.

## Root cause
Two independently-authored tree walkers with two independently-authored notions
of "what counts", which drifted apart over time:

- `src/embed/preflight.rs::check_index_scope` (pre-fix): `WalkBuilder::new(root)
  .hidden(true).git_ignore(true).filter_entry(exclude ignored_paths)`, counting
  **all** files regardless of extension.
- `src/retrieval/sync.rs::sync_project`: `WalkBuilder::new(root).hidden(false)
  .build()`, with an inline `match ext { … }` extension allowlist and **no**
  `ignored_paths` filter.

There was no shared source of truth for "which files get embedded", so the
guard's estimate and the indexer's behaviour could not be kept in agreement.

## Evidence
### The `ignored_paths` config reaches only the guard
`src/tools/semantic/index.rs` computed `p.config.ignored_paths.patterns` and
passed it to `check_index_scope` (preflight) at the project-scope branch, while
`client.sync_project(&project_id, &root, opts)` received no such list. `grep`
for `ignored_paths` across `src/` confirms: `config/project.rs` (definition),
`embed/preflight.rs` (param), `tools/semantic/index.rs` (preflight call) — never
`retrieval/sync.rs`.

### `build_index` does not exist
`symbols(name="build_index", path="src")` → 0 matches. Only docstring/comment
references remained (in `preflight.rs`), plus an unrelated test fixture string in
`ast_chunker.rs`.

## Hypotheses tried
1. **Hypothesis:** codescout's own `.codescout/` files pollute the index
   (guard skips them via `ignored_paths`, indexer doesn't).
   **Test:** semantic_search for near-verbatim memory text.
   **Verdict:** rejected — `.codescout` content *is* walked into `code_chunks`
   but tagged `language=markdown`, which the default search hides; no *visible*
   pollution. (Folded into the related markdown-collection finding.)
2. **Hypothesis:** markdown is not indexed at all (chunker drops it).
   **Test:** doc-only semantic_search probes (verbatim from CLAUDE.md /
   PROGRESSIVE_DISCOVERABILITY.md).
   **Verdict:** rejected — a file-count census (682 `.md` tracked vs index
   `file_count`=1062) proved markdown *is* indexed; the probes were blind because
   the default `mode="code"` adds a `must_not language=markdown` clause.
3. **Hypothesis (the real one):** the guard and the indexer walk the same tree.
   **Test:** read both `WalkBuilder` configs + the `ignored_paths` wiring.
   **Verdict:** confirmed divergence — see Root cause.

## Fix
Single source of truth + walker alignment:

- **`src/embed/mod.rs`** — new `pub fn lang_for_ext(ext: &str) -> Option<&'static str>`:
  the one extension→language allowlist, shared by indexer and guard.
- **`src/retrieval/sync.rs`** — `sync_project` now calls `crate::embed::lang_for_ext(ext)`
  instead of its inline `match`.
- **`src/embed/preflight.rs`** — `check_index_scope` walks `.hidden(false)` +
  `.git_ignore(true)` and counts only files where `lang_for_ext(ext).is_some()`;
  the `ignored_paths` parameter is dropped (the indexer never honoured it, and
  gitignore + the extension allowlist already exclude `.git`/binaries/`target`);
  docstring rewritten to reference `sync_project`.
- **`src/tools/semantic/index.rs`** — caller no longer computes/passes the
  `ignored` list.

Uncommitted on `feat/per-request-workspace-pinning` as of 2026-06-02 — record the
**master-side** SHA here after cherry-pick (see CLAUDE.md § "After cherry-pick").

## Tests added
All in `src/embed/preflight.rs` `tests`:

- `check_index_scope_counts_only_indexable_extensions` — an 8 KB `.bin` blob is
  excluded; only a 13 B `.rs` counts → under threshold → Clear. (Red pre-fix: old
  code counted the blob and tripped the threshold.)
- `check_index_scope_counts_hidden_non_gitignored_files` — a hidden `.config.toml`
  must be counted, proving `hidden(false)`. (Red pre-fix: `hidden(true)` skipped
  it → file_count 0 → Clear.)
- `check_index_scope_respects_gitignore` — strengthened to gitignore an
  *indexable* (`.rs`) file, so only the gitignore rule (not the extension filter)
  can exclude it.

Red-before/green-after is by construction (reasoned, not re-run against pre-fix
code — the fix had already landed). Verified green: `cargo test --lib` (2575
passed, 7 ignored); `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt`
clean.

## Workarounds
N/A — guard-accuracy defect; no user action was required to work around it.

## Resume
N/A (fixed). Related, **not** addressed by this fix: `sync_project` still indexes
non-gitignored `.codescout/` and `.claude/` markdown into `code_chunks` (visible
only via `semantic_search(mode="full")`), because neither walker excludes those
dirs by basename — `ignored_paths` is still unused by the indexer. That is a
separate decision (honour `ignored_paths` in `sync_project` vs. accept the
co-mingling); see the related markdown-collection bug file.

## References
- `src/embed/mod.rs` (`lang_for_ext`)
- `src/embed/preflight.rs` (`check_index_scope`)
- `src/retrieval/sync.rs` (`sync_project`)
- `src/tools/semantic/index.rs` (caller)
- `edeaa96c` — commit that added the `md|mdx`/`toml` allowlist and renamed the walker
- docs/issues/2026-06-02-markdown-chunks-collection-vestigial.md
