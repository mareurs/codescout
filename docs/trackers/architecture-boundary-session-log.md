---
id: '51baab12f451feb6'
kind: tracker
status: active
title: Architecture Boundary Measurement — Session Log
tags:
- architecture
- measurement
- session-log
topic: architecture-boundary-measurement
entry_prefix:
- F
- W
entry_high_water_F: 4
entry_high_water_W: 3
---

# Architecture Boundary Measurement — Session Log

## Purpose

Frictions and counterfactual wins while validating the architecture instrument. Baseline, decisions and resume state belong to [Architecture Boundary Measurement](architecture-boundary-measurement.md). This is a prose ledger; the librarian allocates IDs. Status vocabulary follows docs/templates/session-log.md.

## F-1 — Raw import inspection falsified a green probe's static population

**Valid:** dated 2026-09-13

**Observed:** 2026-09-13, after the 12-test suite and self-test passed, the first raw reference in baseline-actions.json was crate::ast::anyhow::Result. Calling resolve_relative on anyhow::Result reproduced it. A second production call reduced crate::lsp::base to crate::lsp::b.

**Expected (plan):** A passed fixture suite would permit collecting a bounded static baseline, followed by raw-reference calibration.

**Got (scouted reality):** Fixtures covered cycles, whitespace and registration controls but not external-root identity or the letters as inside identifiers. Two new tests observed two failures before their fixes; all 14 passed afterward. At the same frozen HEAD, the corrected raw edge list has 315 rows against 379 previously; the earlier figure is rejected evidence, not a before/after architecture improvement.

**Severity:** high — invented edges would have entered a durable architectural baseline despite green controls.

**Status:** promoted-to-bug-tracker

**Rests on:** docs/issues/archive/2026-09-13-architecture-probe-invents-internal-imports.md and docs/issues/archive/2026-09-13-architecture-probe-mangles-as-identifiers.md; scripts/architecture-boundary-probe.py; tests/test_architecture_boundary_probe.py. The architecture tracker owns baseline bounds and verification status.

**Workaround:** Validate raw reference identity on both external and internal roots before publishing totals. Keep lexical results explicitly bounded; do not claim compiler resolution.

## W-1 — Applied mutations established which probe regressions the tests actually catch

**Valid:** dated 2026-09-13

**Observed:** 2026-09-13, four isolated copies of the production probe were mutated separately: restrict cycles to length two; disable test-field masking; erase shared-prefix action fields; remove defaults from the deployed feature lane.

**Pattern:** Apply the candidate mutation, run the real test suite, then inspect the named failure rather than treating a nonzero exit alone as a kill.

**Counterfactual:** The longer-cycle mutation returned to the original faulty behavior; its specific regression failed. The other copies each failed in the corresponding field-mask, action-prefix or deployed-feature test. No source in the shared checkout was mutated for this check.

**Confirming data points:** Four candidates applied, four detected, zero surviving; each run executed 12 tests. Logs: .codescout/measurements/architecture-boundary/2026-09-13/mutation-cycle.log, mutation-field.log, mutation-action.log and mutation-deployed.log. These are bounded site checks, not a mutation score over every probe function.

**Status:** validated

**Rests on:** the retained mutation logs and isolated copies under /tmp/codescout-boundary-mutations-M3Ro6F/. Import normalization was checked separately through observed red/green regressions, not included in this mutation count.

**Promote-when:** No new promotion proposed; this applies the existing Testing Discipline law.

## F-2 — The renderer already reads absent exit_code as "running", so storing job state without publishing it inverts that inference

**Valid:** dated 2026-09-15

**Observed:** 2026-09-15, scouting before the slice-1 job-record change. `format_run_command` (`src/tools/run_command/output.rs:454-537`) matches on `result["exit_code"]` and renders the `None` arm as `… running  (query <id>)`, carrying a comment that asserts the background payload is "`output_id`, `hint`, `stdout` and nothing else". That arm was written one day earlier, at `cc57cd28`.

**Expected (plan):** The unconditional running-claim lived at one site — the `hint` string built in `spawn_background_command` (`src/tools/run_command/inner.rs:89-144`) — making step 0 a one-line honesty fix.

**Got (scouted reality):** Two sites assert running-ness, and the second infers it from the **absence** of `exit_code`. Once a background job carries terminal state, absent stops meaning running: an exited job still renders `… running` unless its status is published under the key the renderer already reads. A job record holding state no renderer consults is `cluster/declared-not-wired` — written, never reachable — so the read path is not a later step but a condition of the write being worth anything. The archived bug also records the standing contract this slice supersedes: that a backgrounded command's exit status is not available through this tool even when everything works.

