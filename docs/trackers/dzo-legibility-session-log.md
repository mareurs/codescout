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
| F-2 | 2026-06-13 | med | plan-drift | fixed-verified | Plan Task 6 test omits `overflowed` arg in a `write_record` call |
| F-3 | 2026-06-13 | low | plan-drift | fixed-verified | Plan Task 2 `is_body_bearing` won't compile — non-Copy enum moved behind shared ref |
| F-4 | 2026-06-13 | med | plan-drift | fixed-verified | 2b plan: wrong `ToolContext` import + fictional `mk_project_ctx` harness, caught pre-dispatch |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-06-13 | med | Existence-check a flight-recorder target before ranking it | A mirela phantom (`CalendarService`, 3 truncated fetches) would have ranked ~#4 and opened a campaign against code not in the repo | validated |
| W-2 | 2026-06-13 | med | Independent adversarial review after verbatim-spec subagent execution | Key-uniqueness + `collect_bodies` leaf-invariant assumptions would carry silently into Phase 2b's consumer as dedup/double-flag bugs | validated |

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

## F-2 — Plan Task 6 test omits the `overflowed` arg in a `write_record` call

**Observed:** 2026-06-13, pre-dispatch reconnaissance for the legibility-scan-engine plan (Phase 2a), about to dispatch Task 1 subagent-driven.

**When:** Scouting the `write_record` 16-arg signature (`src/usage/db.rs:105`) against the plan's Task 6 test before dispatch.

**Expected (plan):** Task 6's test issues three `write_record` calls, each 16 args matching `conn, tool_name, latency_ms, outcome, overflowed: bool, error_msg, …, project_root`.

**Got (scouted reality):** the 2nd call (the `edit_code` error row) skips the `overflowed: bool` argument — `write_record(&conn, "edit_code", 1, "error", Some("ambiguous …"), …)` — 15 args, with `Some(&str)` landing in the `overflowed: bool` slot. Hard `cargo test` compile error (type mismatch + wrong arg count). The other two calls are correct 16-arg.

**Probable cause:** plan author hand-wrote three positional calls; the overflow=false case (error row, not overflow row) is where the boolean is least salient, so it was dropped.

**Workaround:** insert `false,` after `"error",`. Plan fixed inline this session; Task 6 implementer briefed with the corrected test.

**Severity:** med — would have failed the Task 6 implementer's first `cargo test`; controller absorbs the drift, or the subagent flails on a type error in copied-verbatim plan code.

**Status:** fixed-verified — plan edit landed before any subagent ran.

**Fix idea / Pointer:** Plan Task 6 Step 1, this session. Root-cause class: positional multi-arg test calls in plans are fragile across signature changes — a named/struct-arg `write_record` would make this class impossible.

---

## F-3 — Plan Task 2 `is_body_bearing(kind: SymbolKind)` won't compile — non-Copy enum moved behind a shared ref

**Observed:** 2026-06-13, Task 2 implementation (over-budget-body detector), subagent-driven execution of the legibility-scan-engine plan.

**When:** Implementer ran the plan's Task 2 production code; `cargo test` failed to compile before the GREEN step.

**Expected (plan):** `fn is_body_bearing(kind: SymbolKind)` called as `is_body_bearing(s.kind)` inside `collect_bodies(syms: &[SymbolInfo])`.

**Got (compiler):** `SymbolKind` derives `Debug, Clone, Serialize, Deserialize, PartialEq, Eq` — NOT `Copy` (`src/lsp/symbols.rs:38`). `s` is `&SymbolInfo`, so `s.kind` is a non-Copy field behind a shared reference; `is_body_bearing(s.kind)` moves out of borrowed content → E0507. Plan code does not compile as-written.

