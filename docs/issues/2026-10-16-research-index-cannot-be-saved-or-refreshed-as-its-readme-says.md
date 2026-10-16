---
id: '1709e5706e0b5bfa'
kind: bug
status: fixed
title: 'BUG: the research index cannot be saved or refreshed the way its README says'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
- librarian
- research-index
- documentation
closed: 2026-10-16
opened: 2026-10-16
owner: marius
related:
- docs/research/README.md
- docs/augmentations/docs-research-README.yaml
severity: low
unverified: 'The local catalog still holds 15 entries for the research files (re-measured 2026-10-16 via doc gather: 15 rows in params.entries). Params are catalog-local and not in git, so the committed fix cannot rebuild them; rebuild by hand per the sidecar prompt.'
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

Fixed 2026-10-16 in `docs/research/README.md`; the README was the wrong side. Verified: `artifact_augment` and `artifact` are in `RETIRED_TOOL_NAMES` (`src/server.rs:6450-6459`); `doc(action="gather")` on `5086e3c7c0b9d83c` returned `context: {}` and `hints: []` (no `gather_from` in params, and `src/librarian/tools/refresh.rs` reads gather sources only from `params.gather_from`).

Changes: the save step now reads `doc(action="augment", id=..., merge=true, augment={params_path: ...})` (the form the sidecar prompt already used); the read example `artifact(action="get", ...)` became `doc(action="get", ...)`; step 4 says the rebuild is by hand and the tracker has no gather source; § Suspected stragglers no longer claims "Refreshed by gather pass" and says why (`gather_from` is a params value, params do not travel through git, the sidecar carries none). Decision: no gather source was configured, because `gather_from` is a params value and cannot be committed, so a configured source would exist on one machine only; the by-hand rebuild the sidecar prompt describes stays the procedure.

Fix commit `2d56edc2` on branch `worktree-agent-aa532b30b45f11948` (to be merged to experiments), patch-id `9787be24db6a362ca759ab8dee4085108e823018` (shared with the two sibling doc fixes).

## Tests added

None — documentation of a procedure; the retirement itself is pinned by the test at `src/server.rs:6868-6870`, which asserts the name is not registered; no test was added for this README.

## Workarounds
Rebuild `entries` by globbing `docs/research/` by hand, then write them with `doc(action="augment", merge=true, params_path=...)`.

## Resume

N/A — fixed. Separate, not done here: rebuild the local catalog `entries` from the files' frontmatter (catalog-local, not in git). Archive once on experiments with the gate green.

## References
- `docs/research/README.md`, `docs/augmentations/docs-research-README.yaml`
- `docs/superpowers/plans/2026-05-08-researcher-tracker.md`
