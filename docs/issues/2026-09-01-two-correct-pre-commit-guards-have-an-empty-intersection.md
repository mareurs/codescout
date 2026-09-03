---
id: '1efc6488cb2b8946'
kind: bug
status: open
opened: 2026-09-01
closed:
severity: high
owner: marius
related: []
title: Two correct pre-commit guards have an empty intersection on an entangled index
owners:
- marius
tags:
- cluster/shared-resource-carries-no-owner
topic: shared-checkout commit coordination
---

# BUG: two correct pre-commit guards have an empty intersection on an entangled index

## Summary

`pre-commit-foreign-index.sh` accepts **only** a pathspec commit when the shared index
holds another session's paths. `pre-commit-ledger-counts.py` accepts only a commit whose
index carries every ledger count **together with** its member files — which, once two
sessions' coupled changes are entangled in one index, is only the **bare** commit. Neither
guard is wrong. Their intersection is empty, and a session that satisfies one is refused by
the other with no path between them that does not involve a human choreography or
`--no-verify`.


**This file was itself blocked by the defect it describes, and that is the worked example.**
Filing it means adding an 18th member to `IC-17` and bumping that cluster's count — and on
2026-09-01 the count lived in `docs/trackers/issue-clusters.md` while a peer was mid-edit in it,
and the run's rulings lived in `docs/trackers/sdd-ruling-log.md` while a *different* peer was
mid-edit in that. Two files, two owners, neither splittable by pathspec. Cost: two cross-session
round-trips (`codescout-3e` landed `5e5383e3`, `codescout-8a` landed `c00e383b`) before a single
line of this could be staged. Nothing here was hard; the coupling was.

**And the coordination that resolved it is the evidence for the root cause, not a workaround
around it.** Both owners were enumerable, both were reachable, both answered within minutes.
Enumeration was never the binding constraint — which is `IC-17`'s discriminator (*would a
complete, correct peer listing have prevented this?* — **no**) holding on the very file that
records it.
## Symptom (Effect)

Every proper subset of the entangled set is refused; only the whole set passes. Measured by
simulation against the real hooks (`GIT_INDEX_FILE` + `git read-tree HEAD`), session
`codescout-3e`, 2026-09-01, with a HEAD-only control row:

```
b7 pair (bug file + ledger)          FAIL  IC-17 17-vs-16, IC-12 1-vs-0
b7 bug file only                     FAIL  IC-3  22-vs-23
3e pair (2 bug files + ledger + OB)  FAIL  IC-3  23-vs-22
3e bug files only, no ledger         FAIL  IC-17 16-vs-17, IC-12 0-vs-1
ALL FIVE                             PASS
HEAD only (control)                  PASS
```

The observable cost, same day, same checkout: **nine cross-session messages, two windows
where the shared working tree was red, one refused commit, and roughly two hours of two
sessions' wall time** to land five files that no single session could commit.

## Reproduction

Two sessions on one checkout, both with a coupled pair staged — a `cluster/<slug>` count in
`docs/trackers/issue-clusters.md` and the bug file it counts.

1. Session A stages its bug file and bumps its cluster's count.
2. Session B stages its bug file and bumps its cluster's count.
3. Either session runs `git commit -- <its own paths>` → `ledger-counts` refuses: the
   pathspec index carries A's count and HEAD's member set for B's cluster.
4. Either session runs `git commit` (bare) → `foreign-index` refuses: the shared index
   holds the other's paths.

There is no third form.

## Environment

Linux, git 2.x. codescout `experiments`. Any checkout shared by two or more Claude Code
sessions; this machine routinely runs six in this tree.

