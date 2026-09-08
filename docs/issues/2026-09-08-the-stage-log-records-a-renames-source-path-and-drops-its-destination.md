---
status: open
opened: 2026-09-08
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/selector-narrower-than-its-population
kind: bug
title: The stage log records a staged rename under its SOURCE path with the DESTINATION blob, so the destination path has no row and the capture guard reads that zero as "mine"
topic: shared-checkout gate correctness
---

# BUG: a staged rename is recorded at its source path, so its destination is unattributable

## Summary

`scripts/post-index-change-stage-log.sh` enumerates staged pairs with:

```bash
git diff --cached --raw | awk -F'\t' '{ split($1, a, " "); print a[4] "\t" $2 }'
```

`--raw` emits an ordinary change as `:<modes> <shas> <status>\t<path>` and a **rename** as
`:<modes> <shas> R<score>\t<srcpath>\t<dstpath>`. Rename detection is on by default
(`diff.renames`, git ≥ 2.9), so the second form occurs routinely. The awk takes `$2` — the
**source** path — and discards `$3`.

So a staged rename is recorded as **(destination blob, SOURCE path)**. The destination path
receives no row at all, and the row that *is* written attributes the new blob to the old path.

## Symptom (Effect)

`scripts/pre-commit-foreign-index.sh` looks a pair up by `$2 == blob && $3 == path`. For the
destination path there is no row, so `owner` is empty, the `-n` test fails, and the lookup falls
to `else` → `mine`. The guard named *"refuse an index commit carrying another session's staged
paths"* passes, and a peer's `git commit` sweeps up the rename.

**Archiving a bug file is a rename.** It is the commonest rename in this corpus, so the guard is
structurally blind at the destination of every archive move.

## Reproduction

No race needed.

1. `doc(action="move", …)` a bug file to `docs/issues/archive/`, then
   `git add -- <old> <new>`. `git status --short` shows one `R` line.
2. `grep '<archive path>' .git/session-stage-log` → **0 rows**.
3. `grep '<old path>' .git/session-stage-log` → a row carrying the **destination** blob.

## Root cause

Verified at the bytes against a real rename rather than derived from reading:

```
$ git show --raw -M a762dceb
:100644 100644 e6ec7776 a8bd650f R089⇥docs/issues/2026-09-08-…-confirms-it.md⇥docs/issues/archive/2026-09-08-…-confirms-it.md

$ … | awk -F'\t' '{ split($1, a, " "); print a[4] "\t" $2 }'
a8bd650f⇥docs/issues/2026-09-08-…-confirms-it.md
```

`a8bd650f` is the **destination** blob (`git rev-parse a762dceb:docs/issues/archive/…` returns
it). The path printed beside it is the **source**. Stage-log line 14 holds exactly that pair.

The selector is `$2`. Its population is *the paths a raw row carries*, which is one for a change
and two for a rename. It runs to completion, returns a well-formed answer, and the excluded
member is never examined — so nothing is short, nothing is marked, and the guard's later zero
reads as *"nobody staged it"* rather than *"never looked at"*.

## Evidence — two hypotheses eliminated, both plausible

A peer (`5399543d`) filed the reader-side defect at `61b88853c19d313c` and named two candidate
causes for the missing row, correctly declining to choose:

- **Retention** (`STAGE_LOG_MAX_RETAINED`, default 1000). The log stands at **1002** rows, so
  eviction is live, not theoretical. **Eliminated anyway:** the same `git add` produced rows at
  lines 14, 16, 57, 81 and 96 — all far inside the retained window. Rows are newest-first and
  eviction drops the tail, so a same-instant sibling would have survived exactly as line 81 did.
- **Race** (a peer's commit landing inside the `add`→recorder window). **Eliminated:** the
  recorder *did* run and *did* observe this rename — line 14 carries its destination blob. It
  was not missed; it was filed under the other path.

Line 81 is worth keeping in view:

```
-	00000000	docs/issues/2026-09-08-…-confirms-it.md
```

owner `-`, null blob — the deletion half, recorded unattributable. So the source path collects
*two* rows for one rename while the destination collects none.

## Hypotheses tried

1. **Retention evicted it.** Tested by locating the sibling rows' line numbers against the 1000-row
   window. Rejected — all well inside it.
2. **A race dropped it.** Tested by looking for *any* row carrying the destination blob. Rejected —
   line 14 has it, under the source path.
3. **The recorder skips blanket forms.** Rejected by reading: the skip is for `-A`, `-u`, `.` and
   directories; this was an explicit two-path pathspec.

## Fix

Not implemented. The enumeration must emit a row per path, not per row:

```bash
git diff --cached --raw | awk -F'\t' '{
    split($1, a, " ");
    print a[4] "\t" $2;
    if (NF >= 3) print a[4] "\t" $3;   # rename/copy destination
}'
```

Decide two things rather than assume them:

- **Whether the source path should still get the destination blob.** Today it does, and that is
  its own hazard: a peer who later stages the *old* path can match a pair recorded for someone
  else's rename. Emitting the destination row does not remove that.
- **`-C` (copy detection).** Off by default, but if enabled the same three-field shape appears and
  the same fix covers it — worth handling in the parser rather than relying on the config.

## Tests added

None yet. It needs no race: seed a temp repo, stage a rename, run the recorder, and assert a row
exists for the **destination** path.

Pair it with an ordinary single-path staging that asserts exactly one row — without that control
the test is monotone under a change that emits both `$2` and `$3` unconditionally, which would
write a garbage second row from a two-field line's empty `$3`.

## Workarounds

After staging an archive move, check attribution by the **source** path, not the destination —
and read the pair's blob, since the source path carries both the deletion row and the
destination blob.

## References

- `scripts/post-index-change-stage-log.sh` — the enumeration.
- `scripts/pre-commit-foreign-index.sh` — the `else → mine` fallthrough this feeds.
- `docs/conventions/shared-checkout-commit-sequence.md` — step 6, why a capture is reported
  rather than repaired.

## Attribution

The capture that exposed it, the reader-side root cause, and the two candidate explanations are
sessionId `5399543d`'s, who flagged the missing row as unresolved rather than guessing. The
third cause is `59112612`'s.
