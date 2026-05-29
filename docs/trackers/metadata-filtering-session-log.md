# Session Log — Template

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries via `edit_markdown(action="insert_before", heading="## Template
> for new entries", content=...)`. Add a row to the Index / Wins Index
> table for each new entry — the indexes are the eval surface, the
> sections are the evidence.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-05-28 | med | architectural | fixed-verified | Filter engine is compile-to-SQL only; entry-grain filtering needs a new in-memory evaluator |
| F-2 | 2026-05-28 | med | architectural | fixed-verified | tracker_design archetypes already structure entries, but heterogeneously — no common collection contract for a generic filter |
| F-3 | 2026-05-28 | med | release-pipeline | fixed-verified | Review-polish fix-up verified with check+clippy, not cargo test — shipped a 4-test schema-version regression |
| F-4 | 2026-05-29 | high | tooling-output | fixed-verified | artifact(get, full=true) buffered body truncated at ~36 KB silently dropped U-19..U-25 during retrofit parse |
| F-5 | 2026-05-29 | med | api-ergonomics | open | params_schema/render_template change forces full merge=false re-send (silent-reset foot-gun) |
| F-6 | 2026-05-29 | low | api-surface | open | No artifact(delete) action — throwaway cleanup needs rm + reindex; orphans augmentation |
| F-7 | 2026-05-29 | med | silent-failure | open | entry_filter silently returns empty on unknown/typo'd field (asymmetric with SQL engine error) |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-05-29 | high | Reconcile parsed buffered-body entry count against preview.headings before structured writes | Incomplete 15/22-entry index ships silently; entry_filter under-reports with no error or diff | validated |

---

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Copy this block when appending a new friction. Allocate the next free
ID. Add a matching row to the Index table.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Copy this block when appending a new win. A win without a
**Counterfactual** is marketing — name what would have happened
without the pattern, with at least one piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Status:** validated | promoted-to-permanent-docs | archived

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — Filter engine is compile-to-SQL only; entry-grain filtering needs a new in-memory evaluator

**Observed:** 2026-05-28, pre-spec reconnaissance while brainstorming a metadata-filtering feature for trackers.

**When:** About to write a design doc whose approaches claimed they "reuse the filter engine" to filter entries *within* a tracker (not just across artifacts).

**Expected:** `src/librarian/filter.rs` exposes a reusable evaluator that can match the `{field:{op:value}}` AST against arbitrary key/value maps, so per-entry tracker rows could be filtered with the same syntax used for artifact frontmatter.

**Got:** The AST *type* is reusable (`FilterNode` enum lines 10-15; `LeafOp` + `FromStr` lines 18-48), but the ONLY engine is `compile(&FilterNode) -> SqlFragment{sql, params}` (lines 91-243) which emits a SQL `WHERE` clause. Two blockers on reusing it at entry grain: (1) `ALLOWED_FIELDS` (lines 75-89) is a hardcoded allowlist of artifact *columns* (`kind, status, topic, ..., id`) and `compile_leaf` rejects any field outside it (anti-injection) — so arbitrary entry fields like `category` / `severity` are rejected outright; (2) op semantics live in SQL (`LIKE %v%`, `EXISTS json_each(...)` array membership, `LIKE 'v%' ESCAPE '\'`) and must be re-implemented in Rust for an in-memory path. All callers (`find`, `count_matching`, `catalog_summary` in `catalog/find.rs`) go AST -> SQL.

**Probable cause:** The filter subsystem was designed for catalog-grain filtering (artifact frontmatter columns) only; there was never a need to evaluate the AST in memory against non-column data.

