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


### 2026-09-02 — the first observed downstream failure, and it named the wrong component

Sequence, in one session (`ffb95976`), during a mutation check on
`src/librarian/tools/context.rs`:

1. `edit_file` changed an **unstaged** line from `1,` to `2,` — **ok**.
2. `cargo test` ran and failed in the way `2,` predicts, proving the working tree held `2,`.
3. `edit_file` was asked to change `2,` to `51,` and **refused**:

```
old_string not found in src/librarian/tools/context.rs.
Nearest content at lines 689-690:
                1,
                51,
```

4. `read_file` over the same range, seconds later, returned `2,`.

Steps 3 and 4 contradict each other, and step 3 is not a hedge — it **quotes** the content it
claims to have found, at a line number.

**Ruling out the obvious explanation is what made this expensive.** `edit_file` reads its
target through `read_edit_target` (`src/tools/edit_file/mod.rs:678`), a bare
`std::fs::read_to_string` with no cache, and `not_found_msg` is handed *that same string*, so
the display and the matcher cannot disagree. A two-edit scratch-file probe did not reproduce
it either. Both findings point away from the tool and toward the disk — but the reading they
invite is *"a cache that only affects indexed source files"*, and a bug filed on that premise
would have sent the next reader hunting for something that does not exist.

**What actually happened, confirmed at the byte level rather than inferred.** Peer commit
`c22fb929` landed at 21:04:14 and its hook run stashed the working tree. That patch is still
on disk and contains the mutation:

```console
$ grep -n -E '^[-+] +(1|2|51),$' ~/.cache/pre-commit/patch1788372254-1160295
16248:-                1,
16249:+                2,
```

So for the duration of that hook run the file on disk carried `1,` — exactly what `edit_file`
read and reported. The restore is why `read_file` saw `2,` moments later. The commit that
emptied the file touched nothing under `librarian/`.

**A durable AFTER-THE-FACT oracle, which this file did not previously name.** Every remedy
listed above and in `IC-12` — `artifact_event` byte counts, `wc -c` against
`git show HEAD:<path>` — must be run *during* the window, which is the one thing a surprised
reader cannot do. But pre-commit writes its stash to `~/.cache/pre-commit/patch<epoch>-<pid>`
and **never deletes it**, so the window is reconstructible afterwards from three commands that
need no foresight:

```bash
ls -la --time-style=+%H:%M:%S ~/.cache/pre-commit/ | grep patch  # every window, timestamped
grep -c '<your path>'        ~/.cache/pre-commit/patch<epoch>-<pid>   # was your file stashed?
grep -n  '<the line you wrote>' ~/.cache/pre-commit/patch<epoch>-<pid>
```

**And `git log` is not a complete index of the windows.** Two of the four stash events
bracketing this session's mutation run (`20:57:31`, `21:01:28`) correspond to **no commit at
all** — an aborted or rejected hook run stashes just the same. The patch directory is the
complete record; the commit log is a subset of it.
### 2026-09-04 — a REFUSED commit opens the same window, and the commit log cannot see it

Two stash windows from one session, 39 seconds apart, **the first from a commit the gate
refused**:

```
23:47:49   attempt 1 — pre-commit stashes, `ledger-counts` REFUSES, hooks roll back,
           stash restored. Nothing committed. No commit object exists.
23:48:28   attempt 2 — retry as a pathspec commit; stashes again, succeeds (fe6364bc).
```

A peer's `doc(action="update")` calls at ~23:47 had written 295 lines to a bug file, unstaged.
Their `doc(action="move")` ran inside a window and `fs::rename` correctly moved the 279-line
**HEAD** version it found. The archive looked perfect: complete, well-formed, plausible —
just the wrong bytes.

**Four consequences, and the third is the one that changes how this is investigated.**

1. **The stash precedes any hook verdict**, so a commit the gate BLOCKS costs a peer exactly
   what a successful one does. Every hook in this repo's chain — `ledger-counts`, the
   staged-path checks, `rustfmt` — refuses *after* the tree has already been emptied.
2. **A refusal invites an immediate retry**, so the natural response to being blocked opens a
   second window seconds later. The committer experiences one failure and one fix; the peer
   is exposed twice.
3. **A commit-log scan is a LOWER BOUND on stash windows, and it under-counts non-randomly.**
   Refused commits leave no object, so any attempt to correlate a transient failure with
   "who committed when" misses them entirely — and misses them precisely in the sessions
   hitting gates, i.e. the ones doing ledger, coupling or archive work, which is the same
   population most likely to be mid-edit on a shared file. Both parties in the 2026-09-04
   instance reconstructed the window from the git log and both saw one commit where there
   were two.
