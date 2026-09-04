---
id: b86ba0380dc36436
kind: bug
status: open
title: 'BUG: a peer''s pre-commit stash window makes another session''s atomic file read/rename operate on the wrong bytes — or on no file at all'
owners:
- marius
tags:
- cluster/transient-shared-state-lies-to-readers
opened: 2026-09-03
severity: high
---

# BUG: a peer's pre-commit stash window makes another session's atomic file read/rename operate on the wrong bytes — or on no file at all

> **This file was filed 2026-09-03 with the wrong cause and broadcast to five sessions before
> being retracted the same hour.** It originally claimed `doc(action="move")` wrote a stale
> snapshot and failed to delete its source. **That is false.** `src/librarian/tools/mv.rs:75`
> is a bare `std::fs::rename(&old_full, &new_full)?` — one atomic syscall, no copy, no unlink —
> which cannot produce a stale copy and cannot leave a source behind. The observations were
> real; the mechanism was invented. The original slug and its `IC-8` tag are withdrawn.

## Summary

`pre-commit` stashes **all unstaged work in the repository** before running any hook, and
restores it afterwards. On a shared checkout that opens a window, several times an hour, in
which the working tree does not hold what its owner put there. Any *other* session reading or
moving a file during that window silently operates on **HEAD bytes**, or on **no file at all**.

The operation need not be racy in itself. `fs::rename` is atomic and still moved the wrong
file: atomicity guarantees the operation is indivisible, never that its *input* is the one the
caller meant.

## Symptom (Effect)

Three distinct symptoms from one evening, each invisible to the party that caused it.

**1. An archive move captures HEAD content.** `doc(action="move")` after two
`doc(action="update")` calls left two files where a rename was expected:

```
M  docs/issues/<name>.md              295 lines — working content
A  docs/issues/archive/<name>.md      279 lines — byte-identical to HEAD (52682ddd)
```

The destination is a plausible, complete, well-formed bug file. Nothing about it looks wrong.

**2. A tracked source file reports ABSENT to the compiler**, observed by session `66523284`:

```
error[E0583]: file not found for module `reindex`
  = help: to create the module `reindex`, create file "src/librarian/tools/reindex.rs"
```

The file was present throughout — 73,951 bytes, `M` in git, their uncommitted work intact.
Re-running clippy immediately: exit 0. **This is wider than reversion**: a tracked file
reverted to HEAD would compile as an older file, not vanish. For some part of the window the
path does not exist at all, and anything walking the tree sees a hole.

**3. A frontmatter field written seconds earlier is missing after an archive move**, observed
by session `9f839a8e` — same shape, an `edit_file(frontmatter=…)` write sitting unstaged when
the rename ran.

**4. A WRITE reports success and is silently undone** — reported by `66523284`, and the worst
of the four. `cargo fmt` ran, reported success, and formatted a tree that `pre-commit` then
restored out from under it; a later `cargo fmt --check` disagreed with the `fmt` that had just
"succeeded", leaving behind a double blank line `fmt` had fixed seconds earlier.

**Nothing failed.** The first three symptoms are a bad *read* — you get wrong bytes, or an
error. This one is a bad *write*: the tool's own success return is the lie. A session that
formats, sees success, and commits through a path without a `rustfmt --check` hook — or with
`--no-verify` — ships an unformatted tree and never learns. It also means **running the gate's
`cargo fmt` inside someone else's stash window is a silent no-op**, which is a second and
quieter failure mode of
`docs/issues/2026-09-03-the-gates-first-step-reformats-every-peers-uncommitted-rust.md`: that
file records `fmt` doing too much to other people, this records it doing nothing at all and
saying otherwise.

**Direct confirmation that a REFUSED commit stashes**, same session, from the hook's own output
rather than decoded from patch epochs:

```
rustfmt --check (staged files)...........Failed
[INFO] Restored changes from /home/marius/.cache/pre-commit/patch1788470003-1659508
```

Three independent routes to that conclusion now — `2cb44cd3`'s two-window epoch decode,
`66523284`'s refusal above, and this session's own `ledger-counts` refusal. The hook says it out
loud, which makes `66523284`'s the cheapest to cite. They then took the predicted retry
(`cargo fmt`, re-stage, commit again), opening window two voluntarily — the mitigation failing
exactly as predicted, by the party who had just read the prediction.

