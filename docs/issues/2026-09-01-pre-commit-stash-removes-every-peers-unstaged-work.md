---
id: '8cc95806a7b5f37a'
kind: bug
status: open
title: 'BUG: every session''s commit removes every other session''s unstaged work from the working tree for the duration of its hooks'
tags:
- cluster/transient-shared-state-lies-to-readers
---

# BUG: every session's commit removes every other session's unstaged work from the working tree for the duration of its hooks

## Summary

`pre-commit` clears all unstaged changes from the working tree before running hooks and
restores them afterwards. On a single-user checkout that is correct and invisible. On this
six-session checkout it means **any** session's commit silently reverts **every** other
session's in-flight edits, on disk, for the length of that commit's hook run — and any
concurrent read in that window (a `cargo` build, a test run, an editor reload,
`git status`) sees a tree that was nobody's intent.

## Symptom (Effect)

Visible on every commit in this repo, and easy to read as routine noise:

```
[WARNING] Unstaged files detected.
[INFO] Stashing unstaged files to /home/marius/.cache/pre-commit/patch1788288371-2136698.
observe the tree during the hook.........................................Passed
[INFO] Restored changes from /home/marius/.cache/pre-commit/patch1788288371-2136698.
```

The two `[INFO]` lines are the whole event. Nothing marks that the files stashed belong to
*other* sessions, and the peer whose work vanished gets no line at all — in their session
there is no output, only a window in which the file on disk is not what they wrote.

## Reproduction

Measured 2026-09-01 in an isolated throwaway repo, deliberately **not** on the shared
checkout. A hook that records what it sees on disk:

```yaml
      - id: observe
        entry: sh -c "cat fileA.txt > observed.txt"
        language: system
        pass_filenames: false
        always_run: true
```

```bash
printf 'COMMITTED\n' > fileA.txt && git add -A && git commit -qm init
printf 'DIRTY-peer-in-flight-edit\n' > fileA.txt      # unstaged, as a peer's WIP would be
pre-commit run
```

Result:

```
what the hook SAW on disk while running:  COMMITTED
what is on disk NOW (after restore):      DIRTY-peer-in-flight-edit
```

The hook read the **index** version off the filesystem while the working tree held the
dirty version. This is an observation of the tree during the window, not a reading of the
documentation.

## Environment

pre-commit 4.6.2 (pipx), git, Linux. `codescout` on `experiments`; six live sessions in
this checkout across three `CLAUDE_CONFIG_DIR` profiles, routinely with two or three
holding unstaged `src/**` edits.

## Root cause

`pre_commit/staged_files_only.py:108`:

```python
def staged_files_only(patch_dir: str) -> Generator[None]:
    """Clear any unstaged changes from the git working directory inside this
    context.
    """
    with _intent_to_add_cleared(), _unstaged_changes_cleared(patch_dir):
        yield
```

The context wraps the whole hook run. It is entered whenever `stash` is true — the normal
commit path (`run.py`; `pre-commit run --all-files` does not stash, which is why the
manual reproduction above needs a bare `pre-commit run`).

The design assumes **one writer**: clearing unstaged changes is how a hook gets to see
exactly what the commit will ship, which is correct and is the same reasoning
`scripts/pre-commit-ledger-counts.py` uses when it reads the index. What does not hold on a
shared checkout is the premise that the unstaged changes being cleared are the committer's.
They are everyone's, and the tree is one.

**Window length is the hook run.** That makes it the same quantity `9e493b20` shortened for
a different reason: whole-tree `cargo fmt --check` held it for ~2000 ms, and per-file
`rustfmt` holds it for ~40 ms. So that commit already cut this exposure by the same factor,
without knowing it — which is worth recording, because it means the remedy direction is
already established rather than hypothetical.

## Evidence

**This was observed live on 2026-08-31, before this file existed, and that observation is stronger than the probe above.** `IC-12`'s entry records it: within a minute of git hooks being enabled here, a session watched its own edited file revert to HEAD content, `git status` report the tree clean, and a `grep` for text it had just written return nothing. It was written up as *"The read-side twin"* inside `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` rather than as its own file, which is why `IC-12` stood at **n=0 tagged** with *"nothing to tag yet"* in its `**Members:**` line. This file is that missing member: the class was not found today, only filed today.

