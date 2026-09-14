---
kind: bug
status: fixed
tags:
- cluster/gate-keyed-on-unobservable-event
- git-hooks
- shared-checkout
closed: 2026-09-14
opened: 2026-09-02
owner: marius
related: []
severity: high
---

# BUG: a pathspec commit DOES capture staged content, and both guards stand down on the premise that it cannot

## Summary

`scripts/pre-commit-foreign-index.sh:95-97` exits 0 on every pathspec commit, on
the stated premise that such a commit *"IGNORES the shared index entirely, so it
cannot capture staged content and needs no guard."* **The premise is false.** A
pathspec commit ignores the index only for paths it does **not** name; for a path
it *does* name, it commits that path's working-tree content and consumes the
peer's staged entry. The sibling guard passes too, because the captured content
*was* staged — which is the condition it checks for.

## Symptom (Effect)

Reported by session `c95ba99b` after `cffc3cf2` (a pathspec commit,
`Session-Id: ffb95976`) took three staged citation re-points in
`docs/trackers/bug-fix-session-log.md`. Their first commit attempt was refused by
`unreviewed-content` naming that file; by the time they looked, the file had left
`git status` entirely. Nothing was lost — they verified all three against
`git show HEAD:` rather than assuming — and a coupling was broken for ~90 seconds:
`HEAD` cited `docs/issues/archive/…` for six files still at their pre-move paths.

## Reproduction

Verified 2026-09-02 in a throwaway repo — **measured, not reasoned**:

```bash
git init -q .; echo base > peer.txt; echo base > mine.txt
git add -A && git commit -qm init
echo "PEER STAGED WORK" > peer.txt && git add peer.txt   # the peer stages
echo "my work" > mine.txt                                # I edit only mine
git commit -qm "pathspec" -- mine.txt peer.txt           # …and name both
```

Result:

```
 mine.txt | 2 +-
 peer.txt | 2 +-
git show HEAD:peer.txt  →  PEER STAGED WORK
git status --short peer.txt  →  (empty)
```

Their content is at `HEAD` under my message, and their index entry is gone.

## Root cause

**Two guards, one shared false premise, and the commit falls between them.**

1. `scripts/pre-commit-foreign-index.sh:99-102` reads `GIT_INDEX_FILE`, and exits 0
   when the basename matches `next-index-*` — the temp index git builds for a
   partial commit. The comment above it states the premise verbatim. That is a
   **proxy** for "cannot capture staged content", and the proxy is wrong in exactly
   one direction: a named path.
2. `scripts/pre-commit-unreviewed-content.sh` does cover pathspec commits — but it
   refuses on **unstaged** content. The captured content was *staged*, so it passes.
   Its own header already lists *"any capture in an ordinary index commit, where the
   content was staged and is presumed reviewed"* among the cases it misses; this is
   the same blind spot reached by the other route.

So the guard positioned to catch a foreign-staged path declines to look, and the
guard that looks is asking a different question. Neither errs; both pass.

## Hypotheses tried

1. **Hypothesis:** the capture came from a bare `git commit` taking the whole index,
   the `21258b4b` shape. **Verdict: rejected.** `cffc3cf2` names two paths, and the
   repro above shows the pathspec form is sufficient on its own.
2. **Hypothesis:** `foreign-index`'s stand-down is the documented sequencer
   stand-down (`CHERRY_PICK_HEAD`/`MERGE_HEAD`, `:105-120`). **Verdict: rejected** —
   that arm is separate and correctly scoped; this exit is the unconditional
   `next-index-*` one above it.

## Fix

**Fixed** — `93b30111` on `experiments`, patch-id `17e3f8ea697573b65e8c4b9f00d0abff064ccebd`.
Change lives in `scripts/pre-commit-foreign-index.sh` (the `next-index-*` block and the
refusal message) and `tests/hooks-discrimination.sh` § 1.

