---
id: '1dcfdd70de0fcc73'
kind: tracker
status: active
title: Test-Escape Hardening — interventions from the entry-graph Stage 2 review lessons
topic: test escape hardening
entry_high_water_I: 8
entry_prefix: I
expects_augmentation: docs/augmentations/docs-trackers-test-escape-hardening.yaml
---

## Overview

Every real defect in the entry-graph Stage 2 feature was a **discovery** problem in an
untested seam — the implementers wrote correct-looking code, tests passed, and only expensive
Opus reviews (2 per-task + 1 whole-branch) caught them, each because the controller
hand-injected the right lens. The lesson: those lenses must move LEFT into cheaper, standing
mechanisms so they catch by default without a human remembering.

**Defense-in-depth gradient (cheap/early → expensive/late):**
auto-loaded memory → skill prompts → static Rust lints/CI → per-task reviewer → whole-branch review.

## The five defects and what catches each

| # | Defect | Nature | Cheapest mechanical catch | Interventions |
|---|--------|--------|---------------------------|---------------|
| 1 | Table-copy migration dropped a later-added column (`slug`) → dangling FK for one open | static | schema-invariant `#[test]` | I-1, I-4, I-5 |
| 2 | Non-discriminating test: asserted only "an error occurred"; guard-deletion still passed | semantic | mutation testing only | I-3, I-4, I-5 |
| 3 | `resolve_cite_ref` shipped 2 of 3 branches untested | semantic | mutation testing (branch) | I-3, I-4, I-5 |
| 4 | Write/read asymmetry: id-keyed backlinks invisible for slug-less targets; both tests used slugged targets | semantic | mutation testing | I-3, I-4, I-5 |
| 5 | Unescaped user string in a `LIKE` pattern (`%`/`_` acted as wildcards) | static | grep `#[test]` for LIKE-without-ESCAPE | I-2, I-5 |
| 6 | Unreachable fixture: the assertion is exactly right, but the test DATA cannot reach the code path | semantic | mutation run against the whole assertion FAMILY, not just the new test | I-8 |

(#6 added 2026-08-29 from a different work stream; the heading's count is the
original set. It is a distinct mechanism from #2 — there the assertion was weak,
here the assertion is precise and the data never reaches it.)

Key finding from the feasibility exploration: a `call_graph`-based "untested new symbol"
detector (I-6) is **structurally blind** to #3/#4 (a function that IS called by tests but
whose branches aren't exercised reads as covered) and useless on #2. The only mechanical
catch for the three semantic defects is **mutation testing scoped to the diff** (I-3).

## Interventions

### I-1 — Schema-invariant test (defect #1)
Generalize the one-off regression test into a loop: parse `SCHEMA_SQL`'s `CREATE TABLE artifact`
column list and assert, after a single `open_with_workspace` on every legacy seed fixture,
that every declared column + every required index (`ux_artifact_slug`, …) survives and
`pragma_foreign_key_check` is empty. Reuses `column_exists`, `SCHEMA_SQL`, seed fixtures — only
a tiny `parse_create_table_columns` helper is new. Cheap-and-robust; `cargo test`-gated for free.

**Valid:** dated 2026-07-17

### I-2 — LIKE-escape helper + gate (defect #5)
The escape idiom is copy-pasted in ~5 inline sites across two idioms with no shared helper
(see bug `docs/issues/archive/2026-07-17-like-escape-idiom-duplicated-no-shared-helper.md`). Extract
`escape_like_pattern` (the Rust-side needle-escaping) and route `filter.rs` + `augmentation.rs`
through it; add a source-scan `#[test]` (mirroring `claude_md_contains_no_deprecated_tool_names`)
asserting every `LIKE` literal has a paired `ESCAPE` clause.

**Valid:** dated 2026-09-02

