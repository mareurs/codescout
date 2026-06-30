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
gate skips oversized roots — but neither *excludes* dependency/artifact trees by default. A large
un-ignored tree (e.g. backend-kotlin's `python-services/` with `[ignored_paths] patterns = []`) is
still walked and embedded in full: wasted embed-server time and a code index polluted with
dependency source. This tracker holds the UX-policy decision we explicitly deferred, not the crash
fix (that ships under the OOM issue).

## Proposal (not yet decided)
Add a conservative built-in default-ignore set to the indexer walk. Touch points that must stay in
lockstep (the guard estimate and the actual walk previously diverged — see
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
Open — captured 2026-06-30, deferred during the streaming-fix work stream. **Not blocking** the OOM
fix: streaming `sync_project` + the background scope gate resolve the crash on their own.
