---
id: e973bdaec27f9fdb
kind: bug
status: fixed
title: 'BUG: /tmp probe/test runs write artifacts into the real shared global catalog — 28 dead /tmp rows, 3 still active+augmented trackers'
tags:
- catalog
- test-isolation
- librarian
closed: 2026-07-20
---

## Summary

The shared global catalog (`~/.local/share/librarian/catalog.db`) contains **28 rows whose `abs_path` is under `/tmp`** (per `librarian(action="doctor")`, 2026-07-17). Three of them are `kind=tracker, status=active` **with augmentation rows attached**: `/tmp/tmp.2SFGgbxwpM/probe.md`, `/tmp/tmp.JPX56poL6D/probe.md`, `/tmp/tmp.eiF4hTZrXy/probe.md` (ids `2132e820a2ff9ebd`, `a606cb4c38ff0205`, `3b8c6c17ce186cca`; created ~2026-05-16). The `mktemp`-style dirs are long gone.

## Why it's a bug, not just junk

Some probe/test/experiment flow ran against the **production** catalog instead of an isolated one — the write side had no guard against cataloging artifacts under temp roots. Consequences:

- These rows are `active`, so they surface in `scope="all"` finds, semantic search (they have vec chunks if embedded), and augmentation staleness lists — ghost results with no file behind them.
- They join a wider population of 179 `missing_file` rows (77 `stefanini/AI-enablement`, 52 `stefanini/IATA`, 11 `stefanini/southpole`, 5 `mirela/deployment`, 6 other) — but the /tmp subset is distinct: those never should have been written at all (the others are deleted/renamed repos, tracked separately in `2026-07-17-catalog-dead-rows-no-gc.md`).

## Discovered

2026-07-17, while auditing augmentation coverage across the catalog for the tracker-management redesign survey (`SELECT ... FROM artifact_augmentation JOIN artifact ...` showed the three /tmp trackers among 54 augmented artifacts).

## Fix directions

1. **Cleanup:** `librarian(action="doctor", fix="prune_missing", root=...)` per dead /tmp root — awkward, since each `tmp.XXXX` dir is its own root; may need a batch path or root=/tmp support (refused today if path exists? /tmp exists — check semantics).
2. **Prevention:** refuse (or route to an ephemeral catalog) any `artifact(create)` / reindex whose target root is under the system temp dir, unless explicitly opted in; and/or make test harnesses set an isolated catalog path (cf. `docs/conventions/test-env-isolation.md`).

## Status log
- 2026-07-19 — Complementary **ongoing-GC lifecycle shipped on `experiments`** (see [[2026-07-17-catalog-dead-rows-no-gc]]): any future dead rows (incl. stray temp-rooted ones that slip past the temp-write guard) now get `missing_since`-stamped and hidden-from-find past a 14-day grace, and are surfaced in `doctor` `catalog_health`. Does not change this bug's disposition — its prevention + cleanup were already done; still `open` pending the shared cherry-pick to master.

- 2026-07-17 — opened. Read-only survey; no cleanup performed.
- 2026-07-18 — **PREVENTION shipped** on `experiments` (not yet master): a temp-write guard now refuses artifact create/reindex when the workspace root is under the OS temp dir AND the catalog is the real/shared one (file-backed, outside temp) — `src/librarian/tools/temp_write_guard.rs`, wired into create + reindex; env escape hatch `CODESCOUT_ALLOW_TEMP_WORKSPACE=1`. Also fixed a latent test-isolation gap (`tests::make_server` used the real catalog) by isolating it under `.codescout/`. **CLEANUP capability shipped** (see the batch `doctor(fix=prune_missing)` dry-run/apply in [[2026-07-17-catalog-dead-rows-no-gc]]) but NOT yet executed — the 28 /tmp rows still need a gated dry-run→confirm→apply against the real catalog (needs `cargo rb` + /mcp reconnect first). Spec docs/superpowers/specs/2026-07-17-catalog-hygiene-prevention-cleanup-design.md; plan docs/superpowers/plans/2026-07-18-catalog-hygiene-prevention-cleanup.md. Kept `open` until cleanup runs + ships to master.
- 2026-07-18 (cont.) — **CLEANUP DONE + SOURCES FIXED.** Pruned all 31 dead `/tmp` roots via per-root `doctor(fix=prune_missing, root=...)` (46 artifact + 6 commit rows, incl. the 3 augmented probe trackers `tmp.2SFGgbxwpM/JPX56poL6D/eiF4hTZrXy` and 18 codescout-test-suite fixtures); verified 0 orphaned augmentations/links/events (FK cascade + vec0 trigger). Note: raw sqlite3 CANNOT delete artifact rows (the artifact_vec cascade trigger needs the vec0 module only codescout loads) — must prune via the librarian tool. **Sources corrected:** (a) codescout test suite — `make_server` was using the real catalog, fixed by `.codescout/`-isolation (commit 7141ac6e); (b) prompt-engineering harness CLI path — `LIBRARIAN_DB` per-scenario isolation (prompt-engineering commit f33b6a6); (c) harness SDK+MCP path (`_open_mcp_session`) — exact `env=` fix documented, user folding into in-progress WIP. Remaining `open` reason: not yet on master.
