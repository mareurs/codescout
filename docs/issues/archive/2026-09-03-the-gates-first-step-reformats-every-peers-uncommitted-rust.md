---
id: 863b801018a947e4
kind: bug
status: fixed
title: 'BUG: the mandated gate''s first step is workspace-wide cargo fmt, so following it correctly rewrites every peer''s uncommitted Rust'
owners:
- marius
tags:
- cluster/blast-radius-exceeds-visibility
closed: 2026-09-24
opened: 2026-09-03
severity: medium
---

# BUG: the mandated gate's first step is workspace-wide `cargo fmt`, so following it correctly rewrites every peer's uncommitted Rust

## Summary

`CLAUDE.md` § *Development Commands* requires `cargo fmt` as step 1 of a four-command gate
before completing any task. `cargo fmt` formats the **entire workspace**, not the paths being
committed. On a checkout shared by six sessions, that means a session doing everything right
modifies in-flight files belonging to sessions it cannot see and has no reason to think about.

**Measured twice in one evening, in opposite directions, between two sessions who had each
just written about the hazard.** This is not a carelessness failure — it is a rule that cannot
be complied with without doing it.

## Symptom (Effect)

A peer's uncommitted file acquires hunks its author did not write. Concretely, on
`src/prompts/mod.rs` — a peer's in-flight work — after this session ran the gate:

```
-             A renamed or moved directory silently empties this gate, and both assertions \
-             below are `is_empty()`, which is monotone under removal: an empty corpus \
+                 A renamed or moved directory silently empties this gate, and both assertions \
+                 below are `is_empty()`, which is monotone under removal: an empty corpus \
```

String-continuation re-indentation — rustfmt's, not the author's. The author's own content
additions sit in the same diff, so `git diff` afterwards shows a **mixed** hunk set with no
marker separating the two authors.

## Reproduction

1. Two sessions share a checkout. Session A has uncommitted, unformatted `.rs` edits.
2. Session B, touching entirely different files, follows the gate: `cargo fmt` first.
3. Session A's files are reformatted in place. Neither session is notified.

Reproduced 2026-09-03 in both directions:

| time | who ran `cargo fmt` | whose in-flight files were rewritten |
|---|---|---|
| ~22:1x | this session (`12dee32b`) | peer's `src/prompts/mod.rs` |
| ~23:14 | peer (`ffb95976`) | this session's four files |

## Environment

Linux, `experiments`, six live codescout sessions in one checkout at the time (socket
enumeration at 23:05:20+03:00 and 22:06:39+03:00 — the population turns over roughly five of
six members per hour, so both counts are timestamped rather than treated as stable).

## Root cause

`cargo fmt` has **no file-list form** — it formats every target of every workspace member.
`.pre-commit-config.yaml:144-171` already records this and works around it *for the hook*,
scoping `cargo-fmt` to staged files with `pass_filenames: true` and citing the measurement
(~2000 ms whole-tree vs ~40 ms single-file). That comment even names the concurrency hazard
directly: *"That duration is not a comfort metric. It is the window in which another session's
write lands in pre-commit's after-snapshot and is attributed to this hook."*

**So the hook is already scoped and the human-facing gate is not.** The instruction in
`CLAUDE.md` is a bare `cargo fmt`, and that is the copy an agent follows before committing.
The knowledge exists in the repo; it just is not on the surface anyone reads before running
the command.

inferred from `.pre-commit-config.yaml:144-171` and the two observations above — the
whole-workspace behaviour of `cargo fmt` is documented upstream and was not separately measured
here.

### A second, opposite failure mode: `fmt` doing NOTHING and saying otherwise

Added 2026-09-04. Everything above is `cargo fmt` doing too much — reaching files it was not
asked about. The inverse also happens and is harder to catch: **`cargo fmt` can format a tree
that `pre-commit` then restores out from under it, report success, and have changed nothing.**
Observed by `66523284`: `fmt` succeeded, a later `cargo fmt --check` disagreed with it, and a
double blank line `fmt` had fixed seconds earlier was back. The only reason it surfaced at all
is that a `rustfmt --check` hook refused the next commit.

So step 1 of the gate has two opposite failure modes on a shared checkout, and they want
different remedies. **Scoping `fmt` to changed paths addresses the first and not the second** —
a scoped `fmt` inside a peer's stash window is still a no-op that returns success. The second is
the stash race
(`docs/issues/archive/2026-09-03-pre-commit-stash-window-feeds-peers-wrong-bytes-or-enoent.md`,
symptom 4), and its precondition is checkable in advance: `git status --porcelain` on your
inputs before running the gate.

Worth stating explicitly, because a fix for one reads as a fix for both. A session that scopes
`fmt`, sees success, and commits through a path with no `rustfmt --check` ships an unformatted
tree and never learns — the failure is in the **success return**, which no amount of scoping
repairs.

