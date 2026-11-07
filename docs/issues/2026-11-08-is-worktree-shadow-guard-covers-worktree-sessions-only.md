---
id: ee030095cae1cf76
kind: bug
status: open
title: is_worktree_shadow guards worktree sessions only, so other callers can still allocate an entry id on a worktree shadow
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- librarian
- worktree
opened: 2026-11-08
owner: marius
related:
- docs/issues/archive/2026-11-05-append-entry-worktree-override-bypasses-main-only-rule.md
severity: low-medium
unverified: Not reproduced by running; read from code only.
---

# BUG: is_worktree_shadow guards worktree sessions only, so other callers can still allocate an entry id on a worktree shadow

## Summary

Commit `52440e6b9` made `append_entry` refuse prose-ledger id allocation when the named row is the calling worktree session's own shadow of a main ledger (archived bug `8b466b2c4502a6ea`). The guard `is_worktree_shadow` returns `Ok(false)` at once unless the session is a worktree, and it is applied only on the prose-ledger branch and only when a current project exists. By reading the code, at least three other callers can still allocate an id on a worktree shadow, which is the id-collision class of the fixed bug, and the params-ledger branch has no allocation guard at all.

## Class fit

Class `IC-14` (a guard's coverage is narrower than its name). The guard and its refusal text speak of a worktree shadow and "id allocation is not supported from a worktree checkout", but the predicate decides by the session type (`cp.main_root.is_some()`) and by the row lying under the session's own worktree root, so it covers one shape of "a caller touching a shadow" and nothing else. It differs from the class's other members in that the limit is not hidden: the function's own doc comment has a "Scope" paragraph that names the uncovered callers (a main-checkout session, a call with no current project) and calls the narrowing deliberate, while the call-site comment in `append_entry.rs` and the archived bug's closing state do not. The gap is a recorded decision whose consequence (an id collision of the already-fixed kind) was left unfiled until now.

## Symptom (Effect)

No collision was observed for these residual cases; nothing was run. The symptom the fixed bug recorded (observed 2026-11-05, archived bug `8b466b2c4502a6ea`) was `append_entry` returning an id (`LMC-4`) with `section_written: true` for a row whose path lies in a linked worktree, against the served guide's "From the MAIN checkout only". Per `src/librarian/tools/append_entry.rs:239-244` the consequence of allocating on a shadow is that the shadow's distinct `artifact_id` misses main's reservation, so main and the worktree issue the same id, and for a prose ledger "nothing can repair it afterwards". The residual cases below would produce the same outcome if the code reading holds.

## Reproduction

NOT reproduced by running. Cargo was not permitted in the session that wrote this record; everything below was confirmed by reading code paths only. Plan, modelled on the test `prose_allocation_is_refused_on_a_worktree_shadow_of_a_main_ledger` (`src/librarian/tools/append_entry.rs:1379`) and its helpers `worktree_prose_fixture()` (line 1315, ledger `docs/trackers/ledger.md` with `entry_prefix: HY` and newest entry `HY-10`, worktree at `<main>/.worktrees/feat`) and `catalogue_ledger` (line ~1344):

1. Common setup: `worktree_prose_fixture()`; `catalogue_ledger(&ctx, &main_id, &main_abs)`; fork the shadow with `worktree::resolve_write_target(&mut cat, &ctx, &main_id)` exactly as the model test does; assert `shadow_id != main_id` and that the shadow's `abs_path` equals `wt_abs`.
2. Case (i), main-checkout session: build a second context over the same catalog whose `current_project` is `Some(CurrentProject { abs_path: main_root, git_root: main_root, main_root: None, umbrella: None })`. Call `append_entry` with `{"id": shadow_id, "id_prefix": "HY", "title": "from main", "body": "b", "anchor_heading": "# Ledger"}`. Expected today (from the code read): `Ok` with id `HY-11` written into the shadow's file. Expected after a fix: the `LibrarianRecoverableError` the model test asserts. (Whether the catalog can be shared between two contexts, or one context's `current_project` must be swapped, is to be checked when writing the test; the builder used is `TestToolContextBuilder::new(Catalog)`.)
3. Case (ii), no current project: a context built without `.with_current_project(...)` (as `mk_ctx()` at line 479 does). Same call. Expected today: `Ok`, `HY-11`.
4. Case (iii), worktree A names worktree B's shadow with B outside `main_root`: the fixture's worktree sits under `main_root`, which is refused by the OTHER disjunct (`is_main_checkout_artifact`), so B must live elsewhere (a sibling temp directory, like the `~/.codex/worktrees/...` path of the original report). Fork B's shadow from a context whose `git_root` is B, then call from a context whose `git_root` is A (`main_root: Some(main)`). Expected today: `Ok`.
5. Control for each: after the call, allocate from main on `main_id` and observe `HY-11` again (the collision), then assert it is refused after the fix.
6. Params-ledger branch: repeat step 2 with an `entry_collection` ledger; expectation not established by reading (see Root cause).

