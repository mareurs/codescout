---
id: '53ed87262cfbdcbf'
kind: bug
status: open
title: 'BUG: staging a split hunk with update-index costs two refusals and a wrong repair before the recipe is named'
tags:
- cluster/authorship-unrecoverable-after-the-fact
- commit-hooks
- shared-checkout
opened: 2026-10-29
---

## Summary
A session that must commit only its own hunks of a file a peer is also editing can stage them with `git update-index --cacheinfo`. That path costs two refusals and a wrong repair attempt before the recipe that works is named. The recipe is written down (`docs/conventions/shared-checkout-commit-sequence.md`, § *The entangled single file* and § *A hunk the recorder can attribute*). The first refusal does not name it, and no text printed before an attempt says that `update-index` stages an unattributed hunk. The recorder behaviour itself is known and fixed as `43a50b8d8af976be`; this issue is only about how long the route to the recipe is.

## Class fit
The staged hunk is mine, but the stage log stamps it `-`, which reads as foreign to everyone. Authorship of staged content cannot be recovered after the fact, so the guard infers it from the recorder's list of staging commands. That is the trait of `cluster/authorship-unrecoverable-after-the-fact`. It differs from `43a50b8d8af976be` in what is broken: the attribution rule works as designed, and the fault is the route a session takes through the refusals.

## Symptom (Effect)
Measured 2026-10-29, session `cee8fc8d`, committing my F-201 entry from `docs/trackers/bug-fix-session-log.md`. A peer had uncommitted F-198, F-199 and F-200 entries in the same file. In order:

1. Staged a version holding only my hunks: `git update-index --cacheinfo 100644,<blob>,<path>`. `git diff --cached --name-only` showed only my path.
2. `git commit -m ...` was refused: `Refusing a bare commit: the index holds paths staged by another session.` The staged-by row read `(unrecorded)`, and the hint read `git commit -- <your paths>   # <- none of the staged paths look like yours`. The same refusal names `scripts/commit-mine.sh` as the fallback.
3. Ran `git add <file>`, then `git update-index --cacheinfo` back to my blob, hoping to claim the path. The row stayed unattributed.
4. `scripts/commit-mine.sh -m ...` was refused: `leaving staged — UNATTRIBUTED` and `nothing staged is yours`. This is the first refusal that names the convention page and its section.
5. Read the page. Its § *A hunk the recorder can attribute* says a restage of identical content does not repair the row, and gives a scratch-work-tree construction. Step 3 had been the repair it says does not work.
6. Reset the index entry to HEAD's blob with `update-index`, ran `GIT_DIR=... GIT_WORK_TREE=<scratch> git add <path>` of my snapshot, then `scripts/commit-mine.sh -m ...`. That committed `07fece587` with only my hunks. The peer's hunks stayed in the working file.

Two refusals, one wrong repair, then the page. About a dozen tool calls, counted loosely.

## Reproduction
Needs a file a second session also has uncommitted edits in.

1. Build a file holding HEAD plus only your hunks. `git hash-object -w <file>` gives its blob.
2. `git update-index --cacheinfo 100644,<blob>,<path>`.
3. `git commit -m x` is refused as in step 2 above. `scripts/commit-mine.sh -m x` is refused as in step 4.

Result commit: `07fece587`, parent `e98b003a7`.

## Environment
Linux, shared checkout on branch `experiments`, other live sessions holding the same file. Hooks as of 2026-10-29.

## Root cause
- **The recorder claims a `(blob, path)` pair only during `git add`, `git rm --cached` and `git commit`.** `update-index` and `git add -p` stamp `-`. Measured 2026-10-29: after step 1 the bare-commit refusal named the path `(unrecorded)`. Already recorded in `43a50b8d8af976be`.
- **Nothing printed before an attempt warns against `update-index`.** I chose it without reading the page, because the page is not part of the text any hook prints before the first refusal. Measured as the order of events in the six steps above. The page itself notes that its entangled-file case is not in the served tail (`scripts/commit-sequence-tail.txt`), and asks whoever meets the case "a second time" to decide on one line there. I could not establish whether today counts as that second time.
- **Not an error in the page.** Its first construction uses `git add`, which the recorder does claim, so that form is valid. Its cost is that it rewrites the peer's working file for a few seconds, which is why the scratch-work-tree form exists.

## Evidence
- Commits: `07fece587` (the split commit), `e98b003a7`.
- `docs/conventions/shared-checkout-commit-sequence.md`, both sections named above.
- `docs/issues/archive/2026-09-30-commit-mine-leaves-a-coupled-path-behind-when-the-recorder-cannot-attribute-a-hunk-staged-file.md` (`43a50b8d8af976be`).
- The refusal texts of steps 2 and 4, quoted above.

## Hypotheses tried
- Claim the path with a whole-file `git add`, then `update-index` back to my blob: refused, as the page predicts.
- Plain `git commit` of the staged blob: refused by the bare-commit guard.

## Fix
Not started. Options, cheapest first:
1. Add one line to `scripts/commit-sequence-tail.txt`, which every refusing hook prints: a hunk staged by `update-index` or `add -p` is unattributed; stage a split with a real `git add` from a scratch work tree (see the page, § *A hunk the recorder can attribute*).
2. Let the bare-commit refusal for an `(unrecorded)` row name that section, not only `commit-mine.sh`.
3. Add a script (for example `scripts/stage-hunks.sh`) that does the scratch-work-tree add. Needs a decision: the recipe is a short shell snippet, and a script is one more surface to maintain.

`tests/hook_config.rs` has `every_refusing_hook_emits_the_shared_tail`, and a test pins the tail's separate-calls wording. Any change to the tail needs those to pass.

## Tests added
N/A — not started. A change to the tail needs the emitting-hooks test to pass with the new line.

## Workarounds
The scratch-work-tree recipe in the page, § *A hunk the recorder can attribute*. Verified working 2026-10-29 (`07fece587`).

## Resume
Pick option 1 or 2, and decide on option 3. Read the page's paragraph that begins "This case is NOT in the served tail" first. Do not file the recorder behaviour again: it is `43a50b8d8af976be`.

## Related research
N/A

## References
- `43a50b8d8af976be` (fixed, archived): the recorder stamps hunk-staged files `-`.
- `docs/conventions/shared-checkout-commit-sequence.md`.
- `scripts/commit-mine.sh`, `scripts/post-index-change-stage-log.sh`, `scripts/commit-sequence-tail.txt`.