## Evidence

### The damage is non-destructive but unattributable

rustfmt is idempotent and the gate requires formatted code before any commit, so the *content*
outcome is what the peer's own gate would have produced. The cost is attribution and review:
the peer reads a diff containing hunks they did not write, in a file they are mid-edit on.

### Neither party could tell after the fact

The peer tried to determine whether their `fmt` had touched this session's files and reported:
*"I tried mtimes and they're useless here: all five read 23:19:29, which is my own pre-commit
hook's stash-and-restore cycle rewriting them, not evidence about content."* So the obvious
instrument is destroyed by an adjacent mechanism, and the honest answer was "I can't tell you".

## Hypotheses tried

1. **Hypothesis** — it is enough for each session to be careful. **Test** — two sessions, both
   of whom had written about this hazard *in this session*, did it to each other within an
   hour. **Verdict** — rejected. Care is the wrong instrument (`CLAUDE.md` § *Observer
   Blindness*).

## Fix

**FIXED — both failure modes, by three commits that landed under other bug files.** This file
was never pointed at them, so it stayed `open` after its defect was gone. Closed 2026-09-24
after re-deriving each anchor (all three reachable from `experiments`; patch-ids recomputed,
and the two already recorded elsewhere matched).

- **Mode 1 — `fmt` reaches peers' files.** `2caf55c5` added `scripts/fmt-mine.sh`, which
  formats only what `scripts/file-provenance.py` attributes to the running session and refuses
  the rest (the *scope* direction above, implemented as a mechanism rather than the *check
  first* policy). `02a86104` then made it step 1 of the documented gate, closing the last step
  that left the sibling bug `mitigated`. `scripts/gate.sh` runs it today.
  Tracked in `docs/issues/archive/2026-09-09-the-documented-gates-first-command-rewrites-every-peers-uncommitted-rust.md`.
- **Mode 2 — `fmt` reports success and pre-commit's stash-restore reverts it.** `074b749e`
  removed the stash: the commit-stage checks run through `scripts/pre-commit-run.sh`, and the
  `cargo-fmt` check reads `:<path>` from the index instead of the working tree, so it checks
  the bytes being committed. Tracked in
  `docs/issues/archive/2026-09-03-pre-commit-stash-window-feeds-peers-wrong-bytes-or-enoent.md`.

The two candidate directions originally listed here were: scope the gate's `fmt` to changed
paths, or run `cargo fmt --check` first and proceed only on your own files. The first shipped,
keyed on provenance rather than `git diff`.

## Tests added

None. A test would have to assert about the *instruction text*, not behaviour — and
`src/prompts/mod.rs` already pins the gate sentence byte-for-byte
(`claude_md_gate_lists_its_four_commands_in_the_load_bearing_order`), so any fix here must move
that test with it. That coupling is itself a reason to treat the change as non-trivial.

## Workarounds

Run `cargo fmt --check` first and read which files it names. If they are all yours, run `fmt`.
If they are not, either stage-scope or tell the checkout before you run it.

## Resume

Nothing left. Closed as fixed; see § *Fix* and § *Fix provenance*.

## References

- `.pre-commit-config.yaml:144-171` — the same problem, already solved for the hook, with the
  measurement and an explicit statement of the concurrency window.
- `docs/conventions/shared-checkout-commit-sequence.md` § 4 — amended this session with the
  neighbouring hazard (a pathspec commit takes the *working tree*, so it can carry a
  concurrent session's writes; `docs/trackers/issue-clusters.md` named as the hot file).
- `docs/issues/archive/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md` — adjacent,
  and the mechanism that destroyed the mtime evidence above.
- Class note: filed `IC-1` on the remedy test — the blast radius of a write is wider than the
  set of peers the writer can see, and the remedy is an ownership/scoping protocol over the
  shared resource rather than a provenance channel after the fact (which would be `IC-10`).

## Fix provenance

- **SHA:** `2caf55c5` (on `experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `35f0ccc37590baafad49fcc4c8f5519877b23853` — content hash of the diff; survives rebase and cherry-pick.

`feat(scripts): fmt-mine.sh formats only this session's Rust and refuses the rest` — mode 1's mechanism.

- **SHA:** `02a86104` (on `experiments`)
- **patch-id:** `9660b6ca0767622206ed721bee6d39ac982740fc`

`docs(gate): step 1 is ./scripts/fmt-mine.sh, not bare cargo fmt` — puts mode 1's fix on the path the gate actually follows.

- **SHA:** `074b749e` (on `experiments`)
- **patch-id:** `4c3958557408b19cdf60354a5f8288167e4342e4`

Stops pre-commit from stashing, and makes the `cargo-fmt` check read the index — mode 2.