**Severity:** med — would have shipped a job record whose state no caller could observe, with a green suite, and would have re-fixed a one-day-old bug in the wrong direction.

**Status:** mitigated

**Rests on:** `src/tools/run_command/output.rs:454-537`; `src/tools/run_command/inner.rs:89-144`; `docs/issues/archive/2026-09-14-a-backgrounded-gate-command-was-summarised-as-exit-0-while-its-buffer-held-the-failure.md` (artifact `1f280b2def570b97`, fixed at `cc57cd28`). Slice-1 decision in `docs/trackers/architecture-boundary-measurement.md`.

**Workaround (superseded by what shipped — recorded because the reasoning changed):** The entry first proposed publishing the terminal status into `exit_code` on the background payload. That conflates two payloads and is wrong for the READ path: a later `tail @bg_x` is the reader's own result, so writing the job's code into its `exit_code` would claim `tail` exited 7. What shipped instead: the spawn payload keeps asserting no `exit_code` at all — correct, and now true by construction because the response is emitted at spawn time rather than after a 5s wait — and the job's outcome travels in a separate `jobs` array attached by `OutputBuffer::job_states_in`. The renderer's three-state match is therefore untouched; only its stale comment changed.

## W-2 — The classifier's whole test module was non-discriminating, and only a test driving the real renderer showed it

**Valid:** dated 2026-09-15

**Observed:** 2026-09-15, slice-2 design pass. `usage::content_tests` held five tests of `classify_content_result`, and **every one built its `Vec<Content>` by hand** — including `classify_detects_overflow_by_output_id_not_legacy_key`, whose subject is the overflow envelope. Added one test that instead drives `Tool::call_content`'s buffered arm for real and feeds its actual output to the classifier.

**Counterfactual — measured, not argued.** Mutating the buffered arm to emit the compact summary instead of the JSON envelope (`to_string_pretty(&buffered)` → `to_string_pretty(&raw_summary)`, one occurrence, applied in an isolated worktree) reds **1 of 18** tests in that module: the new one. The other **17 pass**, among them `classify_detects_overflow_by_output_id_not_legacy_key` and `record_content_populates_friction_fields_on_overflow` — two tests named for precisely the property the mutation destroys. Under that mutation `overflowed` is silently `false` for every buffered `OutputForm::Text` call (`grep`, `symbols`, `references`, `tree`, `read_file`, `memory`, `library`, `call_graph`, `symbol_at`, `tree`), `is_friction` goes quiet, and `usage.db` records buffered results as inline. Nothing would have failed.

**What the scout changed about the design.** The slice-2 proposal reads as "introduce a boundary". The seam already exists and one concern already migrated to it: `types.rs` holds the typed `Value` from `self.call(...)`, and field-aware path-stripping runs there — moved up after operating on rendered text corrupted file content (`docs/issues/archive/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md`). Slice 2 is finishing that migration, not starting one, which lowers its risk and its urgency together.

**Doc-vs-code drift found on the way, all three corrected here:** `OutputForm`'s own doc said *"`Text`: inline AND buffered output use `format_compact`"* — the buffered arm has no `output_form` branch at all and `format_compact` fills the envelope's `summary` field; `cap_probe.rs` generalised *"its primary content block is never JSON"*, true of the inline path only; and `classify_content_result` cited a `classify_result` that exists nowhere in the repo. The first two would each have sent a reader to fix a defect that does not exist — which is the shape this scout nearly fell for itself.

**Status:** validated

**Rests on:** `src/usage/mod.rs` (`content_tests::the_renderer_and_the_classifier_agree_about_overflow`); `src/tools/core/types.rs` (buffered arm, `OutputForm`); `src/tools/core/cap_probe.rs`. Gate green all four lanes. Sibling coverage deliberately not duplicated: `core::tests::a_compact_rendered_read_still_carries_the_worktree_notice` drives the same `Text` + `format_compact` fixture through the SMALL path.

## W-3 — A cancelled CI matrix cell bounds the CELL, never the property — the same population-for-member substitution, running backwards

**Valid:** dated 2026-09-16

**Observed:** 2026-09-16. A peer reported CI run `35055091243` (head `43fdc0ea`) as *"23 success, 1 cancelled, 0 failed"* and stated the gap honestly rather than letting it read as coverage: *"windows never completed against 43fdc0ea, so nothing failed there and nothing passed there either."* That sentence is **true of the job cell** and **over-states the gap about the test**. Re-derived every claim rather than quoting it, and the cell-level statement dissolved one level down.

