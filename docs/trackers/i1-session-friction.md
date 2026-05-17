---
title: I1 Refactor — Session Friction Log
date: 2026-05-17
parent: docs/trackers/goal-tracker-cross-pollination.md
purpose: Lightweight scratch surface for frictions observed while working through the I1 refactor plan. Cheap to append; promotable to formal bug files later.
---

# I1 Refactor — Session Friction Log

Living log of observations during I1 refactor work
(`docs/superpowers/plans/2026-05-17-i1-refactor.md`).

**Two entry types — both wanted:**
- `F-N` — **Friction.** What we did, expected, got, root cause, fix pointer.
  Add immediately when something surprises in a bad way.
- `W-N` — **Wins.** A pattern, instinct, or tool combo that paid off — especially
  when it prevented a worse outcome that would otherwise have stayed invisible.
  Add when you notice "we just caught X because of Y" or "this would have
  silently bitten us without Z".

**Why both:** A log that only catalogs failures slowly retires the practices
that prevented failures. Positive entries are the *control group*. When we
refactor the prompts/skills/tooling, W-N entries say what to keep; F-N entries
say what to change.

**Distinct from:**
- `docs/trackers/goal-tracker-dogfood-log.md` — `DF-N` for goal-tracker
  **pipeline** behavior (gather/refresh/synth pipeline frictions)
- `docs/issues/<date>-<slug>.md` — formal codescout MCP tool bug trackers
  (heavyweight, one per file). Promote F-N codescout entries to formal
  trackers when fix work begins.
- `docs/trackers/skill-frictions.md` — superpowers skill frictions
- `docs/trackers/tool-usage-patterns.md` — tool-selection quality (T-N)

**Severity scale (F-N only):**
- `low` — annoying, easy workaround, no data loss
- `med` — slowed work meaningfully; workaround exists but is ugly
- `high` — blocked progress until worked around; risk of silent bad output
- `crit` — silent corruption / lost work

