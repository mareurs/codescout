---
kind: bug
status: fixed
tags:
- cluster/declared-not-wired
closed: null
opened: 2026-09-01
owner: marius
related: []
severity: medium
---

# BUG: `git apply --cached` stages content the recorder can never attribute, so the guard refuses your own commit

**Fixed** at `92dfa4e4`, patch-id `53cfd23a6ce03bc2f4d9fb38c11b6fc1607edaf7`.
Verified on `experiments`: gate green (fmt clean, clippy 0, lean 3413/0, default 5022/0),
`tests/hooks-discrimination.sh` 32 → 41, and the filed reproduction re-run returns the session
id where it returned `-`.

> **`stash` is named in *Root cause* as sharing the grammar mismatch, and it is — but it has no
> observable consequence, so it is deliberately not fixed.** Probed 2026-09-01 rather than
> reasoned about: `git stash push -- s.txt` and bare `git stash push` each produce **no log rows
> at all**, because stash clears the index, `git diff --cached --raw` comes back empty, and the
> log is rewritten with nothing in it. There is no (blob, path) pair to claim or mis-claim. The
> verb's presence in `staging_op()`'s list is therefore inert rather than wrong. Recorded because
> the root cause reads as naming two defects and only one of them can be observed — the same
> loudness question this fix asked of its own deleted guards, turned on the bug file.

## Summary

`scripts/post-index-change-stage-log.sh` lists `apply` among the seven verbs that may claim a
staged pair. It can never do so. `argv_paths()` returns the non-flag tokens after the verb,
and for `git apply` that token is the **patch file**, never the path being staged — so
`names_path()` cannot match, every `git apply --cached` degrades to owner `-`, and
`scripts/pre-commit-foreign-index.sh` then refuses the stager's own commit, naming an owner
who does not exist.

The verb being present in the list is what makes this invisible: the handling reads as
shipped.

## Symptom (Effect)

```
git apply --cached mine.patch     # stages 2 of 6 hunks — succeeds silently
git commit -F msg.txt

refuse an index commit carrying another session's staged paths.......Failed
  theirs:
      docs/trackers/bug-fix-session-log.md
Staged by:
      (unrecorded) — staged before this guard was installed, so no
          session claimed it.
```

`.git/session-stage-log` after the apply:

```
-	9a594237	docs/trackers/bug-fix-session-log.md
```

The `-` is correct behaviour for an unrecognised write and wrong here: the write *was*
recognised as a staging op, and this session *was* the stager.

## Reproduction

Any single-session checkout, no peers required. **Run 2026-09-01 in a throwaway repo with
the hook copied in, and it carries its own positive control** — the same session, same repo,
same hook, one verb apart:

```
git init repro && cp scripts/post-index-change-stage-log.sh .git/hooks/post-index-change
printf 'a\nb\n' > f.txt && git add f.txt && git commit -qm base
printf 'a\nX\nb\n' > f.txt && git diff f.txt > p.patch && git checkout -- f.txt
export CLAUDE_CODE_SESSION_ID=REPRO-SESSION-ID

git apply --cached p.patch
  cat .git/session-stage-log  ->  -                 62c7bd5  f.txt

git reset -q -- f.txt; printf 'a\nY\nb\n' > g.txt; git add g.txt      # control
  cat .git/session-stage-log  ->  REPRO-SESSION-ID  bedc259  g.txt
```

The control matters: without it, a `-` is equally consistent with the hook not firing, the
env var not reaching it, or the log not being written at all. One line apart, `add` records
the id and `apply` does not, which leaves only the argv grammar.

## Environment

Linux, git 2.x, branch `experiments`, hooks installed via `scripts/install-hooks.sh`.

## Root cause

Two functions in `scripts/post-index-change-stage-log.sh` disagree about what `apply`'s
argv means, and only one of them was updated for it.

- `staging_op()` (`:149`) matches `add | rm | mv | restore | apply | update-index | stash`
  and returns 0 — so the write is eligible to claim.
- `argv_paths()` (`:189`) matches the same seven verbs, then prints every subsequent
  non-flag token as a candidate **repo-relative path**. That is true of `add`, `rm`, `mv`,
  `restore` and `update-index`. It is false of `apply`, whose positional argument is a
  patch file, and false of `stash`, whose positionals are subcommands.

