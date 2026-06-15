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
| F-5 | 2026-06-13 | med | plan-drift | fixed-verified | 2b Task 8 missed adapter `is_write` write-gating + get_guide table, caught pre-dispatch |
| F-6 | 2026-06-13 | high | correctness | fixed-verified | 2b `limit` corrupted reconcile (auto-closed below-cut defects); review caught it |
| F-7 | 2026-06-13 | low | process | mitigated | Interrupted dispatch may have run; verify state before re-dispatch |
| F-8 | 2026-06-13 | low-med | ux/render | fixed-verified | `render_template` attached but not auto-applied; backlog body stays a placeholder after write |
| F-9 | 2026-06-13 | med | phantom-limitation | fixed-verified | Relocated 11 trait impls to clear a "collision" `edit_code` already resolved via the qualified name_path |
| F-10 | 2026-06-14 | low-med | tool-shape | mitigated | `legibility_scan(project=)` scopes only the recorder lane; the symbol lane follows the ACTIVE project |
| F-11 | 2026-06-15 | low | tool-shape | fixed-verified | `edit_code insert position=before` for an attribute leaves a blank line before the doc comment; fmt won't remove it |
| F-12 | 2026-06-15 | med | environment | mitigated | After an MCP reconnect/restart the active project resets to home; run_command cwd silently follows — a git query reads the WRONG repo |
| F-13 | 2026-06-15 | low-med | tool-shape | mitigated | `edit_code insert position=after` on a file's LAST top-level symbol fails ("cannot determine end … AST parse failed") |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-06-13 | med | Existence-check a flight-recorder target before ranking it | A mirela phantom (`CalendarService`, 3 truncated fetches) would have ranked ~#4 and opened a campaign against code not in the repo | validated |
| W-2 | 2026-06-13 | med | Independent adversarial review after verbatim-spec subagent execution | Key-uniqueness + `collect_bodies` leaf-invariant assumptions would carry silently into Phase 2b's consumer as dedup/double-flag bugs | validated |
| W-3 | 2026-06-13 | high | Whole-module review after verbatim-spec execution caught a Critical per-task tests missed (W-2 promote-when MET) | `limit`/reconcile data-corruption bug would have silently corrupted the backlog on first real limit+write scan | validated |
| W-4 | 2026-06-13 | high | First full legibility loop closed; trait-forwarder collision cleared by relocating the impl ~~before the body refactor it blocks~~ | ~~un-refactorable until the forwarder moved out~~ | **CORRECTED by F-9** — the move was *sufficient but unnecessary*; `edit_code` resolves the qualified `impl Trait for Type/method` form, so the body was editable in place |
| W-5 | 2026-06-13 | high | name_collision removed: language-agnostic by subtraction (a per-language signal cannot live in a language-agnostic AST scanner) | 19+ collision rows kept tempting idiomatic-code relocations; two such moves were already done before the council's audit caught the phantom | promoted-to-docs |
| W-6 | 2026-06-14 | med | Close an over_budget_body by extracting cohesive phases into named helpers; the instrument (not LoC) sets the stopping point | onboarding.rs stays a 393-line truncating body — every `symbols(include_body)` fetch buffers, no clean retrieval path | validated |
| W-7 | 2026-06-14 | med | Pre-edit scout of the NEXT target before reusing a refactor template — check for lock/`!Send`/await invariants that constrain the seam | Naive reuse of the onboarding async-phase template on ArtifactAugment/call wraps a `!Send` lock-held region in an `async fn` → compile failure / broken lock-drop ordering; a subagent would flail | validated |
| W-8 | 2026-06-15 | med | Apply W-7: extract SYNC value-logic helpers from a lock-held `!Send` body, leaving the lock skeleton + async tail in place | ArtifactAugment/call stays a 284-line truncating body; or a naive async-phase extraction breaks `!Send`/lock-drop ordering | validated |
| W-9 | 2026-06-15 | med | Scout decides async-vs-sync: a lock-free async body keeps async helpers (complement of W-8); a predicate closure borrowed across `.await` needs `Box<dyn Fn + Send + Sync>` | Reusing W-8's sync-only template wraps genuinely-async LSP calls in sync fns (won't compile); a bare `+ Send` closure yields an inscrutable "future is not Send" error | validated |
| W-10 | 2026-06-15 | med | Third seam shape: ReadMarkdown's read branches are fully sync (no lock, no await) → sync helpers; only path-resolution is async. Scout decides a 3-way shape, not a binary | Reusing either prior template misclassifies the seam — forcing pointless `.await`-less async fns or assuming a lock that isn't there | validated |

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

## F-5 — 2b plan Task 8 understated scope: missed adapter `is_write` write-gating + the get_guide librarian table, caught pre-dispatch

**Observed:** 2026-06-13, pre-dispatch reconnaissance for Phase-2b Task 8 (advertise the new librarian action).

