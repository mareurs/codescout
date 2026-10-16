---
id: '1709e5706e0b5bfa'
kind: bug
status: open
title: 'BUG: the research index cannot be saved or refreshed the way its README says'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
- librarian
- research-index
- documentation
opened: 2026-10-16
owner: marius
related:
- docs/research/README.md
- docs/augmentations/docs-research-README.yaml
severity: low
unverified: The catalog-local entry count (about 15 of 41 files) comes from the triage agent and was not re-measured.
---

# BUG: the research index cannot be saved or refreshed the way its README says

## Summary
The research index tracker (`docs/research/README.md`, id `5086e3c7c0b9d83c`) documents two operations that do not work at HEAD. Its save step calls the retired tool `artifact_augment`, and it says the index is "Refreshed by gather pass" while the tracker has no gather source, so a gather pass finds nothing.

## Symptom (Effect)
- An agent following the README's save step gets an unknown-tool error: `artifact_augment` was folded into `doc(action=augment)` on 2026-09-02 and is not registered.
- `doc(action="gather")` / a refresh for this tracker returns no research files, so the index can only be rebuilt by hand. The 2026-10-16 triage found the local catalog's `entries` well behind the directory (about 15 entries for 41 files); that count is catalog-local and was not re-measured by me.

## Reproduction
At `a16f7d42`:
1. `docs/research/README.md:69-70`: `artifact_augment(id="5086e3c7c0b9d83c", merge=true, params_path="<abs path to the rebuilt entries JSON>")`.
2. `src/server.rs:6456-6459`: `artifact_augment` is in `RETIRED_TOOL_NAMES` ("folded into `doc(action=augment)` (2026-09-02)"); the test at `:6868-6870` asserts it is not registered.
3. `src/librarian/tools/refresh.rs:43-47`: gather sources come only from `params.gather_from`.
4. The committed sidecar `docs/augmentations/docs-research-README.yaml` has the keys `schema_version`, `prompt`, `entry_collection`, `params_schema`, `render_template` and no `gather_from`.
5. `docs/research/README.md:199`: "(Refreshed by gather pass. None at index creation time.)"

## Environment
codescout, branch experiments, HEAD `a16f7d42`.

## Root cause

Prose true when written, code moved: the README was last changed on 2026-08-30 (`c2039a16`; the save step itself dates from `952ad795`, 2026-08-26), and `artifact_augment` was retired on 2026-09-02, after it. The retirement did not update this README. Separately, the tracker's gather configuration is a params value, which cannot travel through git, so no checkout has a gather source unless it is set in the local catalog; the committed sidecar carries none.

## Evidence
See Reproduction; lines read 2026-10-16. Provenance: the original plan for this tracker (`docs/superpowers/plans/2026-05-08-researcher-tracker.md`) was found by the effort-steering Stage A critic experiment to leave `gather_from` unset; the 2026-10-16 triage found the gap still present at HEAD, plus the retired tool name. Related archived records: `docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md`, `docs/issues/archive/2026-05-09-artifact-update-drops-params.md`.

## Hypotheses tried
N/A.

## Fix
Not started. Replace step 4 with the current `doc(action="augment", id=..., merge=true, params_path=...)` (or `doc(action="update", patch={params: ...})`), and either configure a gather source that can enumerate `docs/research/*.md` or remove the "Refreshed by gather pass" claim and keep the by-hand rebuild the sidecar prompt already describes.

## Tests added
None — documentation and catalog configuration.

## Workarounds
Rebuild `entries` by globbing `docs/research/` by hand, then write them with `doc(action="augment", merge=true, params_path=...)`.

## Resume
Decide whether the index should be gatherable; then fix the README text accordingly.

## References
- `docs/research/README.md`, `docs/augmentations/docs-research-README.yaml`
- `docs/superpowers/plans/2026-05-08-researcher-tracker.md`