`names_path()` is strict by design — its own comment says a miss records `-` and
over-refuses, which is recoverable, while a false hit claims a peer's file silently, which
is not. That asymmetry is right and is why this fails in the safe direction. The defect is
not the strictness; it is that `apply` was added to a list whose contract it does not meet.

**This is `IC-3`'s "a matcher that can never match" family**, at n=6 before this. Every
piece is individually correct — the verb list, the extractor, the matcher — and no input
connects them for this verb. A unit test constructs `git add <path>`, the shape that works.

## Evidence

Measured 2026-09-01 on the W-91 promotion. The tracker
`docs/trackers/bug-fix-session-log.md` carried six diff hunks: two mine, four a peer's
uncommitted `F-91`/`W-93`. `git apply --cached` of my two staged 35+/7- with zero peer
content in the staged diff — verified by `git diff --cached | grep -c "F-91\|W-93"` → `0`.
The commit was then refused anyway.

**The cost is specific and worth naming: it disables the only remedy for the problem the
guard exists to solve.** A pathspec commit records the **working tree**, so on a file
holding both sessions' edits it captures the peer's — the exact defect filed at
`docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`. Splitting
the file is the only correct move, `git apply --cached` is the only tool that splits it,
and the recorder cannot attribute it. So the guard refuses the one route that respects it.

**Workaround used, and it is legitimate rather than a bypass.** Re-stage the identical blob
under a verb whose argv does name the path:

```
sha=$(git rev-parse :<path>)
git reset -q -- <path>          # drops the (blob,path) pair so a fresh one is created
git update-index --cacheinfo 100644 $sha <path>
```

The row then reads this session's id, which is **true** — this session did stage that blob.
Note the `reset` is load-bearing: rows are keyed on the (blob, path) pair and a re-write of
the same content "introduces no new pair and reassigns nothing", so re-staging without the
reset leaves the `-` in place. Note also that `git reset -- <path>` is safe *only* after
confirming the staged diff holds nothing of a peer's; the guard's own text warns that a
reset on a shared index can take a peer's work out seconds before they commit it.

## Hypotheses tried

1. **Hypothesis:** the hook did not fire for `git apply --cached`.
   **Test:** read `staging_op()` — `apply` is in the match list; a row for the path was
   written, with owner `-`. Firing is not the problem.
   **Verdict:** rejected.
2. **Hypothesis:** `CLAUDE_CODE_SESSION_ID` was absent in the hook's environment.
   **Test:** the same session's `git add`-staged rows carry the id.
   **Verdict:** rejected.
3. **Hypothesis:** `argv_paths()` returns the patch file, which cannot match the staged path.
   **Test:** re-staged the identical blob with `git update-index --cacheinfo 100644 <sha>
   <path>` — argv now names the path — and the row flipped from `-` to the session id with
   no other change.
   **Verdict:** **confirmed.**

## Fix

Options, unranked — each needs the reproduction run first.

- **Derive the paths from the index diff, not from argv, for verbs whose positionals are not
  pathspecs.** `git diff --cached --name-only` against the pre-write index names exactly
  what changed. Strictly more accurate than argv for every verb, and it removes the argv
  grammar from the trust boundary. Cost: the hook would need the previous index state,
  which `post-index-change` does not hand it.
- **Drop `apply` and `stash` from `staging_op()`'s list.** Honest and one line: both stay
  unattributed, but the *reason* stops being a silent grammar mismatch and the lists agree.
  Does not fix the workflow — it documents the hole rather than closing it.
- **Special-case `apply`**: when the verb is `apply`, take the paths from the patch file's
  own `+++ b/<path>` lines. Cheap, exact, and reads the artifact git is about to apply.
  Watch `-p<n>` and `--directory`.

Deliberately not proposed: relaxing `names_path()`. Its strictness is the thing standing
between this guard and the silent false-claim failure it was built after.

## Resolution — 2026-09-01

