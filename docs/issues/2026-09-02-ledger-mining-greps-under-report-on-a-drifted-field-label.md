---
kind: bug
status: mitigated
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-02
opened: 2026-09-02
owner: marius
related: []
severity: medium
unverified: No regression test — nothing gates the field-label form, so the corpus can drift back the same way tomorrow. Four fields remain genuinely ABSENT (OB-12 Status; OB-13 Plausible-answer property, Vigilance, Status) and were deliberately not authored. The 6 repaired labels were verified by re-running the documented grep; the absent four were not repaired at all.
---

# BUG: the observer-blindness ledger's own mining greps under-report on a drifted field label

## Summary

`docs/trackers/observer-blindness.md` § *How to mine this* publishes greps as the
ledger's read interface, including the design worklist that `H-N` and `I-N` are
documented to consume. The greps are line-anchored on `**Field:**`. Four entries had
written the label and its content inside a **single** bold span (`**Field: content**`),
which the pattern cannot match — so the worklist query returned **2** where ground
truth was **3**, silently omitting an open design task.

## Symptom (Effect)

The documented worklist command:

```
grep -B4 '^\*\*Mechanism status:\*\* none yet'   docs/trackers/observer-blindness.md
```

returned **2** matches. Entries actually declaring `none yet`: **3** — `OB-4`, `OB-5`,
`OB-10`. `OB-10` was invisible, because its line read:

```
**Mechanism status: none yet.** Two candidate shapes, in the order this ledger prefers them:
```

No error, no warning. A smaller number that reads exactly like an answer.

Full presence sweep at time of measurement — **10 field misses across 4 of 14 entries**:

| entry | fields invisible to the documented pattern |
|---|---|
| `OB-2` | Mechanism status |
| `OB-10` | Mechanism status |
| `OB-12` | Blind party, Vigilance, Mechanism status, Status |
| `OB-13` | Blind party, Plausible-answer property, Vigilance, Status |

## Reproduction

Main checkout, `experiments`, 2026-09-02, before the repair commit:

```
grep -c '^\*\*Mechanism status:\*\* none yet' docs/trackers/observer-blindness.md   # 2
grep -n  '^\*\*Mechanism status: '            docs/trackers/observer-blindness.md   # the drifted form
```

Ground truth is obtained per section rather than per file, because a file-wide count
cannot say which entry is missing:

```
for n in $(seq 1 14); do
  awk -v id="## OB-$n —" 'index($0,id)==1{f=1;next} /^## OB-[0-9]+ —/{f=0} f' \
    docs/trackers/observer-blindness.md | grep -m1 '^\*\*Mechanism status'
done
```

## Environment

codescout `experiments`, main checkout, 2026-09-02. No codescout code path is implicated —
the defect is in the corpus and in a documented shell recipe.

## Root cause

The field-label form is a **convention enforced by nothing**.

`src/util/librarian_guard.rs` makes the file server-only on the strength of its declared
`entry_prefix`, so hand-edits are refused and every write goes through
`artifact(action="append_entry" | "update")`. Neither call inspects the *shape* of the
field block it writes — `append_entry` writes the heading and the caller's body verbatim.
The guard protects the id namespace and says nothing about the labels, which is the part
the read interface depends on.

The ledger's template states the requirement — *"keep the labels **exactly** as written,
line-anchored, so the greps in* How to mine this *keep working"* — and a template is a
policy, not a mechanism. Measured 2026-09-02: **4 of 14 entries had drifted**, including
the two most recent, one of them (`OB-13`) written the same day by a session actively
working this ledger.

Two sub-causes, needing different remedies and mixed together in the raw count:

1. **Mis-formatted (6 of 10)** — the label exists but is bolded together with its content,
   so a presence sweep cannot see it. Mechanically repairable without touching any claim.
2. **Absent (4 of 10)** — the field was never written. `OB-13` carries its disposition in
   prose (*"Not promoted, and the population is honest about itself"*) rather than in a
   `**Status:**` field. Not repairable without **authoring a peer's disposition**, which is
   a different act from repairing a pointer.

## Evidence

Documented query against per-section ground truth, 2026-09-02:

