---
id: 132d17cd509eafa8
kind: bug
status: fixed
title: 'BUG: the foreign-index guard''s "re-stage by explicit path" remedy is a no-op when the blob is already staged'
tags:
- cluster/hint-composed-without-the-request
closed: 2026-10-01
opened: 2026-09-24
owner: marius
related: []
severity: low
---

## Summary

When the foreign-index pre-commit guard refuses because a path's stage-log row is
`unnamed` (a "blanket add"), its remedy text says *"Re-stage by explicit path and the bare
commit passes."* That is false whenever the content is already staged — the common case,
since the refused session just staged it. `git add -- <path>` of an identical blob does not
change the index, the stage-log records nothing new, the `unnamed` row stands, and the next
commit is refused again with the same text. What works is unstaging first
(`git reset -q -- <paths>`) and then adding by explicit path, which the text never says.

## Symptom (Effect)

Measured 2026-09-24 on a 169-path docs commit: staged with `git add --pathspec-from-file=<f>`
(recorded as `-` / `unnamed` — the recorder does not read the file), refused; re-staged with
every path named on the command line, refused identically, the stage log still holding
`-\t<blob>\t<path>\tunnamed` for every row; `git reset -q --pathspec-from-file=<f>` then the same
named `git add` produced `09093108…\t<blob>\t<path>\tnamed` rows and the bare commit passed
(`cb54d062`). Three commit attempts where the guard's own instruction promised one.

## Reproduction

1. Stage a path so the recorder marks it `unnamed` (e.g. `git add --pathspec-from-file=f`).
2. `git commit` → refused, remedy "Re-stage by explicit path".
3. `git add -- <the same path>` (content unchanged) → `grep <path> .git/session-stage-log` still
   shows only the `unnamed` row.
4. `git commit` → refused again.

**Reproduced 2026-10-01** in a throwaway repository with this repository's own hooks. A directory
add logged `-  <blob>  sub/b.txt  unnamed` and the guard exited 1; a plain re-add left the log
byte-identical and the guard still exited 1; `git reset -q -- sub/b.txt` then `git add --
sub/b.txt` turned the row `named` (owner the stager). The guard kept refusing until *every* refused
path had been re-staged that way. `--pathspec-from-file` was recorded as `unnamed` as well.

## Root cause

The recorder attributes staging from index CHANGES (keyed on `(blob, path)`), and an identical
re-add is not a change. The remedy text is written as if any explicit `git add` re-records.
Separately, the "blanket add" cause line lists `-A`, `-u`, `.` and directories but not
`--pathspec-from-file`, so a reader cannot tell from the text that their form was the cause.

## Fix

Chosen 2026-10-01: change the text, not the recorder. The remedy now says why a plain re-add fails
(it records nothing), the step that works (`git reset -q -- <path>`, index only, then `git add --
<path>`), and limits that step to paths the reader wrote. Making a re-add of an already-staged blob
record a claim would have been a recorder change that attributes by stager, which the 2026-09-07
bug already names as the defect; the text keeps that decision with the reader.

**The restriction is the design point, not a caveat.** The old sentence was a no-op, which made it
accidentally safe. An effective remedy that named no condition would let a reader re-claim a path a
peer blanket-staged, which is the capture the guard exists to stop. Unsure whose a path is: the
text sends the reader to the pathspec commit the refusal already offers. The cause line also names
`--pathspec-from-file`.

The same defect was recorded independently as F-4 of the embedder-stack-ops session log
(2026-09-15, in a private worktree with no peer), whose candidate wording branched on shared versus
private index. One rule covers both: own paths only, otherwise pathspec.

## Tests added

In `tests/hooks-discrimination.sh`, after the `pre-staged` / `unnamed` split cases: the premise,
so the text can change back only if the behaviour does (a plain re-add leaves the row unowned and
the guard refusing); the step that works (reset then add claims it, and the bare commit passes);
three needles on the text (why a re-add fails, the own-paths limit, the old promise gone); and
`--pathspec-from-file` recorded as `unnamed` and named in the cause.

Red before the change (4 failures), 172 passed and 0 failed after. The first draft of the
"unstage first" needle was vacuous: another sentence in the same refusal warns about `git reset`, so
it passed against the unfixed text. It was replaced with phrases only the new remedy contains.
Mutated one per clause on the final bytes, each killed by its own assertion (171/1): the mechanism
sentence, the own-paths clause, the false promise reintroduced, and `--pathspec-from-file` dropped.
The suite runs in CI as its own job; the local four-command gate does not run it.

## Fix provenance

- **SHA:** `32ced364f1509e4b22063223059fb96c96509221` (`experiments`)
- **patch-id:** `1b7e4b083b6e093e1530621781159791e25f3490` (`git show <sha> | git patch-id --stable`)

Verified on `experiments` 2026-10-01 by running `tests/hooks-discrimination.sh` directly (172
passed, 0 failed) and the four mutations above. No Rust changed, so the four-command gate was not
run and its result would say nothing about this fix.

## References

- `docs/conventions/shared-checkout-commit-sequence.md` — the sequence the refusal prints
