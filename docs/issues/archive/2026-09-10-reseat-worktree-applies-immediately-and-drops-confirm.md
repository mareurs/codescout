---
id: 10a89f48360c9071
kind: bug
status: fixed
title: 'BUG: reseat_worktree applies immediately and never reads confirm'
owners:
- marius
tags:
- cluster/accepted-parameter-silently-dropped
closed: 2026-09-10
opened: 2026-09-10
severity: high
---

# BUG: reseat_worktree applies immediately and never reads confirm

## Summary
`librarian(action="doctor", fix="reseat_worktree")` accepts a `confirm` argument, same as
every other `fix=` mode, but the `"reseat_worktree"` match arm in `run_fix` never reads it —
every no-collision worktree-scoped row the active scope admits is deleted-and-reseeded
(`artifact::upsert` + `graft::graft_rows`) on the SAME call that reports it, whether
`confirm` is `true`, `false`, or omitted entirely.

## Symptom (Effect)
A caller who runs `librarian(action="doctor", fix="reseat_worktree")` expecting a dry-run
preview (the behavior every other `fix=` mode gives, and the behavior three separate
surfaces documented for this one too, until this bug's own I1 fix corrected them) instead
gets an applied repair: worktree-scoped catalog rows are re-keyed onto their main-repo
`id`/`abs_path` and their worktree-scoped predecessor rows are deleted via
`graft::graft_rows`, in the same response that lists what would supposedly need
`confirm=true` to apply.

## Reproduction
1. `git rev-parse HEAD` on `doctor-per-project-isolation` at the time of filing:
   `4d928f2d` (branch tip before this session's commits).
2. Register an active worktree so `scan_worktree_scoped` has a `no_collision` row to find
   (see `run_fix_rehome_dry_run_then_confirm_migrates_rows` and neighboring tests in
   `src/librarian/tools/doctor.rs` for the fixture shape).
3. Call `librarian(action="doctor", fix="reseat_worktree")` with `confirm` omitted (or
   explicitly `false`).
4. Observe the response's `"reseated"` array is non-empty and the underlying catalog rows
   are already re-keyed — no second call with `confirm=true` was needed or possible.

## Environment
Rust MCP server `codescout`, branch `doctor-per-project-isolation` (worktree at
`/home/marius/work/claude/codescout/.worktrees/doctor-per-project-isolation`), main
checkout at `/home/marius/work/claude/codescout` on branch `experiments`. No transport- or
OS-specific factor — the mechanism is pure Rust control flow.

## Root cause
`run_fix`'s `"reseat_worktree"` match arm (`src/librarian/tools/doctor.rs:1722-1737`) calls
straight into `reseat_worktree(ctx, &mut doctor_scope, scope_fallback)` — it never inspects
the `confirm: bool` parameter `run_fix` itself received (`run_fix`'s signature,
`src/librarian/tools/doctor.rs:1636-1641`, takes `confirm` for its other arms —
`prune_missing`, `repair_frontmatter_id`, `rehome` all branch on it). `reseat_worktree`'s own
signature (`src/librarian/tools/doctor.rs:1988-1992`) takes `ctx`, `scope`, `scope_fallback`
— no `confirm` parameter at all — so there is no dry-run branch to take even if `run_fix`
forwarded the flag.

*measured 2026-09-10: `grep(pattern="\"reseat_worktree\" =>", context_lines=15)` over
`src/librarian/tools/doctor.rs` on the worktree checkout — the match arm body
(lines 1722-1737) contains no reference to `confirm` anywhere in its 16 lines, unlike the
`"prune_missing"` arm immediately above it, which branches on `if !confirm { … dry run … }`
before ever calling a mutating function.*

## Evidence
### `run_fix`'s `"reseat_worktree"` match arm — no `confirm` read
```
        "reseat_worktree" => {
            // Require, not Literal — same reasoning as the report path's own
            // resolve_scope call above `call()`'s catalog lock: `reseat_worktree`
            // is a search-shaped repair (it finds rows, then acts on the ones
            // found), so `all` without an umbrella has nothing to widen to.
            let (effective_scope, scope_fallback) = super::scope::resolve_scope(
                scope,
                ctx.current_project.as_deref(),
                super::scope::UmbrellaPolicy::Require,
                super::scope::Scope::Project,
            )?;
            let cat = ctx.catalog.lock();
            let mut doctor_scope = scope::DoctorScope::new(effective_scope, ctx, &cat.conn)?;
            drop(cat);
            reseat_worktree(ctx, &mut doctor_scope, scope_fallback)
        }
```
(`src/librarian/tools/doctor.rs:1722-1737`)

### `reseat_worktree`'s own signature — no `confirm` parameter
```
fn reseat_worktree(
    ctx: &ToolContext,
    scope: &mut scope::DoctorScope,
    scope_fallback: bool,
) -> Result<Value> {
```
(`src/librarian/tools/doctor.rs:1988-1991`)

### Pre-existing at `0f060fe3`, not introduced by this branch
`git show 0f060fe3:src/librarian/tools/doctor.rs` (buffered, `@cmd_8a84ae2b`) contains
`fn reseat_worktree(ctx: &ToolContext) -> Result<Value> {` at line 1554 — an earlier
signature (pre-`DoctorScope`), but already with no `confirm` parameter. `git
merge-base --is-ancestor 0f060fe3 experiments` confirmed `0f060fe3` is an ancestor of
`experiments`, committed 2026-09-09 07:35:57 — well before this branch's
`doctor-per-project-isolation` isolation work began. The drop is pre-existing, not
introduced by this branch's scope/isolation changes.

## Hypotheses tried
1. **Hypothesis:** The coordinator's original review (round 2, I1) claimed this as a bug
   introduced by the current branch's work.
   **Test:** `git show 0f060fe3:src/librarian/tools/doctor.rs | grep -n "fn reseat_worktree"`
   plus `git merge-base --is-ancestor 0f060fe3 experiments`.
   **Verdict:** rejected — confirmed pre-existing at `0f060fe3`, dated 2026-09-09, an
   ancestor of `experiments`. See Evidence above.
   **Evidence link:** "Pre-existing at `0f060fe3`" above.

## Fix

**FIXED on `experiments` 2026-09-10 at `ebaae018`** (patch-id
`36855953323d3af112677cca5499977c9c8f5e16`; verified with
`git merge-base --is-ancestor ebaae018 experiments`). Fix option (a) from the original
plan — thread `confirm` through rather than rename the parameter out of the shared
vocabulary.

`reseat_worktree` now takes `confirm: bool` and `run_fix`'s `"reseat_worktree"` arm
forwards it. **The branch sits INSIDE the row loop, not at the top of the function**, and
that placement is the substance rather than a detail: it is reached only after scope
admission, the `registered`-row skip, the `no_collision` classification and the
`artifact::get` race check, so the preview is drawn from the same population the apply
walks. An early return would have previewed a population computed by different code from
the one that writes — the same defect in miniature that Task 6 fixed by giving the report
and the repair one shared `admit()` gate. `prune_missing`'s dry run records the identical
obligation in its own comment (*"the dry-run preview's totals never promise more than
`confirm=true` would actually delete"*).

The response gains `"mode"` (`dry_run` | `applied`) and `"would_reseat"`. **Both arrays
are always present, one of them empty** — so a caller keying on `reseated` reads zero
from a preview rather than `null`. An absent key is a silence, and this response is read
by an operator deciding whether a destructive re-key has already happened.

**The three doc surfaces that documented the exception are back to the unqualified rule**
(`librarian.rs`'s `fix` schema description, `reseat_worktree`'s doc comment,
`src/prompts/guides/librarian.md`'s doctor-repairs section), and
`TOOL_SURFACE_CHAR_BUDGET` ratcheted back `56_548 → 56_492` — re-measured by a report
run, not derived by subtracting 56. The budget log records **why** the bytes came back,
because the distinction is a live hazard: they were returned by fixing the mechanism, not
by trimming the warning, and a reader scanning for reclaimed bytes must not read this as a
precedent for shortening an operative fact to fund something else. The general form is
worth having on that surface — **documenting a defect costs schema budget that every
session pays on every request, so a doc-only fix for a code defect is a rental, not a
settlement.**
## Tests added

`reseat_worktree_dry_runs_unless_confirm_is_true` (`src/librarian/tools/doctor.rs`).

**It asserts both halves on ONE fixture, and that is the design rather than brevity.** A
test asserting only that the dry run left the catalog alone is MONOTONE under *"the repair
is broken and never writes anything"* — it passes just as well against a `reseat_worktree`
gutted to a no-op, which is the likeliest future mutation. Driving `confirm=true`
afterwards from the same seeded state is what makes the first half discriminating: the
only difference between the two calls is `confirm`, so no function that ignores it can
produce both outcomes. The catalog assertions read the DB directly rather than the
response, because the response is the thing under test and cannot be its own witness.

**Two production mutations, each observed red and reverted** — not one per feature but one
per direction, since a dry-run gate can fail by never previewing or by never applying:

| mutation | site | result |
|---|---|---|
| dry-run branch deleted (`if !confirm` → `if false`) — exactly the pre-fix behaviour | `reseat_worktree`'s row loop | **only the new test reds**; the five pre-existing `reseat_worktree` tests all PASS |
| `run_fix` forwards `false` instead of `confirm` — repair permanently inert | `run_fix`'s match arm | **5 of 6 red** |

**The first row is the more useful measurement.** It does not merely show the new test
works; it shows the five tests that already existed could never have caught this bug — the
coverage gap measured rather than asserted.

**Four existing tests were flipped to `confirm=true` and given a `"mode"` assertion,
because adding the dry-run branch would otherwise have gutted them silently.** Their
claims are absence-shaped — *"`reseated` is empty"*, *"`abs_path` unmoved"*, *"a
registered row must not be reseated"* — and are therefore monotone under a dry run that
reseats nothing at all, so a preview would satisfy every one of them without exercising
the collision or registered-skip logic they exist to pin. **The two changes do different
jobs and should not be conflated:** the `confirm=true` flip protects the original
assertions from vacuity, while the `"mode"` assertion is what actually fires under the
inert-repair mutation. The flip alone would have been silent there.
## Workarounds

None needed — fixed. Historically: treat every `fix="reseat_worktree"` call as live and
irreversible-without-a-manual-`graft`, regardless of `confirm`.
## Resume

Nothing owed. Fixed at `ebaae018`, on `experiments`, with a regression test and two
observed mutation reds.
## References
- `src/librarian/tools/doctor.rs:1722-1737` (`run_fix`'s `"reseat_worktree"` match arm)
- `src/librarian/tools/doctor.rs:1988-2058` (`reseat_worktree` function and its doc comment)
- `src/librarian/tools/librarian.rs:108` (`fix` param schema description — corrected by I1)
- `src/prompts/guides/librarian.md` (doctor-repairs guide section — corrected by I1)
- `docs/trackers/issue-clusters/IC-15-accepted-parameter-silently-dropped.md` (cluster
  membership)
- Whole-branch review round 2 report, coordinator-directed session, 2026-09-09/2026-09-10
  (I1, I2)
