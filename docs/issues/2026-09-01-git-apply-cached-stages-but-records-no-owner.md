---
kind: bug
status: open
tags:
- cluster/declared-not-wired
closed: null
opened: 2026-09-01
owner: marius
related: []
severity: medium
---

# BUG: `git apply --cached` stages content the recorder can never attribute, so the guard refuses your own commit

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
