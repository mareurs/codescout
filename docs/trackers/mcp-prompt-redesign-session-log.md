# Session Log — MCP Prompt Channel Redesign

> **Topic:** MCP prompt channel redesign (Surfaces A/B/C/D + sever librarian concat).
> **Plan:** `docs/superpowers/plans/2026-05-19-mcp-prompt-channel-redesign.md`
> **Spec:** `docs/superpowers/specs/2026-05-19-mcp-prompt-channel-redesign-design.md`
> **Evidence base:** `docs/architecture/mcp-channel-caps.md`

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
| F-1 | 2026-05-19 | med | plan-prose | fixed-verified | Plan Task 11 underspecifies tests broken by source.md rewrite |
| F-2 | 2026-05-19 | med | architectural | fixed-verified | Pre-dispatch scout missed `SYMBOL_NAV_TOKEN` dead path + cross-file test |
| F-3 | 2026-05-19 | med | architectural | fixed-verified | Scout undercounted `ToolContext` sites by ~2.5×; compiler-driven loop fixed it |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-05-19 | med | Pre-dispatch scout enumerated SERVER_INSTRUCTIONS-asserting tests | Would have failed ≥4 unplanned tests inside subagent | validated |
| W-2 | 2026-05-19 | med | Compiler-driven enumeration for struct-field threading | Grep-only would have missed ~17 sites | validated |
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

## F-1 — Plan Task 11 underspecifies tests broken by `source.md` rewrite

**Observed:** 2026-05-19, U4 pre-dispatch reconnaissance.

**When:** About to dispatch U4 implementer to rewrite `src/prompts/source.md` (~21KB → ~1.8KB) and add the 4 redesign-invariant tests from Task 10.

**Expected (plan):** Task 12 names exactly one pre-existing test that may break — `server_instructions_template_has_symbol_nav_token` — and tells the implementer to "remove or update" it. Implication: a handful of other test breaks at most.

**Got (scouted reality):** At least **6 pre-existing tests assert on `SERVER_INSTRUCTIONS` content** that the rewrite invalidates:

1. `prompts::tests::static_instructions_contain_key_sections` (`src/prompts/mod.rs:276`) — asserts `## Tool Routing & Gotchas`, `## Output System`, `## Rules` headings; new shape has none.
2. `prompts::tests::iron_law_8_promotes_call_graph_before_references` (`:294`) — asserts Iron Law 8 exists; new shape has only 5 Iron Laws.
3. `prompts::tests::server_instructions_template_has_symbol_nav_token` (`:804`) — asserts the `{{symbol_navigation_block}}` token; rewrite removes it.
4. `prompts::tests::build_server_instructions_substitutes_symbol_nav_token` (`:814`) — asserts the post-substitution `### Symbol Navigation Patterns` heading; new shape doesn't include that heading.
5. Snapshot test at `:927` — `check_or_update_snapshot("server_instructions.md", SERVER_INSTRUCTIONS)` — fixture file `tests/fixtures/prompt_surfaces/server_instructions.md` (25KB) must be regenerated.
6. `server::tests::prompt_surfaces_reference_only_real_tools` (`src/server.rs:1486`) — has an **allowlist tripwire** for unused entries. After source.md shrinks from 44KB to 1.8KB, many of the 50 allowlisted tokens (`acknowledge_risk`, `architecture`, `cargo`, `cat`, `class`, `gradle`, `mvn`, `pytest`, `python`, `npm`, `pnpm`, `yarn`, …) lose their backtick references → test fires "unused allowlist entries" failure. The allowlist must be shrunk in lockstep with the rewrite.

There's also a latent issue: `src/prompts/source.md:645` contains a literal `{{include: memory-templates.md}}` token. The `build_server_instructions` function only substitutes `{{symbol_navigation_block}}` — the `{{include:}}` text leaks into the live `SERVER_INSTRUCTIONS` constant verbatim. Not our problem (the rewrite removes it), but flagged for transparency.

**Probable cause:** Plan author scanned source.md for tokens but did not enumerate all tests asserting on its content. The `prompt_surfaces_reference_only_real_tools` allowlist tripwire is non-obvious — added specifically to prevent rot, but it kicks in here too.

**Workaround:** U4 implementer prompt will enumerate all 6 expected failures + the snapshot regen + the allowlist shrink, so the subagent attempts them as part of the task rather than discovering them mid-run.