**What the logs actually hold — CORRECTED 2026-09-16, and the correction is this entry's best datapoint; see *The class* below.** First reading: the test grepped out of three job logs as `... ok` — `ubuntu-latest / default`, `macos-latest / default`, `windows-latest / no-features` — so the uncovered cell was called *"exactly (windows × default), both factors independently green"*. **That is wrong, and wrong in this entry's own direction.** The peer opened the CANCELLED cell's log, which I never did, and the test is in it: `test usage::content_tests::the_renderer_and_the_classifier_agree_about_overflow ... ok` at `2026-09-16T04:24:58.9888921Z`, fifteen seconds before the 04:25:13Z kill. Re-verified here against job `104663485783` — the line is at log line 5323, and `grep -c "test result:"` on that same log returns **0**. So **the test has a verdict in every cell including the cancelled one; the SUITE has no verdict there.** The missing artifact is the `test result:` summary line, not the test's outcome. The kill is `cancel-in-progress` (`.github/workflows/ci.yml:15`, exempting `master` only), fired when `f5f48b42` pushed `64411fe0`.

**The `cfg(feature` argument, kept and demoted.** It was built to bridge a gap that turned out not to exist, so it no longer carries the verdict — retained because it still bounds the *feature* question independently of the log, and because deleting it would take the control with it. **Why that cell could not have differed anyway, and the residue that stays.** `cfg(feature` count across the three files on the path — `src/tools/core/types.rs`, `src/tools/output_buffer.rs`, `src/usage/mod.rs` — is **0**, and `pub mod usage;` (`src/lib.rs:59`) carries no attribute; `lib.rs`'s three gates are at `:14` / `:28` / `:39`. **Control, because a zero from a grep is otherwise indistinguishable from a broken pattern:** the same pattern returns `src/heartbeat.rs: 1`, `src/lib.rs: 3`, `src/sqlite_vec_ext.rs: 1`. What that rules out is a feature-conditional *source* difference in the path. What it does **not** rule out is a transitive dependency difference pulled in by a default feature — so the residue is real and small, and the honest statement is *"no feature-conditional code in the uncovered cell"*, never *"the cell is covered"*.

**Counterfactual — what I would have shipped without the scout.** *"Green everywhere except Windows, where it is unknown."* Honest-sounding, under-reports the evidence by a whole platform, and leaves a phantom follow-up — re-run the Windows lane — that nothing needed. The peer would have kept a correct-but-weaker record, and the next reader of it would have had one fewer platform than the run actually bought.

**The class.** CLAUDE.md's § *Testing Discipline* law says an assertion computed over a POPULATION cannot verify a claim about a MEMBER — an aggregate green read as per-member coverage. **This is the same substitution run backwards: an aggregate GAP read as a per-member gap.** Both replace the member's status with the population's, and the reverse direction is the harder one to notice because it errs toward *under*-claiming, which reads as rigour. The tell is a coverage statement whose unit is a **job** when the question's unit is a **test**.

**And the discriminator this entry first published was itself an instance of the class it names.** It read *"grep the test name out of the SIBLING cells' logs"* — which treats the cancelled cell's **conclusion** as bounding its **log contents**: the same substitution one level finer, committed while writing the entry against it. A cancelled job's log is not empty; it holds every line written before the kill. The correct discriminator is the cheaper one that got skipped: **grep the cancelled cell's OWN log first, and fall back to siblings only if it is genuinely empty.** Sibling-bracketing is the weaker instrument, reached for because the cell was labelled `cancelled` and the label was read instead of the artifact.

That is **three** instances of one class inside one exchange: the peer's *"nothing passed there either"*, the sibling-only discriminator, and treating `cancelled` as a statement about content. All three were committed by parties actively writing about the class — § *Observer Blindness*'s measured finding verbatim, and the reason the remedy is a standing move (open the log) rather than a resolution to read labels more carefully.

**Status:** validated

