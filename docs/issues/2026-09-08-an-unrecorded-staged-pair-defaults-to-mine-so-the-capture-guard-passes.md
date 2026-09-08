---
id: '61b88853c19d313c'
kind: bug
status: open
title: An unrecorded (blob, path) pair falls to the `mine` branch, so the foreign-index guard passes on a capture
tags:
- cluster/guard-narrower-than-its-name
topic: shared-checkout commit safety
---

# BUG: an unrecorded (blob, path) pair falls to the `mine` branch, and the guard passes

## Summary

`scripts/pre-commit-foreign-index.sh` printed **Passed** while commit `a762dceb` swept a peer's
staged archive rename into a commit about something else. This is at least the **fifth** recorded
failure of this one guard, each previously fixed per-instance:

| bug | shape |
|---|---|
| `b8ee3f60e048f35c` | passed a bare commit carrying a peer's paths |
| `2eb51b4f12ed784e` | an **absent** stage log makes it pass silently |
| `1e5b89d4cf5483f2` | a transiently-empty index destroys ownership |
| `d3c590885a0878c0` | `git apply --cached` stages content the recorder can never attribute |
| **this one** | a **present** log with **no row for one staged path** |

Each prior fix taught the *recorder* to be honest about a state it could see. None changed what
the *reader* does when it finds no row at all.

## Symptom (Effect)

`codescout-92` (`59112612`) ran `git add -- <old> <new>` for an archive move and confirmed the
single `R` rename line. My `git commit` landed in the window before theirs. Result:

```
$ git show --name-status -M --format='' a762dceb
M   docs/issues/2026-09-08-the-write-lock-refusal-cannot-tell-...md        mine
R089 docs/issues/2026-09-08-the-cluster-refusal-names-a-field-not-a-file-and-the-index-confirms-it.md
  -> docs/issues/archive/2026-09-08-the-cluster-refusal-...md              THEIRS
M   src/agent/write_guard.rs                                              mine
```

I had staged with an explicit pathspec (`git add -- <two paths of mine>`) and then run a **bare**
`git commit`, which is the index-commit form this guard exists for. All four pre-commit checks
reported `Passed`.

## Root cause — the reader's default, verified at the bytes

`scripts/pre-commit-foreign-index.sh:164-185`:

```bash
prior="$(awk -F'\t' -v b="$blob" -v p="$path" \
    '$2 == b && $3 == p { print $1 "\t" $4; exit }' "$log")"
IFS=$'\t' read -r owner route <<< "$prior"
if [ -n "$owner" ] && [ "$owner" != "$me" ]; then
    theirs+=("$path")
    ...
else
    mine+=("$path")      # <-- an EMPTY owner lands here
fi
```

A `(blob, path)` pair with **no row in the log** yields an empty `$owner`, fails `-n`, and is
classified **mine**. `theirs` stays empty, `((${#theirs[@]})) || exit 0` fires, and the guard
passes.

**This is the exact direction `a987df96`'s own ruling forbade**, quoted from the bug that fix
closed: *"`unknown` over-refuses recoverably — a reader reads a message — where `mine`
under-refuses silently, and nothing is emitted for anyone to recover from. Prefer the noisy wrong
answer wherever the quiet one is unobservable."*

That ruling was applied to the **recorder** (write `-` rather than claim a pair) and never to the
**reader**. The recorder can only be honest about pairs it saw; the reader is the only party that
can be honest about pairs nobody saw. So the rule was implemented on the side that cannot enforce
it.

## Evidence

Ownership was recorded **correctly** — this is not an attribution failure:

```
$ grep 'the-cluster-refusal-names-a-field-not-a-file' .git/session-stage-log
59112612-…   a8bd650f   docs/issues/2026-09-08-the-cluster-refusal-…md   named   retained
59112612-…   e6ec7776   docs/issues/2026-09-08-the-cluster-refusal-…md   named   retained
59112612-…   d2d947f5   …                                                named   retained
-            00000000   …                                                not-staging  retained
59112612-…   54cff418   …                                                named   retained
```

Five rows, four naming the peer. But for the **archive** path — the addition half of the rename,
and the entry my commit actually carried:

```
$ grep -c 'archive/2026-09-08-the-cluster-refusal-names-a-field-not-a-file' .git/session-stage-log
0
```

**Control, because a zero must not be a broken grep:** the same log holds **39** other
`docs/issues/archive/` paths and 42 `/archive/` paths overall. Archive paths are recordable and
routinely recorded; this one specifically is not there.

## What I did NOT determine — stated because the next reader will assume I did

**Why that row is missing.** Two candidates, not separated:

1. A **race** — my `git commit` ran between the peer's `git add` and the recorder's write.
2. **Retention** — `post-index-change-stage-log.sh` prunes on a `max_retained` cap, so a fresh row
   could have been evicted.

The reader-side defect above holds either way and is independently sufficient: whatever caused the
gap, a guard against capture must not resolve *"I have never heard of this pair"* to *"it is
mine"*. But the missing row is a second defect if it is (2), and a third guard hole if it is (1),
and I am not claiming which.

I also did not check whether the **deletion** half should have caught it. Row 4 above is
`(00000000, old_path)` owned by `-`, which is foreign and would have routed to `theirs` had the
lookup matched — so either the abbreviated-blob widths differ between recorder and reader, or that
entry was not enumerated. Unexamined.

## Fix

Not implemented. The reader-side change is small and its direction is already ruled: classify an
unfound pair as **unknown-and-foreign**, not as mine, and say which pairs were unfound. The prior
art for the message shape is the `-` owner branch three lines above, which already exists and
already prints a route.

Whether that over-refuses in practice is a real question — a cold log after `install-hooks.sh`
would refuse everything — and `2eb51b4f12ed784e` (absent log) is the precedent for how that was
handled before. Read it first.

## Workarounds

Use the composed form the guard's own header prescribes, which is what I failed to do:

```
git add <paths>            # satisfies pre-commit-unreviewed-content
git commit -- <same paths> # the pathspec is what ignores the shared index
```

I ran the first and then a **bare** commit. The header states this in the file and I had read the
file.

## Explicitly NOT repaired

`docs/conventions/shared-checkout-commit-sequence.md` step 6: *"If a commit captures another
session's file: stop. Do not reset or amend. On a shared tree the repair destroys work the defect
only mislabels."* The content is correct and in history; only the attribution is wrong, which is
the cheaper of the two failures. `codescout-92` reported it on that basis and asked for no repair.

The capture's one real cost was downstream and they fixed it at `1d77ffb8`: it made the guide's
*"re-point an archive move's citations in the same commit as the move"* impossible, so HEAD
carried two dangling refs to the pre-archive path for a few minutes.

## References

- `docs/issues/archive/2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md` — the
  nearest sibling, and the source of the `unknown`-over-`mine` ruling this file says was applied
  to the wrong side.
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — the parent
  record of both capture shapes.
- `scripts/pre-commit-foreign-index.sh:164-185` — the `else` branch.

## Attribution

Detected and reported by sessionId `59112612-5fc8-4b31-8c8c-e19220d99eac`, who derived it from
`git log` on their own archived path rather than inferring it, and who declined the repair
deliberately and asked that the refusal be on the record rather than read as an oversight. The
capturing commit is mine, sessionId `5399543d-22d6-4ed9-9ebb-876be459989f`.