```
documented grep returns: 2
ground truth:
  OB-4   **Mechanism status:** none yet — this is the open worklist row.
  OB-5   **Mechanism status:** none yet — designed, and the design is **n
  OB-10  **Mechanism status: none yet.** Two candidate shapes, in the ord
```

After repairing the 6 mis-formatted labels:

```
documented grep returns: 3   (ground truth 3)
remaining misses:  OB-12 Status | OB-13 Plausible-answer property | OB-13 Vigilance | OB-13 Status
```

## Hypotheses tried

1. **Hypothesis** — the ledger's published verification is stale, so the drift is already
   recorded somewhere. **Test** — read § *How to mine this*. **Verdict** — rejected. It
   reads *"Verified 2026-08-31 at 3 entries — definers 3, `**Blind party:**` 4"*, which is
   correctly dated and correctly scoped to when it was taken. The claim did not decay; the
   **corpus** moved past it, and nothing re-reads a dated measurement.
2. **Hypothesis** — loosen the pattern to `^\*\*Field:` so it matches both spellings.
   **Verdict** — rejected on this repo's own measured precedent.
   `docs/trackers/issue-clusters.md` § *One slug, two spellings* records the identical
   choice and its outcome: *"A parser hardened against a naming inconsistency does not
   harden the corpus — it removes the pressure to fix it"*, and the trap then *"stayed
   armed for every hand query and fired again the same day."* The repair is one spelling,
   not two patterns.
3. **Hypothesis** — author the 4 absent fields as `unknown` so the sweep is complete.
   **Verdict** — rejected. Three of the four are on `OB-13`, filed the same day by another
   session; choosing its `Status` or `Vigilance` value is writing that session's judgement
   for it. Repairing a label changes no claim; supplying a missing one does.

## Fix

Repaired the 6 mis-formatted labels through the catalog
(`artifact(action="update", patch={body_edits: […]})`, six scoped `edit` operations —
`edit_markdown` is refused on this file). No claim text altered; each edit moves `**` two
positions.

This is a **corpus repair, not a root-cause fix**, hence `status: mitigated`. Nothing
prevents the next entry drifting the same way.

- **SHA:** recorded in the repair commit on `experiments`.
- **patch-id:** recorded alongside it.

**Root-cause candidate, not built:** a test asserting that every `## OB-N — <title>`
section carries the template's fields in line-anchored form. Same role
`tests/issue_clusters.rs` already plays for `cluster/` tags on bug files, over a corpus of
14 sections, and it would have caught all 10 misses on the day each was written.
Generalises to any declared ledger whose read interface is a documented grep — `R-N`,
`U-N` and `H-N` publish the same style of recipe.

## Tests added

**None.** That is the gap, and the reason this is `mitigated` rather than `fixed`: the
repair restores the query and adds no guard, so the defect recurs on the next entry written
by anyone who copies a neighbouring entry rather than the template. The candidate test is
described under **Fix**.

## Workarounds

When reading this ledger's worklist, derive per section rather than per file — a file-wide
`grep -c` cannot report which entry it failed to see:

```
for n in $(seq 1 <high-water>); do
  awk -v id="## OB-$n —" 'index($0,id)==1{f=1;next} /^## OB-[0-9]+ —/{f=0} f' \
    docs/trackers/observer-blindness.md | grep -m1 '^\*\*Mechanism status'
done
```

## Resume

Write the field-shape test described under **Fix** — assert every `## OB-N — <title>`
section in `docs/trackers/observer-blindness.md` matches `^\*\*<Field>:\*\*` for each
template field, excluding the fenced template block. Then decide, with the entries' authors,
whether the 4 absent fields (`OB-12` Status; `OB-13` Plausible-answer property, Vigilance,
Status) should be authored or whether the template's field list is advisory for some entry
shapes — the test's strictness depends on that answer, so settle it before writing the
assertion.

## References

- `docs/trackers/observer-blindness.md` § *How to mine this* — the greps this defect breaks
- `docs/trackers/issue-clusters.md` § *One slug, two spellings* — the same choice, measured,
  with the corpus-repair verdict
- `docs/issues/archive/2026-09-01-an-unbalanced-fence-silently-disables-every-line-anchored-field.md`
  — the sibling: a fence disabling field detection wholesale, where this is one label at a time
- `src/util/librarian_guard.rs` — why the file is server-only, and why that guard does not
  reach the label form