**Impact scale (W-N only):**
- `low` — small efficiency gain
- `med` — caught something we'd plausibly have missed
- `high` — caught a problem that would have been expensive to debug later
- `prevented-disaster` — would have shipped a real bug without this pattern

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-05-17 | low | codescout-tool | promoted-to-bug-tracker | `read_file(@buf_id, json_path="$.symbols[0].body")` returns 0 lines (see bug-tracker.md #2) |
| F-2 | 2026-05-17 | low | codescout-tool | promoted-to-bug-tracker | `read_file(@buf_id, start_line=N, end_line=M)` empty when N is past midpoint (see bug-tracker.md #3) |
| F-3 | 2026-05-17 | low | self-friction | wontfix-false-alarm | Predicted `cargo test -p codescout` would fail — actually works fine |
| F-4 | 2026-05-17 | low | self-friction | wontfix-false-alarm | Predicted plan's `src/librarian/...` path was wrong — actually correct |
| F-5 | 2026-05-17 | low | subagent      | open | T-1 subagent placed cross-check test at module-level scope, not inside `mod tests` block as plan said |
| F-6 | 2026-05-17 | low | plan-prose     | open | T-2 Step 3 says "18 tests" but Step 1 fixture has 20 |
| F-7 | 2026-05-17 | med  | plan-prose     | open | T-3 title says `GatherSource::GoalChildren variant` but body adds a standalone fn, not an enum variant |
| F-8 | 2026-05-17 | high | plan-prose     | open | T-3's code snippet uses `cat.get(id).augmentation.archetype` — three field accesses that don't compile against actual structs |
| F-9 | 2026-05-17 | med  | architectural | open | Augmentation prompt stored per-artifact at creation; `archetype_goal().prompt_template` edits don't propagate to existing trackers without explicit re-augment |
| F-10 | 2026-05-17 | high | rust-serde    | mitigated | Serde `flatten + default` doesn't handle missing internally-tagged discriminator — custom `Deserialize` impl required |
| F-11 | 2026-05-17 | med  | codescout-tool | promoted-to-bug-tracker | `grep` on `@tool_*` buffer false-negatives on a string present in the buffer (see bug-tracker.md #4) |
| F-12 | 2026-05-17 | low  | plan-prose   | mitigated | T-12 plan payload sketch omitted required `note.text` field; event_create rejects silently if `let _ = ...await` swallows the error |
| F-13 | 2026-05-17 | high | multi-agent  | mitigated | `git reset --soft HEAD~1` on stale HEAD during concurrent work blew away parallel agent's T-13 commit; recovered via reflog SHA |
| F-14 | 2026-05-17 | med  | multi-agent  | open      | F-N namespace shared across concurrent sessions — parallel session's W-6 references "F-11" but their friction was renumbered to F-12 after mine landed first |
| F-15 | 2026-05-17 | high | codescout-tool | mitigated | `artifact_augment` schema description says `params` is "gather config"; actually it's the data params — calling `merge=false, params={}` wiped live tracker state (acceptance_signals, children, criterion, progress_log) |
| F-16 | 2026-05-17 | med  | codescout-tool | open      | `artifact_augment(merge=false)` also overwrites `render_template` / `params_schema` / `append_mode` / `history_cap` with `excluded.*` in `augmentation::upsert`'s `ON CONFLICT DO UPDATE` — schema description (post-F-15) still doesn't warn callers; passing them as None silently wipes them |


## Wins Index

| ID | Date | Impact | Title |
|----|------|-------:|-------|
| W-1 | 2026-05-17 | high   | Friction tracker forced externalization of T-3 plan-shape errors *before* subagent dispatch — F-8 caught by reading code, not by failing tests |
| W-2 | 2026-05-17 | med    | F-5 lesson absorbed into T-2 subagent prompt; T-2 ran first-try clean (test placement correct) |
| W-3 | 2026-05-17 | high   | Inline-vs-subagent decision made per-task (not per-skill-default) — T-3 inline saved ~25 min and eliminated re-dispatch risk |
| W-4 | 2026-05-17 | high   | Attempting end-to-end verification (live gather) caught release-binary deployment gap *before* declaring Phase 1 "shipped" — MCP runs release, dev builds invisible to live tools |
| W-5 | 2026-05-17 | high   | Post-MCP-reload gather returned populated `deterministic_child_statuses` array (3/3 children deterministic) + new "3 items gathered from..." hint — DF-1 empirically verified, not just structurally fixed |
| W-6 | 2026-05-17 | high   | Integration test for T-12 immediately caught F-12 (silent failure from swallowed event_create error) — `let _ = ...await` is dangerous without the test sandwiching it |
| W-7 | 2026-05-17 | high   | Reflog scouted via `git reflog -10` after destructive op; T-13 commit blown away by stale `HEAD~1` reset was recovered by quoting the SHA directly, not relative ref |
| W-8 | 2026-05-17 | high   | Single `artifact_refresh(action="gather")` call simultaneously verified T-3 (deterministic_child_statuses populated) + T-9 (refresh_meta with real delta `C-3 in-progress→done`) + DF-1 fix (context no longer `{}`) — multi-feature smoke test in one probe |
| W-9 | 2026-05-17 | high   | T-12 gate_check note event payload matched amendment D11 spec byte-for-byte at first live observation — `tag/gate_passed/text/evidence/refresh_at` all present and shaped correctly without iteration |
| W-10 | 2026-05-17 | high | F-15 muscle-memory drove scouting `augmentation::upsert` SQL before re-augmenting L1 with new prompt — surfaced both the surgical part (`created_at` / `last_refreshed_at` / `refresh_count` preserved by `ON CONFLICT DO UPDATE`) and the destructive part (`render_template` / `params_schema` / `append_mode` / `history_cap` overwritten, logged as F-16) |

## W-1 — F-8 caught before subagent dispatch via mandatory "log the friction first" discipline

**Observed:** 2026-05-17, T-3 reconnaissance.

**What happened:**
Per the workflow ("watch for friction or misses"), I scouted the actual struct
shapes for `ArtifactRow`/`AugmentationRow` before crafting the T-3 subagent
prompt. That scouting surfaced **three compile-breaking errors in the plan's
code snippet** — `cat.get(id).augmentation.archetype` chains through fields
that don't exist on any of the three real structs.

**Counterfactual (what would have happened without the tracker):**
If I'd dispatched a haiku subagent with the plan's literal code, it would have
hit ~10 compile errors mid-implementation, likely:
1. Burned 30-60s spinning on `archetype: field not found on AugmentationRow`
2. Either reported BLOCKED or invented a wrong workaround (e.g. fabricating
   a method, or guessing at SQL-derived archetype lookup)
3. Required a follow-up dispatch with corrected code, doubling token cost
4. Friction surface would have been the subagent's confused output, harder to
   read than a plan-text gap

**Why the tracker helped:**
The friction tracker (1) made "what's the actual shape?" a deliberate
pre-dispatch step rather than implicit faith, and (2) forced me to externalize
the discrepancy as F-8 with corrected shapes — which then became reference
material for the inline implementation.

**Pattern to keep:**
- Always scout the seams before delegating. Especially for tasks where the
  plan's code is more than ~30 LOC, or touches structs the plan author may
  not have re-verified.
- Externalize the discrepancy as a friction entry *before* implementing.
  The act of writing it down disciplines the reading.
- The user's "lets watch for friction" framing isn't ceremony — it's load-bearing.

**Status:** ongoing — keep applying to T-4..T-13.

---

## W-2 — Plan-lesson absorbed into next subagent prompt; pattern recurrence prevented

**Observed:** 2026-05-17, T-2 dispatch.

**What happened:**
T-1's subagent (F-5) placed the new cross-check test at module-level scope,
not inside the existing `mod tests` block. Plan said "In tests module" but
didn't anchor with explicit symbol insertion target. T-2's prompt was
amended with a "F-5 lesson" paragraph explicitly telling the subagent
"put the unit tests inside the `#[cfg(test)] mod tests { ... }` block exactly
as written in the plan. Don't promote, don't split."

**Counterfactual:**
Without the F-5 entry to refer to, the T-2 prompt would have been a copy of
the T-1 prompt with the same "In tests module" vague phrasing. T-2's
subagent would likely have repeated the same placement choice.

**What we observed:**
T-2 subagent placed all 20 tests inside `mod tests` as instructed.
First-try clean. No re-dispatch needed.

**Pattern to keep:**
- After each subagent run, scan for placement / scope / structural drifts
  from the plan, log them as F-N entries.
- Cite the F-N entry in the *next* subagent's prompt as a specific
  "don't repeat this" instruction.
- Frictions become prompt material. The log isn't just documentation — it's
  active load for prompt engineering across the session.

**Status:** validated; reuse for T-4, T-5 prompts.

---



## W-3 — Inline implementation chosen for T-3 paid off: clean first-compile, all tests green

**Observed:** 2026-05-17, T-3 implementation.

**What happened:**
After F-8 caught the plan's compile-breaking code, I had a choice: rewrite the
plan and dispatch a subagent on the corrected text, or implement T-3 inline.
I asked the user (AskUserQuestion) and they chose inline. The result:

- Function compiled clean on first `cargo build` (4s)
- 4 unit tests pass first-run
- 2 integration tests pass first-run
- clippy `-D warnings`: clean
- All 424 librarian tests pass

Total time from F-8 discovery to commit `c968391a`: ~5 minutes of focused
inline edits. **No iteration loops.**

**Counterfactual:**
The patch-plan-then-dispatch alternative would have meant:
1. Rewrite Task 3 in the plan (~10 min): replace ~50 LOC of broken code with
   ~50 LOC of correct code, update prose, add F-7/F-8 references
2. Commit the plan patch as its own change
3. Craft a subagent prompt re-quoting the corrected task (~5 min)
4. Dispatch (haiku model: ~3-5 min; sonnet: ~5-8 min)
5. Verify subagent output (~3 min)
6. Possible re-dispatch if subagent strayed (~10 min) — F-5-class risk
Total: 25-40 min, with re-dispatch risk.

Inline cost ~5 min; no re-work risk because the cost of an inline mistake is
my own next edit, not a subagent re-run.

**When inline wins:**
- Plan code is wrong in non-trivial ways (multiple compile errors, type-shape
  mismatches) — at that point the "corrected prompt" approaches "I'm
  implementing it anyway"
- The task touches <3 files and has clear architectural shape from
  reconnaissance
- Reconnaissance already happened — the data shapes are loaded into the
  controller's context, throwing them out to a subagent costs re-discovery
- The task is on the critical path (Phase 1 architectural pivot) and downstream
  tasks (T-4, T-5) depend on it being right

**When subagent dispatch wins:**
- Plan is reliable; cognitive cost is in execution not design
- Task is mechanical (boilerplate / fixture / config edit)
- Cost of mistake is bounded (test fails locally, retry cheap)
- Controller's context bandwidth is more valuable than the agent dispatch cost
- T-1, T-2, T-13 were good subagent candidates; T-3 wasn't

**Pattern to keep:**
Make the inline-vs-subagent call **per task**, anchored to plan quality + task
shape, not by default. The skill says "fresh subagent per task" but skill
principles aren't iron law when the task is the architectural pivot and the
plan has fictional code.

**Status:** validated as a heuristic. T-4 and T-5 should be subagent
candidates again (T-4 is prompt-text replacement, T-5 is a test addition).

---


## W-4 — Live verification attempt caught release-binary deployment gap pre-ship

**Observed:** 2026-05-17, post-T-5 end-to-end verification.

**What happened:**
After Phase 1 landed (all 5 commits, 424 tests pass), I called
`mcp__codescout__artifact_refresh(action=gather, id=d2cd00fc837e53f2)` on
the live L1 goal-tracker to confirm DF-1 was empirically resolved. The
response showed:
- The OLD pre-T-4 prompt (7-clause rule 1, not 1a/1b/1c)
- `context: {}` — still empty

**Counterfactual (the silent-failure path I almost took):**
Without attempting live verification, the natural next step would have
been to say "Phase 1 ships" and move to Phase 2. Future sessions would
have observed:
- `mcp__codescout__artifact_refresh(gather)` returning empty context
- Confusion: "the tests passed, why is gather still broken?"
- A potential spiral of trying to debug the code, missing the
  binary-deployment angle entirely

**Root cause (already documented in CLAUDE.md):**
> "To test changes via the live MCP server, always run `cargo build --release`
> first, then restart the server with `/mcp`. The MCP server runs the
> release binary — dev builds are not picked up."

The dev tests prove correctness; live MCP needs release binary + reload.

**Pattern to keep:**
- For any change that touches MCP tool behavior, run dev tests AND attempt
  one live tool call to confirm deployment path. The dev-vs-release gap
  is a known footgun; verification surfaces it cheaply.
- For complex multi-task work like Phase 1, build release at the end of
  the phase, not at each task. Bundle the deployment cost.
- Mention `/mcp` reload to the user explicitly — the tool's docs don't
  call it out and it's not auto-detected.

**Status:** validated — apply at the end of Phase 2 and beyond.

---


## W-5 — Live verification confirmed DF-1 empirically fixed after MCP reload

**Observed:** 2026-05-17, post-`/mcp` reload by user.

**What happened:**
`artifact_refresh(action=gather, id=L1)` returned:
```json
"context": {
  "deterministic_child_statuses": [
    {"child_id": "C-1", "archetype": "audit_issues",  "status": "active",      "basis": "deterministic"},
    {"child_id": "C-2", "archetype": "reflective",    "status": "done",        "basis": "deterministic"},
    {"child_id": "C-3", "archetype": "task_list",     "status": "in-progress", "basis": "deterministic"}
  ]
},
"hints": ["3 items gathered from deterministic_child_statuses"]
```

Compared to DF-6 pre-fix baseline (`context: {}`, `hints: []`), this is
the predicted post-fix shape. **All three children resolve deterministically
via the Rust kernel.** The hints array surfaces the count.

**Bonus observation (F-9 surfaced):** The `prompt` field in the same
response still shows pre-T-4 rule 1 because augmentation prompts are
stored per-artifact at creation. Without manual re-augmentation, the LLM
reads the old prompt — but the new context key still populates. Half-fix
is empirically visible; full fix needs the L1's prompt re-augmented.

**Closing the dogfood loop:**
DF-1's status flipped through three stages:
1. **Observed** 2026-05-17 — `context: {}` at goal creation
2. **Structurally fixed** by commit c968391a (T-3 wiring + 6 tests)
3. **Empirically verified** by this gather call after MCP reload

The eval signal is real: pre/post-fix diff of the gather response is
quantifiable and reproducible. The friction tracker's purpose paid off.

**Pattern to keep:**
- Always close the verification loop after a release-binary deployment.
- A "structural fix passes tests" is a smaller claim than "live MCP
  observation matches predicted shape." Both checks belong in the dogfood
  cycle.

**Status:** validated; archive when Phase 4 wraps and the audit issue
linked to DF-1 (if any) gets marked fixed.

**Independent confirming run (2026-05-17, continuation session after
compaction):** Re-ran `artifact_refresh(gather)` on the same L1 after a
fresh `/mcp` reload. Same shape, all three children resolve
deterministic, same statuses. **This is a second confirming data point
on independent infrastructure** (different conversation, post-compaction
context). The W-5 pattern (verification-after-deployment) reproduced
cleanly.

---


## W-7 — Reflog scouted after destructive op; SHA-quoting recovered parallel commit

**Observed:** 2026-05-17, continuation session, during F-13 recovery.

**Pattern:** After realizing my `git reset --soft HEAD~1` had moved HEAD backward through an unfamiliar commit (parallel agent's T-13), the response was: stop, run `git reflog -10`, *read the actual sequence of HEAD movements*, identify the SHA I needed to land on, and `git reset --soft <explicit-sha>`. Not a relative ref.

**Counterfactual:** Without the reflog scout, the natural next move would have been "git reset --soft HEAD~1 again" or "git commit --amend" — either of which would have permanently orphaned the parallel agent's T-13 commit. Reflog retention is 90 days by default, but in a public-repo cherry-pick-to-master workflow, an orphaned commit lost from the branch tip is effectively gone the moment it's pushed.

**Confirming data points:**
- This turn: reflog showed `8b067616 HEAD@{0}: reset...` and `d8b38f26 HEAD@{1}: commit: feat(audit_issues)... (T-13)`. The SHA `d8b38f26` was the recovery target.
- General pattern: every `git reset` during multi-agent work should be preceded by `git reflog -N` in the same command, and the destination should be a SHA, not a relative ref.

**Impact:** high — prevented silent destruction of another agent's commit. The reflog discipline is also the foundation for any future "undo my last N git ops" tooling.

**Promote-when:** This W should graduate into a CLAUDE.md `## Git Workflow` rule alongside the F-13 fix-idea. Recommended Iron-Law-class wording: *"During concurrent work on a shared branch, never `git reset` to a relative ref. Always: (1) run `git reflog` in the same command, (2) identify the explicit SHA, (3) reset to the SHA."*

**Status:** validated — one data point, but high-severity counterfactual. Promote at next CLAUDE.md edit batch.

---


## W-8 — Single gather call simultaneously verified 3 features (T-3 + T-9 + DF-1)

**When:** Live verification of T-9..T-13 post-MCP-reload.

**Pattern:** Call `artifact_refresh(action="gather", id=L1_goal_id)` once. In one response payload, read:
- `context.deterministic_child_statuses` → confirms T-3 (Yak's gather-time child resolution) is wired
- `context.refresh_meta` → confirms T-9 (Rust-owned refresh metadata) is computed and surfaced
- `context` is non-empty object (not `{}`) → confirms DF-1 (the original "gather returns empty context" bug) is fixed end-to-end

**Counterfactual:** Without this realization, would have written three separate verification probes (one per feature) and burned ~3× the round-trips. The gather endpoint is the natural integration point — every goal-tracker injection feature lands in `context`, so one call exposes all of them. Bonus: revealed F-15 (params wipe) by surfacing the absent params — which is *not* what the verification was looking for, but the smoke-test surface area caught it anyway.

**Confirming data points:**
- 1 gather call surfaced 3 features as observable JSON.
- `children_status_delta` revealed staleness (`C-3 in-progress → done`) — proved kernel resolved fresh state.
- Detected F-15 as a side-effect (params field empty in same payload).

**Impact:** high — pattern for any future archetype kernel verification: hit the gather endpoint first, drill into the keys it adds to `context`.

**Promote-when:** Reused on a second archetype (e.g., when failure_table or audit_issues grow their own kernel-time injection). At that point, document as "gather-as-integration-test" pattern in `docs/PROGRESSIVE_DISCOVERABILITY.md` or a new `docs/testing-archetypes.md`.

---

## W-9 — gate_check event payload matched D11 spec byte-for-byte at first observation

**When:** T-10/T-12 live verification (gate-pass path).

**Pattern:** Amendment D11 (`docs/superpowers/specs/2026-05-17-goal-tracker-amendment.md`) specified the exact gate_check note event shape: `{kind: "note", payload: {tag: "gate_check", gate_passed: bool, text: string, evidence: {children_count, children_done, signal_count_total, signal_count_met}, refresh_at: iso8601}}`. The T-12 implementer subagent (and the controller's fix in `augment.rs::ArtifactAugment::call`) had to derive the JSON shape from the amendment doc + the integration test sketch.

**Counterfactual:** Spec-impl drift is the most expensive late-binding bug in goal-tracker. If the payload had been missing a field or used a different key (`passed` vs `gate_passed`, or omitted `refresh_at`), no integration test would have flagged it — the note event just gets stored as-is, and consumers reading the timeline would silently get wrong/missing data. Catching this at first observation (not at "weeks-later consumer breakage") is high-impact.

**Confirming data points:**
- Live `artifact_event(list, kinds=["note"])` returned the exact field set: `tag, gate_passed, text, evidence{4 keys}, refresh_at`.
- `evidence.children_count: 3 / children_done: 3 / signal_count_total: 4 / signal_count_met: 4` matched expected.
- `text` was the human-readable summary from the spec ("auto-close gate passed: 3/3 children done, 4/4 signals met").

**Impact:** high — validates the discipline of writing the event payload schema *in the spec* (D11), not in implementer hands. The amendment doc was the source of truth; both the impl and the verification probe consumed it.

**Promote-when:** Reused for `external_signal` events or future audit-issues auto-promotion events. Then graduate "event payload defined in spec, validated in live probe" to a CONTRIBUTING.md addendum.

## W-10 — F-15 muscle-memory drove scouting `augmentation::upsert` before re-augmenting L1 with new prompt

**When:** H-8 close. After editing `archetype_goal().prompt_template` in source, the existing L1 dogfood goal `d2cd00fc837e53f2` carried the OLD frozen prompt in its augmentation row. Closing H-8 fully required a `merge=false` re-augment of L1 — the exact tool call F-15 had previously weaponized to wipe live data.

**Pattern:** Before issuing the merge=false call, read `augmentation::upsert` directly (`symbols(name="upsert", include_body=true)` on `src/librarian/catalog/augmentation.rs`). The body revealed two coexisting truths in the same SQL statement:

```sql
INSERT INTO artifact_augmentation (artifact_id, prompt, params, last_refreshed_at, refresh_count,
  created_at, updated_at, render_template, params_schema, append_mode, history_cap)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
ON CONFLICT(artifact_id) DO UPDATE SET
  prompt = excluded.prompt,
  params = excluded.params,
  render_template = excluded.render_template,
  params_schema = excluded.params_schema,
  append_mode = excluded.append_mode,
  history_cap = excluded.history_cap,
  updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
```

**Surgical (good):** `created_at`, `last_refreshed_at`, `refresh_count` are NOT in the UPDATE list — ON CONFLICT preserves them. So a re-augment is safe for refresh history.

**Destructive (caught):** `render_template`, `params_schema`, `append_mode`, `history_cap` ARE in the UPDATE list and use `excluded.*` — they get overwritten by whatever the caller sends, which is None/false if absent. This is F-15's shape extending past the `params` field.

For L1 specifically those four fields were already null/false, so the call was safe. But the discovery itself is the win: scout caught a future F-15 footprint that the post-F-15 schema description doesn't surface.

**Counterfactual:** Without scouting the upsert, I'd have either (a) skipped re-augmenting L1 entirely ("closing H-8 for new instances is enough") — leaving the dogfood goal carrying the broken old prompt and H-8 still effectively unresolved for the instance the audit had named, or (b) blind-fired the merge=false call and gambled. Path (a) accepts partial close; path (b) is F-15 redux on a goal with refresh_count > 0.

**Confirming data points:**
1. The upsert SQL was readable in one `symbols(...)` call and explained the semantic in 30 lines — the same body the `upsert_preserves_refresh_count_on_update` test pins.
2. F-16 came from the same scout pass — one read produced both a win and a finding.
3. Verification after the call: `artifact(get, id=d2cd00fc837e53f2, full=true)` showed the new prompt landed AND all 4 acceptance_signals + 3 children + 2 progress_log entries intact.

**Impact:** high — if H-8's close had skipped L1 re-augment, the dogfood would have invisibly continued producing the broken behavior the audit had logged, and the "fixed" status on H-8 would have been a lie for the instance that mattered.

**Promote-when:** never a single rule — the lesson is meta: **`merge=false` augment is partly surgical, partly destructive; read the upsert before each new call shape, not just the schema doc.** Belongs in the same future doc that captures F-15's lesson about reading the schema before calling.
## F-2 — `read_file(@buf_id, start_line=N, end_line=M)` empty when N is past midpoint

**Observed:** 2026-05-17, T-1 prep. After hitting F-1, fell back to
`read_file(path=@buf_id, start_line=35, end_line=76)`.

**Steps:**
```
symbols(path="src/librarian/tools/tracker_design.rs", symbol="archetype_goal", include_body=true)
→ output_id: @tool_34c4d80f (76-line body, but summary only shows ~34 lines)
read_file(path="@tool_34c4d80f", start_line=35, end_line=76)
→ {"content": "", "total_lines": 4}
```

**Expected:** Lines 35–76 of the buffered body.

**Got:** Empty string + `total_lines: 4` (which is wrong — the buffer should be 76 lines).

**Workaround used:** Re-read directly from the source file:
`read_file(path="src/librarian/tools/tracker_design.rs", force=true, start_line=300, end_line=340)`.

**Root cause hypothesis:** The output buffer for `symbols` truncates
body display to fit a token budget but reports the truncated tail-line
count when queried via start_line. The buffer is effectively only the
first N lines of the 76-line body, and `total_lines: 4` reports
remaining-after-N — not total.

**Fix pointer:** Same future bug tracker as F-1 (related).

**Status:** promoted-to-bug-tracker 2026-05-17 — see `docs/issues/bug-tracker.md` #3.

---

## F-3 — Plan's `cargo test -p codescout` invocation didn't match crate layout

**Observed:** 2026-05-17, T-1 execution by haiku subagent.

**Predicted symptom:** Plan text says `cargo test -p codescout --lib ...`
and the workspace `Cargo.toml` lists `members = [".", "crates/codescout-embed"]`
so I assumed `-p codescout` would fail.

**Actual observation when ran manually:**
```
$ cargo test -p codescout --lib librarian::tools::tracker_design::tests
running 6 tests
... all 6 pass, finished in 0.22s
```

**Reality:** The workspace root package is **named** `codescout` (the
`[package] name = "codescout"` in `Cargo.toml`), even though the
binary is in `src/main.rs`. So `-p codescout` IS the right package
specifier. The plan was correct; my prediction was wrong.

**Lesson:** Verify cargo metadata before guessing — `cargo metadata --format-version 1 | jq '.packages[].name'` would have caught this in seconds.

**Status:** wontfix-false-alarm. Keeping for the self-friction signal — pre-execution
friction predictions need evidence, not assumptions.
## F-4 — Plan referenced `src/librarian/...` paths; codescout reports `source: lib:librarian_mcp`

**Observed:** 2026-05-17, T-1 prep.

**Predicted symptom:** Plan path `src/librarian/tools/tracker_design.rs`
contradicted codescout's response carrying `"source": "lib:librarian_mcp"`.
I concluded the file must live at `crates/librarian-mcp/src/tools/...` and that
the plan was wrong.

**Actual observation:**
```
$ find . -name "tracker_design.rs" -not -path "./target/*" -not -path "./.worktrees/*"
./src/librarian/tools/tracker_design.rs
```
**One file. Project root. Exactly where the plan said.**

The `source: lib:librarian_mcp` annotation is codescout's *registered library*
metadata for cross-project semantic search — the file is **registered as part of
a library named librarian_mcp** for indexing purposes, but its on-disk path is
`src/librarian/tools/tracker_design.rs`. Other branches in `.worktrees/`
have a separate `crates/librarian-mcp/` extraction in progress; that's not
the current branch.

**Reality:** Plan was correct. Codescout's `lib:` label confused me.

**Lesson:** When codescout returns `source: lib:X`, treat as cross-project
indexing metadata, not as a path hint. Confirm path with `tree` or `find`.

**Status:** wontfix-false-alarm. Keeping for the codescout-tool UX signal —
`source: lib:X` is a known confusion point worth surfacing in tool docs.
## F-5 — T-1 subagent placed cross-check test at module-level scope, not inside `mod tests`

**Observed:** 2026-05-17, T-1 post-execution review. Cargo test filter
`librarian::tools::tracker_design::tests` returned 6 tests, not the
expected 7. Hunting found `archetype_goal_prompt_contains_all_rule_1_constants`
at file lines 531-554, *outside* `mod tests` (which spans 558-761).

**Plan text said:** "In src/librarian/tools/tracker_design.rs tests module:"

**Got:** Test at the top-level of the parent module, with `#[test]`
annotation. Uses bare `archetype_goal()` and bare const names (no
`super::` prefix) since it's already in the parent scope.

**Steps to reproduce the spec-compliance gap:**
```
symbols(path="src/librarian/tools/tracker_design.rs") shows:
  Function 531-554 archetype_goal_prompt_contains_all_rule_1_constants
  Module   558-761 tests
                   tests/{6 children, new test NOT among them}
```

**Functional impact:** None. Rust accepts `#[test]` at any module scope.
Test runs correctly with `cargo test --lib librarian::tools::tracker_design::archetype_goal_prompt_contains_all_rule_1_constants`
(omit `tests::` from the path).

**Spec-compliance impact:** Subagent claimed "All 7 tracker_design
tests pass" implying the new test joined the existing 6 inside
`mod tests`. The truthful count is 6 in `mod tests` + 1 sibling.
Two-stage review would have caught this on a `find_in_module_tests`
assertion.

**Root cause hypothesis:** Subagent's `edit_code(insert)` placed the
new function relative to `archetype_goal`'s symbol, not inside the
`tests` module symbol. Plan said "tests module" but didn't anchor
with explicit `symbol="tests"` insertion target.

**Fix candidates:**
1. Relocate the test inside `mod tests` (cosmetic only — adds `super::` prefixes)
2. Leave it — Rust idiom allows both, and the test reads cleaner without prefixes
3. Update plan to be precise about insertion target for future subagents

**Recommendation:** Option 3 (update plan template wording). Don't
relocate the test — it works, and the file already passes clippy.
Pin this for the eventual plan-prose corrections commit.

**Status:** open (deferred — cosmetic).

---

## F-6 — Plan Task 2 Step 3 says "all 18 tests pass" but Step 1 fixture has 20 tests

**Observed:** 2026-05-17, T-2 verification.

**Plan text:**
- Step 1 fixture defines 20 `#[test]` functions (counted by hand and via codescout `symbols`)
- Step 3 text reads: "Expected: all 18 tests pass."

**Got:** Cargo runs 20 tests. Off-by-2.

**Root cause hypothesis:** Plan author wrote Step 3 first with an
estimate ("about 18 tests"), then expanded Step 1's fixture to 20 but
didn't update Step 3.

**Impact:** Harmless — the 2 extra tests are `failure_table_done_when_all_pass`
and `failure_table_active_when_any_fail` (added rigour beyond the rough
estimate). Subagent silently accepted the higher count.

**Fix candidates:**
1. Update plan Step 3 to read "all 20 tests pass" — accurate but cosmetic
2. Replace count with "all tests in Step 1's fixture pass" — robust to
   future fixture growth
3. Leave alone; this kind of drift is expected

**Recommendation:** Option 2 in the plan-corrections commit.

**Status:** open (deferred — cosmetic).

---

## F-7 — T-3 plan title says `GatherSource::GoalChildren variant` but body adds a standalone fn

**Observed:** 2026-05-17, T-3 reconnaissance.

**Plan text:**
- Section title: `### Task 3: GatherSource::GoalChildren variant + dispatch`
- Implementation in Step 3: `pub fn gather_goal_children(ctx: &ToolContext, children: &[(...)]) -> Result<Value>` — a standalone function, NOT a new enum variant.

**Got:** Architecturally simpler than the title suggests — no enum-variant
plumbing through `gather_all`'s `match` dispatch. The function is called
directly from `refresh.rs::call` after `gather_all` returns.

**Root cause hypothesis:** Plan was iterated mid-write. Early sketch
proposed a `GatherSource::GoalChildren { children: Vec<...> }` enum
variant, then was simplified to a sibling function. Title wasn't updated.

**Impact:** Subagents reading only the title would invest effort
extending the enum (touching `gather_all`'s match, `Args` deser, etc.)
when the body says don't.

**Fix candidates:**
1. Update plan title to `### Task 3: gather_goal_children helper + refresh dispatch`
2. Restore the enum-variant approach (more uniform but heavier — needs a way
   to inject children into the gather sources list from outside the
   refresh-call path)

**Recommendation:** Option 1 in the plan-corrections commit. The
function approach is sound.

**Status:** open (deferred — will be patched alongside other plan-prose fixes).

---

## F-8 — T-3 plan code snippet uses `cat.get(id).augmentation.archetype` — wrong on three counts

**Observed:** 2026-05-17, T-3 reconnaissance. Verified actual struct shapes via codescout `symbols(name=..., include_body=true)`.

**Plan code:**
```rust
let row = cat.get(artifact_id);
let archetype = r.augmentation
    .as_ref()
    .and_then(|a| a.archetype.clone())
    .unwrap_or_default();
let params = r.augmentation
    .as_ref()
    .map(|a| a.params.clone())
    .unwrap_or(Value::Null);
```

**Actual struct shapes:**

`ArtifactRow` (`src/librarian/catalog/artifact.rs:8-24`):
```rust
pub struct ArtifactRow {
    pub id, abs_path, kind, status, title, owners, tags,
    pub topic, time_scope, source, created_at, updated_at,
    pub file_mtime, file_sha256, confidence,
}
// NO `augmentation` field. Augmentation lives in a separate SQL table.
```

`AugmentationRow` (`src/librarian/catalog/augmentation.rs:6-24`):
```rust
pub struct AugmentationRow {
    pub artifact_id, prompt, params (raw JSON String),
    pub last_refreshed_at, refresh_count, created_at, updated_at,
    pub render_template, params_schema, append_mode, history_cap,
}
// NO `archetype` field. Params is a raw JSON String, not Value.
```

**Three errors in the plan code:**
1. `cat.get(id)` returns `Option<ArtifactRow>` — no `.augmentation` field.
2. Correct path: `augmentation::get(&cat, artifact_id)` returns `Option<AugmentationRow>`.
3. `AugmentationRow.archetype` doesn't exist. The archetype must come from elsewhere — the plan's own fallback hint says "infer from the goal's own `children[].archetype` field" — so the **caller** (refresh.rs) extracts archetype from the goal's params and passes it in.

**Correct signature:**
```rust
pub fn gather_goal_children(
    ctx: &ToolContext,
    children: &[(String, String, String)], // (child_id, artifact_id, archetype)
) -> Result<Value>
```

**Inside the function:**
```rust
let aug_row = augmentation::get(&cat, artifact_id)?;
let params: Value = aug_row
    .as_ref()
    .map(|a| serde_json::from_str(&a.params).unwrap_or(Value::Null))
    .unwrap_or(Value::Null);
let status = child_status_pure(archetype, &params);
```

**Impact:** A subagent following the plan literally would generate ~10 compile errors before discovering the truth. With this F-8 entry on file, future subagent prompts can cite "see F-8 for correct shapes" or include the corrected code inline.

**Status:** open — will be fixed when T-3 implementation lands (inline,
since the corrections are too involved to delegate without a fully
rewritten prompt).

---

## F-9 — Augmentation prompt stored per-artifact; template edits don't propagate

**Observed:** 2026-05-17, post-T-4 live verification.

**Symptom:** Phase 1 commits landed. Release binary rebuilt. MCP reloaded.
Re-ran `artifact_refresh(action=gather, id=L1)` expecting new 1a/1b/1c prompt.
Got: **OLD pre-T-4 prompt** (7-clause rule 1) in the response, even though
`context.deterministic_child_statuses` now populates correctly (the T-3
half of the fix is live).

**Root cause:** `artifact_augment(id, prompt=...)` writes the prompt string
into the `artifact_augmentation` SQL row at creation time. The string is
stored verbatim — there's no template-resolution at refresh time. When
`archetype_goal()` in tracker_design.rs changes, it only affects **future**
trackers created via `librarian(action="tracker_design", intent="goal: ...")`
+ `artifact(create, augment=...)`. Existing trackers keep their original
prompt.

**Impact:** Live verification of prompt changes for an existing goal-tracker
requires explicit re-augmentation (`artifact_augment(id, merge=false,
prompt=<new from tracker_design output>)`). Without that step:
- The pipeline correctly injects deterministic_child_statuses (T-3)
- But the LLM reads the OLD prompt and doesn't know to consult that context
- Net effect: the new context key is invisible to the LLM through the
  old prompt's rule 1 phrasing

**Fix candidates:**
1. **Manual refresh per tracker** — call `artifact_augment(id=L1, merge=false,
   prompt=<fresh>)` after every relevant `archetype_goal()` edit. Tedious;
   easy to forget.
2. **Template field** — store the archetype name on the augmentation row,
   re-resolve prompt at refresh time from `archetype_goal()` if it matches.
   Architectural change; bigger.
3. **Bump trigger** — add a "prompt_version" field; refresh detects mismatch
   and offers re-augmentation. Middle ground.

**Recommendation:** Document in F-9 for now; ship option 1 manually
post-Phase-1 for the L1 goal-tracker; defer option 2/3 to a later
architectural pass.

**Status:** open (deferred — workaround documented).

---

## F-10 — Serde `flatten + default` doesn't handle missing internally-tagged discriminator

**Observed:** 2026-05-17, T-6 implementation.

**Symptom:** Plan code:
```rust
#[serde(default = "default_freeform")]
#[serde(flatten)]
pub kind: AcceptanceSignalKind,
```
Combined with `#[serde(tag = "kind", rename_all = "snake_case")] enum AcceptanceSignalKind { Freeform, ... }`.

Backward-compat test (legacy signal `{"description":"x","met":true}` without
`kind` field) failed with:
```
Error("missing field `kind`", line: 1, column: 45)
```

**Expected:** Plan's design suggests when `kind` is absent, `default_freeform()`
fires and the signal deserializes as `Freeform`.

**Got:** Serde's internally-tagged enum handling looks for the `kind` tag
BEFORE checking defaults. Missing tag = hard error, regardless of
`#[serde(default)]` on the field.

**Root cause:** Known serde limitation. `#[serde(flatten)]` on a field
typed as an internally-tagged enum means the discriminator is read
from the parent struct's JSON object. If the discriminator is absent,
serde dispatches "no variant matched" before any field-level default
kicks in.

**Fix applied:** Replaced derive with a custom `Deserialize` impl on
`AcceptanceSignal` that probes for `kind` and falls back to
`AcceptanceSignalSpec::Freeform` when absent:

```rust
impl<'de> Deserialize<'de> for AcceptanceSignal {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(d)?;
        // ... probe each field, fall back to Freeform if kind missing
    }
}
```

Now both `{"description":"x","met":true}` and
`{"description":"x","met":true,"kind":"freeform"}` parse identically as
Freeform.

**Pattern to keep:**
- Serde derive shortcuts work great until they don't. For backward-compat
  on tagged enums, expect to write a custom Deserialize when the legacy
  JSON shape predates the tag.
- 16 unit tests including the legacy-shape test caught this immediately
  — adding the contract test in the same commit prevented silent shipping.

**Status:** mitigated (custom impl ships). Could be reopened if a future
Rust version of serde supports the combo natively.

---

## F-11 — `grep` on `@tool_*` buffer false-negatives on a string present in the buffer

**Observed:** 2026-05-17, continuation session, during W-5 re-verification.

**When:** After `artifact_refresh(gather)` returned `@tool_351afcd6` (10007
bytes), tried to confirm presence of the new context key via:

```
grep(pattern="deterministic_child_statuses", path="@tool_351afcd6", context_lines=15)
```

**Expected:** ≥1 match (the buffer contains the key — verified via
subsequent `read_file` showing it at ~line 78).

**Got:** `{"matches": [], "total": 0}` plus a suggestion to use
`symbols(name='deterministic_child_statuses')` since "Pattern looks like
a symbol name."

**Probable cause:** Either `grep` on `@tool_*` buffers doesn't operate on
the raw buffer text the way it does on filesystem paths, OR the
pattern's `_` boundary tripped a tokenizer-style match heuristic, OR
the suggestion routing intercepted the search before it ran. Falling
back to `read_file(path=@tool_, json_path=...)` worked correctly on the
same buffer — so the data IS reachable, just not via `grep`.

**Workaround:** Use `read_file(path=@tool_, json_path="$.field")` or
`read_file(path=@tool_, start_line=N, end_line=M)` for buffer
inspection. Reserve `grep` for filesystem paths.

**Severity:** med — silently mis-routes verification queries; the
false-negative cost is potentially expensive ("Oh, the fix didn't
land!") if the user doesn't re-check via another tool.

**Status:** promoted-to-bug-tracker 2026-05-17 — see `docs/issues/bug-tracker.md` #4. Original observation: codescout tool friction; med severity (silently mis-routes verification queries).

**Fix idea:** Either `grep` on `@tool_*` should run on raw text and not
emit the "looks like a symbol" suggestion (the suggestion is
filesystem-context advice misapplied to a JSON buffer), OR the docs
should explicitly say `grep` is filesystem-only — `read_file` is the
buffer-inspection tool.

---

## F-12 — Plan T-12 payload sketch omitted required `note.text` field; silent failure

**Observed:** 2026-05-17, T-12 implementation.

**Plan sketch:**
```rust
artifact_event::create(ctx, &goal_id, EventKind::Note, json!({
    "tag": "gate_check",
    "gate_passed": ...,
    "evidence": {...},
    "refresh_at": now.to_rfc3339(),
}))
```

**Got after first run:** `gate_check_note_event_emitted_on_autoclose` test
asserted exactly one gate_check note in the timeline. Saw zero.

**Root cause:** `event_create::validate_payload` requires every `note`
kind payload to carry a `text` string field. Plan sketch's payload
shape passed `tag/gate_passed/evidence/refresh_at` but no `text`.
Validator rejected with `RecoverableError("note.text required")`.

**Silent failure path:** I'd used `let _ = event_create::call(...).await;`
to make emission best-effort (don't fail the augment if audit emission
fails). The discarded error masked the validation rejection — the
gate_check audit was silently dropped on every passing gate.

**Fix:** Added `text` field with a human-readable summary:
```rust
"text": format!("auto-close gate passed: {}/{} children done, {}/{} signals met", ...)
```
Test now passes; payload validates.

**Pattern to keep:**
- Integration tests catch silent-failure bugs that unit tests can't.
  The unit test would have proven evaluate_gate works; only the
  integration test surfaces that the downstream event emission also
  works.
- `let _ = .await` is dangerous for any tool call that has a validator
  upstream — the error is the only signal. Replace with explicit
  match-and-log when the failure mode matters.
- When adapting plan-sketch payloads, scan the receiving validator's
  required-fields list before assuming the sketch is complete.

**Status:** mitigated (text field shipped; W-6 logs the test sandwich
that caught it).

---

## F-13 — `git reset --soft HEAD~1` on stale HEAD erased parallel agent's intervening commit

**Observed:** 2026-05-17, continuation session, during attempted amend of my reconnaissance commit.

**When:** Tried to surgically remove "contested" content (DF-1 status block + i1-session-friction.md additions authored by parallel session) from my own commit `8b067616`. Ran `git reset --soft HEAD~1` thinking HEAD~1 was the pre-my-commit state. By the time the command actually executed, the parallel agent had committed T-13 (`d8b38f26`) on top. **HEAD~1 was now MY commit, and HEAD itself was the parallel agent's T-13.**

**Expected:** Reset moves HEAD back to before my commit, leaves my staged content for re-edit.

**Got:** Reset moved HEAD back through the parallel agent's T-13 commit — `d8b38f26` vanished from the branch tip. Their entire `tracker_design.rs` schema-widening work was sitting in my index as if I were about to commit it. **If I had recommitted, T-13 would have become orphaned in reflog only.**

**Probable cause:** `git reset` semantics evaluate `HEAD~N` at execution time, not at observation time. There's no transaction between `git log` (read) and `git reset` (write). With another agent actively committing, HEAD can move arbitrarily in the gap.

**Workaround:** Caught via `git reflog -10`, which showed the destructive move:
```
8b067616 HEAD@{0}: reset: moving to HEAD~1
d8b38f26 HEAD@{1}: commit: feat(audit_issues)... (T-13)
8b067616 HEAD@{2}: commit: docs: ship reconnaissance substrate
```
Recovered by `git reset --soft d8b38f26` — quoting the explicit SHA, not a relative ref. Branch restored, both commits intact.

**Severity:** high — silent destruction of another agent's work. Recovery only possible because reflog retained the SHA and I noticed within seconds. A retention-window or longer gap and the commit would have been irrecoverable except via the parallel agent's local reflog.

**Status:** mitigated — recovered this incident; root cause (HEAD-relative ops during concurrent work) un-fixed without convention change.

**Fix idea:**
- Convention: never `git reset HEAD~N` during concurrent work — always quote the explicit SHA after re-reading `git log` in the same command.
- CLAUDE.md `## Git Workflow` rule update (proposed in same session).
- Longer-term: worktree-per-session would have prevented the race entirely (see W-5 design notes + the multi-agent worktree-UX design candidate).

---

## F-14 — F-N namespace shared across concurrent sessions; cross-references go stale

**Observed:** 2026-05-17, continuation session, while scouting the trackers after parallel agent stopped.

**When:** Cross-checking parallel agent's W-6 entry. Discovered the index row reads "Integration test for T-12 immediately caught **F-11** (silent failure from swallowed event_create error)..." — but in the current index, F-11 is *grep on `@tool_*` buffer* (mine), and F-12 is *silent failure from swallowed event_create error* (theirs).

**Expected:** Cross-references in the wins index should point to the friction they actually caught.

**Got:** Stale cross-reference. The parallel session originally allocated their friction as F-11. My F-11 landed first via commit `8b067616`. Their friction got renumbered to F-12 in the index. Their W-6 body / index row was never updated to reflect the new ID.

**Probable cause:** F-N namespace is shared (one global counter per tracker file) but allocated independently by each concurrent session. There is no locking, no atomic "next free ID" allocation, no visibility into what the other session is about to add.

**Workaround:** Fixed inline this turn (W-6 row updated F-11 → F-12). Manual editorial pass after-the-fact.

**Severity:** med — failure mode is silent: the broken reference reads plausibly until someone tries to follow it. Worse, the cost grows with cross-reference depth (W-N → F-N → "fixed by commit XYZ" chains break in cascade).

**Status:** open — design gap, not just an editorial slip.

**Fix idea (sketch — earns its keep after one more recurrence per Snow Lion two-concretes rule):**
- **Cheap fix:** session-prefixed IDs, e.g. `S1-F-1`, `S2-F-1`. Decentralized, zero infra, immediately namespaced. Cost: longer IDs, prefix lookup needed when promoting to permanent docs.
- **Correct fix:** coordinator-allocated IDs via a librarian artifact (atomic claim of next free ID). Requires infra; benefit only after >2 concurrent sessions become common.
- **Defer for now:** add to the multi-agent design candidate alongside worktree-UX (F-13 + F-14 are two concretes for the same fault line — shared resources with no transaction between observe and act).

---


## F-15 — `artifact_augment` schema description says `params` is gather config; actually it's data params

**When:** Re-augmenting L1 goal-tracker (`d2cd00fc837e53f2`) to pick up post-T-8 prompt. Called `artifact_augment(id=..., prompt=<fresh>, params={})` based on the tool's JSON schema description: `"Optional gather config (gather_from, format, max_tokens). Defaults to {}."`.

**Expected:** Either (a) `params={}` is ignored because no gather config is being set, or (b) `params={}` writes an empty gather config (gather_from/format/max_tokens were never set anyway), leaving the data params untouched.

**Got:** `params={}` overwrote the entire data params payload. The L1 augmentation row went from `{acceptance_signals, children, criterion, progress_log, status}` (4 signals, 3 children, 1 progress entry) to `{}`. Discovered when the next `artifact_refresh(action="gather")` call returned `params: {}` and `context: {}` (because the goal-tracker injection skips when `params.acceptance_signals + params.children` are absent).

**Probable cause:** The schema description on `src/librarian/tools/augment.rs:50` is **stale**. Reading `Args.params` field at line 16: `params: Option<Value>`. Reading the create/replace path (`if !a.merge` at line 228+): `prompt` is required, `params` is folded into the augmentation row as the canonical data params payload. The description was likely copied from an earlier `gather` config concept that never landed. The merge=true and merge=false paths *both* treat `params` as data params — the description applies to neither.

**Workaround:** Recover from in-scrollback snapshot. The previous `artifact(action="get")` response (taken minutes earlier) had the full params under `augmentation.params`. Re-augmented with `merge=false, prompt=<fresh>, params=<full restored payload>`. State byte-identical to pre-incident.

**Severity:** high — silent destructive op. No error raised, no preview, no undo. If the artifact_get snapshot hadn't been in scrollback (e.g., post-`/compact`), recovery would require git-archeology against the SQLite catalog or a fresh body parse.

**Status:** mitigated (live tracker recovered) — but the schema description still misleads.

**Fix idea:**
1. Update `src/librarian/tools/augment.rs:50` description: `"The data params payload. On merge=false (default), fully replaces existing params. On merge=true, RFC 7396 merge-patched into existing params."`
2. Consider: refuse `merge=false, params={}` if an augmentation already exists and has non-empty params — require explicit `params=null` or a confirmation flag. This is the same destructive-op-on-stale-state pattern as F-13 (git reset on stale HEAD).
3. Add a regression test: `merge_false_with_empty_params_on_existing_augmentation_warns_or_refuses` (TBD on semantics — but the silent wipe is the worst outcome).

**Cross-reference:** Same family as F-13 — destructive op on shared mutable state, no scout step, no confirmation. The reconnaissance discipline this session loaded would have caught this if applied to the tool itself (read `ArtifactAugment::call` before invoking with `params={}`).

## F-16 — `artifact_augment(merge=false)` also wipes `render_template` / `params_schema` / `append_mode` / `history_cap`

**When:** H-8 close. Re-augmenting L1 dogfood `d2cd00fc837e53f2` with a new prompt. Post-F-15 the schema description warns that `params` is data and full-replaces on merge=false; it doesn't warn that the sibling fields (`render_template`, `params_schema`, `append_mode`, `history_cap`) ALSO get clobbered.

**Expected:** After F-15's schema description fix landed (commit `2005d9fa`), `merge=false` would only put `params` at risk; sibling fields would be preserved across re-augments.

**Got:** `augmentation::upsert`'s `ON CONFLICT DO UPDATE` clause lists six fields with `= excluded.<field>`: `prompt`, `params`, `render_template`, `params_schema`, `append_mode`, `history_cap`. All six come from the caller's `Args` payload — if absent in the call, they default to None/false and overwrite whatever the existing row had. So a naive "just update the prompt" merge=false call wipes any non-default render_template, schema, append_mode, or history_cap silently.

**Probable cause:** F-15 fix narrowed to the `params` field's description because that was the field weaponized in the original incident. The same destructive semantic on the four sibling fields wasn't surfaced because the original failure mode didn't touch them. Single-instance bug-fix didn't generalize to the broader class.

**Workaround for this session:** L1 has all four fields at their default values (None / false), so my merge=false call had no destructive footprint. Confirmed by reading the upsert SQL before issuing the call (see W-10) and verifying L1's augmentation row after.

**Workaround in general:** Before any `merge=false` re-augment, read `augmentation::upsert` directly OR query the existing row and pass every non-default sibling field back in the call. Schema description still doesn't tell callers this — a future fix should either (a) widen the description to cover all six replaced fields, or (b) restructure the upsert to only overwrite fields the caller explicitly named.

**Severity:** med — silent state loss for any tracker with a non-default render_template, schema, append_mode, or history_cap. Doesn't fire on the common case (most trackers don't set them), but a re-augment on the wrong tracker shape is hard to detect after the fact.

**Status:** open — description-only fix is straightforward but I'm not stacking it on this commit; H-8's close is the focused work. F-16 is the next-session candidate alongside H-9 / H-10 / S-4 follow-ups.