**Rests on:** GH run `35055091243`, job `104663485783` (windows/default, CANCELLED — carries the test's `... ok` at log line 5323 and zero `test result:` lines), job `104663485769` (windows/no-features), plus the two `default` cells; `.github/workflows/ci.yml:13-15` (`concurrency` / `cancel-in-progress`), `:279-283` (matrix — `local-embed` is `--features local-embed --no-default-features`, so it does **not** cover the default set and is not a fourth green cell for this purpose). Verified independently of the peer's report; their numbers matched on every count I re-derived.

## F-3 — The cost that justified a narrowing is 115 ms, so the narrowing is the defect

**Valid:** dated 2026-09-16

**Observed:** `794db556f3cfdf93`'s *Fix* section names the cost to weigh before
building the `doctor` check — *"hashing every artifact's bytes on every `doctor`
run is O(corpus)"* — and proposes a `file_mtime` pre-filter as *"the obvious
narrowing"*, noting that the filter is itself an instance of the cluster the bug
is filed under. That framing survives reading and does not survive measurement.

**Measured** against the live catalog (4,944 rows, 2026-09-16):

| arm | files hashed | wall |
|---|---|---|
| hash every row | 4,943 (83.2 MB) | **115 ms** |
| `mtime` pre-filter | 458 | 25 ms |

The narrowing buys **90 ms** on the largest catalog on this machine. It is not a
narrowing worth an unsound selector, so the right move is to **not build it** —
which avoids the class rather than managing it. The pre-filter's false-negative
count today is **0 of 124**, and that zero is deliberately not the argument:
`git checkout`, `touch -r`, `rsync --times` and restore-from-backup all preserve
mtime across a content change, so the filter is unsound in principle while being
clean in this sample. Citing the zero would be the population-for-member
substitution this ledger already carries twice.

**Second finding, which decides the check's scope.** Divergence is **124 / 4,944
globally (2.5%)** but **6 / 1,712 inside codescout (0.35%)** — 118 of the 124 rows
belong to other repos this catalog has indexed. An unscoped check reports a
worklist that is 95% someone else's, so it must join `ROW_GRAIN_SCOPED_CHECKS`.
`every_declared_check_is_scope_gated_or_a_named_exemption` already reds the build
on the omission, which is the mechanism doing the remembering.

**Third finding, which decides what the check may CLAIM.** The predicate cannot
separate *"a failed `update` lost the catalog half"* from *"the file was edited by
a writer that does not reach the catalog"* — `edit_file`, native `Edit`, `git
checkout`. Both produce stored ≠ disk, and the corpus is dominated by the second.
The repair is `reindex` either way, so the check is useful for both; but naming it
for the bug that prompted it would publish a value correct in one frame under a
name stating another (`IC-24`). It is named for what it observes.

**Cost if unexamined:** an unsound `mtime` selector shipped for 90 ms, a check
reporting 124 rows where 6 are actionable, and a finding whose name asserts a
cause it cannot establish.

**Status:** fixed-verified — all three folded into the check as built.

## F-4 — The check's NAME refused a direction it could not establish; its detail line asserted one anyway

**Valid:** dated 2026-09-16

**Observed:** `6fab2977` named its check `row_behind_file` rather than
`failed_update_divergence`, and the doc comment says why in as many words — a failed
`update` and a non-librarian write produce the identical row, so naming it for the bug
that prompted it would publish a value correct in one frame under a name asserting
another. The **detail line** of the same function then read *"the row describes a
previous version of this file"*.

**The predicate is `on_disk != stored_sha`, which is symmetric.** It fires when the row
is behind its file and when it is ahead of one, and both are reachable because the two
writers order oppositely **on purpose**: `update` writes disk first, so a failed upsert
leaves the row BEHIND; `create` writes catalog first and file last (`create.rs:420` then
`:459`, BUG-058 — a failed upsert must leave no orphan file), so a partially-written file
there leaves the row describing content that never landed, AHEAD. *"A previous version"*
is false in exactly that case, and it is a per-row claim the predicate cannot establish.

**The discipline was applied to the NAME and not to the message beside it.** Both are
public text emitted by one function, both were written in the same commit, and the
argument against over-claiming in the first is the argument against it in the second.
What separated them was that the name was a **decision** — held in mind, argued, recorded
in a doc comment — while the detail line was **prose written to be helpful**, and prose
does not present as a claim requiring support.

**Fix:** the detail now says the row and the file describe different content and names
`reindex`, which is true in both directions. The **name stays**: right for the dominant
case (all 11 rows it fires on here are non-librarian writes that left the row behind),
and a wire string is public vocabulary whose rename costs every citation. An observed
AHEAD instance is the rename trigger.

**Re-verified rather than cited**, per § *Testing Discipline*'s rule that a red is
evidence only for the assertion that produced it: changing the bytes of
`check_row_behind_file` invalidated its three KILL verdicts, so all three were re-run —
inverted predicate, deleted empty-hash abstention, missing-file abstention made to fire —
plus a **fourth** aimed at the new text itself (remove `reindex` from the detail), so the
remedy-naming assertion is proven live against the current string and not the old one.
All four KILLED.

**Cost if unexamined:** a `doctor` finding that tells its reader which direction the
divergence ran, on a predicate that cannot know, in the one case the two write orderings
were deliberately made to differ.

**Status:** fixed-verified.

## Entries

Append new observations through doc append_entry. Do not treat confirming results as independent catches.
