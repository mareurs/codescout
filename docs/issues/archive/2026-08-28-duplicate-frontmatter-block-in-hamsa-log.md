---
id: 5c4f87e68cdad584
kind: bug
status: fixed
title: 'BUG: prompt-hamsa-audit-log.md carries two frontmatter blocks — the second is inert body text, and every catalog-facing field in it is a lie nothing reads'
owners:
- marius
tags:
- librarian
- frontmatter
- tracker
closed: 2026-08-28
---

# BUG: prompt-hamsa-audit-log.md carries two frontmatter blocks

## Summary

`docs/trackers/prompt-hamsa-audit-log.md` opens with **two** consecutive YAML
blocks. Only the first is frontmatter; the second is body text that happens to
look like YAML. It has sat in git since 2026-06-14 and survived every reindex
since, because nothing checks for it.

## Symptom (Effect)

`head -30 docs/trackers/prompt-hamsa-audit-log.md`:

```
---
id: '59ebeebb6ed05c89'
kind: tracker
status: active
title: Prompt Hamsa — Audit Log
tags:
- prompt-hamsa
- prompt
- audit
expects_augmentation: true
---

---
id: '59ebeebb6ed05c89'
kind: tracker
status: active
title: Prompt Hamsa — Audit Log
owners: []
tags:
- prompt-hamsa
- prompt
- audit
topic: null
time_scope: null
---

# Prompt Hamsa — Audit Log
```

## Reproduction

```
git rev-parse HEAD          # 14aab5ff at time of filing
head -30 docs/trackers/prompt-hamsa-audit-log.md
```

Sweep for the class across every tracker (counts `kind:` lines in the first 60):

```
for f in docs/trackers/*.md; do
  n=$(awk 'NR<=60 && /^kind: /{c++} END{print c+0}' "$f")
  [ "$n" -ge 2 ] && echo "$n $f"
done
```

**Measured 2026-08-28: exactly one hit** — this file. Not a widespread class.

Note the naive version of this sweep — counting bare `---` lines — reports 22
files and is **all false positives**: session logs use `---` as an entry
separator, which is the same string in a different position. Count a
frontmatter-only key like `kind:` instead.

## Environment

- Branch `experiments` @ `14aab5ff`, linux, codescout 0.15.0.
- Present on a fresh clone, so this is committed content, not catalog drift.

## Root cause

Unknown — not yet measured. What is established:

- `git log -S'time_scope: null' -- docs/trackers/prompt-hamsa-audit-log.md`
  returns exactly one commit: `fec17cd8` (2026-06-14). The second block was
  introduced there and never removed.
- The second block's shape — `owners: []`, `topic: null`, `time_scope: null`,
  explicit nulls, reflowed tags — is the **canonical serialization** that
  `artifact(update)` emits. That shape is documented as its own defect in
  `docs/issues/archive/2026-07-13-artifact-update-frontmatter-null-churn.md`
  (fixed) and `docs/issues/archive/2026-08-16-artifact-update-reserializes-frontmatter-on-a-field-patch.md`
  (fixed, BL-36).

So the *likely* mechanism is a writer that emitted a fresh canonical block
without detecting the hand-authored one already present — but this is
**inferred from the block's shape, not measured**. Both cited defects are fixed,
so the writer that did it may no longer exist. Do not treat the inference as the
diagnosis; reproduce before fixing.


### Tested 2026-08-28 — the inference above holds, within a stated scope

The file said *"the writer that did it may no longer exist"* and told the reader not
to treat that as the diagnosis. It is now tested, because the fix itself is the test:
removing the orphan required a **write** through `artifact update --body`, and the
canonical block **did not come back**. A writer that re-serialises without detecting
an existing block would have re-emitted it in that same call.

**Scope the claim honestly.** That exercises ONE writer path — full-body replace.
It does not exercise `append_entry` or `update_entry`, which are the paths this
tracker actually takes in normal use. If the orphan ever returns, the mechanism is
live on a path this test did not reach; that is the re-open trigger, not a repeat
of the original inference.
## Evidence