Option 3 (read the patch's own headers). **Option 1's stated cost was wrong, and running the
reproduction is what showed it.** That option reads *"derive the paths from the index diff, not
from argv … cost: the hook would need the previous index state"* — but the hook **already**
iterates `git diff --cached --raw` (`:256`), and the failing row carried the correct path
(`-\t62c7bd5\tf.txt`) all along. argv was never the path *source*. It is the **restriction**: the
set this invocation intended to stage, which is what stops a session's `git add a.txt` claiming a
peer's already-staged `b.txt` sitting in the same `--raw` output.

So Option 1 is not blocked on plumbing — it is unsafe, because dropping the restriction re-opens
the false-claim failure `names_path()` exists to prevent. Right conclusion, wrong reason, and the
wrong reason would have justified the change had the plumbing turned out to be cheap. Option 3 is
correct for the reason Option 1 is not: a patch's `+++ b/<p>` / `--- a/<p>` headers are the same
**kind** of thing argv is for `add`.

### The fix

A `patch_paths()` helper reading default-prefix headers only, and `argv_paths()` routing `apply`'s
positionals through it. Strict by the same asymmetry as `names_path()`: a `--no-prefix` patch
emits nothing and degrades to `-`. Loose matching would let a stray `--- ` line inside a patch's
own **content** — a diff of a diff, which this repo produces — name a path the invocation never
touched.

### Two mechanisms were written, then deleted for failing the loudness law

The first version also carried a bail on `-p<n>` / `--directory`, plus output buffering to let
that bail fire when the flag followed the patch file. **Mutation testing killed zero tests for
either**, and no reachable caller could be named:

- a `--no-prefix` patch fails the `a/` match and emits nothing already;
- where a prefixed patch under an odd `-p` could reach `names_path()`, the existing-owner lookup
  (`:295`) resolves the row first;
- the buffering existed only to serve the bail.

Both deleted rather than given tests to justify them — *a guard nothing reaches is decoration
however loudly it is written*. The reasoning is recorded at the site so the absence reads as a
decision.

**One mutant survives and is annotated rather than tested.** Deleting `sub(/^[ab]\//, "", p)`
kills nothing, because `names_path()` suffix-matches and `a/f.txt` still matches target `f.txt`.
No killing case was constructible. It stays as a **contract** line, not a guard: `names_path`
documents its input as a repo-relative path, and if it ever tightens to exact matching — the
direction its own comment leans — the line becomes load-bearing with no test to notice its
removal. Said at the site, so the green tick is not read as coverage.

### Tests — `tests/hooks-discrimination.sh` § *apply --cached claims what the PATCH names*

32 → 41 cases. Nine, covering the claim (single, new-file `--- /dev/null`, deletion
`+++ /dev/null`, multi-file), the restriction that must survive it (a co-staged path the patch
never named stays with its stager), and two over-refusals (`-p0`, stdin).

**One fixture was broken and produced a false RED that outlived the fix.** `git diff --cached
doomed.txt` after `git rm` is ambiguous — the path is gone from the working tree — so git errors,
the patch is empty, `apply` fails with *"No valid patches in input"*, and the assertion fails
against a case that never ran. It reads exactly like a defect in `patch_paths`. The `--` is now
annotated on the fixture line with what breaks without it.

Mutation-verified per site: matching nothing kills 6, loosening the header match kills 1, printing
the filename again kills 6, not recording the verb kills 6.

### Verification

The filed reproduction, re-run against the fix — same repo shape, same blob:

```
apply --cached  ->  REPRO-SESSION-ID   62c7bd5   f.txt      (was `-`)
control (add)   ->  REPRO-SESSION-ID   bedc259   g.txt
```

## Tests added

None yet. `tests/hooks-discrimination.sh` is the natural home — it already runs a
discrimination matrix over these hooks, and the reproduction above is a case for it. Note
what the case must assert: **the row carries the session id**, not merely that the commit
succeeds. A commit-succeeds assertion is monotone under "the guard was removed".

## Workarounds

The `update-index` re-stage above. It is three commands and it records the truth.

## Resume

Run the reproduction, then pick between the three Fix options. If the patch-file-parsing
option wins, check `-p<n>`/`--directory` handling before writing the extractor — a wrong
path there is a **false hit**, which is the failure direction `names_path()` exists to
prevent.

## References

- `scripts/post-index-change-stage-log.sh` — `staging_op()`, `argv_paths()`, `names_path()`.
- `scripts/pre-commit-foreign-index.sh` — the refusing half.
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — the
  defect the split-staging remedy exists for.
- `docs/trackers/issue-clusters.md` — `IC-3`, "a matcher that can never match" family.
