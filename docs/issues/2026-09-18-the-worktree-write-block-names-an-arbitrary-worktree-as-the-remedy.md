---
status: open
opened: 2026-09-18
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/hint-composed-without-the-request
kind: bug
---

# BUG: the worktree write block names an arbitrary worktree as the remedy, and its trigger is now permanent

## Summary

`guard_worktree_write` refuses a write when linked worktrees exist and the session has not
called `activate`. The refusal is **correct**. Its hint is not: it prescribes
`workspace(action='activate', path="<wt_list[0]>")`, where `wt_list[0]` is whatever
`std::fs::read_dir` hands back first — a value with no relationship to the caller, their
cwd, or their request. The main repo, which is the answer in essentially every case, is
offered second and parenthetically.

Two things make this worth filing rather than tolerating. **Following the hint is not
merely wasted** — it activates a foreign worktree as the session's *home* project, so
subsequent writes land in the wrong checkout and mint worktree-scoped catalog rows
(`id = sha256(abs_path)`). And **the trigger condition is now permanent and monotonically
growing**: `scripts/mutation-probe.sh` deliberately keeps one worktree **per session,
forever**, so the guard fires for every session on every `/mcp` reconnect — including the
overwhelming majority who have never worked in a worktree.

## Symptom (Effect)

First `doc()` write of a session, immediately after a `/mcp` reconnect:

```
Write blocked: git worktrees detected but workspace(action='activate') has not been
called. Worktrees: [/home/marius/work/claude/codescout.worktrees/check-codescout-integration,
/home/marius/work/claude/codescout.worktrees/mutation-d52899fd-..., ...12 total]
```

with the hint:

```
Call workspace(action='activate', path="/home/marius/work/claude/codescout.worktrees/check-codescout-integration")
to select the write target (or use "/home/marius/work/claude/codescout" for the main repo).
```

`check-codescout-integration` is an unrelated agent branch. The reader is in the main repo
and has no business in any of the twelve.

## Reproduction

Measured 2026-09-18T07:01–07:04Z at HEAD `44dff9ac`, in the main checkout.

1. `/mcp` (reconnect — spawns a server with no activation).
2. Any `doc()` write. Observed with `doc(action="rekey_prefix")`: refused as above.
3. `workspace(action="activate", path="<main repo>")` → `read_only: false`.
4. Re-run the same call → `doc(action="rekey_prefix") requires 'id', 'from' and 'to':
   missing field 'id'`.

Step 4 is the discriminator: the error moves from the worktree block to a *parameter*
error, so the write path is demonstrably clear. The activation returning `ok` does not
establish that on its own.

## Environment

Linux, `experiments`, main checkout at `/home/marius/work/claude/codescout`, twelve linked
worktrees, six sessions sharing the checkout.

## Root cause

`src/tools/core/guards.rs:61-65` composes the hint:

```rust
let hint = format!(
    "Call workspace(action='activate', path=\"{}\") to select the write target (or use \"{}\" for the main repo).",
    wt_list[0],
    root.display()
);
```

`wt_list` comes from `list_git_worktrees` (`src/util/path_security.rs:540-574`), which
iterates `std::fs::read_dir(.git/worktrees)` and pushes `paths.push(...)` in iteration
order. **There is no sort anywhere**, so `[0]` is filesystem readdir order.

Measured 2026-09-18: `ls -U .git/worktrees` returns
`check-codescout-integration, mutation-d52899fd, mutation-aa272bed, mutation-29420e72, …`
— matching the refusal's order exactly, and differing from both alphabetical
(`check-…, mutation-29420e72, mutation-3aa55c01, …`) and creation order. So the prescribed
path is arbitrary, not "the oldest" or "the first alphabetically".

**The discriminator is in scope and unconsulted.** `root` is bound eight lines above at
`guards.rs:56` and is already interpolated into the same `format!` — as the parenthetical.
The session id is also present in every probe worktree's own path
(`mutation-<sessionId>`), so "is this tree mine?" is answerable from the strings the
function already holds.