**Corrected 2026-09-02 (not by this file's author):** this block previously read
`core.hooksPath` → `scripts/`. That is false — `core.hooksPath` is **unset** here
(`git config --show-origin --get-all core.hooksPath` → exit 1, every scope), which is this
repo's *healthy* state and is asserted by
`tests/hook_config.rs::a_set_core_hookspath_must_point_at_a_directory_that_exists`; a *set*
value silently disabled every hook here for a day
(`docs/issues/archive/2026-08-30-core-hookspath-points-at-pre-rename-path.md`). Hooks reach
`scripts/` via pre-commit's generated `.git/hooks/pre-commit` plus `language: system`
entries in `.pre-commit-config.yaml`. Recorded rather than silently amended because the
false line had already been copied into a second file and a commit message before anyone
checked it — an `## Environment` block reads as background, so nothing treats it as the
claim it is.

**Second correction, same day, same source — when a hook edit actually reaches anyone.**
pre-commit clears unstaged changes before running hooks (`staged_files_only.py`; it is the
root cause of `8cc95806a7b5f37a`, filed here the day before), so a `language: system` entry
executes the **committed** copy of its own script rather than the one in your editor.
Reproduced independently in a throwaway repo before acceptance, three cases:

```
hook-script edit UNSTAGED             -> ran the committed copy; the edit is inert
hook-script edit STAGED               -> ran the edit
further UNSTAGED edit over committed  -> ran the committed copy again
```

So **the moment a hook edit goes live is `git add`, not the editor save** — and that has two
consequences for this file. *For testing:* a proposed fix to either guard cannot be validated by
editing it and committing something, because the edit does not execute until it is staged; §
*Resume*'s probe has to be run with the change staged. *For the class this bug is filed under:*
that same `git add` makes the new rules live **for every other session on the checkout at
once**, with no signal to any of them — a session midway through the commit sequence can have
the guards change under it, authored by a peer it cannot see. That is
`cluster/shared-resource-carries-no-owner` holding about the **guards themselves**, not only
about the index they read.

*Reported by `codescout-8a` and reproduced here rather than accepted. The mechanism is also
visible in this session's own commit output without any probe: every commit made against a
dirty tree printed `Stashing unstaged files to …` and `Restored changes from …`, and the one
made against a clean tree printed neither.*

## Root cause

Two guards read the **same** discriminator in **opposite** directions, and nothing composes
them.

- `scripts/pre-commit-foreign-index.sh:95-98` exits 0 when
  `${GIT_INDEX_FILE##*/}` matches `next-index-*` — the temporary index git builds for a
  pathspec commit. So with a foreign path staged, the pathspec form is the **only** accepted
  one. Its own header (`:91-94`) states this is *"the same discriminator
  scripts/pre-commit-unreviewed-content.sh uses, read in the opposite direction."*
- `scripts/pre-commit-ledger-counts.py:12` reads *"the INDEX and nothing else — `git
  ls-files` for the population, `git show :<path>`"* for content, and `main()`
  (`:314-413`) has **no pathspec exemption at all**. Under a pathspec commit the active
  index is HEAD plus the named paths, so any count whose member falls outside the pathspec
  is compared against HEAD's corpus and mismatches.

*Read in the source 2026-09-01 — the two exemption paths and their absence — not inferred
from the simulation above, which was run by another session and whose staged state no
longer exists to re-run.*

*Every line number in this section was re-checked against `HEAD` on 2026-09-02 and four of
five had drifted — `foreign-index` by one, `main()` from `:290-340` to `:314-413` — shifted by
`3bf2f5f5`, a commit by this file's own author, made after this section was written. The
conclusions did not move; only the coordinates did. Re-derive them before quoting rather than
trusting the pair, and note that `git status` reported worktree == index for all three scripts
at that check, so the copy read is the copy that executes (see § Environment).*

The deeper cause is `IC-17`'s missing field: **the git index records what changed and never
who changed it.** Both guards are workarounds for that absence, and they work around it with
opposite assumptions about which commit form is safe. Give the index an owner and the
tension dissolves — `foreign-index` could accept a bare commit restricted to your own
staged subset, which is exactly the form `ledger-counts` wants.

## Evidence

`foreign-index`'s pathspec exemption, `scripts/pre-commit-foreign-index.sh:95-98`:

```sh
idx="${GIT_INDEX_FILE:-}"
case "${idx##*/}" in
    next-index-*) exit 0 ;;
esac
```

`ledger-counts`' corpus source, `scripts/pre-commit-ledger-counts.py:12`:

```
This reads the INDEX and nothing else -- `git ls-files` for the population, `git show :<path>`
```

`main()`'s only early exits are `--fixture-*` (stdin-pure test modes) and *"ledger neither
staged nor on disk"*. No branch consults `GIT_INDEX_FILE`.

### Fresh instance 2026-09-03 — and it sharpens the claim in two places

sessionId `12dee32b-d0db-40c0-b79a-770720fb3eab`. A docs-only pathspec commit of 8 paths (6
class files under `docs/trackers/issue-clusters/`, the trunk ledger, one new bug file) was
refused by `ledger-counts` naming **a file outside the pathspec**: a peer's
`classify-conflates-two-malformed-reasons-under-one-message`, staged `A` and tagged
`cluster/unclassified` with no matching `**Members:**` append. `refuse an index commit
carrying another session's staged paths` **Passed** in the same run — both guards behaving
exactly as designed, in opposite directions, on one commit.

**1. "No ordering of correct steps escapes it" is right, but "no move" is the wrong word, and
the difference is load-bearing.** The hook's own text names a legal move — leave your bug file
unstaged, since `git ls-files` is the population. That move exists only when the unpaired file
is **yours**. When it is a peer's you cannot take it, because step 6 of
`docs/conventions/shared-checkout-commit-sequence.md` forbids the repair. So the precise claim
is that the set of **permitted** moves is empty, not the set of moves. Worth the extra word:
`--no-verify` *is* a move, the config argues one line above that *"a hook that fires on every
commit to say nothing is how `--no-verify` gets learned"*, and describing the state as "no move
available" is exactly the framing under which a blocked session reaches for it — in the
entangled case, when the guard is most load-bearing. **The exit is social, not procedural.**
This one cleared only when another session acted: the owner was identified positively
(`pre-commit-foreign-index`'s stager field, the session registry, and `Session-Id` trailers on
the `result-cap-marker-gate` branch — three instruments, three different sources, all naming
`c95ba99b`), asked, and committed their pair at `964df77e`. The blocked commit then passed as
`20083067` with nothing about it changed.