**The sharpest detail belongs to that entry and is not reproduced here:** `git stash list` is EMPTY throughout, because pre-commit writes a patch under `~/.cache/pre-commit` rather than using `git stash`. The instrument a reader would reach for to check confirms the false reading. Read `IC-12` for that argument and for the oracle it prescribes — `artifact_event` byte counts, or `wc -c` against `git show HEAD:<path>`, never `git status`.

What this pass adds is three things the earlier write-up did not have: a **deterministic isolated reproduction** (above, runnable in a throwaway repo without blocking anyone), the **mechanism at its source** (`staged_files_only.py:108`), and the observation that the exposure window is exactly the **hook runtime**, which ties the remedy to a change already made.

Beyond the reproduction: the `[INFO] Stashing unstaged files` line appears in the output of
essentially every commit made in this checkout today, including the four in this session.
At the time of one of them, `git status --short` showed another session's modified
`src/librarian/tools/get.rs`, so that file was reverted and restored across a hook run
belonging to a commit that did not touch it.

`.pre-commit-config.yaml`'s own header already notes the stash in passing —
*"pre-commit stashes all unstaged work once per installed stage"* — as an argument about
where to install hooks. Its consequence for concurrent readers is not drawn there.

## Impact

A peer running `cargo test`, `cargo build`, or any file read during another session's
commit can observe the index version of their own uncommitted work. The failure is
**transient and unreproducible**: by the time anyone looks, the tree is restored and every
diagnostic agrees the files are fine. A build failure from this looks like flakiness, and
`git status` afterwards reports the truth, which is what makes the earlier lie hard to
credit.

Not merely possible: **observed** on 2026-08-31, where the reading session saw its own work absent and `git status` agreed. What has not been observed is a concrete *downstream* failure — a build or test that failed because of it — and that is stated deliberately rather than assumed away. The window is short, and short windows produce rare events, not absent ones.

## Fix

**Not fixed, and probably not fixable in `pre-commit`.** The stash is load-bearing for the
tool's central guarantee. Options, in order of how much they buy:

- **Shorten the window** — already in progress and already measured: `9e493b20` cut the
  dominant commit-stage hook from ~2000 ms to ~40 ms. This does not close the class; it
  reduces the probability.
- **Per-session worktrees** (`git worktree`), which is the only remedy that actually
  removes the shared resource rather than narrowing exposure to it. Cost is real: the
  catalog is keyed by `abs_path`, so worktrees have their own reconciliation story — see
  memory `worktree-merge-catalog-reconciliation`.
- **Not a candidate:** asking sessions not to commit while others hold unstaged work. That
  is a coordination rule addressed to a party with no way to know.

## Tests added

None. A regression test would have to assert on a race window inside another process's
hook run; the isolated probe above is reproducible and is the artefact to re-run instead.

## Workarounds

None that are free. A session doing something timing-sensitive with uncommitted work can
stage it (staged content is not stashed), which is also the practice that
`docs/issues/2026-09-01-an-unstaged-pre-commit-config-blocks-every-session.md` recommends
for a different reason.

## Resume

Decide whether this stays a known-and-accepted property or motivates per-session
worktrees. If it stays: add one line to `.pre-commit-config.yaml`'s header drawing the
consequence for concurrent readers, so the next person who meets an unreproducible build
failure has somewhere to land. That is a knowledge fix and this ledger's own standard says
so — see `IC-12`'s `Mechanism status`.

## References

- `IC-12` (`cluster/transient-shared-state-lies-to-readers`) in
  `docs/trackers/issue-clusters.md` — this is the class's first tagged member; it had stood
  at n=0 *on evidence*, after an archive pass that looked and found nothing transient.
- `docs/issues/2026-09-01-an-unstaged-pre-commit-config-blocks-every-session.md` — the
  other shared-state defect in the same tool, found in the same pass.
- `OB-10` in `docs/trackers/observer-blindness.md` — the class covering resources whose
  holder gets no signal.
- `9e493b20` — shortened this window from ~2000 ms to ~40 ms for an unrelated reason.