## Reproduction

Not deterministic — it is a race, and reproducing it means winning one:

1. Session A leaves unstaged modifications to any file.
2. Session B commits **anything**, anywhere in the repo.
3. Session A reads, moves, compiles or hashes one of those files during the hook run.
4. A gets HEAD bytes, or `ENOENT`.

**A FAILED commit opens the window identically, and this is what breaks the obvious
mitigation.** `pre-commit` stashes *before* any hook verdict, so a refused commit costs peers
exactly what a successful one does — and a refusal invites an immediate retry, so the natural
response to being blocked opens a second window seconds later. Confirmed in this session's own
transcript, on a `ledger-counts` refusal:

```
[INFO] Stashing unstaged files to /home/marius/.cache/pre-commit/patch1788460537-3900173.
refuse a stored count, or a class gaining a member it does not name…Failed
[INFO] Restored changes from /home/marius/.cache/pre-commit/patch1788460537-3900173.
```

Session `2cb44cd3` decoded two windows 39 seconds apart from the patch-file epochs — 23:47:49
(refused, rolled back, **no commit object exists**) and 23:48:28 (`fe6364bc`, succeeded).
**A commit-log scan is therefore a lower bound on stash windows**, because refused commits
leave nothing to scan.

## Environment

Linux, `experiments`, six live codescout sessions in one checkout, `pre-commit` framework.

## Root cause

`pre-commit`'s stash-unstaged-then-restore cycle is repository-wide and has no notion of other
processes. It is correct in intent — hooks should see the index, not the working tree — and the
intent does not survive a shared checkout.

**Exposure bound, contributed by `ffb95976`, and much narrower than "any read":** the stash
touches only **unstaged** work. A file with no unstaged modifications is byte-identical to HEAD
inside the window and outside it, so it cannot be affected. The exposed population is exactly
*files carrying unstaged modifications, read by another session, during someone else's commit*
— and it is checkable in advance: `git status --porcelain` on your inputs before a long read
tells you whether you are exposed at all. Symptom 2 shows the bound is on *unstaged-ness*, not
on tracked-ness: a tracked file with unstaged edits is squarely in scope.

measured 2026-09-03 23:47–23:49: byte-identity of the captured destination against
`git show 52682ddd:<path>` (279 lines, `status: open`, `Still owed`, no `closed:`), and
`fe6364bc`'s committer timestamp `23:48:28+03:00` with `Session-Id` trailer `2cb44cd3` falling
inside the window.

## Evidence

### `move` is innocent, established two independent ways

- **By reading:** `src/librarian/tools/mv.rs:75`, `std::fs::rename`.
- **By behaviour**, run by `2cb44cd3` as a disposable probe *specifically to test the
  catalog-body hypothesis this file originally asserted*: create an artifact, rewrite its body
  through `doc(action="update", patch={body_edits})` so the change goes via the catalog, then
  move it. Destination carried the post-update body; source deleted. Three correct moves
  against the one bad one, one exercising the exact implicated path.

### The misleading symptom is the expensive part

*"file not found for module X"* reads as *"you deleted a source file"*. The natural response is
to hunt for what you broke, or to restore from git — **which would destroy the reader's own
uncommitted work in response to a phantom.** The standard diagnostic confirms the lie: reading
the file again, at that instant, agrees that it is gone.

### A withdrawn contrast case

`66523284`'s three clean archive moves were offered as a contrast supporting a timing/cache
hypothesis. They are consistent with the false mechanism *and* with the true one, so they
discriminated nothing — those moves simply did not coincide with a peer commit. Recorded as
withdrawn rather than deleted: a corpus that keeps only the evidence which survived teaches
nothing about how the wrong call gets made.

## Hypotheses tried

1. **Hypothesis** — `move` serialises a catalog-held body instead of re-reading `abs_path`.
   **Test** — read `mv.rs`; then `2cb44cd3`'s behavioural probe. **Verdict** — **rejected**,
   twice, independently. This was the filed claim, and it was broadcast to five sessions before
   either test was run. The check skipped was one function body.
2. **Hypothesis** — a longer gap between update and move lets a cache go stale. **Test** —
   there is no cache. **Verdict** — rejected in mechanism, *correct in correlate*: a longer gap
   is more time for a peer's commit to land inside it.
