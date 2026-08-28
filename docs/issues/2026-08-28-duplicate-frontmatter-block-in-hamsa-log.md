---
id: a843bfdefaf6347b
kind: bug
status: open
title: 'BUG: prompt-hamsa-audit-log.md carries two frontmatter blocks — the second is inert body text, and every catalog-facing field in it is a lie nothing reads'
owners:
- marius
tags:
- librarian
- frontmatter
- tracker
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

Not started. Deleting lines 12–25 is almost certainly right, but per CLAUDE.md
(*run the reproduction before reading the fix plan*) confirm first that no
consumer reads the second block — `expects_augmentation: true` appears only in
the first, and this tracker is one of the 22 currently reporting
`augmentation_declared_but_absent` on this machine, so the two are worth
checking together rather than in sequence.

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

Delete the second block (lines 12–25 of
`docs/trackers/prompt-hamsa-audit-log.md`), then `librarian(action="reindex")`
and re-run `artifact(action="find", filter={"rel_path": {"contains": "prompt-hamsa"}})`
to confirm `kind`/`status`/`id` are unchanged. Consider the `doctor` check in the
same pass.

## References

- Introduced: `fec17cd8` (2026-06-14)
- `docs/issues/archive/2026-07-13-artifact-update-frontmatter-null-churn.md`
- `docs/issues/archive/2026-08-16-artifact-update-reserializes-frontmatter-on-a-field-patch.md`
- Found during the 2026-08-28 cross-machine catalog repair pass; see
  `docs/trackers/bug-ledger-resume-2026-08-28.md`

