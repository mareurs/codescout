---
id: b86ba0380dc36436
kind: bug
status: fixed
title: 'BUG: a peer''s pre-commit stash window makes another session''s atomic file read/rename operate on the wrong bytes — or on no file at all'
owners:
- marius
tags:
- cluster/transient-shared-state-lies-to-readers
closed: 2026-09-07
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


### A fifth symptom: the window INVERTS a peer's measurement, silently and in one direction

Derived 2026-09-06 by sessionId `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`, while another session
in this checkout was running a mutation test. It is a stronger claim than the symptoms above
and belongs beside them rather than inside one of them.

Those are all about an *operation on wrong input* — a read, a rename, a build seeing HEAD bytes
or `ENOENT`. This one is about a **measurement returning a plausible wrong answer**:

1. A session mutates production code to check that a guard is load-bearing. The mutation is
   **unstaged**, which is the normal state of an experiment.
2. Any peer commits — even strictly by pathspec, touching only its own files. `pre-commit`
   stashes *all* unstaged work repo-wide for the duration of its hook run.
3. If the mutating session's `cargo test` build reads the file inside that window, it compiles
   the **unmutated** code.
4. The mutation **survives**. The run reports a green test where a red was expected.

A surviving mutation reads as *"this guard is dead code"*, and the remedy that invites is to
**delete a guard that was fine**. So the failure is not merely a lost result: it manufactures a
confident wrong conclusion, in the direction of removing a working check, and neither party's
output records that the window opened. `pre-commit` prints
`Stashing unstaged files to …` / `Restored changes from …` in the *committer's* terminal — the
one session for whom nothing is wrong.

**The asymmetry is why it stays unnoticed:** only the party running an experiment is harmed,
and only the party committing knows the window happened. Neither can see both halves.

**This is not hypothetical arithmetic — the same run produced a genuine surviving mutation for
an unrelated reason**, and the two are indistinguishable from the output alone. A doctor check's
archive-path filter survived its mutation because the *test fixture* excluded the row by status
before the filter was consulted. That took investigation to separate from "the code is dead",
and a stash-window survivor would have looked identical while being neither.

**Consequence for this checkout:** a mutation window and any peer commit are mutually exclusive,
in both directions, and nothing enforces it — on 2026-09-06 it was arranged by message between
two sessions and held only because both complied. Staging the mutation would protect it (the
stash takes *unstaged* work only), but a staged mutation is one absent-minded `git commit` away
from being committed, which is worse.
### A sixth symptom: it makes a WORKING tool emit a correct-looking refusal, and the debugging goes to the tool

Measured 2026-09-07 by sessionId `8dba66b0-af4b-4cda-a333-54a0605b318e`, with the peer commit
identified by timestamp rather than inferred.

`doc(action="update", patch={body_edits: […]})` refused with
`body_edits[0]: heading '## F-117 — …' not found`, and listed the available headings. The heading
was there — two singleton calls passing the byte-identical string succeeded minutes before and
after. So it read as a librarian defect and was chased as one: a throwaway artifact was created
to reproduce it, four hypotheses were ruled out (batch arity, heading punctuation, heading
length, edit ordering), and a bug file was written against a component that was working
correctly.

The cause: `## F-117` had been appended to `docs/trackers/bug-fix-session-log.md` in that
session and was **uncommitted**. A peer committed `e4a01763` at `08:17:08+03:00` — between that
session's own `491ed828` at `08:14:58` and its next call. For the duration of that hook run the
tracker on disk was HEAD's copy, which does not contain the section.

**Verified at the bytes, not inferred.** `git show 491ed828:docs/trackers/bug-fix-session-log.md`
yields **0** occurrences of `F-117` and **2** of `F-116`. The `F-116` figure is the control: it
shows the file and the counting method are sound, so the `0` is a measurement rather than a
broken grep, and it pins the discriminator as precisely *committed vs not*.

**The tool was right.** Its error was true about the bytes it was handed and false about the
caller's document, and nothing available to it can tell those apart. That is what separates this
from the symptoms above: they describe an operation on wrong input yielding a wrong *result*,
while this yields a **correct refusal** whose only defect is that the reader attributes it to the
refusing component. The refusal even shipped a helpful `Available headings:` list — accurate,
and accurate about the wrong document.

**The tell, and it is cheap and specific:** the target was a heading that session had added and
not yet committed. If a heading-addressed operation fails on a section you created this session
and have not committed, read `git log --format='%h %cI' -3` for a peer commit at that instant
**before** debugging the tool. Anything already in HEAD is immune — which is exactly why this
never reproduces against a fixture, and why the throwaway artifact above cleared four hypotheses
and still missed the cause.

**And the fifth symptom nearly landed in the same session.** Three production-path mutations were
run that evening to confirm a guard was load-bearing, each unstaged for the minute it took to
compile and run — precisely the window § *A fifth symptom* describes. All three produced the
expected red, so nothing was lost. Recorded because the exposure was real and unnoticed at the
time: the mutual exclusion two sessions arranged by message on 2026-09-06 was an agreement
between those two sessions on that evening, and nothing carried it into 2026-09-07.
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

**FIXED 2026-09-07 — `074b749e`, patch-id `4c3958557408b19cdf60354a5f8288167e4342e4`.**

The first direction below — *stop stashing* — is the one that shipped, and it turned out to
cost far less than this section assumed. `pre-commit` 4.6.2 exposes no way to disable the
stash, so the route was to stop using the framework: the four commit-stage checks now run
from `scripts/pre-commit-run.sh` through a direct shim.

**The assumption that made this look expensive was that the checks needed the stash.** Read
rather than inferred, three of the four never did — `unreviewed-content`, `foreign-index` and
`ledger-counts` all read the INDEX (`git rev-parse ":$path"`, `git diff --cached --raw`,
`git show :<path>`). Only `cargo-fmt` read the working tree, because it passed filenames to
`rustfmt`. It was rewritten to read `:<path>` and feed rustfmt on stdin, which is a
correctness improvement in its own right: it now checks the bytes being committed rather than
the bytes on disk.

All six symptoms above are eliminated rather than mitigated — there is no window, because
nothing stashes. *Observed rather than claimed:* the commit that landed this printed no
`[INFO] Stashing unstaged files` line, where every commit earlier that day did, and two peers'
dirty files sat untouched through it.

**Residual:** `pre-commit install` succeeds against any config (measured) and would reinstate
the framework over the shim. The normal install path no longer invokes it;
`install-hooks.sh --check` reports which shim is live.

**NOT ARCHIVED, deliberately.** This file and its sibling
`2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md` are cited **26 times across
16 files**, including `scripts/pre-commit-run.sh`, `scripts/pre-commit-cargo-fmt.sh` and
`tests/hooks-discrimination.sh` — written by this fix and citing this file as their rationale.
Moving it would make every one of those a dead path that `audit_doc_refs` scores `high`.
`status: fixed` already removes it from the open-bug query. Archive both together and
deliberately, if at all.

<details><summary>The directions as filed, when none of them were costed</summary>

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

</details>

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

## Fix provenance

- **SHA:** `074b749e` (on `experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `4c3958557408b19cdf60354a5f8288167e4342e4` — content hash of the diff; survives rebase and cherry-pick.

The same commit closes `2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md` — one
mechanism, two observers, deliberately not folded and not superseded (§ *References*).