**2. The asymmetry, observed live rather than reasoned about.** A concurrent peer
(`66523284`) committing `docs/trackers/prompt-hamsa-audit-log.md` against the *same broken
index* saw `(no files to check) Skipped` and committed straight through, then reported
"pathspec commit clears it" as a general remedy. It does not: `files:` decides whether
`ledger-counts` **runs**, matched against the committer's own paths, and once it runs the
pathspec is invisible to it. Two sessions, one index, opposite outcomes, neither reading
wrong. **So the population that meets this bug is exactly the sessions doing bug-file or
ledger work** — a session committing Rust never sees it, which is why it stays open.

**3. Why publishing the correction did not work, which is the transferable part.**
`.pre-commit-config.yaml:122-135` already states this, in terms, and records that *"two
sessions misread this the same way within an hour"*. Two more of us misread it identically
anyway, so "state it in the config" is **falsified** here rather than unlucky. The reason is
read-surface, not diligence: the warning lives in the hook's **config**, the misreading happens
at the hook's **output**, and `(no files to check) Skipped` is pre-commit's generic string — not
the hook's — so the one surface a committer actually reads is the one surface that cannot carry
the correction. That is `CLAUDE.md` § *Observer Blindness*'s own shape: a bound published to
the enforcement layer, whose audience never reads it. A concrete remedy that informs without
widening the refusal (which the config rightly argues against): `always_run: true` plus an
explicit in-scope / out-of-scope line the hook **prints itself**.

## Hypotheses tried

1. **Hypothesis:** one guard is simply wrong and should be relaxed.
   **Test:** read both guards' stated rationale and the defect each was built against.
   **Verdict:** rejected. `foreign-index` exists because `1b40dabd` committed a peer's entire
   `OB-6` entry under another session's message; `ledger-counts` exists because a count
   without its member is a silently-wrong ledger. Relaxing either restores a live defect.

2. **Hypothesis:** a consent channel — one guard accepts an asserted "the other session
   agreed."
   **Test:** reasoned against the guard's purpose.
   **Verdict:** rejected, and it should stay rejected. A guard that reads an asserted consent
   is a guard you can talk out of refusing.

3. **Hypothesis:** hand choreography is a sufficient answer.
   **Test:** performed once, 2026-09-01 — five steps, no `--no-verify`: b7 reverted its count
   bump → 3e committed `1fc91b93` → b7 restored and committed `39f64a5b`.
   **Verdict:** deferred. It works and it is not a fix: it costs two hours, requires both
   sessions live and responsive, and leaves the tree red mid-sequence for every other
   session on the checkout.

## Fix

Not designed. Three directions, none costed:

- **An owner field on the index** — the root remedy `IC-17` names. `foreign-index` already
  resolves a staged path to a `Session-Id` via `$GIT_DIR/session-stage-log`, so the data
  exists; what is missing is a commit form that commits *your* subset of the shared index
  without a pathspec.
- **Teach `ledger-counts` the pathspec index.** When `GIT_INDEX_FILE` is `next-index-*`,
  evaluate counts against `HEAD ∪ pathspec ∪ the shared index`, so a coupled pair split
  across two sessions is not read as a broken count. Narrower, and it weakens the guard in
  exactly the case a genuine miscount looks the same.
- **Per-session worktrees**, which dissolve this and most of `IC-17` — and which
  `pre-commit-unreviewed-content.sh`'s own header already names as the only complete answer.


### Direction 3 is FALSIFIED — per-session worktrees do not dissolve this (2026-09-02)

Reported by `codescout-8a`, who hit the same empty intersection **inside a private linked
worktree** (`.worktrees/audit-shards-t7`) during a `git rebase` — the one configuration this file
named as the escape.

1. The rebase stopped mid-replay on a transient `index.lock`. The staged content was commit
   `79a03aab`'s own diff, replayed by their own rebase. **A linked worktree's index is
   per-worktree**, so no peer could have staged into it.
2. `git commit -C 79a03aab` was refused by `pre-commit-foreign-index.sh` as *"paths staged by
   another session"*, attributed to `(unrecorded)`.
3. The hook's own prescribed remedy is **structurally unavailable** there:
   `fatal: cannot do a partial commit during a cherry-pick.`

