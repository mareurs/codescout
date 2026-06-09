---
status: fixed
opened: 2026-06-09
closed: 2026-06-09
severity: medium
owner: marius
related: []
tags: [index, retrieval, freshness, entry-points, sync_project]
kind: bug
---

# BUG: index-state sidecar written at one `sync_project` call site — dead in CLI + bin paths

## Summary
The index-freshness sidecar (`.codescout/index-state.json`) was written only inside `IndexProject::call` (the MCP `index` tool). `sync_project` has three *project* entry points; the CLI `codescout index` (which the companion session-start hook invokes) and the standalone `src/bin/sync_project.rs` wrote no sidecar. The feature was dead in its primary path while passing every unit test.

## Symptom (Effect)
After `b5d63cb6`, the release CLI:
```
$ codescout index --project <root>
added=347 updated=0 deleted=180 elapsed_ms=11795
$ cat <root>/.codescout/index-state.json
cat: .../.codescout/index-state.json: No such file or directory
```
Index reported success; no sidecar produced. `index(action="status")` emitted no `git_sync` envelope for CLI/bin-indexed projects.

## Reproduction
1. Check out `b5d63cb6` (pre-fix).
2. `cargo build --release`
3. `target/release/codescout index --project <root>`  (or `target/release/sync-project <root>`)
4. `ls <root>/.codescout/index-state.json` → absent.
Only the MCP `index(action="build")` path produced the sidecar.

## Environment
Linux; Rust; codescout `experiments`; retrieval stack (Qdrant + dense/sparse embedders) up.

## Root cause
The side-effect was placed at a single *caller* (`src/tools/semantic/index.rs` `IndexProject::call` success arm) instead of at the operation's chokepoint. `references(RetrievalClient/sync_project)` shows 5 call sites: 3 project — `index.rs:304` (MCP), `src/main.rs:259` (CLI), `src/bin/sync_project.rs:29` — and 2 library — `index.rs:130`, `src/agent/mod.rs:1493`. Only the MCP project site recorded freshness; the CLI and bin sites did not.

## Evidence
Live CLI run printed `added=347 …` then `No such file or directory` for the sidecar (see Symptom). 46 unit tests + a hook functional test passed because none exercised `main.rs`/`bin` — they tested `write_index_state`/`git_sync_status` directly or the MCP path.

## Hypotheses tried
1. **Hypothesis:** the sidecar write in `IndexProject::call` covers all index paths. **Test:** live CLI `codescout index`. **Verdict:** rejected — the CLI calls `sync_project` directly via `main.rs:259`, bypassing `IndexProject::call`. **Evidence:** Symptom.
2. **Hypothesis:** there are 2 project entry points. **Test:** `references(RetrievalClient/sync_project)`. **Verdict:** rejected — 3 project + 2 library; discovered the standalone `src/bin/sync_project.rs`.

## Fix
Moved the write into `sync_project` (the chokepoint), gated by `SyncOpts.record_index_state` (default false). The 3 project sites set it true; the 2 library syncs leave it false (no library-checkout pollution). Removed the scattered write from `IndexProject::call`. Implementation: `src/retrieval/sync.rs` (struct field + gated write), `src/tools/semantic/index.rs`, `src/main.rs`, `src/bin/sync_project.rs`. Commit **`10dcfb9f`** (experiments-side; **not yet on master** — cite the master SHA here after cherry-pick). Architecture per the Snow Lion ADR (one chokepoint for a cross-cutting side-effect).

## Tests added
`src/retrieval/index_state.rs` unit tests (three-query sandwich: write → fresh → move HEAD → assert behind → reindex → fresh) cover write/read/`git_sync_status`. Entry-point *coverage* was verified by a live run, not a unit test — the gap was precisely that unit tests bypass `main.rs`. Live proof: `codescout index` (CLI) → sidecar == HEAD; reconnected MCP `index(action="status")` → `git_sync` `behind:1` → after reindex `up_to_date`.

## Workarounds
Pre-fix: use the MCP `index(action="build")` tool (the only path that wrote the sidecar). Post-fix: none needed.

## Resume
N/A — fixed and live-verified. Archive to `docs/issues/archive/` after `10dcfb9f` ships to `master` (`git branch --contains <fix-sha>` shows `master`).

## References
- Tracker: `docs/trackers/2026-06-09-index-freshness-signal-for-consumers.md` (286ac62b)
- Session-log: `docs/trackers/index-freshness-session-log.md` (F-1 + W-1)
- Recon: `docs/trackers/reconnaissance-patterns.md` R-21
- Snow Lion memory: `cross-cutting-side-effects-at-the-chokepoint`
- Commits: `b5d63cb6` (incomplete placement), `10dcfb9f` (chokepoint fix)