**Re-verified 2026-09-02, both halves.** Helper: `escape_like_pattern` at
`src/librarian/util.rs:26` with five unit tests, and `filter.rs:246` / `gc.rs` / `augmentation.rs`
routed through the paired `ESCAPE '\\'` form. Gate: `like_escape_idiom_is_not_inlined_outside_helper`
(`util.rs:112`) plus a sibling `sql_descendant_like_is_not_inlined_outside_helper` (`:172`) the
intervention did not ask for. Checked because a `status: done` in params is a record asserting a
completed action, which is `IC-8`'s whole subject.

### I-3 — Mutation testing at the ship boundary (defects #2/#3/#4)
`cargo-mutants` is not yet a repo dev-dependency. Add a diff-scoped `cargo mutants --in-diff <range>`
pass to the Standard Ship Sequence in `docs/RELEASE.md` (pre-cherry-pick to master). Surviving
mutants = untested behavior. Diff-scoping keeps cost tractable; ship-boundary cadence keeps it
off the per-edit hot path. This is the only mechanism that would have caught the non-discriminating
guard test (#2).

**Valid:** dated 2026-09-02

**Re-verified 2026-09-02.** `docs/RELEASE.md:206-219` carries
`cargo mutants --in-diff /tmp/ship.diff --package codescout` and states in the same block that it is
*"Advisory, not a hard gate — `cargo-mutants` is not yet a workspace dev-dependency"* — which matches
this intervention's own title rather than overstating it.

### I-4 — Standing review-lens bullets (defects #2/#3/#4/#1)
The superpowers `task-reviewer-prompt.md` / `code-reviewer.md` carry only a generic
"edge cases covered?" — none of the four lenses. Add standing bullets: mutation-thinking
("would this test pass if the target code were deleted/inverted?"), branch-coverage (a test per
new branch), round-trip completeness (every writer shape surfaced by a reader test), and a
cross-migration-seam check. **Ownership caveat:** superpowers is a marketplace plugin (cache,
not owned) — editing the cache is ephemeral. Durable home is the owned buddy `testing-snow-leopard`
(carries the lenses as doctrine); the superpowers edit is an upstream suggestion.

**Valid:** dated 2026-09-02

**Re-verified 2026-09-02 in the durable home, and the first search said the opposite.** Grepping the
**labels** (`L3`/`L4`/`L5`) returned **0 across all five locations** — four plugin caches and the
source repo — which reads exactly like an unshipped intervention. A positive control (confirm the
file is readable, then search the lens *content* rather than its label) found all of them, under the
names the skill actually uses:

| claimed as | lives at |
|---|---|
| L3 assert-on-cause | Operating Principle 3 — *"assert on the specific cause (message substring, error variant, or field)"* |
| L4 branch-pairing | Self-Trap 7, *"Branch pairing gap"* — *"both the present and the absent side"* |
| L5 round-trip completeness | Heuristic 8 + § Properties — including the *"target always has a slug"* shared-precondition warning, which is `R-42` verbatim |

Also present: mutation-thinking (Operating Principle 3 and Phase 3's *"what single mutation would I
miss?"*) and a `Mutation-survival:` field the skill's own test format requires. Migration-seam is the
thinnest — covered only as *"or migration pair"* under Round-trip, not as its own lens.

**The label/content gap is the finding, not a footnote.** This ledger names interventions by a
private shorthand (`L3`…`L5`) that the destination artifact never adopted, so every future
verification of I-4 and I-5 hits the same false negative. Renaming is not the fix — the destination
is a different repo and the names there are better; what this entry owes is the mapping table above,
so the next check searches for behaviour rather than for a label. `R-3`, met inside the sweep that
re-derives it.

### I-5 — Durable recall (all defects)
codescout memories: `catalog-sql-hazards` (#1, #5) and `test-design-discipline` (#2/#3/#4) —
DONE 2026-07-17. Pending: `testing-snow-leopard` SKILL.md gains L3 (assert-on-cause), L4
(branch-pairing), L5 (round-trip completeness); reconnaissance SKILL.md gains two seam classes
(schema-migration ordering; writer-shape↔reader-surfacing). **Eval caveat:** the reconnaissance
edit is a scout-behavior change → requires re-scoring `docs/evals/reconnaissance-output.md`
(baseline n=0). The superpowers/testing-snow-leopard edits are not eval-gated.

**Valid:** dated 2026-09-02

**Re-verified 2026-09-02, three surfaces.** Memories `catalog-sql-hazards` and
`test-design-discipline` both present in this project's memory list. Reconnaissance seam classes:
`references/seam-classes.md` in the **served** `codescout-companion@1.20.0` cache carries both
(schema-migration ordering; writer-shape↔reader-surfacing) — checked at the served copy, not the
source, per `R-89`. `testing-snow-leopard` L3–L5: present under different names — see I-4's mapping
table, and the label/content false negative recorded there applies identically here.

**Still open and unchanged:** the eval caveat. `docs/evals/reconnaissance-output.md` baseline remains
n=0, so the claim that the seam-class edit improved scout behaviour is still unmeasured. Re-dating
this entry does not discharge that — it was not measured today either.

### I-6 — Untested-new-symbol detector (deferred, low-yield)
Buildable cheaply from `call_graph(direction=callers)` + a `tests/` name-path heuristic, but only
catches wholly-orphaned new functions — the degenerate case, not the actual #3/#4 shapes. Kept
as deferred: mutation testing (I-3) dominates it on coverage-per-value.


### I-7 — Deprecated-tool-name gate over the `get_guide` bodies (gate-scope)

**Valid:** dated 2026-08-16

The entry's load-bearing sentence is a census of a moment: *the ten `get_guide` bodies were
the only prose surface with no drift gate*, with `prompt_surfaces_reference_only_real_tools`
building its list from exactly three entries. Both counts move — guides are added, and the
gate this intervention shipped is itself a fourth surface — so the claim is anchored to the
day I-7 opened and shipped (per this file's own `## History`). Declared 2026-09-01.

The ten `get_guide` bodies are the fourth prose surface the model reads, and were the
only one with no drift gate at all. `prompt_surfaces_reference_only_real_tools`
(`src/server.rs:1839`) builds its `surfaces` list from exactly three entries —
`server_instructions.md`, `onboarding_prompt.md`, `build_system_prompt_draft` — and no
guide body appears in it. A guide is auto-injected on the first call that triggers its
topic, so a stale tool name there reaches the model exactly as one in
`server_instructions` would.

**Why denylist, not allowlist.** Measured 2026-08-16: the ten bodies carry **179
distinct backticked snake_case tokens** against roughly 30 real tools. An allowlist
would need ~150 non-tool entries, and the existing gate's two-way tripwire (every
allowlist entry must still appear backticked in some surface) would turn every guide
edit into a maintenance event. That is the trade F-9 left undecided; the measurement
decides it. The same conclusion was already written in the codebase for `CLAUDE.md` at
`src/prompts/mod.rs:1101-1105` — *"It is prose, so an allowlist guard is unusable
here"* — and that comment cites F-9 by name.

The gate iterates `GUIDE_TOPICS` and calls `topic_body`, rather than a hand-written
list, so an eleventh guide is covered the moment it is registered — which is the exact
failure mode the intervention exists to prevent.

**Mutation-verified, not merely green.** Adding `semantic_search` to
`DEPRECATED_TOOL_NAMES` fails the test with `get_guide body 'symbol-navigation'
references deprecated tool name: semantic_search` — it names the guide and the token.
Reverted after the check.

Closes **F-9** in `docs/trackers/archive/prompt-guide-refactor-session-log.md`, open
since the prompt-guide-refactor work stream.

### I-8 — Mutate the whole assertion family, not just the new test (defect #6)

I-3 says surviving mutants are untested behaviour, and scopes the run to the
diff. **That scoping is exactly what lets defect #6 through.** The mutant is
killed by the test you just wrote, the run comes back clean, and the
PRE-EXISTING tests asserting the same property are never observed at all. They
can be green in the broken world with nothing anywhere saying so.

**Defect #6 is not a weak assertion — that is #2.** Here the assertion is
precisely the right one. The *fixture* cannot reach the code path, so the
assertion never runs against the case its name claims. The tell: the test's
**data** lacks the property under test while its **name and shape** claim
otherwise.

**Measured 2026-08-29 (this session's instance).** `src/tools/read_file.rs`
carried two tests named `..._chunk_fits_the_threshold_it_is_measured_against`,
asserting exactly the property a live defect violated — that an inlined chunk
must fit `TOOL_OUTPUT_BUFFER_THRESHOLD` or `call_content` re-wraps the response.
Both fixtures are 1200 **short** lines. The defect only fires when a **single**
line exceeds the whole budget, and with short lines the budget always stops at a
line boundary long before the safety valve is reached. Disabling the fix and
running the whole `fits_the_threshold` family gave **2 passed / 1 failed**: both
incumbents green with the defect present. Fix `61476cb5`; bug
`docs/issues/archive/2026-08-28-tool-buffer-grep-returns-envelope-not-stdout.md`.

**A sibling instance the same afternoon — found by codescout-97 in
`src/librarian/filter.rs` (BL-47), theirs not mine — is the worse shape.** A
*differential* test (`eval_matches_compile_on_fixture`, two engines required to
agree on one AST) whose fixture table declares no array column at all, so it
could not reach the `tags`/`owners` `in`/`nin` branch that was broken. Two-engine
agreement advertises maximum rigour while the fixture omits the disputed type
entirely — which is why a test's *form* is no evidence about its *reach*.

**Procedure — read first, mutate second.** The two instances below were found by
different methods, and the difference is not incidental: it is whether the
missing property is **structural** or **distributional**.

- **Structural — read the fixture.** The fixture's *shape* cannot express the
  property: an absent column, an absent field, a type that is never constructed.
  Visible in one read of the fixture's declaration, before any test is run. This
  is the cheap path and it should be tried first — ask "can this data reach the
  branch?" and look.
- **Distributional — run the mutation.** The fixture's shape *permits* the case
  and its values never produce it: 1200 lines that could each have been long and
  none are. Reading does not settle it, because nothing in the fixture announces
  the absence. Here you need the family run: disable the fix, take the shared
  phrase out of the test names, and run `cargo test --lib <phrase>` rather than
  the single test. Any sibling still green is a fixture that cannot reach the
  path — fix it, or record why it cannot. Cost is one extra filtered run,
  seconds.

The distinction was supplied by codescout-97 on the correction below; the
first draft of this section flattened both into "run a mutation", which made the
intervention more expensive than it needs to be.

**Complement, not a substitute: assert the fixture's premise inside the test.**
Both repairs here now assert the discriminating property of their own data
*before* asserting behaviour (`widest > INLINE_BYTE_BUDGET`;
`front.len() * 2 >= whole.len()`), so a later edit that flattens the fixture
fails on the premise instead of silently becoming another blind copy.
`util::shrink_guard::tests::a_uniform_fixture_cannot_tell_the_arms_apart` goes
further and pins the trap itself as an executable warning.

**Scope of the claim, corrected 2026-08-29.** The first draft said "three
instances — all found by running a mutation, none by reading". Both halves were
wrong, and the corrected accounting is smaller and more useful:

| Case | What it was | How found |
|---|---|---|
| `read_file` (mine) | blind incumbent, **distributional** (1200 short lines) | whole-family mutation, 2 passed / 1 failed |
| `librarian::filter` (codescout-97's, BL-47) | blind incumbent, **structural** (no array column) | reading the fixture's `CREATE TABLE` |
| `shrink_guard` (mine) | **not an instance** — a trap anticipated while writing a NEW test | reasoning about the fixture at write time |

So: **two** discovered blind incumbents, not three, found by two different
methods — and the third case was a trap avoided, which I had counted as a
discovery to reach a tally of three. codescout-97 caught the method error; the
over-count was mine and is corrected here rather than quietly dropped, because a
tracker that inflates its own evidence is the thing this file exists to catch.

**A sibling class, deliberately NOT merged into this one.** A *new* test that
goes compile-error → green has never been observed failing for a behavioural
reason, so it too is unverified — but it is a different mechanism with a
different trigger and a different population, and it is tracked separately in
codescout-97's `W-73`. Its datapoints are their `guard_stale_binary` wiring test
and this session's CM-7 test in `read_file.rs`; both were mutation-checked after
the fact.

The two classes were briefly summed into one three-datapoint claim, which is how
the over-count above happened. They are two claims at two datapoints each:

| Class | Population | Trigger | Detection |
|---|---|---|---|
| compile-error → green (`W-73`) | NEW tests | the test never ran red | mutate the new test |
| unreachable fixture (this entry, #6) | PRE-EXISTING tests | data cannot reach the branch | read the fixture (structural) or mutate the family (distributional) |

Keep them apart. A test can be in either, both, or neither, and the remedies
differ — merging them yields a bigger number and a vaguer instruction.

**Limit, unchanged:** nothing either session has measured shows how often the
family run finds a blind sibling when the diff-scoped run is already green.
That needs a deliberate sweep neither of us has run.

**Valid:** dated 2026-08-29
## History

### 2026-08-16 — I-7 opened and shipped same day (tracker-hygiene sweep → verify-open → fix)

Route worth recording, because no single step would have found it. The tracker-hygiene
sweep archived the prompt-guide-refactor session log; its distill step ran verify-open
on that log's open frictions; F-9 re-confirmed **at the bytes** (the `surfaces` array is
literally three entries) rather than being taken on trust; it was rehomed here as I-7
because this tracker owns test-escape interventions; and I-7 was then the only
`proposed` mechanical entry, which is what this tracker's own prompt says to take first.

A friction that had sat open across a whole work stream closed within a day of the
sweep that touched it — the argument for distill-then-archive doing verify-open rather
than a bare archive.

### 2026-07-17 — created from the entry-graph Stage 2 self-reflection
Four parallel deep-exploration subagents grounded the four defense layers (static-lint infra,
review-lens/recon-seam encoding, coverage-diff feasibility, buddy/memory homes). I-5 memories
(`catalog-sql-hazards`, `test-design-discipline`) written this session; LIKE-duplication bug filed.
Remaining interventions scheduled — I-1/I-2 via subagent-driven TDD, I-3 as a RELEASE.md edit,
I-4/I-5 skill edits (reconnaissance staged behind its eval).


### 2026-07-17 — interventions implemented (5 of 6; I-6 deferred)

All actionable interventions shipped this session. **I-1** schema-invariant guard `every_schema_sql_artifact_column_survives_every_migration_path` (commit `3e2459da`, mutation-verified: dropping a column from the migrate_v6 table-copy fails it). **I-2** extracted `escape_like_pattern` to `src/librarian/util.rs`, routed the two Rust-side sites, added the DRY gate `like_escape_idiom_is_not_inlined_outside_helper` (commit `14bd8b55`). **I-3** RELEASE.md now recommends `cargo mutants --in-diff` before cherry-pick (commit `1af2be8e`; advisory until `cargo-mutants` is adopted as a dev-dependency). **I-4/I-5** codescout memories `catalog-sql-hazards` + `test-design-discipline` (commit `fb9fd244`); `claude-plugins@627bbde` added L3/L4/L5 to buddy `testing-snow-leopard` and the two Phase-1 seam classes to `reconnaissance`. Bug filed for the LIKE duplication (`docs/issues/archive/2026-07-17-like-escape-idiom-duplicated-no-shared-helper.md`, now fixed for the 2 Rust-side sites by I-2; SQL-side chains remain out of scope).

**Open follow-up:** the `reconnaissance` SKILL.md edit is a scout-behavior change — establish/re-score the `docs/evals/reconnaissance-output.md` baseline (n=0) via prompt-tdd to bless it. **I-6** stays deferred (mutation testing / I-3 dominates a call_graph reachability detector on coverage-per-value).