4. **Atomicity is no defence.** `fs::rename` is atomic and still got the wrong file, because
   the wrongness is in the *tree*, not in the operation. Any "use an atomic write" mitigation
   is answering a different question.

**Instrument, and it sees what the git log cannot.** `pre-commit` names its stash file
`~/.cache/pre-commit/patch<epoch>-<pid>`, so the filename carries the window's start time to
the second — for refused commits too. `date -d @<epoch>` over that directory reconstructs
every window a machine has opened recently, including the ones that produced no commit. That
is how the 23:47:49 window above was found, after the git log had already been read and had
shown only 23:48:28.

**Misattribution cost, and it was paid.** The peer filed this instance as a **high-severity
bug against `doc(action="move")`** — a stale-snapshot-plus-missed-unlink defect — and
broadcast it to five sessions before reading `src/librarian/tools/mv.rs` and retracting
within the hour. `move` is `std::fs::rename` and can do neither thing. Three independent
checks came back clean while the wrong hypothesis was live: two archive moves verified at the
bytes, and a disposable probe that wrote a body *through the catalog*
(`doc(action="update", patch={body_edits})`) before moving it, specifically to test the
"serialises a catalog-held body" theory. All three were correct — because none of them ran
inside a stash window. **A defect that only manifests during another process's 2-second
window will pass every deliberate probe**, which is what makes the transience in § *Impact*
worse than "hard to reproduce": it actively produces exculpatory evidence for the wrong
component.

## Impact

A peer running `cargo test`, `cargo build`, or any file read during another session's
commit can observe the index version of their own uncommitted work. The failure is
**transient and unreproducible**: by the time anyone looks, the tree is restored and every
diagnostic agrees the files are fine. A build failure from this looks like flakiness, and
`git status` afterwards reports the truth, which is what makes the earlier lie hard to
credit.

**Observed 2026-09-02** — see the Evidence subsection below. The realised failure was a
**refused tool call whose error message quoted the stashed content as fact, at a line
number**. That is worse than the build failure anticipated here: a build failure is a
*symptom*, whereas this is a confident, specific, wrong answer about a file's contents — and
it implicates the component that *read* the file rather than the one that emptied it. The
session came within one call of filing a bug against `edit_file`, and the two diagnostics it
ran (no cache in `read_edit_target`; a scratch-file probe that did not reproduce) both pointed
away from the real cause while looking like progress.

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

**And the mitigation must be on the VICTIM's side, not the committer's** — which is not
obvious, and the obvious alternative does not work. "Don't commit while a peer is mid-edit"
fails on two counts established 2026-09-04: the stash happens before any hook verdict, so a
*refused* commit exposes peers identically, and no committer can see who is mid-edit anyway.
Staging is the only lever, and only the session holding the uncommitted work can pull it.

## Resume

Decide whether this stays a known-and-accepted property or motivates per-session
worktrees. If it stays: add one line to `.pre-commit-config.yaml`'s header drawing the
consequence for concurrent readers, **and name `~/.cache/pre-commit/patch<epoch>-<pid>` as the
after-the-fact oracle** (Evidence, 2026-09-02) — that is the one instrument a surprised reader
can still use, because it survives the window that everything else requires you to be inside
of. That is a knowledge fix and this ledger's own standard says
so — see `IC-12`'s `Mechanism status`.

## References

- `docs/issues/2026-09-03-pre-commit-stash-window-feeds-peers-wrong-bytes-or-enoent.md` —
  **the other half of this same mechanism, and deliberately a separate file.** This record
  covers the session whose work *vanishes*; that one covers the session that *reads or writes
  the wrong bytes* during the window — HEAD content served to a concurrent reader, `ENOENT` on
  a tracked file, the window opening on **failed** commits too, and a `cargo fmt` that
  reported success and changed nothing because the tree was restored underneath it. One
  mechanism, two observers, noticed and reproduced differently. Not folded and not superseded:
  a `supersedes` edge would flip one of them to `superseded` and hide it from the default
  query while both halves are still open. The shared `cluster/` tag is what makes them one
  query rather than one file.
- `IC-12` (`cluster/transient-shared-state-lies-to-readers`) in
  `docs/trackers/issue-clusters.md` — this is the class's first tagged member; it had stood
  at n=0 *on evidence*, after an archive pass that looked and found nothing transient.
- `docs/issues/2026-09-01-an-unstaged-pre-commit-config-blocks-every-session.md` — the
  other shared-state defect in the same tool, found in the same pass.
- `OB-10` in `docs/trackers/observer-blindness.md` — the class covering resources whose
  holder gets no signal.
- `9e493b20` — shortened this window from ~2000 ms to ~40 ms for an unrelated reason.
