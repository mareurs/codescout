---
id: '048fc80383fc1a75'
kind: bug
status: open
title: librarian reindex, link_scan and audit_doc_refs accept a `path` argument and run project-wide without saying so
tags:
- cluster/accepted-parameter-silently-dropped
opened: 2026-10-30
owner: marius
severity: low
unverified: The source that parses `path` per action was not read, so the root cause is unknown. The three symptoms come from this session's tool outputs and were not re-run.
---

# BUG: librarian reindex, link_scan and audit_doc_refs accept a `path` argument and run project-wide without saying so

## Summary
Three `librarian` actions accepted `path="<one file>"` without an error and then ran over the whole project. `reindex` also wrote to every catalog row it judged stale, not only to the named file. The response does not say that the argument was ignored.

## Class fit
`IC-15`, an accepted parameter that is silently dropped. Here the caller names one file and gets a project-wide scan, with the argument echoed or ignored and no note. It differs from the class's other members in that the same intent needs a different parameter name per action: the `audit_doc_refs` response itself says to narrow with `paths=[...]`.

## Symptom (Effect)
All three observed on 2026-10-30, branch `experiments`, in one session:

- `librarian(action="reindex", path="docs/trackers/friction-deep-dive-2026-10.md")` ran for more than 120 seconds (the harness moved it to the background). Its result: `"added": 11, "updated": 14, "removed": 39, "unchanged": 2176`, `"scope": "project"`, and `"rel_path": "docs/trackers/friction-deep-dive-2026-10.md"` echoed. The 39 removals equal the 39 `missing_file` findings that `doctor` had listed for stale `.claude/worktrees/agent-*` rows.
- `librarian(action="link_scan", path="docs/trackers/archive/resume-tool-surface-structural-mechanisms.md")` returned the project-wide counts: `ambiguous[50 of 664]`, `dangling[50 of 702]`, `cross_repo[50 of 96]`, the same as the unscoped run.
- `librarian(action="audit_doc_refs", path="docs/trackers/resume-queue-index.md")` returned `n_files_scanned=2901`, `n_refs_found=90458`, and the hint "narrow with paths=[...]".

## Reproduction
Not re-run on 2026-10-30. Call any of the three actions with `path` set to one file on a catalog that holds more than that file, and compare the counts with the same call without `path`.

## Environment
codescout MCP server on branch `experiments`, Linux, catalog with about 2,200 artifacts.

## Root cause
Unknown. Not read in the source on 2026-10-30. The three responses above are the only evidence. The `reindex` response did record the stale-row cleanup honestly in its counts, so the damage was visible after the fact; the argument being ignored was not.

## Evidence
The three result summaries quoted above, from this session's tool outputs. For `reindex`, the removed rows were rows whose files did not exist, so the extra writes were cleanup the next unscoped `reindex` would also have done.

## Hypotheses tried
1. **Hypothesis:** `path` is a documented per-file scope for these actions. **Test:** compare counts with the unscoped runs. **Verdict:** rejected for the three observed calls. The counts match the project-wide runs.

## Fix
Not started. Options: refuse an unknown or unsupported argument with a hint naming the right one (`paths=` for `audit_doc_refs`), or honour `path`. A refusal matches how the other tools report unknown parameters.

## Tests added
None.

## Workarounds
Pass `paths=[...]` to `audit_doc_refs`. For `reindex` and `link_scan`, assume the whole project is touched and read the counts, not the echoed path.

## Resume
Read the `librarian` action dispatch in `src/librarian/tools/` for how `path` is parsed per action, then decide between refusing and honouring it.

## References
`docs/trackers/issue-clusters/IC-15-accepted-parameter-silently-dropped.md`; the hygiene sweep of 2026-10-30 (`docs/trackers/tracker-hygiene-log.md`).
