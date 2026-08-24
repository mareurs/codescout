---
status: active
tags:
- indexing
- config
- memory
- ux
opened: 2026-06-30
owner: marius
related:
- docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md
- docs/issues/archive/2026-04-18-memory-leak-x-session-freeze.md
kind: tracker
---

# Tracker: indexing scope — default-ignore globs (deferred)

## Why this exists
Deferred follow-up from the 68 GB OOM (`docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md`). The
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
`docs/issues/archive/2026-06-02-preflight-sync-walker-divergence.md`):
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

**Default set EXPANDED (2026-07-13, under `docs/issues/archive/2026-07-10-oom-blast-radius-cgroup-cap.md` item 3).**
`default_ignored_patterns()` went from 10 → 25 entries. Added: `venv`, `site-packages`,
`.mypy_cache`, `.pytest_cache`, `.ruff_cache`, `.tox`, `.nox`, `.next`, `.nuxt`, `.svelte-kit`,
`.turbo`, `.parcel-cache`, `.gradle`, `.terraform`, `.dart_tool`, `.stack-work`, `.direnv`, `.idea`.

### The "default-on vs opt-in" question is now ANSWERED — by a criterion, not a vote

This tracker's open question was *"Default-on risks silently skipping code a user wants indexed."*
The resolution is a rule that makes the answer mechanical:

> Because the code index compiles these with **gitignore semantics**, a bare name prunes at **any
> depth**. Therefore a default may only name a directory that **cannot plausibly contain
> hand-authored source.**

Under that rule, default-on is safe — *for entries that pass it*. The entries that fail it stay
opt-in, permanently.

### Candidates from this tracker that were REJECTED

- **`models`** — proposed above, but it is a very common *source* directory (Django / Rails / domain
  models). At any depth it would silently prune `src/models/`. This is exactly the failure the
  tracker warned about, and the proposal contained it.
- **`checkpoint-*`** — not shipped. Plausible enough, but unlike the caches it is a *glob*, and the
  list is currently bare-name/directory-oriented. Revisit with the glob work below.
- **`vendor` / `bin` / `obj` / `out`** — legitimate source dirs in some ecosystems.
- **`*.pt` / `*.safetensors` / `*.onnx` / `*.gguf`** — already excluded from *embedding* by the
  `lang_for_ext` allowlist, so listing them saves walk time but not memory. Not the OOM lever they
  look like.

The rejections are pinned by a regression test
(`default_ignores_cover_dependency_trees_but_never_plausible_source_dirs`, `src/config/project.rs`),
not just by comment — a future edit cannot quietly re-add `models` and start dropping source.

**Still open (this tracker):**
- **Librarian matcher inconsistency** — UNCHANGED and now more consequential: the librarian markdown
  indexer shares the `[ignored_paths]` *list* but compiles it with plain `globset::Glob`
  (`compile_ignore`), so bare names do **not** match nested dirs there the way the code index's
  gitignore matcher does. The 15 new defaults therefore have **narrower reach in the librarian than
  in the code index**. Unify or document.
- **File-extension / glob patterns** (`checkpoint-*`, weight files) — needs the list to grow beyond
  bare directory names first.
- **Un-ignore path** — still no way to *remove* a built-in default per project. Not urgent while the
  defaults obey the criterion above, but it is the escape hatch if one ever proves wrong.
- Aggregate walk/embed budget; cgroup `MemoryMax`/`MemorySwapMax=0` cap (now drafted in
  `contrib/systemd/README.md`); `oom_score_adj=200` review (**done** — inherited from the terminal's
  KDE `app.slice` scope, not set by codescout, and the bias is correct; see the same README).