**Probable cause:** plan author assumed `SymbolKind: Copy` (most small enums are; this one isn't). Pre-dispatch recon READ the derives but did not trace the by-value-move implication at the plan's call site.

**Workaround / fix:** implementer changed the signature to `is_body_bearing(kind: &SymbolKind)` and the call to `&s.kind`. `matches!` works identically through a reference (match ergonomics). All three legibility tests green. Committed `a3882850`.

**Severity:** low — self-resolved by the implementer in one signature change; downstream gate (compiler) caught it; no controller round-trip lost.

**Status:** fixed-verified — landed in the Task 2 commit `a3882850`.

**Fix idea / Pointer:** Recon-miss class — when scouting a type for a plan, trace ownership (Copy vs move) at each plan call site, not just field/variant names. Sibling to F-2; two datapoints now that plan-embedded hand-written Rust carries compile-level defects shape-matching recon misses. Candidate R-N (miss) for `reconnaissance-patterns.md` if a third lands.

---

## W-2 — Independent adversarial review after subagent-driven 2a execution surfaced 5 latent Phase-2b risks (verdict APPROVED)

**Observed:** 2026-06-13, end of Plan 2a (legibility scan engine) subagent-driven execution. After all 7 tasks landed green + clippy-clean, dispatched a fresh read-only reviewer (capable model) over the whole `src/legibility/mod.rs`.

**Pattern:** After executing a plan whose per-task code was handed to subagents VERBATIM (so per-task "spec compliance" is trivially true — the tests are the only real check), run ONE independent adversarial whole-module review before calling the work done. Per-task tests verify the cases the plan author imagined; the independent reviewer traces invariants the tests don't assert and surfaces forward-design risks for the next phase.

**Verdict:** APPROVED — no correctness defects. Reviewer adversarially traced `body_text` slicing (0-indexed, bounds-guarded), the recorder SQL (`project_root` scoping excludes foreign + NULL repos; `other` bucket NULL-safe; no truncation/other double-count — the sole writer `classify_content_result` makes `overflowed=1` and `outcome!='success'` mutually exclusive), and the scorer (structural-gate enforced; total deterministic sort; `name_path`→`rel_file` fallback resolves the `"(file)"` un-mappable target). All against the real schema + extractors, not docs.

**Carry-forward for Phase 2b (5 latent findings, none a current bug):**
1. `Candidate.key = "<rel_file>::<name_path>"` is NOT unique — one symbol can be both over-budget AND a name-collision, or two same-named over-budget methods share a `name_path`. Harmless until a consumer dedups by key. 2b fix: include `defect` in the key, or document non-uniqueness.
2. `collect_bodies` assumes body-bearing symbols are tree LEAVES (true for all current extractors). A future extractor emitting nested fns/closures-as-children would double-flag outer+inner. Pin the invariant with a comment or guard.
3. `over_budget_bodies` measures RAW source bytes, not rendered `symbols(include_body=true)` output (which adds wrapping) — under-reports near the 10 KB threshold. Matches spec wording, not the real overflow trigger exactly.
4. `COUNT(DISTINCT cc_session_id)` drops NULL sessions (pre-v0.10 rows) — only undercounts the informational `sessions` field, never the score.
5. `i64 as u32` cast in the row mapping wraps above ~4.2B — unreachable under 30-day retention.

**Counterfactual:** Without the review, findings 1 and 2 (silent assumptions about key uniqueness and symbol-tree shape) carry into Phase 2b's tracker-reconcile consumer and surface THERE as dedup / double-flag bugs — far costlier to trace once the librarian augmented-artifact layer sits on top. The review cost one subagent and turned two silent assumptions into documented invariants before any consumer depends on them.

**Confirming data points:** (1) this review, 2a. (2) Pending: whether 2b actually trips one of the latent findings.

**Impact:** med — no current bug fixed, but two latent API assumptions surfaced before a consumer locks them in.

**Promote-when:** a second verbatim-spec plan's post-execution review catches a forward-design risk the per-task tests missed → promote to subagent-driven-development practice as "after verbatim-spec execution, the final whole-module review is non-optional; per-task spec-compliance is vacuous when the controller supplied the code."

**Status:** validated — review complete, verdict APPROVED, carry-forward captured.

---

## F-4 — 2b plan code drifted from librarian internals (wrong `ToolContext` import + fictional test harness), caught pre-dispatch

**Observed:** 2026-06-13, pre-dispatch reconnaissance for the Phase-2b legibility_scan plan, before dispatching Task 1 subagent-driven.

**When:** Scouting the librarian handler conventions against the plan's skeleton (the plan was written from the `audit_doc_refs` *shape* without reading its exact `use` lines or test harness).

**Expected (plan):** handler imports `use crate::tools::core::ToolContext;` + `use crate::librarian::tools::RecoverableError;`; tests use a `mk_project_ctx() -> (ToolContext, EnvGuard, TempDir)` harness with `#[serial_test::serial]`.

**Got (scouted reality):** (1) `ToolContext` is the **librarian's OWN** struct (`src/librarian/tools/mod.rs:82`), distinct from `crate::tools::core::types::ToolContext` (`src/tools/core/types.rs:58`) — two different types. `audit_doc_refs` imports `crate::librarian::tools::{RecoverableError, Tool, ToolContext}`; the plan's path is the wrong type and would fail to compile against `find/create/get/augment::call`. (2) The real test harness is `mk_smoke_ctx(root: PathBuf) -> ToolContext` (audit_doc_refs tests L652) using an **in-memory** catalog (`Catalog::open_in_memory()`) — NO `EnvGuard`, NO `#[serial_test::serial]`. The plan's `mk_project_ctx()` tuple + serial attributes were fictional/over-cautious.

**Probable cause:** the 2b plan was authored from the `audit_doc_refs` *structure* (functions, reconcile shape) which I scouted, but I did not read its `use` block or `mod tests` harness before writing the skeleton — so the import path and harness were guessed from the more-common `crate::tools::core` convention.

**Workaround / fix:** plan corrected inline this session — Task 1 import merged to `use crate::librarian::tools::{RecoverableError, ToolContext};`, and a "## Pre-execution corrections" section instructs copying `mk_smoke_ctx` (in-memory, no serial) wherever the plan said `mk_project_ctx`. Implementers briefed with the corrected forms.

**Severity:** med — (1) is a hard compile error the Task-1 implementer would inherit; (2) would have caused test-harness flailing across Tasks 5/6/7/9. Both caught before any dispatch.

**Status:** fixed-verified — plan edits + briefs landed before any subagent ran.

**Fix idea / Pointer:** Recon-miss-then-caught class: when authoring a plan against a template module, scout the template's `use` block + `mod tests` harness, not just its function shapes. Sibling to F-2 (2a plan-drift, same root: plan written from shape without scouting). Phase-2b plan, this session.

---

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top.
     Status vocabulary + entry templates: docs/templates/session-log.md -->