**Severity:** med — would have caused subagent to flail through 4-6 unplanned test failures with 1-2 retries each; controller would absorb the drift mid-dispatch.

**Status:** mitigated — U4 implementer prompt will name all known failure sites; entry stays open until U4 lands clean.

**Fix idea / Pointer:** U4 implementer dispatch, this session. Future plan-writing for prompt-surface changes should grep for `SERVER_INSTRUCTIONS.contains` / `SERVER_INSTRUCTIONS.find` / snapshot calls as a Phase-0 check.

---

## W-1 — Pre-dispatch scout enumerated `SERVER_INSTRUCTIONS`-asserting tests before subagent saw them

**Observed:** 2026-05-19, U4 reconnaissance via `codescout-companion:reconnaissance`.

**Pattern:** Before dispatching a subagent to rewrite a content surface that any constant is `include_str!`'d from, grep the codebase for asserts on that constant (`<CONST>.contains`, `<CONST>.find`, snapshot calls referencing the surface). Enumerate every failing test in the dispatch prompt so the subagent attempts them as part of the task instead of discovering them mid-run.

**Counterfactual:** Without the scout, U4 implementer would have run the 4 planned `redesign_invariants` tests, hit the 6 unplanned `SERVER_INSTRUCTIONS`-asserting failures, and reported either DONE_WITH_CONCERNS (and we'd loop) or BLOCKED. Cost estimate: 1-2 extra subagent round-trips per surprise test (×6 = 6-12 round-trips), plus controller context absorbed by each follow-up.

**Confirming data points:**
1. F-1 (this session) — 6 unplanned tests catalogued in advance.
2. Prior W-2 in `bug-fix-session-log.md` — same pattern for type-shape, also single datapoint.

**Impact:** med — saves an entire subagent retry cycle for a high-fanout content change.

**Promote-when:** A third pre-dispatch scout catches a similar enumerable-but-uncited-in-plan test breakage. At 3 datapoints, promote to CLAUDE.md as "Before rewriting any `include_str!`'d content file, grep for asserts on the constant it backs."

**Status:** validated — single fresh datapoint plus one matching prior datapoint in `bug-fix-session-log.md`.

---

## F-2 — Pre-dispatch scout missed dead code path + cross-file test

**Observed:** 2026-05-19, U4 spec compliance review.

**When:** Reviewing U4 implementer's deletion of 8 tests (vs the 6 enumerated in F-1) to verify the 4 extras were justified.

**Expected (F-1):** 6 sites that need touching when source.md is rewritten — 4 tests in `src/prompts/mod.rs`, 1 snapshot regen, 1 allowlist tripwire in `src/server.rs`.

**Got:** F-1 missed **two more sites**, both flagged by the spec reviewer:

1. **Dead `SYMBOL_NAV_TOKEN` substitution path.** `build_server_instructions` (`src/prompts/mod.rs:28-117`) calls `render_symbol_navigation_block(&project_languages)` and then `SERVER_INSTRUCTIONS.replace(SYMBOL_NAV_TOKEN, &nav_content)`. The new `source.md` has no `{{symbol_navigation_block}}` token → the `.replace` becomes a no-op → the language-specific `### Rust — Symbol Navigation` content is silently dropped at runtime. F-1's enumeration only scanned `SERVER_INSTRUCTIONS.contains`/`.find`/snapshot — it missed callers that *write into* the constant via `.replace` against a constant token. Result: implementer deleted the only test guarding this path (`build_server_instructions_renders_languages_from_status`) thinking it was assertions on now-removed content, when in fact the test was the regression guard for a still-live (now broken) code path.

2. **`server::tests::server_instructions_documents_goal_tracker_discovery`** (`src/server.rs:1570-1582`) — asserts source.md contains the `goal-tracker` token. F-1's grep was scoped to `src/prompts/` — it missed `src/server.rs` asserts. This is a 7th broken test the implementer encountered but apparently did not address.

**Probable cause:** Recon walked `SERVER_INSTRUCTIONS`-as-value (`.contains` / `.find`) but did not walk the related constants (`SYMBOL_NAV_TOKEN`) or expand scope beyond the file being rewritten. The token-substitution pattern is a hidden write to the constant that only triggers at runtime, so source-only grep on the constant misses it.

**Workaround:** U4 fix-up dispatch will (a) decide whether to restore the token + heading in source.md or rip the substitution code entirely (recommend rip — the redesign intentionally drops language-specific nav from the surface), and (b) update or delete `server_instructions_documents_goal_tracker_discovery`.

**Severity:** med — implementer reported DONE but the dead code path would have shipped silently; spec review caught it. One additional fix-up subagent cycle needed.

**Status:** mitigated — U4 fix-up dispatch addresses both items; entry stays open until U4 lands clean.

**Fix idea / Pointer:** Future pre-rewrite scout for `include_str!`'d content should grep three patterns, not one: `<CONST>.contains`/`.find`/snapshot, `<CONST>.replace` (token writes), and any test referencing tokens from the file by basename. Also widen scope from the prompts crate to the workspace (`grep -r <surface-name>`), since cross-crate asserts exist.

---

## F-3 — Scout undercounted `ToolContext` construction sites by ~2.5×

**Observed:** 2026-05-19, U8 pre-dispatch reconnaissance + post-implementer summary.

**When:** Scouting before dispatching U8 (Plan Task 20: thread `guide_hints_emitted` through every `ToolContext` construction site).

**Expected (scout):** `grep -E "ToolContext\s*\{|ToolContext::new"` on `src/` + `tests/` returned ~13 sites. I dispatched U8 with that enumeration.

**Got (implementer):** Required ~30 sites. The compiler-driven discovery (`cargo build` errors out on every missing field) found them all, but the implementer fell back to a `perl -i -0pe` bulk substitution to keep up. Two files double-inserted (`tests/call_graph_live.rs`, `tests/e2e/edit_eval/runner.rs` — their `section_coverage:` field was on a single line that matched both regex passes); deduped manually.

After landing: `mcp__codescout__grep guide_hints_emitted src/` shows 50+ matches across 13 files in `src/` alone, plus the test trees. Single test file `src/tools/symbol/tests.rs` had 24 separate construction sites.

**Probable cause:** The grep pattern matched only single-line construction openings. Multi-line `ToolContext {\n  field1: ...,\n  ...\n}` constructions where the opener is just `ToolContext {` plus newline did still match — so the undercount is something else. Most likely: the regex matches per-file once when many of those tests use the field-construction inside macros / per-test helpers / nested scopes that the grep tool reports as fewer hits than actual usages. Subsequent verification confirmed `guide_hints_emitted` insertions number ~50 across the workspace.

**Workaround:** Compiler-driven enumeration is more reliable than grep-driven for struct-field additions. The implementer's bulk-substitution-plus-cargo-build loop converged correctly. No regressions: `2408 pass, 2 pre-existing fails` after the field threading.

**Severity:** med — would have caused implementer to grind through many compile errors had they only addressed the 13 sites I enumerated. They were unblocked by the perl pass; concerning is the double-insertion risk if the dedupe was missed.

**Status:** mitigated — implementer absorbed the cost; the work landed clean. Entry stays open as a recon-quality lesson.

**Fix idea / Pointer:** For struct-field additions, scout via `cargo build` after a no-op edit, not via grep on construction patterns. The compiler enumerates exhaustively; grep is for *finding* sites, not *counting* them.

---

## W-2 — Compiler-driven enumeration trumped grep-driven for struct-field threading

**Observed:** 2026-05-19, U8 implementation.

**Pattern:** When adding a non-`Option` field to a struct that's constructed in many sites, use the compiler as the enumerator: add the field, run `cargo build`, fix each "missing field" error in turn. This is exhaustive by construction. Grep-driven enumeration is approximate and may undercount.

**Counterfactual:** Without compiler-driven enumeration, the implementer dispatched with a 13-site list would have hit 17 "missing field" errors, retried, hit more, and either run out of patience or skipped multiple files thinking they were done.

**Confirming data points:**
1. F-3 (this session) — 13 sites enumerated, ~30+ actually needed touching; compiler caught the gap.
2. Pending — a future struct-field addition that uses the same loop.

**Impact:** med — saves multi-iteration rework cost for any wide struct-field threading.

**Promote-when:** A second project task uses this loop and confirms its reliability. At 2 datapoints, promote to CLAUDE.md as "Adding a required field to a widely-constructed struct: trust `cargo build`, not grep."

**Status:** validated — single datapoint, will mature with reuse.

---

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
