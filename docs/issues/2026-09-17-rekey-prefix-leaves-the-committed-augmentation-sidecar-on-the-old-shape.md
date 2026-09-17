---
status: open
opened: 2026-09-17
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/blast-radius-exceeds-visibility
kind: bug
---

# BUG: `rekey_prefix` moves the catalog augmentation and leaves the committed sidecar on the old shape

## Summary

`doc(action="rekey_prefix")` rewrites the augmentation's `params_schema` and `prompt` in the
catalog, and does not touch the committed sidecar under `docs/augmentations/`. The catalog is
machine-local and gitignored; the sidecar is what travels in git. So the rekey is correct on
the machine that ran it and reverts on every other one.

## Symptom (Effect)

Nothing fails locally. On another machine — or after a fresh clone — `reindex` re-attaches the
declared shape from the sidecar, restoring `pattern: ^T-\d+$` over entry ids that are all now
`SRI-N`, and a prompt instructing the maintainer to write `## T-N` headings under a prefix the
ledger no longer uses. The next `append_entry` against that ledger fails schema validation,
naming a pattern nobody on that machine changed.

## Reproduction

Observed, not constructed — this is what happened on the first real use, `404fbbe1`:

```
doc(action="rekey_prefix", id="6f5ec09c63aef864", from="T", to="SRI", force=true)
→ {"schema_patterns_repointed": 1, "prompt_rewritten": true, ...}

git status --short           # the sidecar is NOT among the changed files
grep pattern docs/augmentations/docs-trackers-system-retrospective-improvements.yaml
→ pattern: ^T-\d+$          # catalog says ^SRI-\d+$
```

`git rev-parse HEAD` at the time: `0588558d`.

## Environment

codescout `experiments`, Linux. Affects any augmented ledger with an exported sidecar — 24 of
them in this repo today.

## Root cause

`rekey_augmentation` (`src/librarian/catalog/rekey.rs`) ends with an `UPDATE
artifact_augmentation SET params=?1, params_schema=?2, prompt=?3` and nothing else. The
sidecar is a separate, committed representation of the same shape, written only by
`librarian(action="doctor", fix="export_augmentations")`.

Two representations of one truth, and the rekey updates one. `docs/conventions/cross-machine-catalog-resume.md`
is the standing account of this split: the catalog arrives missing layers after a clone and
**each is silent in a different way**.

**Measured 2026-09-17** by the commands above. The mechanism was also read out of
`rekey.rs`; the reproduction is what makes it a fact rather than a reading.

## Evidence

The plan for this feature listed the sidecar as one of the surfaces to move. The
implementation did not, and no test covered it — the catalog-layer tests assert on
`artifact_augmentation` rows and never look at `docs/augmentations/`, so the omission is
invisible to a green suite by construction.

## Root-cause candidates for the fix

Not settled — recorded so the next session does not re-derive them:

- Call the sidecar export from `rekey_augmentation` inside the transaction. Cheapest, but
  `export_augmentations` **creates and never refreshes** (it skips a path that already has a
  sidecar), so it cannot be reused as-is.
- Give the sidecar writer a refresh mode and call that. Fixes the general problem: any future
  surface that edits a shape has the same hole.
- Refuse the rekey when a sidecar exists, naming the manual step. Loud and cheap, but it makes
  the action unusable on 24 of this repo's ledgers.

## Fix

Not yet fixed. Worked around once — see below.

## Tests added

None yet. Note what the missing test looks like: a rekey against a ledger WITH a sidecar on
disk, asserting the sidecar's `pattern` moved too. Every existing test constructs its
augmentation directly in the catalog, so none has a sidecar and none can observe this.

## Workarounds

What was done for `404fbbe1`, and it is not obvious, because the export fix creates rather
than refreshes:

```
rm docs/augmentations/<sidecar>.yaml
librarian(action="doctor", fix="export_augmentations", scope="project", confirm=true)
```

The tool's own hint names this path: *"Establish which side is correct first; if it is the
catalog, delete the sidecar and re-run to republish its shape."* Verify the catalog is the
correct side before deleting anything — after a rekey it is, by construction.

## Resume

Decide between the three candidates above. If the refresh mode wins, it is a change to
`export_augmentations` rather than to `rekey_prefix`, and the test belongs with it. Then add
the missing rekey test: a ledger with a real sidecar on disk, asserting both representations
moved.

## References

- `src/librarian/catalog/rekey.rs` — `rekey_augmentation`, the incomplete write
- `src/librarian/augmentation_sidecar.rs` — the sidecar format and its export
- [`docs/conventions/cross-machine-catalog-resume.md`](../conventions/cross-machine-catalog-resume.md) — the standing account of catalog-vs-git splits
- `404fbbe1` — the rekey that exposed it, and the manual repair