3. **Hypothesis** — only non-atomic operations are exposed. **Test** — `fs::rename` is atomic
   and was hit. **Verdict** — rejected; atomicity constrains the operation, not its input.

## Fix

Not fixed. Directions, none free:

- **Stop stashing.** `pre-commit`'s reason for the stash is that hooks should see the index.
  Several hooks here already read the index directly (`ledger-counts` via `git show :<path>`),
  so the stash may be buying less than it costs on this repo.
- **Advertise the window.** A marker file peers can check, converting a silent hazard into a
  precondition of the same shape as `git status --porcelain`.
- **Do not** derive a rule from the retracted mechanism. "Staleness before deletion" was
  proposed while the false cause stood, sounded right, and is meaningless under the true one.
  A plausible rule resting on a falsified model is worse than no rule — it survives on
  plausibility and quietly certifies the wrong picture.

## Tests added

None. A regression test would have to win a race deliberately.

## Workarounds

`git status --porcelain <your inputs>` before a long read or a move. Clean ⇒ not exposed.
Dirty ⇒ you are in the population, and a peer commit — **including one that fails** — can
change what you read. After any move, diff both paths before staging; a non-`R` `git status`
is a failed move, not a staging mistake.

## Resume

**Slug corrected 2026-09-04.** This file was opened as
`artifact-move-writes-a-stale-snapshot-and-leaves-the-source`, a claim retracted in
§ *Evidence* the same night: `move` is innocent, `src/librarian/tools/mv.rs:75` is a plain
`fs::rename`, and the retraction was broadcast to five peers. The title had already been
corrected; the filename had not, and a slug is what a reader sees first in a directory
listing. Renamed to match the title, deliberately deferred until the checkout was quiet —
renaming means a `doc(action="move")`, and a move during a peer commit is the very race
this file documents. Done at 03:11 with four peers idle.

**Fold question, resolved: keep both, cross-linked — do NOT supersede.** The earlier
Resume offered *"fold into `2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md`
or supersede it"*. Neither, on inspection:

- A `supersedes` edge is side-effectful — it flips the destination to `superseded`, which
  **hides it from the default query**. The 2026-09-01 record holds the *loss* half with its
  own evidence, and that half is still open and still unfixed. Hiding it to tidy a slug
  would trade a naming defect for a discoverability one, which is the worse of the two.
- Folding would merge two victim classes into one record. One mechanism, yes — `pre-commit`
  stashing all unstaged work repo-wide — but two distinct observers: the session whose work
  **vanishes** (2026-09-01) and the session that **reads or writes the wrong bytes** during
  the window (here). They are noticed differently, reproduced differently, and read
  differently.

Both carry `cluster/transient-shared-state-lies-to-readers`, which is the thing that makes
them one query rather than one file — exactly what the cluster tag is for. Cross-links now
run both ways: this file's § *References* already cited 2026-09-01, and 2026-09-01 now cites
this one.

**Still open, and this is the actual work.** No fix is proposed for the mechanism itself.
The four symptoms are recorded in § *Symptom*; the remedy space is in § *Fix* and none of it
has been attempted. The cheapest partial mitigation observed in practice is **staging**:
`pre-commit` stashes *unstaged* changes, so a staged file is untouched by the window — used
deliberately at 02:0x this session to protect in-flight work through a peer's edit cycle
without committing behind a gate that could not run. That is a workaround for one session,
not a fix for the tree.
## References

- `docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md` — same
  mechanism, loss half, already open.
- `src/librarian/tools/mv.rs:75` — the `fs::rename` that exonerates `move`.
- Class note: retagged to `IC-12` (`transient-shared-state-lies-to-readers`) from a withdrawn
  `IC-8`. `IC-8` was reasoned entirely from `moved: true` being a false record, and it was a
  **true** one — the move happened, correctly, on the bytes present. `IC-12`'s claim fits
  exactly, including its clause that *the standard diagnostic confirms the lie*.
- Contributions: `2cb44cd3` (two-window decode, behavioural probe), `66523284` (the `E0583`
  observation and the tracked-file-reports-absent widening), `ffb95976` (the unstaged-only
  exposure bound), `9f839a8e` (third symptom).