## Environment

Rust codescout server, branch `experiments` (shared, dirty checkout), code read in the working tree on 2026-11-08. Librarian catalog in memory in the tests. Applies to sessions running from a linked worktree or the main checkout, with or without a current project.

## Root cause

Mechanism confirmed by reading the code path for each case; none measured at runtime ("inferred from the source, not measured").

The guard:

- `src/librarian/tools/worktree.rs:56-81`, `is_worktree_shadow(cat, cp, row)`. Line 61: `if cp.main_root.is_none() { return Ok(false); }`. Line 66: `if !under(&row_s, &wt_s) { return Ok(false); }` (the row must be under THIS session's worktree root). Only then does it look for an outgoing `worktree_of` link (line 71) or a `worktree_fork` event (line 77). Its doc comment says: "Scope: only WORKTREE sessions are covered (`cp.main_root` is `Some`); for any other session this returns `false` without looking at the row. A main-checkout session that names a worktree shadow's id, and a call with no current project at all, are therefore NOT caught by this predicate. That is deliberate".
- `src/librarian/tools/worktree.rs:28-39`, `is_main_checkout_artifact`: false when `main_root` is `None`; otherwise true only for a path under `main_root` and not under the session's own worktree root.
- `src/librarian/tools/append_entry.rs:217`: `if a.entry_collection.is_none() {` opens the prose-ledger branch. Line 253: `if let Some(cp) = ctx.current_project.as_deref() {`; lines 259-260: `if is_main_checkout_artifact(cp, &row.abs_path) || is_worktree_shadow(&cat, cp, &row)?` then the refusal "append_entry: id allocation is not supported from a worktree checkout". The guard is nested inside the current-project block and exists only on this branch.
- After the guard, line 315 `resolve_write_target(&mut cat, ctx, &a.id)` and then `allocate_entry_id` run against the returned id. `resolve_write_target` (`worktree.rs:206-282`) returns the id unchanged when there is no current project, when `main_root` is `None`, when the row is unknown, or when `is_main_checkout_artifact` is false ("already shadow, or foreign repo").

Per case (CONFIRMED means: the full path was read and each branch condition checked against the case; none REPRODUCED by running):

- (i) a main-checkout session names a worktree shadow's id: the session has `current_project = Some` with `main_root = None`. `is_main_checkout_artifact` is false (`main_root` is `None`), `is_worktree_shadow` returns false at line 61, so the refusal is skipped; `resolve_write_target` returns the id unchanged (`main_root` is `None`); the allocation runs on the shadow's id. CONFIRMED by reading; not reproduced.
- (ii) a call with no current project: the whole `if let Some(cp)` block at line 253 is skipped and `resolve_write_target` returns at its first `let Some(cp) ... else`. The comment at `append_entry.rs` ~274-277 notes most callers have none. CONFIRMED by reading; not reproduced.
- (iii) a session in worktree A names worktree B's shadow where B is outside `main_root`: `is_main_checkout_artifact` is false (row not under `main_root`), `is_worktree_shadow` returns false at line 66 (row not under A's root), `resolve_write_target` returns the id ("already shadow, or foreign repo"). If B is under `main_root` the first disjunct refuses it, so only the outside-`main_root` shape is open. CONFIRMED by reading; not reproduced.
- The params-ledger branch: the allocation path after line 395 has a guard only for `cites` (`is_main_checkout_artifact`, lines ~413-421) and then `resolve_write_target` (line 424); there is no allocation refusal. The comment at `append_entry.rs:239-244` says a params ledger differs because `merge_worktree`'s renumber repairs it afterwards (`entries_renumbered`, `merge_worktree.rs:452`). I read that comment and the reference to the renumber, but NOT the renumber code, so whether the params branch is safe for cases (i)-(iii) is not established. Left alone by the fixed bug on purpose.

