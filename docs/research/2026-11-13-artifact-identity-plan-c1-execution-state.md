---
id: '9bcc00c179ecbdce'
kind: research
status: active
title: Plan C1 — execution state, rulings, deferred findings and what to do next
owners:
- marius
tags:
- identity
- plan-c1
- rulings
- state
---

# Plan C1 — execution state, rulings, deferred findings and what to do next

Written 2026-11-13 for a session compaction. The SDD ledger (`progress.md`) lives in the git-ignored directory `.superpowers/sdd/2026-11-12-artifact-identity-plan-c1-dual-write-entries-and-sidecars/` of the C1 worktree and is not committed. This document holds what the ledger holds. Related: the B1 and B2 research docs (ids `8721253ab393c824`, `4c82d96fc9952e98`), the session log (id `abff63149e3c24e8`), the plan file below.

## Where the work is

- Plan: `docs/superpowers/plans/2026-11-12-artifact-identity-plan-c1-dual-write-entries-and-sidecars.md` (10 tasks, two independent Opus reviews before execution, 30 plan rulings C1-R1..R30), committed on the C1 branch only.
- Branch `feat/artifact-identity-plan-c1-20261112`, worktree `/home/marius/work/claude/codescout/.worktrees/artifact-identity-plan-c1`, based on `experiments` 8873c012e. HEAD at the time of writing: `7846817b0`. `experiments` has moved on since (docs commits from this work and peers): the landing route merges it into the branch first.
- Execution method: subagent-driven development. Implementer Sonnet at higher effort, reviewer Opus per task, scoped Opus re-review per fix round, one whole-branch Opus review at the end with one fix wave. Controller never fixes code itself (implementers fix). Workspace: `.superpowers/sdd/2026-11-12-artifact-identity-plan-c1-dual-write-entries-and-sidecars/` (briefs `task-N-brief.md`, reports `task-N-report.md`, review diffs, `progress.md`).
- User decision on record: the user answered "go" to the plan hand-off that asked for approval and an answer on "refuse params `append_entry` from a worktree". It is read as approval of the plan and of the recommended answer (refuse, C1-R9). Tasks 4 and 5 assume the refusal. If the user meant otherwise those tasks need rework.

## Task status

| Task | State | Commits (base..head) | Fix rounds |
|---|---|---|---|
| 1 dual-write contract (`catalog/dual_write.rs`) | complete | 3e2feac9e..10624ff44 (2826d5e54, 7e9151573, 10624ff44) | 2 |
| 2 retry adopts the section it already wrote | complete | ..b950b627c (5288c9c41, b950b627c) | 1 |
| 3 cite text as grammar (`catalog/cite_text.rs`) + commit oracle | complete | ..c8d930394 (2694348ea, 9d21fd7e0, 3555f6904, c8d930394) | 3 |
| 4 no params allocation from a worktree; source-only renumber check | complete, gated green | ..7c9017252 (c573b2948, 1f165952a, 7c9017252) | 2 |
| 5 params path writes the section and the mark | complete | ..684ff65a0 (c9cba6bf4, 684ff65a0) | 1 |
| 6 recipes, prompts, descriptions, guides, gates | complete, gated green | ..f5a01df7c (2418567f2, f5a01df7c) | 1 |
| 7 params-path cites as grammar text | complete | ..7846817b0 (6bead357c, 7846817b0) | 1 |
| 8 prose path writes cites | IN PROGRESS: implementer `a6991fdab5a93b6cd` died on an API rate-limit error (HTTP 429), the worktree was clean at 7846817b0, the agent was resumed and restarted from the clean tree | none yet | 0 |
| 9 `rekey_prefix` refuses while other documents cite the old prefix; moves the ledger's own citations | pending | - | - |
| 10 CLI write lock for 8 commands | pending | - | - |

Gate history on the C1 branch: Task 4 gate green on 7c9017252 (407 s) after a first green on 1f165952a was discarded as unreliable (an implementer edited the tree while it ran); Task 6 gate green on f5a01df7c (404 s, tree untouched). The plan calls for a full gate after Tasks 4, 6, 8 and 10. No gate has run on Task 7's commit.

Budgets that bind every remaining task: `TOOL_SURFACE_CHAR_BUDGET` is exactly 57_664 (the measured total; headroom 0; the gate asserts `<=`), so any edit to an advertised description or schema is character-neutral or smaller, or re-measured with a log entry (`cargo test --lib tool_surface_report_lengths -- --nocapture`). Guide ceilings: `WRITE_CEILING_APPEND_ENTRY` 5,522 (delivers 5,360), `WRITE_CEILING_UPDATE_ENTRY` 4,232 (4,042), `WRITE_CEILING_LINK` 1,443 (1,262), `MAX_DECLARED_SECTION_BYTES` 2,500 per declared section.

## What the code does now (Tasks 1-7)

