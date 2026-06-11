---
id: null
kind: null
status: archived
title: null
owners: []
tags: []
topic: null
time_scope: null
---
# Session Log — kotlin-lsp analyzer disk growth

> **Purpose:** Two-sided observation log for the kotlin-lsp unbounded-disk
> work stream (bug: `docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md`).
> Captures frictions (F-N) and wins (W-N) so future sessions inherit the lesson.
>
> **How to use:** Append F-N / W-N entries via
> `edit_markdown(action="insert_before", heading="## Template for new entries", content=...)`.
> Add a row to the Index / Wins Index table for each new entry.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-06-03 | high | architectural | open | Analyzer dir keyed by 128-bit hash ≠ codescout's 64-bit `ws_hash` → cleanup-by-ws_hash fix not viable |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-06-03 | high | Validate an external-tool lever with a black-box probe before implementing the fix | the XDG fix would have shipped as a silent no-op | validated |

---

## Category conventions

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool |
| `subagent` | Subagent produced unexpected output or diverged |
| `plan-prose` | Plan / bug doc drift vs reality (wrong paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted friction that turned out to be a false alarm |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

---

## F-1 — Analyzer dir keyed by 128-bit hash ≠ codescout's 64-bit `ws_hash` → cleanup-by-ws_hash fix not viable

**Observed:** 2026-06-03, systematic-debug pass on the kotlin-lsp unbounded-disk bug, scouting fix shapes before implementing.

**When:** About to evaluate the bug file's proposed fix #2 ("on idle-timeout shutdown / workspace deactivate, remove *that workspace's* analyzer dir"), which presumes codescout can locate the analyzer dir for a given workspace.

**Expected (bug file):** `docs/issues/2026-06-01-...md` Evidence section states the analyzer "Workspace `<hash>` matches codescout's `workspace_hash(workspace_root)` granularity (per worktree)" — implying codescout's own `ws_hash` can address the analyzer dir.

**Got (scouted reality):**
- Live `--system-path` dirs (codescout's `ws_hash`): **16 hex chars** — `c85ec91bdbfd1aee`, `26a9e85d58931839`, `7e868829c00fa9b2`. Source: `src/socket_discovery.rs:10` `workspace_hash` = `DefaultHasher` (SipHash, 64-bit) → `format!("{:016x}", …)`.
- Live analyzer dirs (`~/.config/JetBrains/analyzer/workspaces/*`): **32 hex chars** — e.g. `b45f9bc4ce063fea7ec368df0a904da6`, `4eed18c1fe54c6450704528cd69e7597` (128-bit, almost certainly MD5-of-path per IntelliJ convention — *unconfirmed*).
- **None** of the 3 live `--system-path` hashes appear among the 8 analyzer dirs. Different hash *function* and *width* over the workspace path — not a shared key.

**Probable cause:** The analyzer index is an IntelliJ-internal store that computes its own project-path hash; codescout never influenced it, so the granularity *(per worktree)* coincides but the key *value* does not. The bug file conflated "same granularity" with "same/derivable key."

**Workaround:** Re-rank the fixes. Fix #2 (targeted cleanup keyed on `ws_hash`) is **not viable** without replicating IntelliJ's path-hash (fragile, version-coupled). The viable shapes are: (#1) redirect the analyzer base via env so codescout *owns* the path — *pending empirical proof that the analyzer honors `XDG_CONFIG_HOME` or another base-dir lever*; or (#4) coarse sweep of `~/.config/JetBrains/analyzer/workspaces/*` when no kotlin-lsp JVM is live. Corrected the bug file's Evidence + Proposed-fix framing.

**Severity:** high — had the cleanup-by-ws_hash fix been designed, implemented, and tested against codescout's own hash, it would have silently located *nothing* in production (or, if it tried to map, the *wrong* dirs), shipping a "fix" while the disk kept filling. Caught at recon before any code.

**Status:** open

**Fix idea / Pointer:** `docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md` (bug); next empirical step = prove/disprove `XDG_CONFIG_HOME` redirect for the analyzer dir.

---

## W-1 — Validate the redirect lever with a black-box probe BEFORE writing the fix

**Observed:** 2026-06-03, choosing the fix shape for the kotlin-lsp analyzer-disk bug.

**Pattern:** When a fix depends on an external tool honoring an env var / property / flag, prove it with a throwaway black-box probe (launch the tool with the candidate setting; observe the filesystem or a real request's response) BEFORE implementing. Then implement only the lever that empirically worked.

**Counterfactual:** The bug file's leading fix candidate was `XDG_CONFIG_HOME`. The XDG probe showed it (and `idea.config.path`) is IGNORED — the analyzer resolves `user.home`. Without the probe, the natural path was to set `XDG_CONFIG_HOME` in the kotlin branch, ship it, and watch the disk keep filling (silent no-op), then re-debug from scratch. The probe cost ~2 min and converted "plausible fix" → "proven-dead", redirecting to `-Duser.home` (validated: index moved AND hover still inferred `Int`).

**Confirming data points:**
1. XDG probe — `XDG_CONFIG_HOME`/`idea.config.path` proven ineffective; jar scan confirmed the resolver keys off `user.home`/`APPDATA`, no XDG key, no override property.
2. `-Duser.home` func probe — redirect works AND type resolution intact (`val x: Int`).
3. Integration probe — the real release mux reclaims the redirected home on idle-shutdown.

**Impact:** high — prevented shipping a no-op fix to a disk-filling bug; the probes also de-risked the riskier `-Duser.home` blast-radius before any code landed.

**Promote-when:** a second fix that hinged on an unverified external-tool toggle where a pre-implementation probe changed the chosen lever. At 2 datapoints, promote to CLAUDE.md (pairs with recon R-15).

**Status:** validated

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top.

     F-N fields: Observed / When / Expected / Got / Probable cause /
       Workaround / Severity (low|med|high) / Status / Fix idea.
     W-N fields: Observed / Pattern / Counterfactual / Confirming data
       points / Impact / Promote-when / Status.
     Status vocab — friction: open | wontfix-false-alarm | mitigated |
       fixed-verified | promoted-to-bug-tracker | pinned-as-eval-baseline.
     Win: validated | promoted-to-permanent-docs | archived. -->