**The discriminator is the defect, and it sits one layer above where this bug was filed.** The
guard keys on *"is this staging CLAIMED?"* rather than *"is this index SHARED?"* — and `rebase`
stages through plumbing that writes no `session-stage-log` entry, so a session's **own** paths read
as foreign. That is `IC-2` (a gate keyed on an event it cannot observe substitutes a proxy) rather
than `IC-17`, and it means isolating the resource does not help while the proxy stays wrong.

**Both of the guards they proposed were then MEASURED AND REJECTED, and what shipped is
neither.** Superseded by `docs/issues/archive/2026-09-02-foreign-index-prescribes-a-remedy-git-refuses.md`
(`d5af3d3ceff1d08c`), fixed at `74b9cc67`, patch-id
`0e7feedf232c5ed9e22fd975c6fe36baa109e1d2`. Read that file rather than the two bullets this
paragraph used to carry:

- *exit 0 in any linked worktree* (`--git-dir` ≠ `--git-common-dir`) proves **linked
  worktree**, not **unshared index**. Two sessions in the *same* worktree share its index
  exactly as two share the main checkout's, and the guard already handles that correctly —
  `session-stage-log` lives at `$git_dir/…`, which is per-worktree. It would have disarmed
  the guard where it currently works, on a machine running 16 sessions with worktrees as
  the isolation mechanism. **Rejected.**
- *exit 0 while a rebase or cherry-pick is in progress* is **wider than the defect**.
  Measured 2026-09-02 in a throwaway repo: a rebase stopped with `rebase-merge/` present and
  `CHERRY_PICK_HEAD` absent commits by pathspec fine, so the prescribed remedy still works
  there and the guard must not stand down. **Rejected.**

What shipped is the narrower condition — stand down **only** when `CHERRY_PICK_HEAD` or
`MERGE_HEAD` exists, which is exactly the set where git refuses `git commit -- <path>`. Both
the main checkout and shared worktrees stay fully guarded, and the `no sequencer -> still
refuses` control in `tests/hooks-discrimination.sh` § 7 is the single case that fails if this
is ever widened to an unconditional exit.

**They used `--no-verify` for that one commit and disclosed it**, having identified the content
*positively* first rather than by elimination: `git patch-id --stable` of the staged diff was
`09a31bb672d83b6c03c1694c23bc5802f73baa31`, byte-identical to `79a03aab`'s own. That is the right
shape for the one case where the guard is provably wrong, and it is worth separating from the habit
the guard's own text warns about — the disclosure and the positive identification are what make it
different.

**What this changes here:** § *Fix* direction 3 (*"per-session worktrees, which dissolve this"*) is
wrong as written and must not be costed as an escape. Directions 1 and 2 stand, and both remain
open — **this bug is NOT closed by that fix.** The entangled-ledger deadlock this file documents is
untouched: `74b9cc67` only covers the sequencer-stop route, where git itself refuses the remedy. A
fourth direction is now **shipped** for that route — fix the discriminator, asking a question git
can answer instead of one it cannot.
## Tests added

None. This is a report, not a fix. The simulation harness that produced the six-row table
(`GIT_INDEX_FILE` + `git read-tree HEAD`, with a HEAD-only control) is the right shape for a
regression test and was not retained.

## Workarounds

The five-step hand choreography under *Hypotheses tried* #3. Never `--no-verify`: both
guards are protecting a real defect, and the entangled case is exactly when they are load-bearing.

## Resume

Decide between the three fix directions before building. The cheapest probe: check whether
`ledger-counts` can read the shared index *in addition to* the pathspec index under
`GIT_INDEX_FILE=next-index-*` — `git --git-dir=... show :<path>` against the default index
path — and whether that reintroduces the miscount it was built to catch. If it does, the
owner-field direction is the only one left and this becomes a design task, not a hook patch.

## References

- `docs/issues/archive/2026-09-02-foreign-index-prescribes-a-remedy-git-refuses.md`
  (`d5af3d3ceff1d08c`) — the sequencer-stop route out of this deadlock, and the only one
  fixed so far (`74b9cc67`). It carries the reproductions that rejected both of the
  originally-proposed guards.
- `docs/plans/archive/2026-09-01-shared-checkout-commit-sequence-guide.md` — the friction this bug
  is the guard-side half of; § *Where a two-author commit is genuinely unrepresentable*
  carries the simulation.
- `docs/trackers/issue-clusters.md` § `IC-17` — the class; its *Mechanism status* line
  already names the git index as unowned.
- `docs/issues/2026-09-01-an-unstaged-pre-commit-config-blocks-every-session.md` and
  `docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md` — sibling
  hook defects on the same shared surface.
- `docs/trackers/response-envelope-session-log.md` — `F-2`, `F-3`, `F-5`.
- Simulation method and the six-row table: session `codescout-3e`, 2026-09-01.