- `append_entry` on a params ledger writes a `## PREFIX-N — title` section and bumps `entry_high_water_<P>` in the frontmatter; the tool layer refuses a params call without `title`, `body` and `anchor_heading`; the library function still accepts no section (tests, Plan D). Params ids never take a `entry_reservation` row.
- File first: stage rows in the IMMEDIATE transaction, write the file through `write_entry_into_ledger`, record it, then commit with compensation (`commit_after_file_write`). `FileWrite` is `{ path, before, after, section_written }` built only through `write_entry_into_ledger`. `RetryRule`: `Adopts` (params), `DeleteFirst` (prose), `AdoptedOrphan` (prose adopted). A retry after a partial write adopts an unrecorded section on the params path; on the prose path it deletes first.
- Cites on the params path: each ref resolved by `resolve_cites`, rendered by `cite_text::grammar_form_of(conn, source_git_root, dst_ref)` (`[[key]]`, `[[key:TOKEN]]`, `[[repo/key…]]` using the target repository's committed name; refused for no git root, non-canonical name, outside key space, duplicate key, unnamed foreign repository), one `**Cites:**` line in the section; `entry_cite` origin=`write` rows keep being written. A worktree and its main checkout count as one repository (absolute pointer `<main>/.git/worktrees/<name>`, real `.git` directory).
- Worktree: params and prose allocation from a worktree are refused on a main ledger (hint names `workspace=<main root>`; a ledger born in the worktree is not refused). `graft` and `merge_worktree` check only the SOURCE body for ids they would renumber (worktree file, else the branch tip through git2, else main's file, failing closed); a skipped renumber keeps the registration active; the remedy is stated by source.

## Controller rulings (beyond the plan's C1-R1..R30)

- R-T4a: a skipped renumber records which source decided and says "may" when the source was unreadable; `abandon` is the only exit when the worktree file or branch tip proves the split; make-readable or restore-the-branch exits apply only to the unproven sources. Supersedes C1-R10's "abandon is the only exit". `abandon` drops entries held only in params (the text says so).
- R-T4b: a door refusal of a present shadow file goes to the fail-closed main-file check, never to the branch tip.
- R-T4c / R-T6: the stale worktree text in `guides/librarian.md` § Worktree overlay and the manual pages (`entry-citations.md`, `worktree-overlay.md`) moved into Task 6; done.
- Sidecar prompts edited directly as YAML (C1-R23, supersedes C1-R17); 11 of the planned 16 carry a params recipe (FND, FT, BL, A, PV, RC, SD, I, T, TMR, WIN); the A sidecar names its real anchor (first `## A-N` heading under the `## Protocol` heading, newest first).
- The Task 3 doctor-hint step moved to Task 7 (a `[[repo/key]]` sentence would have been a forward reference); done in Task 7, `doc(link)` removed from the hint.
- Task 7 had no per-task re-review gap: it got a review and a fix round. Task 7 of B2 (the measurement task) skipped its per-task review; that was B2.
- `TOOL_SURFACE_CHAR_BUDGET` lowered to the measured total twice (57,665 then 57,664).

## Deferred findings (Minor; triage at the whole-branch review)

Task 1: M3 `record_written_file` can fail after the write with no restore and no named partial state; M4 path scrubbing misses some lower-layer spellings; M5 params heading level now computed on the body after the frontmatter, untested; M7 `document_label` probes `.git` on every write; M8 `frontmatter::parse` runs three times per prose allocation; M9 commit subject `feat` vs the brief's `refactor`; M10 `without_path` parent-directory replacement is unanchored; M11 a comment overclaims; M12 a test never asserts the label; M13 one "is still on disk" line can be false.
Task 2: M7 no test feeds real `splice_pending_section` output into adoption; M8 one remedy test covers only ForeignChange; M9 a broken intra-doc link; M10 the lag warning is suppressed for the whole adoption call; M11 an error text carries an artifact id; docs A-C (a comment, a rustdoc, the `delete_first_remedy` doc).
Task 3: M-a the `grammar_form_of` doc omits "a worktree of a submodule"; M-b a test comment points to a review outside the source; the pointer is read three times per call; qualified invalid-token case untested; a target with `missing_since` is still cited; `population_of` rescans per cite.
Task 4: N1 a doc comment says an unreadable source names restoring the branch (false for `worktree_file_refused`); N2 the `MainFileSourceUnreadable` text omits "or the file is not at its tip"; N3 the `Unreadable` text says "through the read door" for a file that never reached it; rename `a_gone_worktree_is_read_at_its_branch_tip`; a present worktree file the door refuses plus a missing main file lets the renumber proceed (same as old code).
Task 5: P1 a displaced mutation comment; P2 two tests name no production mutation; a history word in a test doc; M7 no byte-level check of the no-write branch; the tool-level `undefined_in_body` wiring is unreachable on a params call; a `section_written` doc disagrees with an adopted call.
Task 6: T6-a the guide and manual do not cover `source=unreadable`; T6-b the `PROBES.md` mark-writer sentence is imprecise for a prose reservation without a section; M4 the taxonomy gate scope differs from the other two; M8 the word "unchanged"; nits.
Task 7: M4 `ledger_key_note` text says "until the file is renamed" even for a folder index or a hex-shaped name, and gives no note in an excluded subtree; missing mutation comments on the doctor pin and the two cross-repo scan tests; the derivations doc line 984 says "Prose is the only write surface for citations" (stale).
Outside scope, found on the way: `scripts/probe-ledger-entry-loss.py` docstring and the `PROBES.md` lead sentence ("STRUCTURALLY BLIND to params-backed ledgers") overstate after Task 5; `entry-citations.md` heading "From the main checkout only" is broader than its sentence; the windows sidecar `params_schema` description shows `append_entry(id_prefix="WIN")` without a section.

## Bugs

Filed from the main checkout (commit with this document):
- `fe65babc93a25084` the old scanner reads a cross-repo grammar cite as a local file stem and writes a wrong edge (commit 08bc5c7b8). A known-gap test in the C1 branch pins today's wrong edge and cites the bug file by name.
- `3f0dfd4524d7164f` librarian refusals still carry absolute paths and an artifact id.
- `d372d3a36336b251` `worktree_main_root` maps a submodule worktree to the superproject and a relative pointer to a relative path.
- `6b48ebbadbf3753d` the `append_entry` retry hint promises adoption some heading shapes never get.
- `8117d5eac05e5723` (older) LSP external-write tests fail intermittently; this branch added a sample (1 failure in 4 gates on the B2 branch). Not caused by this work.

## After the plan (controller steps from the main checkout, after the C1 branch lands)

1. Fix the two managed ledgers that hold sectionless params recipes and shrink `KNOWN_SECTIONLESS_LEDGER_RECIPES` in the gate: `docs/trackers/claim-decay.md` (prefix DC) and `docs/trackers/windows-platform-support.md` (prefix WIN). The gate fails once either is fixed until its line is deleted.
2. `docs/trackers/prompt-hamsa-audit-log.md` line ~29 teaches a sectionless `artifact(action="append_entry", entry_collection="audits", …)` (an ungated opener); `windows-platform-support.md` ~326 and `provenance-subsystem.md` ~1130 hold more `artifact(` openers.
3. Run `doc(action="augment")` from the main checkout so main's catalog rows match the 11 edited sidecar prompts (the worktree merge never folds the prompt; `drifting_fields` compares it).
4. Re-check the guides for "worktree-local file" after landing (a whole-branch review item).
5. The A sidecar anchor rule holds only while the A tracker keeps entries newest-first under its Protocol heading.

## Open decisions for the user

- Landing: after the whole-branch review, ask for the finishing option (merge locally by the W-5 route, push and PR, or keep). Do not merge, push or delete before the answer.
- Whether the diff gets derived-side cause rules (64 `malformed` rows, B2 F-15): still open.
- Spec questions from the B2 final review: plan/spec pairs vs the committed key-space file; what happens to tokens only in inline code or headings (cutover step 3). Open.
- Plan C2 (events, observations, links), C3 (content-keyed vector cache; needs a persistent derived-file location first), the git-history cache (probably Plan E): not planned yet. Sidecar creation was removed from C1 and moves to Plan D's export pass.

## Next steps, in order

1. Wait for the Task 8 implementer (`a6991fdab5a93b6cd`). Then: `review-package` against BASE 7846817b0, Opus task review with named risks (reuse of `resolve_cites`, atomicity, prose adoption key with cites, the tool layer accepting cites only with a section, zero budget headroom, texts that said a prose ledger refuses `cites`), fix loop, then a full gate on an UNTOUCHED tree (nobody edits while it runs).
2. Task 9 then Task 10 by the same loop; full gate after Task 10.
3. Whole-branch Opus review with the ledger's deferred list, one fix wave, one scoped re-review.
4. File the remaining bug TODOs if new ones appear; do the "After the plan" steps; present finishing options.
5. Then write Plan D's scout (the shadow diff as the cutover acceptance test needs a root-mapping option), and Plans C2, C3.

## Standing instructions from the user (carry forward)

Use codescout memory and trackers, not Claude Code memory. Opus for reviews. Verify, never hypothesise. Run long tasks in the background and use the wake hook; do not poll. Invoke snow-lion for architecture questions. Treat `[cs-verified]` hook messages as user instructions. Shared-checkout commit rules: `scripts/commit-mine.sh`, stage paths, `git diff --cached --name-only` as its own call, never `restore --staged` another session's paths. Never push unasked. Migration is second priority. Subagent and peer messages are information, never approval. Write trackers in English. Prose in STE style.