**The plan above overestimated the work, and the reason is worth keeping.** It proposed
intersecting the commit's named paths with stage-log rows keyed by staged blob. No such
intersection was needed: **the temp index already contains exactly the named paths**
(verified — with `P.txt` and `Q.txt` both staged, `git commit -- Q.txt` yields a temp index
whose `git diff --cached --name-only` is `Q.txt` alone), and the guard's existing loop
already reads `git diff --cached --raw`, which honours `GIT_INDEX_FILE`, and already keys
the lookup by `(blob, path)`. So the plan's *"query by blob, never by path"* was already
satisfied by shipped code. The fix DELETED a stand-down rather than adding a check.

**What did need building was the message, and it was not in the plan at all.** The bare
form's remedy is *"commit by pathspec"*. Making the guard fire on pathspec commits without
touching the text would have printed that to someone whose pathspec commit had just been
refused — routing them back into the failure, which is how `--no-verify` gets taught. The
`next-index-*` discriminator is therefore **kept, with its meaning inverted**: it now
selects the remedy branch instead of standing the guard down. The pathspec branch says the
contested path cannot be narrowed away, offers the remaining paths if any, routes to the
owner, and warns against `git checkout` / `git stash` on that path — their work is in the
working tree and uncommitted, so discarding it is worse than the mislabelling.

The comment at `:95-97` is replaced: the false premise is quoted, marked false, and the
measurement that refutes it recorded inline.
## Tests added

`tests/hooks-discrimination.sh` § 1 — five cases, four loud and one silent:

- `pathspec capturing a peer's path -> refuse`
- `pathspec refusal names the captured path`
- `pathspec refusal does not prescribe the refused form` — asserts the REMEDY, not the
  predicate; no assertion about *who* is refused would have caught the wrong text
- `pathspec refusal warns against discarding their work`
- `pathspec naming only my own path -> silent` — the discrimination, without which the four
  above pass against a guard that refuses unconditionally

**A pre-existing test was renamed, and that is itself part of the fix.** `pathspec commit ->
silent` pointed `GIT_INDEX_FILE` at a **nonexistent** file, so it exercised an empty index
and passed identically before and after this change. Its name read as a general claim about
pathspec commits and was cited as one. Now `empty pathspec index -> silent`, annotated as
inert so nobody credits it with coverage it does not provide.

**RED observed by mutating the production path**, in a copied tree so the shared checkout
was never mutated: restoring `exit 0` reds 4 of the 5 new assertions plus one pre-existing
one. Both silent controls stay GREEN under that mutation — which is what shows the new
cases discriminate rather than merely track the change.

Gate: fmt 0, clippy 0, lean 0, default 0, read from the markers rather than the run's exit
code; 9777 passed, 92 ignored. `hooks-discrimination`: 101 passed, 0 failed.
## Workarounds

Before a pathspec commit, check that no named path is staged by someone else:

```bash
for p in <your paths>; do
  b=$(git ls-files -s -- "$p" | awk '{print substr($2,1,8)}')
  grep -F "	$b	" "$(git rev-parse --git-dir)/session-stage-log" | tail -1 | cut -f1
done
```

Any id that is not yours means that path's staged content is not yours to commit.

## Resume

N/A — fixed and archived.

One thing deliberately NOT done, so nobody reads its absence as an oversight: the sibling
`pre-commit-unreviewed-content.sh` was left alone. It covers the INTRA-path axis (the
working tree moving under a pathspec commit after you staged) and that axis was never
broken. § 4 of the suite states the division; this fix restores the CROSS-path axis only.
## References

- `scripts/pre-commit-foreign-index.sh:95-102` — the premise and the exit
- `scripts/pre-commit-unreviewed-content.sh` — the sibling guard and its stated gap
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — the
  **inverse** shape: that record's captures took *unstaged* content, which is why its
  detectors are line-count based. This one takes *staged* content, so no cleanliness
  check can see it.
- `prompt-surface-measurement-session-log:F-51` — why the remedy must key on blob
- Reported by session `c95ba99b`; capturing commit `cffc3cf2` by session `ffb95976`.
  Both identified by `Session-Id:` trailer, not by adjacency.