Origin of the concern: the originally observed allocation (archived bug `8b466b2c4502a6ea`, Symptom) came from an active MAIN session with a per-call `workspace=` override to a worktree outside the main root (`/home/marius/.codex/worktrees/615e/codescout`). The archived bug records that "which exact row the originally observed id was (shadow or worktree-born) is not verifiable"; this record does not resolve that either. What `ctx.current_project` is for a main session with a workspace override was not read here.

## Evidence

- Source lines above, read on 2026-11-08 with codescout `symbols`/`read_file`/`grep`; commit `52440e6b9` subject verified with `git log -n1` ("fix(librarian): append_entry refuses prose id allocation on a worktree shadow of a main ledger, not only on the main row (7351eb231b93a647)"; the id in its subject is the bug's pre-re-point id).
- Model test read in full: `append_entry.rs:1379-1483` (fork via `resolve_write_target`, then `append_entry` on `shadow_id`, assert refusal, file unchanged, 2 artifact rows). Control `prose_allocation_is_allowed_on_a_worktree_born_ledger` (born in the worktree, no lineage) must stay allowed.
- Nothing was executed.

## Hypotheses tried

1. Hypothesis: the guard's outer condition covers main sessions through `is_main_checkout_artifact`. Test: read both predicates. Verdict: rejected; both return false when `main_root` is `None`.
2. Hypothesis: `resolve_write_target` redirects a shadow-naming call back to main. Test: read `worktree.rs:206-282`. Verdict: rejected; a row that is not a main-checkout artifact passes through unchanged.
3. Hypothesis: the params branch has the same unguarded path. Verdict: deferred; guard absence read, repair behaviour (`merge_worktree` renumber) not read.

## Fix

Open; no decision taken. Suggested direction, recorded as an option only: refuse prose-ledger allocation whenever the named row has an outgoing `worktree_of` link (the lineage test already inside `is_worktree_shadow`, `worktree.rs:71`), whatever the session type, and drop the `cp.main_root.is_none()` early return and the `under(&row_s, &wt_s)` precondition for that check. That one rule would cover cases (i), (ii) (if the check is also moved out of the `if let Some(cp)` block, which needs the catalog but not `cp`) and (iii). Costs and questions to settle first: the `worktree_fork` event half of the predicate needs no `cp`, so it can move too, but a legacy shadow with neither record cannot be recognised by any variant; the guard sits inside a block that needs `cp` today, so the signature of `is_worktree_shadow` changes; the refusal hint says "Reserve the id from the main checkout" and would need rewording for a main-checkout caller who named a shadow; and the params-ledger branch needs its own decision after reading the `merge_worktree` renumber.

No fix SHA or patch-id exists.

## Tests added

None yet. Justification: open bug; reproduction is a plan only (see Reproduction). The existing tests cover the worktree-session shapes only: `prose_allocation_is_refused_on_a_worktree_shadow_of_a_main_ledger` (`append_entry.rs:1379`), `prose_allocation_is_allowed_on_a_worktree_born_ledger`, and one test per half (lineage link, fork event) of the shadow check.

## Workarounds

Allocate entry ids from the main checkout against the main row's id, never against the id a worktree write returned. Do not name a worktree shadow's `artifact_id` in `append_entry` from any session. (Convention taken from the refusal hint at `append_entry.rs` ~262-268; nothing enforces it for the cases above.)

## Resume

Write the reproduction tests for cases (i)-(iii) per the plan above, in `src/librarian/tools/append_entry.rs` next to line 1379, run them with `scripts/with-slot.sh cargo test <name>` and confirm they are red on the current tip (that turns "confirmed by reading" into a measurement). Then read the `merge_worktree` renumber (`src/librarian/tools/merge_worktree.rs`, near line 452) to settle the params-ledger branch, then choose the fix option with the owner.

## References

- Archived parent (fixed): `docs/issues/archive/2026-11-05-append-entry-worktree-override-bypasses-main-only-rule.md` (id `8b466b2c4502a6ea`); fix commit `52440e6b9`.
- Earlier bug of the same class, cited in the code: `docs/issues/archive/2026-08-17-prose-ledger-worktree-id-collision.md`.
- `src/librarian/tools/worktree.rs` (28-39, 56-81, 206-282), `src/librarian/tools/append_entry.rs` (217, 239-244, 253-261, 315, 395-424, 1315, 1379), `src/librarian/tools/merge_worktree.rs:452`.
- Class: `docs/trackers/issue-clusters/IC-14-guard-narrower-than-its-name.md`.