**When:** Scouting what a new librarian action actually requires — the plan's Task 8 listed only `librarian.rs` `description`/`input_schema` + a `source.md` grep.

**Expected (plan):** add `legibility_scan` to the librarian tool's `description` + `input_schema`; grep prompt surfaces.

**Got (scouted reality):** two more required edits. (1) `src/librarian/adapter.rs::is_write` (L71) classifies librarian write-actions for write-gating — `reindex`=write, `audit_doc_refs`=write-unless-`emit_tracker:false`, everything else read. `legibility_scan` with `write:true` MUTATES the backlog tracker but is unlisted → would be misclassified as a READ, bypassing the write guard (read-only mode / write serialization). Needs `Some("legibility_scan") => input.get("write") != Some(false)`. (2) `src/prompts/guides/librarian.md` (L227-229) is a get_guide action TABLE listing `tracker_design`/`audit_doc_refs` — needs a `legibility_scan` row. (`source.md` only name-drops `tracker_design` as an example, not an exhaustive list → no change; the `prompt_surfaces_reference_only_real_tools` test checks tool NAMES, not actions, so it won't trip.)

**Probable cause:** the plan was written from the `audit_doc_refs` *handler* shape (which I scouted) but not its *registration surface* — the adapter `is_write` arm and the guide table are one level out from the handler module.

**Workaround / fix:** Task 8 brief expanded to cover all four surfaces (description, input_schema, adapter is_write + its test, guide table). The is_write gap is the load-bearing one (write-gating correctness).

**Severity:** med — the is_write omission is a silent correctness bug (write op treated as read), not a compile error; only a scouting pass or a write-gating test would catch it. Caught before dispatch.

**Status:** fixed-verified — expanded scope briefed before any subagent ran.

**Fix idea / Pointer:** "new librarian action" checklist = handler module + dispatch arm + error lists + description + input_schema + **adapter is_write arm** + **get_guide table row**. Sibling to F-4 (both: plan scouted the handler shape, missed the registration surface). Phase-2b Task 8, this session.

---

## F-6 — 2b handler `limit` corrupted the reconcile (auto-closed still-defective below-cut candidates); per-task tests missed it, the whole-module review caught it

**Observed:** 2026-06-13, post-implementation independent review of the 2b `legibility_scan` module (after all 9 tasks landed green).

**When:** Tracing the `call` handler's data flow — both the controller (me) and the adversarial reviewer independently flagged it.

**Bug:** `call` did `grouped.truncate(limit)` BEFORE `reconcile`. On a `write:true` scan with a `limit`, candidates ranked below the cut were absent from the current scan, so reconcile's auto-close pass closed them as "defect gone" — `status:closed` + a bogus `after` re-measure — even though they were still over budget. Silent data corruption of the backlog.

**Why per-task tests missed it:** no task exercised `limit` on the `write:true` path. All 7 module tests stayed green; the bug only fires when `limit` is passed with `write:true`. The plan's own self-review even *claimed* "limit caps the dry-run head only; history never dropped" — the implementation contradicted the spec the plan asserted.

**Fix:** reconcile ALWAYS receives the full grouped set; `limit` slices only the dry-run output head (`&grouped[..n]`). Added a RED→GREEN regression test (`limit_does_not_auto_close_below_cut_candidates_on_write`): two over-budget fns, scan with no limit (both open), re-scan with `limit:1` → assert closed-count == 0.

**Severity:** high — silent data corruption (still-defective targets marked resolved) on a supported param combination. Caught before any real use (no consumer yet).

**Status:** fixed-verified — `eaf341d0`, regression test guards it.

**Fix idea / Pointer:** logic bugs in verbatim-spec plans survive per-task TDD (the tests assert the cases the author imagined); only a whole-module review or a param-combination test catches them. Pairs with W-3. Plan self-reviews can assert a property the code violates — trust the trace, not the claim.

---

## F-7 — Interrupted subagent dispatch may have already run; re-dispatching without verifying state risks double-apply

**Observed:** 2026-06-13, Task 7. The first Task-7 dispatch was interrupted (to summon the Pika); I re-dispatched the same task without first checking the working tree.

**Got:** the interrupted first dispatch had actually run far enough to leave its edits in the working tree before the interrupt landed; the second dispatch found the work done and committed it (and mis-attributed it to a "concurrent session"). End state was correct, but only by luck — a less idempotent task could have double-applied.

**Lesson:** after an interrupted subagent dispatch, scout `git status` / the target file BEFORE re-dispatching. An interrupted Agent tool-use can still have mutated disk. Verify, then decide resume-vs-redispatch.

**Severity:** low — no harm this session (the re-dispatch was idempotent and I verified the git state afterward). Mitigated by the post-hoc forensic check (git log + grep confirmed one clean commit, no duplication).

**Status:** mitigated — lesson captured; verify-before-redispatch is the rule.

---

## W-3 — 2b's post-implementation review caught a Critical the 7 per-task tests missed — W-2's promote-when criterion is now MET

**Observed:** 2026-06-13, end of Phase-2b subagent-driven execution.

**Pattern (same as W-2):** after executing a plan whose per-task code was handed to subagents verbatim, run ONE independent adversarial whole-module review before calling it done. Per-task TDD only checks the author's imagined cases.

**Counterfactual (stronger than W-2's):** W-2 (2a) surfaced *latent* risks. W-3 (2b) caught a **confirmed Critical data-corruption bug** (F-6, the `limit`/reconcile interaction) that all 7 green per-task tests missed — it would have silently corrupted the backlog on the first real `limit`+`write` scan. The review cost one subagent; the bug would have cost a debugging session against a corrupted tracker once a consumer existed.

**Plus the pre-dispatch recon wins this phase:** F-4 (wrong `ToolContext` import + fictional `mk_smoke_ctx` harness) and F-5 (missing adapter `is_write` write-gating + get_guide table) were both caught BEFORE dispatch by scouting the template's `use` block, test harness, and registration surface — not just its handler shape.

**Promote-when (W-2's criterion): MET.** Two independent post-execution reviews (2a latent, 2b Critical) have now caught what per-task tests missed. Promote the practice to a permanent surface: **"after verbatim-spec subagent execution, a whole-module independent review is non-optional — per-task spec-compliance is vacuous when the controller supplied the code."** Craft-shaped (true for any repo) → route to the subagent-driven-development skill / CLAUDE.md, not project memory.

**Impact:** high — prevented shipping a silent data-corruption bug; establishes the review-after-verbatim-execution practice with 2 datapoints.

**Status:** validated — promotion criterion met; awaiting the sync PR to the skill/CLAUDE.md.

---

## F-8 — `render_template` attached but not auto-applied; backlog body stays a placeholder after write (dogfood finding)

**Observed:** 2026-06-13, dogfooding `legibility_scan(write:true)` on codescout (first real backlog).

**When:** After the write succeeded (42 candidates in `params`), reading the rendered markdown body.

**Expected (2b design):** "the librarian renders the sorted table at the top; the Dzo's body prose beneath."

**Got:** body is the initial placeholder (`Auto-managed by ...`); the `render_template` table is NOT projected. `write_backlog` updates `params` via `artifact_augment(merge=true)` but never renders the body. The refresh cycle (`artifact_refresh(gather)` → `artifact(update, commit_refresh=true)`) requires a hand-synthesized body `patch` — `render_template` is not auto-applied even there. Same behavior as `audit_doc_refs` (render_template attached, body = placeholder), so this is a librarian-level gap that 2b inherits, not 2b-specific.

**Workaround:** hand-rendered the 42-row table into the body for the dogfood (`artifact(update, patch={body})`). `params` remains the source of truth — queryable via `artifact(get, entry_filter=...)` and the dashboard.

**Severity:** low-med — UX/readability gap; the data is correct in `params` and the reconcile/auto-close logic is unaffected (it reads `params`, never the rendered body).

**Status:** fixed (`1d82ec14`, fix idea (a)) — `write_backlog` now renders the `.j2` over `params` and writes the managed region, preserving everything from the `## Verdicts` heading onward. New test `scan_write_renders_body_and_preserves_verdicts` pins both halves (managed region re-renders; verdict prose survives) **and** validates the MiniJinja `{% for c in candidates if ... %}` form against a real render — the untested-syntax worry in the fix idea is now closed. Best-effort: a render failure warns and leaves `params` authoritative, never failing the scan. **Verified live:** after the `/mcp` reconnect to the rebuilt binary, a re-scan auto-rendered the codescout backlog body (managed table + `### Closed` section, 29 open / 13 closed) and preserved the two Dzo verdicts verbatim; the hand-rendered table and the F-8 note are gone. The body is now self-maintaining.

**Fix idea:** either (a) `write_backlog` renders `render_template` against `params` and writes the body via `body_edits` after the params update, or (b) the librarian auto-applies `render_template` on `commit_refresh` without requiring a body `patch`. Also validate the MiniJinja template syntax (the `{% for c in candidates if ... %}` form) once a real render path exists — it is currently untested against an actual render.

---

## W-4 — First full legibility loop closed: get_or_start auto-closed (3036→2463 tok) via trait-move-then-extract

**Observed:** 2026-06-13, executing "use it" — refactor the #1 backlog target and watch the engine auto-close it.

**Pattern:** When a target carries BOTH `over_budget_body` and `name_collision`, and the collision is an inherent-impl + same-file trait-forwarder pair, fix the collision FIRST by relocating the trait-impl *block* to its own file — then the body becomes editable. The collision otherwise blocks the very `edit_code` calls the body refactor needs.

**Why the order is forced:** `edit_code` resolves a symbol via LSP `document_symbols` (per-file) → `find_unique_symbol_by_name_path`, which hard-errors "ambiguous name_path matches 2 symbols" when an inherent method and a trait forwarder share the `<Type>/<method>` name_path in one file. Renaming the inherent method to break the tie is *also* blocked — `edit_code(action=rename)` must resolve the symbol first. The trait-impl block has a distinct name_path (`impl Trait for Type`) — the only collision-free handle. Move it out → per-file collision clears (both the legibility detector and LSP `document_symbols` are per-file) → the body is uniquely addressable.

**Counterfactual:** Without scouting the collision pre-edit, the first body-extraction `edit_code(symbol="LspManager/get_or_start", action=replace)` would have hard-failed "matches 2 symbols" — and logged an `ambiguous_name_path` edit_fail against the very row being fixed. Reconnaissance (reading `find_unique_symbol_by_name_path` + `count_symbols_by_name_path` + the trait block) caught it before any blind edit and reframed the refactor as trait-move-then-extract.

**Outcome:** 2 transformations (`b946171d` move, `95ea8e0e` extract), behavior-preserving (39 `lsp::manager` tests green throughout). Re-scan auto-closed `get_or_start` (3036→2463 tok, 242→196 ln) and swept up the `notify_file_changed` + `shutdown_all` collisions in the same forwarder block — 3 rows closed. First end-to-end run of the instrument: logs → rank → refactor → auto-close with a measured delta.

**Reusable template:** the identical move clears the `LspClientOps` cluster (10 collisions in `client.rs`) — captured as a Dzo verdict in the backlog. One relocation → 10 cleared.

**Promote-when:** a second target where a same-file trait-forwarder collision blocks a needed body refactor and the move-first sequence resolves it. At 2 datapoints, promote to a recon/refactor rule: *"before refactoring a method body, a `count_symbols_by_name_path` > 1 means a trait forwarder shares the name_path and `edit_code` is blocked — relocate the trait-impl block first."* Craft-shaped (Rust trait-impl pattern) → reconnaissance memory / skill, not a one-off.

**Impact:** high — closed the first full loop AND produced a reusable, tested template for the largest remaining collision cluster.

**Status:** validated — **2 datapoints.** (1) `get_or_start` body-refactor (`b946171d`+`95ea8e0e`); (2) the `LspClientOps` cluster (`2b35f2a1`) — one trait-impl move → **10 collisions cleared** at near-zero cost (template amortized the recon). The *trait-move-clears-same-file-collision* template's promote-when is MET → route to a reconnaissance/refactor rule: *"`count_symbols_by_name_path` > 1 on `<Type>/<method>` means a same-file trait forwarder shares the name_path and `edit_code` is blocked; relocate the trait-impl block to its own file."* (The body-refactor-blocking *variant* specifically still has 1 datapoint — get_or_start — since the `LspClientOps` rows were collision-only.)\n\n**CORRECTION (F-9, 2026-06-13):** the framing above overstates the move. `edit_code` resolves the qualified `impl Trait for Type/method` form, so `get_or_start`'s body was editable **in place** — the relocation was *sufficient but unnecessary*, not required. The LspClientOps cluster had zero observed friction, so its move was metric-driven (violates principle 2). Both moves kept (idiomatic, harmless) but earned nothing. The collision was never a real `edit_code` block; the defect class itself was removed (W-5, ADR `docs/adrs/2026-06-13-drop-name-collision-defect.md`).

---

## F-9 — Relocated 11 trait impls to clear a "collision" edit_code could already resolve via the qualified form

**Observed:** 2026-06-13, the council's "was the Ops refactoring worth it?" audit, after the get_or_start + LspClientOps moves.

**Expected (my working assumption, recorded in W-4):** the bare name_path `LspManager/get_or_start` was the only handle; the inherent + trait-forwarder collision *blocked* `edit_code`, so the trait impl had to be relocated before the body could be shrunk.

**Got (verified live):** `edit_code` resolves a colliding sibling by the *qualified* name_path — `find_unique_symbol_by_name_path("impl fmt::Display for SensitiveString/fmt")` returns exactly one symbol. The capability always existed. I read the resolver (`find_unique_symbol_by_name_path`, `symbol_name_matches`, `count_symbols_by_name_path`) but never *tested* the qualified path — and the resolver's own not-found hint already *documents* the form ("Trait impl methods use format 'impl Trait for Struct/method'"). So the two relocations (`b946171d`, `2b35f2a1`) were unnecessary; the bodies were editable in place. Worse, the LspClientOps cluster had **zero** observed friction — a metric-driven move, not a friction-driven one (violates the Dzo's principle 2).

**Severity:** med — real wasted work (11 trait impls moved); caught + corrected, no wrong code shipped. The moves were kept (idiomatic, harmless) but earned nothing.

**Status:** fixed-verified — the misleading ambiguity hint fixed (`c21ad73b`), and the AST-level name_collision detector removed (`919dbe5c`, ADR `docs/adrs/2026-06-13-drop-name-collision-defect.md`).

**Fix idea / pointer:** R-30 in `docs/trackers/reconnaissance-patterns.md` — when a structural feature appears to block a tool, *test the tool's documented alternate addressing form* before refactoring code around it.

## W-5 — name_collision removed: "language-agnostic by subtraction"

**Observed:** 2026-06-13, the worth-it audit escalated (with the Architecture Snow Lion) into "where does the fix belong?"

**Pattern:** A quality signal in a *language-agnostic* AST scanner must be measurable uniformly across every language it serves. `name_collision`'s disambiguator is per-language (Rust trait/impl context, TS declaration kind, Java overload signature); the tree-sitter AST collapses all of them, and for TS declaration-merging the "collision" is idiomatic, not a defect. The right move was **removal (subtraction)**, not building per-language machinery the stateless scanner can't run (the LSP already does it per-language, but importing it defeats the fast index lane).

**Counterfactual:** Without the catch, the backlog kept presenting 19+ `name_collision` rows as actionable, each tempting another zero-friction, metric-driven relocation (the F-9 pattern). Two were already done.

**Confirming data points:**
1. F-9 — two relocations for a collision `edit_code` already resolved.
2. TS `BookMetadata` — interface+namespace merge flagged as a "defect" it isn't.

**Impact:** high — removed a false-priority signal at the source (`919dbe5c`), shipped the real remediation (the qualified-form hint `c21ad73b`), added a guard test (`index_lane_does_not_flag_name_collisions`).

**Status:** promoted-to-permanent-docs — ADR `docs/adrs/2026-06-13-drop-name-collision-defect.md` (Decision + Alternatives carry the reusable rule).

## W-6 — Second full legibility loop: perform_full_onboarding closed (over_budget_body, ~3839→<2500 tok) via 7-phase extraction

**What:** Tier-1 `over_budget_body` target `src/tools/onboarding.rs::perform_full_onboarding` (393 ln / ~3839 tok, 1 observed truncation) refactored by extracting 7 cohesive phases into private module-level helpers (`detect_languages`, `list_top_level_entries`, `build_key_files`, `write_workspace_config_if_needed`, `probe_index_status`, `write_onboarding_memories`, `gather_per_project_protected`) — pure phase extraction, no signature/API change, matched the file's existing free-fn idiom.

**Outcome:** Behavior-preserving — `cargo test` **2864 passed / 0 failed**, identical to the pre-move baseline; clippy `--all-targets -D warnings` + fmt clean. Commit `333d6281`. `symbols(include_body)` went buffered → **returns whole**; `legibility_scan` auto-closed the row (open 22→20). The instrument set the stopping point: the first 6-helper cut measured **2568 tok (still over 2500)**, so a 7th extraction (`gather_per_project_protected`) was added to cross — budget is the trigger, not line count (Heuristic 1). First loop driven by the summoned `legibility-dzo` buddy end-to-end.

**Status:** validated — full loop closed (logs → scan → refactor → green → auto-close). Detailed per-key verdict in the `legibility-backlog` tracker.

## F-10 — `legibility_scan(project=…)` scopes only the recorder lane; the symbol-index lane follows the ACTIVE project

**Observed:** 2026-06-14, opening the Dzo campaign. Ran `librarian(action="legibility_scan", write=false, project="/home/marius/work/claude/codescout")` while the active project was `claude-plugins`.

**Expected:** `project=` targets codescout → ranked codescout candidates.

**Got (scouted reality):** `candidates: [], n: 0`. The `project=` param scopes the usage.db *recorder* lane only (its own doc: "Scopes the recorder lane"); the symbol-index lane — what actually gets ranked for `over_budget_body` — follows the **active** project. With `claude-plugins` (bash/markdown, no large code bodies) active, there was nothing to rank, so the scan returned empty despite the codescout recorder path. Re-running after `workspace(activate, path=codescout)` returned 15 candidates.

**Probable cause:** Two-lane design — only the recorder lane is parameterized; the symbol lane is implicit on the active project, undocumented beyond "Scopes the recorder lane."

**Workaround / rule:** To scan project X, **activate X first** (read-only is fine); do not rely on `project=` alone.

**Severity:** low-med — one wasted scan, self-corrected; but a false-empty result could mislead a less careful run into concluding a project has no legibility debt.

**Status:** mitigated — lesson promoted to the `legibility-dzo` SKILL.md Phase 1 ("Run it with the target project active … or the lanes misalign and the scan returns empty"). Candidate codescout-side fix: honor `project=` for BOTH lanes, or warn when `project=` ≠ active project.

## W-7 — Pre-edit scout: the onboarding async-phase template does NOT transfer to ArtifactAugment/call (lock-held `!Send` region)

**Observed:** 2026-06-14, reconnaissance on the next tier-1 `over_budget_body` target `src/librarian/tools/augment.rs::ArtifactAugment/call` (L106-389, ~3484 tok / 284 ln) BEFORE proposing any refactor move.

**Pattern:** Before reusing a refactor template that worked on target A (perform_full_onboarding: lift cohesive phases, several into `async fn`s), scout target B for **concurrency/lock invariants** that constrain where phases can be cut — read the body for `lock()` scoping, `!Send` guards, and "drop before await" comments.

**Counterfactual:** ArtifactAugment/call's bulk is a single `ctx.catalog.lock()` region deliberately scoped in a bare `{ }` block so the `parking_lot` MutexGuard (`!Send`) drops before the async `event_create` below. Naively lifting "the merge block" into an `async fn` would fail to compile (`!Send` guard held across `.await`) or, if forced, break the lock-drop ordering the comment protects. A subagent handed "apply the onboarding pattern" would have flailed on the borrow/Send errors. The correct seam is **sync** value-logic helpers — the ~80-line goal-tracker auto-close gate operates on parsed `Value`s with no lock/await; extract those, leave the lock-scope skeleton intact.

**Confirming data points:**
1. W-7 (this scout) — onboarding template's async-phase shape blocked by ArtifactAugment's `!Send` lock scope; correct seam is sync helpers.
2. Pending: any future target where a refactor template is constrained by a concurrency/lock/await invariant.

**Impact:** med — saves a failed compile loop (or a flailing subagent) and reframes the next move toward the sync-helper seam. Also confirmed the safety net: 22 inline unit tests in `augment.rs` (incl. `gate_check_note_event_emitted_on_autoclose`, scope-growth, sibling-field preservation) pin the behavior, so the eventual refactor has a green baseline.

**Promote-when:** A second target where a refactor template is constrained by a lock/`!Send`/await invariant the source's comments flag. At 2 datapoints, promote to the Dzo SKILL.md as a Phase-2 scout rule: "before reusing a refactor template, scout the new target for lock-scope/`!Send`/await invariants — they decide which seams are cuttable and whether helpers must be sync."

**Status:** validated — single datapoint; scout completed pre-edit, no move made. Awaiting promotion criterion.

## W-8 — Third legibility loop: ArtifactAugment/call closed (over_budget_body, ~3484→<2500 tok) via SYNC-helper extraction (W-7 applied)

**What:** Tier-1 `over_budget_body` target `src/librarian/tools/augment.rs::ArtifactAugment/call` (284 ln / ~3484 tok). W-7 had established the onboarding async-phase template does NOT transfer here — the body is a lock-held `!Send` region. Applied the corrected approach: extract three **sync** helpers (`validate_merged_against_schema`, `process_goal_tracker_merge`, `create_or_replace_augmentation`) operating on parsed `Value`s or locking internally, leaving `ctx.catalog.lock()`'s bare-block scope and the post-lock async `event_create` exactly in place.

**Outcome:** Behavior-preserving — `cargo test` 2864 passed / 0 failed = baseline, incl. the 22 inline `augment.rs` tests (gate events, scope-growth, schema, sibling preservation); clippy `--all-targets -D warnings` + fmt clean. Commit `ede1c07d`. `symbols(include_body)` buffered → returns whole (284→144 ln); `legibility_scan` auto-closed the row (open 20→19).

**Confirming data points (W-7's promote-when):**
1. W-7 — pre-edit scout identified the lock/`!Send` constraint and the sync-helper seam.
2. W-8 (this) — the prescription executed cleanly: sync helpers crossed the budget with no concurrency regression, first try, no compile thrash.

**Impact:** med — second datapoint for the W-7 pattern. **Promote-when now MET** → promoted to the `legibility-dzo` SKILL.md as a Phase-2 scout rule ("before reusing a refactor template, scout the new target for lock-scope/`!Send`/await invariants — they decide which seams are cuttable and whether helpers must be sync").

**Status:** validated — loop closed end-to-end; W-7 promotion criterion met and actioned.

## W-9 — Scout decided async-vs-sync correctly: `Symbols::call` is a lock-free async region → helpers stay async; predicate widened to `Send + Sync`

**Observed:** 2026-06-15, loop #4 on `Symbols/call` (`src/tools/symbol/symbols.rs`), applying the W-7 pre-edit scout before reusing a phase-extraction template.

**Pattern:** The W-7/W-8 rule is *"scout the seam's async/lock/Send invariants before reusing a refactor template"* — NOT *"always extract sync."* This loop is the **complement** of W-8: `Symbols::call` holds NO lock across its awaits, so its extracted search helpers must stay `async fn` (the ArtifactAugment sync-only constraint would be wrong here). The scout decided the axis correctly in the opposite direction.

**Fresh sub-lesson (craft, reusable):** when a predicate closure `Box<dyn Fn(..) -> bool + Send>` is *borrowed* across `.await` points inside an extracted async helper, the trait object must be `+ Send + Sync` (not just `+ Send`): `&(dyn Fn + Send)` is `Send` only if the referent is `Sync`, and `Tool: Send + Sync` (`src/tools/core/types.rs:388`) requires `call`'s future to be `Send`. The closures capture only `String` (Sync), so widening compiles; and `&(dyn Fn + Send + Sync)` still coerces to `&dyn Fn` at the `collect_matching` call sites (the same auto-trait-drop the code already relied on). Verified by `cargo check` — clean.

**Counterfactual:** Without the scout, reusing W-8's "extract sync helpers" verbatim would have forced sync helpers around genuinely-async LSP calls (`get_or_start`/`workspace_symbols`/`document_symbols` are all `.await`) — immediate compile failure. And without the Send+Sync insight, the first naive `&(dyn Fn + Send)` async-helper signature produces a "future cannot be sent between threads safely" error that reads as inscrutable trait-bound noise — a subagent would flail re-trying signatures.

**Move (`247be16f`, behavior-preserving; `cargo test` 2864 passed / 0 failed = baseline; clippy `--all-targets -D warnings` + fmt clean):** extracted `search_files_restricted` (A), `search_project_symbols` (B), `search_library_symbols` (C — all `async`), and sync `finalize_search_results`. `Symbols::call` 469→164 ln; `symbols(include_body)` buffered → returns WHOLE. Backlog row auto-closed (open 19→18).

**Impact:** med — third datapoint for the W-7 scout rule (already in the Dzo SKILL Phase 2). The Send+Sync-closure-across-await rule is a new, distinct craft rule.

**Promote-when:** the Send+Sync-closure-across-await rule recurs on a 2nd async extraction → promote to the Dzo SKILL as a named async-extraction heuristic (sibling to the lock/`!Send` rule), and/or to codescout `reconnaissance` memory as a Rust-async craft rule.

**Status:** validated — scout fired correctly; move landed first-try-clean (no compile thrash).

## F-11 — `edit_code insert position=before` for an attribute leaves a blank line before the doc comment; fmt won't remove it

**Observed:** 2026-06-15, loop #4 — adding `#[allow(clippy::too_many_arguments)]` to the three new wide-param search helpers (clippy `-D warnings` flagged 8–10 params each; `collect_matching` in the same file already carries the same allow, so it is the local convention).

**When:** After extraction, used `edit_code(action="insert", position="before", body="#[allow(...)]")` to prepend the attribute to each helper.

**Expected:** the attribute lands immediately above the fn's doc comment.

**Got:** the insert placed the attribute, then a BLANK line, then the existing doc comment: `#[allow(...)]\n\n/// doc\nasync fn`. Valid Rust (the attribute applies through the blank line) and lint-suppressing, but `cargo fmt` does NOT collapse the blank line — a persistent blemish, and the attribute sits above the doc comment (non-idiomatic order).

**Workaround (clean, deterministic):** `edit_code(action="replace", attributes=[], body=<doc + #[allow] + full fn>)`. The lead-region capture reaches UP past the blank line and absorbs the orphan attribute (`replaced_lines` started at the orphan's line, not the fn's), so the orphan is removed and the doc→attr→fn order is restored in one call. Re-send the full body — read it fresh with `symbols(include_body)` first to copy verbatim (zero transcription risk).

**Severity:** low — cosmetic; no behavior impact, lint was already suppressed. Costs one extra `replace` per helper.

**Status:** fixed-verified — all three helpers now read doc → `#[allow]` → fn; fmt + clippy clean.

**Fix idea / pointer:** edit_code could offer an `attributes`-append mode (add one attribute without re-sending the body), or `insert position=before` of a lone `#[...]` line could place it adjacent with no blank-line separator.

## W-10 — Scout found a THIRD seam shape: `ReadMarkdown::call`'s read branches are fully sync (no lock, no await) → sync helpers; only path-resolution is async

**Observed:** 2026-06-15, loop #5 on `ReadMarkdown/call` (`src/tools/markdown/read_markdown.rs`), W-7-style pre-edit scout of the async/lock/await shape.

**Pattern:** Three loops, three distinct seam shapes — the scout is what tells them apart, not a template:
- **W-8 ArtifactAugment:** lock-held `!Send` region → **sync** helpers (guard can't cross an await).
- **W-9 Symbols:** lock-free but genuinely **async** (LSP awaits) → **async** helpers + a `Send + Sync` predicate.
- **W-10 ReadMarkdown:** only the path-resolution prelude awaits; the four read branches (multi/single-heading, line-range, default-tiers) hold no lock and contain ZERO `.await` (the `section_coverage.lock()` scopes never cross an await) → **4 of 5 helpers are plain sync `fn`**, one async (`resolve_markdown_source`).

**Counterfactual:** Reusing either prior template blindly would misclassify the seam — forcing `async` on `.await`-less branches (pointless async boundary, noisier borrows) or assuming a lock constraint that isn't there. The scout read the awaits directly and split 4-sync / 1-async correctly; the move compiled after only a `&Path`→`&PathBuf` param-type fix.

**Move (`4d601b5d`, behavior-preserving; `cargo test` 2864 passed / 0 failed = baseline; clippy `--all-targets -D warnings` + fmt clean):** 5 helpers extracted; `call` 446→55 ln; `symbols(include_body)` buffered → WHOLE; backlog row auto-closed (open 18→17).

**Recon sub-miss (craft lesson):** chose `&Path` for the threaded `resolved` param before checking consumers; `mark_seen`/`status`/`markdown_coverage` take `&PathBuf` → 5× E0308 on first `cargo check`, fixed in one cycle by threading `&PathBuf` (forwarded straight to those consumers, so `clippy::ptr_arg` stays quiet). Lesson: **scout the *consumer* param types before fixing an extracted helper's signature** — the threaded type must match what the body forwards to.

**Impact:** med — confirms the scout rule is a 3-way shape decision (sync-lock / async / sync-pure), not a binary. The "match the consumer's param type" sub-lesson is a citable recon rule.

**Promote-when:** the 3-way "scout decides seam shape" framing recurs once more → fold an explicit async-vs-sync decision table into the Dzo SKILL Phase 2.

**Status:** validated — loop closed end-to-end.

## F-12 — After an MCP reconnect/restart, the active project resets to home and `run_command`'s cwd silently follows — a git query then reads the WRONG repo

**Observed:** 2026-06-15, resuming loop #5 after the user ran `/mcp` (reconnect) + `/reload-plugins`. I had activated codescout before the interrupt; expected HEAD to be my loop-#4 commit `247be16f`.

**When:** First action on resume was `git log` / `git merge-base --is-ancestor` to confirm my campaign commits survived an externally-added rename commit.

**Expected:** `run_command` runs in the codescout project root (where I'd activated).

**Got:** HEAD was `9a745d9 feat(hamsa)…` and the log was full of buddy / codescout-companion version bumps — **claude-plugins** history, not codescout. All four campaign commits reported "NOT in history." The `/mcp` reconnect had reset the active project to the home default (claude-plugins), and `run_command`'s cwd followed it. My commits were fine — I was querying the wrong repo. Re-activating codescout and re-running confirmed all four IN history.

**Probable cause:** an MCP server restart re-initializes active-project state to the session's home project; `run_command` resolves cwd from the active project, so a pre-restart `workspace(activate)` does not survive the reconnect.

**Workaround / rule:** After ANY MCP reconnect/restart (`/mcp`, server rebuild), re-`workspace(activate, path=…)` the target project BEFORE trusting `run_command`'s cwd — especially for git/state queries. Treat a surprising HEAD as "wrong repo" before "lost commits."

**Severity:** med — nearly produced a false "my commits were rebased away" conclusion that could have triggered needless recovery. Caught by scouting (the `merge-base` cross-check + noticing the log was the wrong project) before any action.

**Status:** mitigated — re-activate-after-reconnect rule; candidate for the Dzo SKILL Phase 1 / codescout `reconnaissance` memory.

## F-13 — `edit_code insert position="after"` on a file's LAST top-level symbol fails ("cannot determine end … AST parse failed")

**Observed:** 2026-06-15, loop #5 — inserting 5 module-level helper fns into `read_markdown.rs`, whose only items are the `ReadMarkdown` struct and the `impl Tool for ReadMarkdown` block (the impl is the final item in the file).

**When:** `edit_code(symbol="impl Tool for ReadMarkdown", action="insert", position="after", body=<5 helpers>)`.

**Expected:** helpers inserted after the impl block, at module level.

**Got:** `cannot determine end of 'impl Tool for ReadMarkdown' for insert-after — AST parse failed`. The file parsed fine for `symbols()` immediately before; the failure is specific to resolving the END of the file's last top-level symbol for insert-after.

**Workaround:** `position="before"` on the same impl (module order is irrelevant in Rust) — placed the helpers between the struct and the impl, compiled clean. (In loop #4 symbols.rs had a trailing free fn to anchor `insert before` on; here the impl was last, so insert-before-the-impl is the anchor.)

**Severity:** low-med — one failed tool call, immediate clean workaround. Recurs whenever the Dzo adds module-level helpers to a file whose last item is the symbol being split.

**Status:** mitigated — use `insert position=before` an existing symbol when the target is the file's last item. Candidate edit_code fix: treat EOF as a valid insert-after boundary.

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top.
     Status vocabulary + entry templates: docs/templates/session-log.md -->