Only the **first** block is parsed as frontmatter — the catalog agrees with it,
so classification is intact:

```
artifact(action="find", filter={"rel_path": {"contains": "prompt-hamsa"}})
→ kind=tracker, status=active, id=59ebeebb6ed05c89
```

The second block is therefore inert. Its `owners: []` / `topic: null` /
`time_scope: null` assert catalog-facing values that no code path reads — the
failure mode is a **reader** believing them, not a broken query.

## Hypotheses tried

1. **Hypothesis:** widespread across trackers.
   **Test:** the `kind:`-counting sweep above, all of `docs/trackers/*.md`.
   **Verdict:** rejected — 1 of 54 files.

2. **Hypothesis:** it breaks catalog classification.
   **Test:** `artifact(find)` on the file; checked `kind`, `status`, `id`.
   **Verdict:** rejected — all three correct, read from the first block.

## Fix

**Fixed 2026-08-28.** The orphan block is gone; the file now opens with exactly one
frontmatter block — the hand-authored one, which is also the only one carrying
`expects_augmentation: true`.

**How, and why not by hand.** The orphan is *body* to the catalog (`artifact(get)`'s
`$.body` begins with it), so it was removable by a body write rather than a
frontmatter edit. The new body was constructed as a pure line-drop **from the file
itself** — `awk 'NR>=27'` — and passed server-side via
`codescout artifact update <id> --body @/tmp/hamsa-newbody.md`. Nothing was
re-typed, so nothing in 1,573 lines could be silently lost in transcription. A
heading-scoped `body_edits` could not reach it: the orphan sits *before* the first
heading, in no section.

**Verified, not assumed:**

| check | result |
|---|---|
| `git diff` | exactly 14 deletions + 1 trailing-newline insertion. Nothing else moved |
| size | 1573 → 1560 lines, 244040 → 243868 bytes |
| content | 44 `A-N` headings and 30 index rows, both **unchanged** |
| identity | `id` / `kind` / `status` unchanged, per this file's own *Resume* |
| augmentation | survived — `entry_collection: "audits"` still reported, and `entry_filter` still returns rows |

**One claim in this file was wrong and is corrected here.** *Fix* previously said
this tracker "is one of the 22 currently reporting `augmentation_declared_but_absent`",
and suggested checking the two together. It is not among them — a live `doctor` run
lists 13 and hamsa is not one, because its augmentation is present and working. The
two issues were never coupled.
## Tests added

None yet. A `doctor` check (`duplicate_frontmatter_block`) is the natural home:
it is a catalog-drift scanner, this is catalog-facing text that drifted from
the catalog, and one existing hit means the check would start green-with-one
rather than green-with-nothing — which is what makes it worth adding rather
than assuming.

## Workarounds

None needed. Nothing is broken at runtime; the risk is a human reading
`owners: []` / `topic: null` and believing it.

## Resume

**Done — nothing outstanding.** The prescribed verification ran: `reindex`, then
`artifact(find, filter={"rel_path": {"contains": "prompt-hamsa"}})` returning
unchanged `id` / `kind` / `status`.

Re-open trigger, per the scope stated under *Root cause*: **the orphan block
reappears after an `append_entry` or `update_entry` write.** Those are the paths
this fix did not exercise, and a recurrence there means the re-serialising writer is
still live rather than that the original inference was wrong.
## References

- Introduced: `fec17cd8` (2026-06-14)
- `docs/issues/archive/2026-07-13-artifact-update-frontmatter-null-churn.md`
- `docs/issues/archive/2026-08-16-artifact-update-reserializes-frontmatter-on-a-field-patch.md`
- Found during the 2026-08-28 cross-machine catalog repair pass; see
  `docs/trackers/bug-ledger-resume-2026-08-28.md`
