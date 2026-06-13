---
kind: tracker
status: active
title: Session Log — Dzo Legibility Survey
owners: []
tags:
  - legibility
  - dzo
  - flight-recorder
  - reconnaissance
---

# Session Log — Dzo Legibility Survey

> **Topic:** Machine-legibility survey + refactor campaign driven by the
> Dzo (legibility-dzo buddy). Targets are picked from observed friction in
> `.codescout/usage.db` (truncated bodies, grep roulette, edit_code error
> families), not from aesthetics.
> **Scope (2026-06-13):** Phase-1 survey of the codescout flight recorder;
> ranked targets `edit_file/tests.rs` (un-mappable file), `LspManager::get_or_start`
> (over-budget bodies + ambiguous address), the string-keyed action-dispatch
> cluster. No moves yet. May expand to mirela + southpole flight recorders.
> **Status vocabulary:** see `docs/templates/session-log.md` (canonical).

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-06-13 | med | flight-recorder-hygiene | mitigated | `.codescout/usage.db` is cross-project; ranking needs an in-repo existence check |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-06-13 | med | Existence-check a flight-recorder target before ranking it | A mirela phantom (`CalendarService`, 3 truncated fetches) would have ranked ~#4 and opened a campaign against code not in the repo | validated |

---

## F-1 — `.codescout/usage.db` is cross-project; flight-recorder ranking needs an in-repo existence check

**Observed:** 2026-06-13, Dzo Phase-1 survey — ranking legibility targets by truncated-`symbols` recurrence in the flight recorder.

**When:** Aggregating truncated body fetches to rank over-budget targets. `CalendarService` surfaced at 3 truncated fetches with an empty `path` field — alongside genuine codescout symbols.

**Expected:** `.codescout/usage.db` (in the codescout repo root) holds codescout's own tool calls.

**Got:** `CalendarService` resolves to `project_sha=1e8b9eb1`, path
`/home/marius/work/mirela/backend-kotlin/.worktrees/cs-stress-{1,2}/ktor-server/src/main/kotlin/edu/planner/service/scheduling/CalendarService.kt`
— a **mirela** (Kotlin/ktor) stress-test session on 2026-06-11. `grep -rl CalendarService --include=*.rs` over the codescout repo returns nothing. The DB holds **40 distinct `project_sha` values**; it is keyed by commit-SHA-at-call-time and mixes every project the shared (process-global) server touched.

**Probable cause:** the codescout MCP server process is process-global; the stress-test harness pointed the active project at mirela worktrees while telemetry still wrote to this `.codescout/usage.db`. Telemetry is not partitioned per-repo on read.

**Workaround:** existence-check each candidate before ranking (`grep -rl <symbol> --include=*.rs`, `wc -l <path>`, or a `symbols` resolution); exclude symbols absent from the repo. For a clean survey, filter `tool_calls` by the codescout `project_sha` set or by repo path prefix.

**Severity:** med — would have ranked a phantom (mirela) symbol as a codescout target and opened a campaign against code not in the repo.

**Status:** mitigated — phantom excluded this session via the existence check; the root-cause data hygiene of `usage.db` (no per-repo partition on read) is unaddressed.

**Fix idea / Pointer:** candidate `docs/issues/` bug — `usage.db` cross-project contamination — or a `project_sha`/path-prefix filter baked into the Pika + Dzo survey queries. TBD.

---

## W-1 — In-repo existence check before ranking a flight-recorder target caught a cross-project phantom

**Observed:** 2026-06-13, Dzo Phase-1 survey of the codescout flight recorder.

**Pattern:** Before ranking any symbol/path harvested from `.codescout/usage.db`, confirm it exists in the active repo (`grep -rl`, `wc -l`, or a `symbols` resolution). Telemetry DBs are not guaranteed single-project — they are keyed by commit-SHA and shared across every project the process served.

**Counterfactual:** `CalendarService` had 3 truncated fetches — tied with mid-tier codescout targets (`References/call`, `impl Tool for ReadMarkdown`, 2 each). Without the check it ranks ~#4; the Dzo then runs `symbols`/`semantic_search` readings that return empty, churns trying to "find" a phantom, and can open a tracker against code absent from the repo. The check cost one `grep -rl` and removed the phantom — and surfaced the broader contamination (F-1).

**Confirming data points:** (1) F-1 this session — `CalendarService` traced to a mirela `project_sha` via the existence check + a `project_sha` query. (2) Pending: any future `usage.db` survey that harvests a path-less symbol.

**Impact:** med — saves a churn loop against phantom code and prevents a phantom tracker.

**Promote-when:** a second flight-recorder survey (Pika or Dzo) harvests a cross-project phantom → promote to the Pika/Dzo survey method as "filter `tool_calls` by the repo's `project_sha` set (or path prefix) before ranking."

**Status:** validated — single datapoint, phantom caught + excluded before ranking. Awaiting promotion criterion.

---

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top.
     Status vocabulary + entry templates: docs/templates/session-log.md -->
