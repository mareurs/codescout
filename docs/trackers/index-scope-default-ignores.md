---
status: open
opened: 2026-06-30
owner: marius
tags: ["indexing", "config", "memory", "ux"]
related: ["docs/issues/2026-06-19-mcp-server-oom-68gb.md", "docs/issues/2026-04-18-memory-leak-x-session-freeze.md"]
---

# Tracker: indexing scope — default-ignore globs (deferred)

## Why this exists
Deferred follow-up from the 68 GB OOM (`docs/issues/2026-06-19-mcp-server-oom-68gb.md`). The
streaming `sync_project` fix makes indexing OOM-safe (O(batch) peak) and the background preflight
gate skips oversized roots — but at the time neither *excluded* dependency/artifact trees by default
(now wired — see **Status**). A large
un-ignored tree (e.g. backend-kotlin's `python-services/` with `[ignored_paths] patterns = []`) is
still walked and embedded in full: wasted embed-server time and a code index polluted with
dependency source. This tracker holds the UX-policy decision we explicitly deferred, not the crash
fix (that ships under the OOM issue).

## Proposal (not yet decided)
**Wiring is done** (see **Status**). The remaining decision is whether to **expand** the default set
beyond the current bare dir names. Touch points that must stay in lockstep (the guard estimate and
the actual walk previously diverged, now unified via `build_ignore_matcher` — see
`docs/issues/2026-06-02-preflight-sync-walker-divergence.md`):
- `RetrievalClient::sync_project` (`src/retrieval/sync.rs`) — the actual walk.
- `check_index_scope` (`src/embed/preflight.rs`) — the preflight size estimate.
- `lang_for_ext` (`src/embed/mod.rs`) — the extension allowlist (single source of truth).

Candidate globs: `**/.venv/`, `**/site-packages/`, `**/node_modules/`, `**/models/`,
`**/checkpoint-*/`, `*.pt`, `*.safetensors`, `*.onnx`, `*.gguf`.

Open questions:
- **Default-on vs opt-in?** Default-on risks silently skipping code a user wants indexed.
- **Override path:** per-project `[ignored_paths]` already exists; a built-in default set would
  union with it — need a way to *un*-ignore a default.
- **Overlap with .gitignore:** `WalkBuilder` already honours `.gitignore`, so most of these are
  excluded in normal repos. The real gap is committed/vendored deps or `[ignored_paths] = []`.

## Also deferred (from the OOM issue, parked here for visibility)
- Aggregate walk/embed budget (file count + total bytes) as a hard stop, distinct from the
  per-file `security.max_index_bytes`.
- cgroup `MemoryMax`/`MemorySwapMax=0` blast-radius cap for MCP servers; review `oom_score_adj=200`.

## Status

**Code-index wiring SHIPPED (2026-06-30, plan `abstract-dazzling-peacock`, on `experiments`).**
`sync_project` and `check_index_scope` now honour `[ignored_paths]` via a shared
`build_ignore_matcher` (gitignore semantics; defaults exclude `.venv`/`node_modules`/`target`/etc.).

**Still open (this tracker):**
- **Expand the default set** — `**/site-packages/`, `**/models/`, `**/checkpoint-*/`, `*.pt`,
  `*.safetensors`, `*.onnx`, `*.gguf`. Current defaults are bare dir names only.
- **Librarian matcher inconsistency** — the librarian markdown indexer shares the `[ignored_paths]`
  *list* but compiles it with plain `globset::Glob` (`compile_ignore`), so bare names don't match
  nested dirs there the way the code index's gitignore matcher does. Unify or document.
- Aggregate walk/embed budget; cgroup `MemoryMax`/`MemorySwapMax=0` cap; `oom_score_adj=200` review.
