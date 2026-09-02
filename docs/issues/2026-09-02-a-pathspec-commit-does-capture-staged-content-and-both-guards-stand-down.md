---
kind: bug
status: open
tags:
- cluster/gate-keyed-on-unobservable-event
- git-hooks
- shared-checkout
closed: null
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

Plan, not yet implemented. The `next-index-*` exit should become conditional: a
pathspec commit needs no guard for the paths it does **not** name, and needs one for
the paths it does. The check is available — intersect the commit's named paths with
the stage-log rows for the **staged blob** of each (see below), and refuse when a
named path's staged entry belongs to another session.

**Query by blob, never by path** — `prompt-surface-headline` aside, this is
`prompt-surface-measurement-session-log:F-51`: the log carries one row per blob a
path has held, and a path-keyed lookup returns `retained` pre-image rows that read
as current.

Also fix the comment at `:95-97`. A false premise stated confidently above a
`exit 0` is worse than no comment, because it answers the next reader's question
before they form it.

## Tests added

None yet. `tests/hooks-discrimination.sh` is the right home — it already covers all
four arms of the sequencer stand-down, and the missing case is *"pathspec commit
naming a path another session staged → refuses"*.

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

Make the `next-index-*` exit at `scripts/pre-commit-foreign-index.sh:99-102`
conditional on the intersection described under *Fix*, and add the
`tests/hooks-discrimination.sh` case. Do not widen it to refuse every pathspec
commit — that would fire on ordinary sequential work by one session, which is the
failure mode the sequencer stand-down was written to avoid.

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
