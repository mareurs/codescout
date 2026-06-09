---
specialist: architecture-snow-lion
scope: project
slug: cross-cutting-side-effects-at-the-chokepoint
created: 2026-06-09
updated: 2026-06-09
tags: [chokepoint, side-effects, entry-points, references-audit, cross-cutting, project-philosophy]
---

**Lesson:** A side-effect tied to "whenever operation X happens" belongs *inside* X's single chokepoint function, gated by an explicit intent flag (default = the safe value) — NOT scattered at each call site. Find the chokepoint and audit entry-point coverage with `references()`, not by editing the call site in front of you.

**Why:** The index-freshness sidecar (`write_index_state`) was first placed in `IndexProject::call` (the MCP tool path) only. But `sync_project` has **three** *project* entry points — the MCP index tool, the CLI `codescout index` (`src/main.rs`), and `src/bin/sync_project.rs` — plus two library syncs. The CLI (which the codescout-companion session-start hook actually invokes) and the standalone bin silently wrote nothing: the feature passed 46 green tests and was **dead in its primary path** at the same time. `references(RetrievalClient/sync_project)` surfaced all 5 sites in one query; a live `codescout index` run (reported added/deleted but produced no `index-state.json`) exposed the gap that the unit harness — which bypasses `main.rs` — could not. Moving the write into `sync_project`, gated by `SyncOpts.record_index_state` (default false; the 3 project sites opt in, the 2 library sites stay false so library checkouts aren't polluted with a `.codescout/` dir), fixed the whole class (commit `10dcfb9f`, correcting `b5d63cb6`). Third datapoint for this project's chokepoint discipline — cf. [[outputguard-cross-cutting-law]] (one chokepoint for progressive disclosure) and [[tool-registration-rule-of-three]] (three sites earn the centralization).

**How to apply:** When adding a side-effect that must fire "on every X": (1) `references()` the function that *defines* X to enumerate ALL callers — never trust the one in front of you; (2) classify them — which want the effect, which must opt out; (3) put the effect in the chokepoint, gated by an explicit flag on the opts struct; (4) default the flag to the **safe** value (here `false`, so foreign/library callers aren't polluted, and a forgotten *exhaustive-literal* call site fails to compile rather than silently misbehaving — `..Default::default()` sites inherit the safe default); (5) verify through a **real entry point** (the CLI/MCP path the consumer uses), not only a unit test that bypasses `main.rs`. Tests prove one path works; `references()` + a live run prove you covered all paths.
