---
kind: bug
status: fixed
tags:
- cluster/blast-radius-exceeds-visibility
closed: 2026-09-17
opened: 2026-09-17
owner: marius
related: []
severity: medium
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

Fixed on `experiments` at **`b9e8680c`**, patch-id
**`7d28c1c2d722a19923d70c555ac2983d580caf2e`**.

**The diagnosis changed on contact, and the chosen remedy was the wrong shape.** This file
costed three fixes and picked *"give the sidecar writer a refresh mode"*. That mode already
existed: `augmentation_sidecar::write_through` keeps an already-committed sidecar true after a
shape change, never CREATES one, byte-compares so an unchanged shape leaves the file and its
mtime alone, and `Authored::Only` refuses to republish a field the call did not author.
`doc(action="augment")` has used it since `2a8decc5`, and its doc comment's worked example is
this bug almost verbatim — a `params_schema` edit that reported success while the committed
YAML kept the superseded shape.

So `rekey_prefix` was never missing a capability. It was **the one shape-editing surface that
did not call the write-through the others already use** — which is a smaller fix, and one that
adds no second publisher to drift out of step with the first.

**One cost that could not be designed away, so it is documented instead.** `write_through`
takes a `&Catalog` and the rekey's transaction holds that borrow, so the sidecar publishes
*after* the commit. A failure there leaves the catalog moved and the file not — the harmful
state `write_through`'s own doc comment names. The error says exactly that, including which
half is ahead and how to republish, rather than reporting a generic write failure. A
`refused` disagreement on an unauthored field is a separate refusal, because which side is
right is `sidecar_shape_drift`'s call and not this one's.

## Tests added

`the_committed_sidecar_follows_the_rekey` — `src/librarian/catalog/rekey.rs`. Observed red
first, failing for the right reason: the sidecar still held `pattern: ^T-\d+$` after the
rekey.

Its load-bearing fixture detail is the `expects_augmentation:` frontmatter key **and** a real
file at that path — `write_through` publishes only to a sidecar the artifact DECLARES and that
already exists, so dropping either turns the test green while guarding nothing. That is also
the answer to why no existing test caught this: every other catalog test builds its
augmentation straight into SQL, and none has a sidecar on disk, so the omission was invisible
to a green suite by construction.

Mutating `Authored::Only(&["params_schema", "prompt"])` to `Only(&[])` KILLS — which is what
shows the test exercises the real publish path rather than merely observing that some write
reached the file.

## Workarounds

No longer needed. The hand repair used for `404fbbe1` — `rm` the sidecar, then
`librarian(action="doctor", fix="export_augmentations", confirm=true)` — is now what the code
does by itself. It remains the right recovery if the post-commit publish ever fails, and the
refusal message names it.

## Resume

N/A — fixed.

## References

- `src/librarian/catalog/rekey.rs` — `rekey_augmentation`, the incomplete write
- `src/librarian/augmentation_sidecar.rs` — the sidecar format and its export
- [`docs/conventions/cross-machine-catalog-resume.md`](../conventions/cross-machine-catalog-resume.md) — the standing account of catalog-vs-git splits
- `404fbbe1` — the rekey that exposed it, and the manual repair