**Why the trigger is permanent.** `scripts/mutation-probe.sh` keeps its worktree by
design: `cleanup()` (`:164-167`) restores the mutated file and removes `$BACKUP`,
`$RUNLOG`, `$MARKER` — never the tree — and `:190` explicitly re-points an *existing*
probe worktree at HEAD. The script's own header (`:26-29`) gives the reason: a kept
worktree is 11 s per run against 87 s cold. That is a good decision for the probe. Its
consequence for this guard is that the predicate *"worktrees exist"* has drifted from
*"you might be in a worktree"* to *"anyone has ever run a mutation probe in this
checkout"*, and the two stopped denoting the same thing.

## Evidence

### The hint's own three tests never read it

`src/tools/core/tests.rs` holds exactly three cases for this guard —
`guard_worktree_write_refuses_when_only_resolved_at_startup` (`:871`),
`guard_worktree_write_allows_after_explicit_activate` (`:892`),
`guard_worktree_write_allows_when_no_worktrees_exist` (`:910`). All three assert on
`is_err()` / `is_ok()`. **None makes any assertion about the hint**, so the remedy text is
untested by construction — CLAUDE.md § *Testing Discipline*, "a suite tests a guard's
PREDICATE and never its REMEDY TEXT".

### Population

`git worktree list` → 12 linked worktrees: 11 `mutation-<sessionId>` (one per session that
has run the probe) plus `check-codescout-integration`. None is transient.

## Hypotheses tried

1. **Hypothesis:** this is the already-filed `986e8146fc44d17d` (*the worktree notice
   prescribes two calls a served peer cannot make*). **Test:** read that file.
   **Verdict:** rejected. That one is `worktree_read_notice` on the **read** path, and its
   defect is that a **peer-serve** client is structurally forbidden from either prescribed
   call; `grep -c 'guard_worktree_write\|Write blocked'` over it returns **0**. Different
   composer, different caller class, different failure.
2. **Hypothesis:** covered by
   `docs/issues/archive/2026-08-20-largest-unclassified-error-is-the-worktree-activate-write-block.md`.
   **Verdict:** rejected — that record is about `err_family` taxonomy and merely counts
   this gate as a population.
3. **Hypothesis:** the eleven worktrees are leaks, so the fix is cleanup.
   **Test:** read `cleanup()` and the `MODE = isolated` branch. **Verdict:** rejected —
   retention is deliberate and load-bearing for the probe's runtime.

## Fix

Not applied. The repair belongs in the hint, not the predicate:

- Lead with `root` — the main repo — and offer the worktree list second. `root` is already
  in scope at `guards.rs:56`.
- If a worktree is to be named at all, name the caller's own
  (`mutation-<CLAUDE_CODE_SESSION_ID>`) rather than `[0]`, or name none.
- Mention the `workspace=<abs path>` per-call pin, which `get_guide("workspace-state")`
  prescribes *in preference to* re-activating and which this guard already honours at
  `guards.rs:47`, but which the refusal never mentions.

**Why `IC-22` and not `IC-14`:** the guard's coverage is correct — it refuses exactly the
writes it should. Only the prose is wrong. Tagging a predicate class with a message defect
would corrupt the count its promotion reads; this is the same line the class's
`the-pre-push-remedy-names-a-refspec-a-zero-commit-pusher-cannot-form` member draws.

## Tests added

None — nothing is fixed yet. A regression test here should assert **shape, not prose**:
that the hint names the main repo, in the form that reds on its removal and survives
rewording.

## Workarounds

`workspace(action="activate", path="<main repo>")` once per session after any `/mcp`
reconnect, or pass `workspace="<abs path>"` on the individual call.

## Resume

Rewrite the hint at `src/tools/core/guards.rs:61-65` to lead with `root`, then add a case
to `src/tools/core/tests.rs` beside the three at `:871-921` asserting the hint names the
main repo. Observe it RED against the current string first.

## References

- `src/tools/core/guards.rs:21-71` — the guard and its hint
- `src/util/path_security.rs:540-574` — `list_git_worktrees`, unsorted
- `scripts/mutation-probe.sh` — deliberate per-session worktree retention
- `docs/issues/2026-09-02-the-worktree-notice-prescribes-two-calls-a-served-peer-cannot-make.md`
  — sibling member of the same class, read path
