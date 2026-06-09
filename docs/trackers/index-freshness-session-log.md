---
kind: tracker
status: active
title: Index Freshness Signal — Session Log
owners: []
tags: [index, retrieval, freshness, autoindex, sync_project]
---

# Index Freshness Signal — Session Log

Work-stream log for the `.codescout/index-state.json` freshness signal + companion
auto-reindex (the O-1 design in
`docs/trackers/2026-06-09-index-freshness-signal-for-consumers.md`, id `286ac62b5a821cec`).

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-06-09 | med | architectural | fixed-verified | Sidecar write at one of 3 `sync_project` project entry points; CLI + bin silently wrote nothing |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-06-09 | high | Live verification through the real entry point + `references()` entry-point audit | 46 green unit tests; feature dead in the CLI path the hook uses | validated |

---

## F-1 — Sidecar write at one call site; CLI + bin entry points silently wrote nothing

**Observed:** 2026-06-09, after committing scope-a (`b5d63cb6`); running the live CLI proof.

**When:** Verifying the index-freshness sidecar end-to-end via `codescout index` (CLI).

**Expected:** `codescout index` writes `.codescout/index-state.json`.

**Got:** `added=347 … No such file or directory`. The write lived only in `IndexProject::call` (MCP path). `references(RetrievalClient/sync_project)` revealed 5 call sites (3 project: `index.rs:304` MCP, `main.rs:259` CLI, `bin/sync_project.rs:29`; 2 library) — only the MCP site recorded freshness.

**Probable cause:** Side-effect placed at a *caller* instead of the operation chokepoint; unit tests bypass `main.rs`, so the gap was invisible to a green suite.

**Workaround:** Pre-fix, only MCP `index(action="build")` wrote the sidecar.

**Severity:** med — caught pre-master by the live proof; would otherwise have shipped dead in the companion-hook path.

**Status:** fixed-verified — write moved to `sync_project`, gated by `SyncOpts.record_index_state`; live-verified `behind:1 → reindex → up_to_date`. Commit `10dcfb9f`.

**Fix idea / Pointer:** `docs/issues/2026-06-09-index-state-write-missed-cli-bin-entry-points.md`; recon R-21.

---

## W-1 — Live verification through the real entry point caught what 46 unit tests missed

**Observed:** 2026-06-09, index-freshness scope-a verification.

**Pattern:** Verify a side-effect through its actual production entry point (the CLI/MCP path the consumer uses), AND enumerate ALL entry points with `references()` on the operation before placing the effect — don't trust the unit harness (which bypasses `main.rs`) or the single call site in front of you.

**Counterfactual:** Scope-a passed 46 unit tests + a hook functional test, all green — and was dead in the CLI path the companion hook invokes. Without the live `codescout index` run the feature ships broken: the companion auto-reindex never fires, and the bug surfaces only as silently-stale search results with no error. `references(sync_project)` turned a "2 entry points" assumption into the real 5 (3 project / 2 library), including a standalone bin nobody remembered.

**Confirming data points:**
1. F-1 (this session) — live CLI proof exposed the empty sidecar; `references()` found the 3rd project entry point.
2. The chokepoint fix verified live through the reconnected MCP server (`git_sync` `behind:1` → `up_to_date`).

**Impact:** high — converts "tests green = done" into "verified through the real path = done"; closes the silent-dead-feature class.

**Promote-when:** a second instance where live-entry-point verification or a `references()` entry-point audit catches a gap unit tests missed → promote a verification-discipline bullet to CLAUDE.md.

**Status:** validated — single strong datapoint; fix is live-verified.

---

## Status vocabulary

Friction: `open | mitigated | fixed-verified | wontfix-false-alarm | promoted-to-bug-tracker | pinned-as-eval-baseline`.
Win: `validated | promoted-to-permanent-docs | archived`.

## Template for new entries

<!-- Insert new F-N / W-N entries above this line; update the Index / Wins Index tables. -->