**Workaround:** Any entry-filter design must budget a NEW `eval_leaf(&FilterNode, &serde_json::Map) -> bool` evaluator (~mirrors `compile_leaf`'s 116 lines, minus the `ALLOWED_FIELDS` gate, since an in-memory match has no injection surface) PLUS a dual-engine consistency test asserting SQL-compile and in-memory-eval agree on a shared fixture. Do not claim "free reuse of the filter engine" in the spec.

**Severity:** med — without this catch, the design doc would have under-scoped the entry-filter component; a subagent implementing from it would have hit the `ALLOWED_FIELDS` rejection wall (or routed entries through SQL) and forced a mid-build plan revision.

**Status:** fixed-verified — 2026-05-29 verify-open pass. Shipped `pub fn eval` + `eval_leaf` with the `ALLOWED_FIELDS` gate dropped (`src/librarian/filter.rs:250,287`) and the dual-engine consistency test `eval_matches_compile_on_fixture` (`filter.rs:588`); commits 41445120 (eval), cbf1f0fe (consistency test). Verified live this session via `entry_filter` on 4 retrofitted trackers.

**Fix idea / Pointer:** Feeds the metadata-filtering design (this work stream). Candidate shape: an `eval` fn beside `compile` in `filter.rs` sharing `LeafOp` + value-coercion; in-memory path drops the allowlist; add a consistency test. Re-scope severity once the design picks params-source vs body-parse vs SQL-table.

---
## F-2 — tracker_design archetypes already structure entries, but heterogeneously — no common collection contract

**Observed:** 2026-05-28, scouting `tracker_design` (piece-2 seam) during the metadata-filtering brainstorm.

**When:** About to claim the spec would introduce a NEW "filterable entries" convention and a NEW tracker_design archetype to teach it.

**Expected:** Structured per-entry data in params is a new convention this feature must introduce; `tracker_design` needs a new archetype.

**Got:** `tracker_design` (`src/librarian/tools/tracker_design.rs`) ALREADY ships archetypes whose params hold structured, schema-validated entry collections — `archetype_failure_table` (line 79) keeps `params.failures: [{id,status,owner,last_seen,notes}]` with an enum-constrained `status` and a `render_template` that already does filtered counts (`selectattr("status","equalto","fail")`). `task_list`/`goal` keep `children`. `child_status_pure(archetype, params)` (`goal_aggregation.rs:43`) already maps archetype+params -> status. The SYSTEM_PROMPT already teaches the params / params_schema / render_template split. BUT each archetype names its collection differently (`failures` vs `children`) — there is NO common key or pointer a generic entry-filter engine could target. Separately, `archetype_reflective` (line 257) is the Path-B shape: params deliberately minimal, "the body IS the tracker" — this is what the prose W/F session logs are, and what retrofitting converts FROM.

**Probable cause:** Archetypes were each designed for refresh/render in isolation, never for a cross-archetype query engine; no uniform entry contract was ever needed.

**Workaround / design pivot:** (a) Do NOT introduce a redundant convention — the entry-in-params pattern exists (`failure_table` is the exemplar). (b) DO add a common **`entry_collection` pointer** on the augmentation naming which params array is the filterable collection (e.g. `"failures"`), or a reserved `entries` key. (c) The F-1 engine filters whatever the pointer names. (d) Retrofitting = `reflective`-shape -> `failure_table`-shape transform. This also means the scout PREVENTED a redundant parallel convention + an engine built against a single invented shape that would silently miss the 2+ existing collection shapes.

**Severity:** med — without this, the engine would target one invented shape and silently fail on existing archetype collections, or a forked convention would split the tracker model.

**Status:** fixed-verified — 2026-05-29 verify-open pass. The `entry_collection` augmentation pointer shipped (commit 6fffc05c; taught in tracker_design via eb04a149) and was used to retrofit 4 trackers this session (tool-usage-patterns, codescout-usage-frictions, codescout-usage-hookify, skill-frictions).

**Fix idea / Pointer:** Spec must define the `entry_collection` pointer + its home (augmentation field vs reserved params key). Scout the augmentation params/record shape before the spec pins that. Feeds F-1's engine target.

---
## F-3 — Review-polish fix-up verified with check+clippy, not cargo test — shipped a 4-test regression

**Observed:** 2026-05-28, executing the filterable-trackers plan (Task 1 review cycle, subagent-driven).

**When:** Code-quality review of Task 1 flagged a missing `schema_version` stamp for the new v7 column as Minor ("v4/v5 stamp their versions; v6 omits one — may be deliberate"). A fix-up subagent added `INSERT OR IGNORE INTO schema_version (version) VALUES (7)` to `run_migrations`.

**Expected:** A harmless consistency improvement matching the v4/v5 stamp pattern.

**Got:** The stamp bumped the schema head to 7, breaking 4 tests that assert head==6 (`mod.rs:268` schema_has_timemachine_tables, `mod.rs:303` migrations_are_idempotent, `migrate_v6.rs:422/437`). The fix-up "verified clean" via `cargo check --lib` + `cargo clippy --lib` — **neither runs tests** — so the regression passed the gate and landed in amended commit `bfc7fc6e`. Caught only because Task 2's implementer ran the FULL suite and reported "4 failed" (initially misattributed as pre-existing).

**Probable cause:** Verification gate for a schema/migration change omitted `cargo test`; check/clippy verify compilation + lints, not behavior. Compounding: I overrode the reviewer's explicit hedge — the `==6` assertions encode deliberate maintainer intent (head version is bumped deliberately, not as a side effect of an additive column add).

**Workaround:** Reverted the stamp (commit `8edd3cc8`) → 2544 passed, 0 failed. The `entry_collection` ALTER is `column_exists`-guarded and works without any version stamp. Process fix: every implementer/fix-up dispatch this session now gates on FULL `cargo test --lib` (0 failures) before commit, not just check+clippy. For schema-version changes specifically, grep for hardcoded version assertions before bumping.

**Severity:** med — caught within one task; cost was investigation + revert. Would have been high if it reached master (4 red tests + a spurious schema bump).

**Status:** fixed-verified

**Fix idea / Pointer:** Revert commit `8edd3cc8` (master SHA TBD on cherry-pick). The check-vs-test verification gap generalizes beyond this session — candidate for promotion to CLAUDE.md's verification discipline if it recurs.

---
## F-4 — artifact(get, full=true) buffered body truncated at ~36 KB silently dropped U-19..U-25 during retrofit parse

**Observed:** 2026-05-29, retrofitting `codescout-usage-frictions` (U-N, id `7226af4c655b62a3`) to be `entry_filter`-searchable.

**When:** Parsing the tracker body to build the structured `frictions` array for `artifact_augment`.

**Expected:** `artifact(action="get", id=…, full=true)` returns the complete 1065-line body; a regex parse over `d['body']` yields all active U-N entries.

**Got:** The get response stored a 36 387-byte `@tool_*` buffer whose `body` field ended at U-18. Parsing yielded 15 entries. The tracker actually runs U-1..U-25 (22 active after archived U-4/U-9/U-16). U-19..U-25 were silently absent — the parsed `body` field carried no truncation marker; only the response's `preview.headings` (server-generated from the full file) listed U-19..U-22 at lines 549–791.

**Probable cause:** Progressive-disclosure overflow — the full body exceeded the inline budget, so the buffered `body` field I parsed was a truncated slice; the tail needed a separate line-range read.

**Workaround:** Reconciled parsed count against `preview.headings`, re-read lines 540–1065 via `artifact(get, start_line/end_line)`, recovered U-19..U-25 (surfacing a new `fixed-verified` status value), then re-augmented `merge=false` with the complete 22-entry array + updated schema enum. Verified `entry_total=22` live after `/mcp` reconnect.

**Severity:** high — had it shipped, the searchable index would have under-reported by 7 of 22 entries; the exact queries the retrofit exists to enable ("open high-severity frictions", "all fixed") would silently miss real rows, with no error and no git diff (catalog-only write) to flag the gap.

**Status:** fixed-verified — re-augmented; `entry_total=22` confirmed post-reconnect.

**Fix idea / Pointer:** When parsing a buffered artifact body for structured extraction, reconcile the parsed entry count against `preview.headings` before trusting it. This session, W-1.

---
## W-1 — preview.headings cross-check caught a silent buffered-body truncation before the index was relied upon

**Observed:** 2026-05-29, retrofitting `codescout-usage-frictions` during the metadata-filtering work stream.

**Pattern:** After augmenting a tracker from a parsed buffered (`@tool_*`) body, verify completeness by reconciling `entry_total` and the max entry ID against the get response's `preview.headings` list — not against the buffered `body` you parsed. The preview is generated server-side from the whole file and is independent of the inline-budget truncation that can clip the buffered body.

**Counterfactual:** The first augment wrote only 15 of 22 U-N entries (F-4). Without the cross-check, that incomplete index would have stood: `entry_filter` queries the retrofit exists to enable would have returned authoritative-looking subsets missing U-19..U-25, with no error and no git diff (catalog-only write) to expose it. The gap would have persisted until a human manually noticed absent rows. The risk was invisible on the other three trackers retrofitted this session (176/178/262-line bodies, all fit inline and parsed complete) — only the one >36 KB body truncated, so a "it worked three times" heuristic would have shipped the bug.

**Confirming data points:**
1. F-4 (this session) — the verification get's `preview.headings` showed U-19..U-22 while the parsed `body` stopped at U-18; that discrepancy was the only signal.
2. Post-reconnect re-verification (`entry_total=22`; `status=open`→2 rows; `status` prefix `fixed`→15 rows) confirmed completeness only because the count was reconciled first.

**Impact:** high — prevented shipping a silently-incomplete searchable index whose under-reporting would be durable and undetectable by normal use.

**Promote-when:** A second instance of buffered-body parsing missing tail content. At 2 datapoints, promote to a codescout convention: "Reconcile parsed buffered-body entry counts against `preview.headings` before structured writes."

**Status:** validated — single datapoint; drift caught and corrected before any consumer queried the index.

---
## F-5 — Changing params_schema / render_template on an augmented tracker forces a full merge=false re-send (silent-reset foot-gun)

**Observed:** 2026-05-29, retrofitting `codescout-usage-frictions`; had to add `fixed-verified` to the status enum after discovering U-19..U-23 used it.

**When:** Widening one enum value in `params_schema` on an already-augmented tracker.

**Expected:** A field-granular patch — add an enum value (or edit the schema/template) without re-sending the whole augmentation.

**Got:** `merge=true` patches `params` **only** — it cannot touch `params_schema` / `render_template` / `prompt` / `entry_collection`. The sole way to change the schema is `merge=false`, which overwrites **all seven** caller-controlled fields; any field omitted silently resets to `None`. So a one-value enum widening forced re-sending prompt + render_template + the full schema + entry_collection + the entire 22-entry params array in a single call.

**Probable cause:** `artifact_augment` has exactly two modes — full replace (`merge=false`) and params-only patch (`merge=true`). No field-granular patch exists for the other six caller-controlled fields.

**Workaround:** Re-sent every field verbatim in one `merge=false` call, keeping prompt/render_template byte-identical to avoid drift; verified `entry_total` afterward.

**Severity:** med — schema/template tweaks are routine maintenance, but each one carries a silent-reset foot-gun + a large re-send. The retrofit guide warns about it; the tool offers no safer path.

**Status:** open

**Fix idea / Pointer:** A `merge=true` that also accepts `params_schema` / `render_template` patches, or a dedicated schema-patch path. Minimum: server rejects a `merge=false` that drops a previously-set field unless an explicit clear is requested.

---

## F-6 — No artifact(delete) action; throwaway cleanup needs rm + reindex and orphans the augmentation

**Observed:** 2026-05-29, cleaning up the `_entry-filter-smoke.md` throwaway tracker after the feature smoke test.

**When:** Removing a deliberately-created throwaway artifact.

**Expected:** `artifact(action="delete", id=...)` removing the file + catalog row + augmentation atomically.

**Got:** The `artifact` action enum is find|get|create|update|move|link|graph|state_at — no `delete`. Cleanup required `rm <file>` (run_command) + `librarian(reindex)` to drop the orphaned catalog row (reindex reported `removed:1`).

**Probable cause:** Delete was never added; `move` covers relocation, removal is out-of-band.

**Workaround:** rm + reindex. Reliable, but two steps; forgetting the reindex leaves an orphaned catalog row, and the augmentation (catalog-only, no disk form) has no cleanup path at all if the row lingers.

**Severity:** low — workaround is reliable; only bites for throwaway/mistaken artifacts. The orphaned-augmentation-on-rm case is a latent catalog-hygiene gap.

**Status:** open

**Fix idea / Pointer:** Add `artifact(action="delete", id)` removing file + catalog row + augmentation atomically; mirror `artifact(move)`'s catalog-aware path.

---

## F-7 — entry_filter silently returns empty on an unknown / typo'd field (asymmetric with the SQL engine's hard error)

**Observed:** 2026-05-29, grounding a friction hypothesis against the shipped implementation.

**When:** Filtering `tool-usage-patterns` with a deliberately-misspelled field: `entry_filter={"verdcit": {"eq": "wrong-tool"}}`.

**Expected:** An unknown field errors — the SQL side does exactly this (`artifact(find)` with a bad field → `no such column`, which was T-010's entire story).

**Got:** `entry_total=10, entries=[]` — silent empty, no error, no `unknown_field` hint. The in-memory `eval`/`eval_leaf` path (`src/librarian/filter.rs:250,287`) deliberately dropped the `ALLOWED_FIELDS` gate (F-1's design rationale: an in-memory match has no injection surface), so any field name is accepted and a field absent from every entry simply never matches.

**Probable cause:** The injection-driven allowlist was removed for the in-memory path and nothing replaced it as a typo/diagnostic guard. The two engines now diverge: SQL errors on unknown fields, eval silently returns empty.

**Workaround:** None at call time — caller must cross-check the result set against expectations (same mitigation as F-4 / W-1: reconcile against an independent view). A typo'd field is indistinguishable from a genuine zero-match.

**Severity:** med — silent wrong results; an authoritative-looking empty set on a typo, the same silent-underreport class as F-4. Risk rose now that 4 trackers are filterable and callers will hand-type field names.

**Status:** open

**Fix idea / Pointer:** `eval` could collect the union of keys present across all entries and surface a soft `unknown_field` warning when a filtered field matches none of them — a warning, not a hard reject (entries are heterogeneous; a field present in some but not all is legitimate). Strong candidate for promotion to a `docs/issues/` bug.

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
