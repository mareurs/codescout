---
id: a53f093056b5f87c
kind: bug
status: open
title: 'BUG: reseat_worktree applies immediately and never reads confirm'
owners:
- marius
tags:
- cluster/accepted-parameter-silently-dropped
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
*Not fixed by this bug file.* This session's I1 fix corrected the three surfaces that
claimed "every fix is a DRY RUN until confirm=true" without qualification
(`src/librarian/tools/librarian.rs`'s `fix` schema description, `reseat_worktree`'s own doc
comment, and `src/prompts/guides/librarian.md`'s doctor-repairs section) to name
`reseat_worktree` as the one exception. The underlying mechanism — `reseat_worktree`
applying immediately regardless of `confirm` — is unchanged and remains open here. A real
fix would either (a) add a genuine dry-run branch to `reseat_worktree`, threading `confirm`
through from `run_fix`, mirroring `prune_missing`'s shape, or (b) rename the parameter out
of the shared `confirm` vocabulary so its absence from this mode's contract is structural
rather than a fact a caller must read the docs to learn.

## Tests added
None. This file documents the pre-existing drop and corrects the surfaces that
misrepresented it; no regression test was added because no behavior changed. A future fix
(see Fix above) should add a test asserting `reseat_worktree` either honors `confirm=false`
as a dry run, or that its schema/docs are the load-bearing contract (already covered by this
session's doc-comment edits, not by an executable test).

## Workarounds
Treat every `librarian(action="doctor", fix="reseat_worktree")` call as live and
irreversible-without-a-manual-`graft`, regardless of `confirm`. There is no dry-run preview
for this mode — read the report (`librarian(action="doctor")` with no `fix`) first, and only
call `fix="reseat_worktree"` once you accept its `"reseated"` list will already be applied.

## Resume
Implement Fix option (a) or (b) above in `src/librarian/tools/doctor.rs`: thread `confirm`
into `reseat_worktree`'s signature (currently `src/librarian/tools/doctor.rs:1988-1992`,
`fn reseat_worktree(ctx: &ToolContext, scope: &mut scope::DoctorScope, scope_fallback:
bool) -> Result<Value>`), add a dry-run branch mirroring `prune_missing`'s
(`src/librarian/tools/doctor.rs`, the `"prune_missing"` arm's `if !confirm { … }` shape a
few dozen lines above the `"reseat_worktree"` arm), and add a regression test asserting a
`confirm=false` (or omitted) call reports without mutating the catalog.

## References
- `src/librarian/tools/doctor.rs:1722-1737` (`run_fix`'s `"reseat_worktree"` match arm)
- `src/librarian/tools/doctor.rs:1988-2058` (`reseat_worktree` function and its doc comment)
- `src/librarian/tools/librarian.rs:108` (`fix` param schema description — corrected by I1)
- `src/prompts/guides/librarian.md` (doctor-repairs guide section — corrected by I1)
- `docs/trackers/issue-clusters/IC-15-accepted-parameter-silently-dropped.md` (cluster
  membership)
- Whole-branch review round 2 report, coordinator-directed session, 2026-09-09/2026-09-10
  (I1, I2)
