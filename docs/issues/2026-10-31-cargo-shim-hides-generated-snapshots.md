---
id: d1da4fc3b9499194
kind: bug
status: fixed
title: Cargo check shim hides generated snapshot artifacts from the author
tags:
- cluster/unclassified
- research
- effort-steering
closed: 2026-11-04
no_fix_commit: fixed on research/author-cargo-artifacts-20261031 at f938c32a0, not merged into experiments; the code it changes is not on experiments
opened: 2026-10-31
owner: marius
severity: medium
unverified: The fix and its Docker RED/GREEN, review and gate results are taken from the Codex session's memory and from git history; none was re-run here. The fix commit is on no remote branch. Whether the fix saves any of the model's budget was never measured (the original record says so too).
---

# BUG: Cargo check shim hides generated snapshot artifacts from the author

## Summary
The experiment's author-facing Cargo shim returns logs from a disposable container but does not return generated snapshot files. Snapshot editing workflows can therefore report a generated .snap.new file that the author cannot read. Caller INSTA_UPDATE is also not forwarded.

## Class fit
This is an experiment-tool contract mismatch between a containerized check and the author's working tree. It differs from the missing build-target and registry-cache issues because compilation succeeds and the output artifact, rather than an input dependency, is unavailable.

## Symptom (Effect)
The actual C-R primary transcript reports a generated snapshot under /testbed, followed by author Read responses: "File does not exist." The CLI records 11 permission_denials entries across the run. The run ends error_max_budget_usd; the saved source later passes the official nine-case target. These observations do not isolate the amount of budget consumed by this mismatch.

## Reproduction
Run the unpaid artifact_probe.py under the private matched-workflow-20261031 evidence directory. It uses the same actual shim and official immutable Tokio image with a dependency-free Rust test writing generated.snap.new.
Observed output: CONTAINER_ARTIFACT_EXISTS=true, INSTA_UPDATE=missing. Probe result: host_artifact_exists=false, caller_INSTA_UPDATE=always, exit_code=0, source_bound=true. No model call or paid retry.

## Environment
Research branch research/checked-real-task-20261024 at checkpoint074c1e3ed3a1cb2f5314c0466485b5ad5ea65814, actual author shim adapter_cli.py loading tools/reliable-agent-eval/external_feedback/adapter.py. Docker network none. Official Tokio image with Rust1.85.1 for the isolated probe; Ruff snapshot workflow observed in the actual repair.

## Root cause
Measured2026-10-31 by the fresh real-container probe above and source inspection: tools/reliable-agent-eval/external_feedback/adapter.py evaluate starts Cargo with fixed explicit environment flags, captures logs, and closes the disposable container. shim returns cargo.log and exit status only. It neither exports generated files nor forwards caller INSTA_UPDATE. The phase-local container closes through docker rm in finally.

## Evidence
Private root: /home/marius/work/claude/codescout/.codescout/resident-private/matched-workflow-20261031.
artifact-probe/result.json plus its source-bound Cargo trace and cleanup receipt reproduce file/env visibility.
repair-friction.json binds selected primary transcript excerpts to tool IDs and counts the literal raw CLI denial records.
arms/C-R/diagnostic-assessment.json preserves the invalid completion and separate9/9 saved-source diagnostic.

## Hypotheses tried
Confirmed: generated-file visibility is lost across the actual shim boundary.
Confirmed: an explicit public caller environment flag is not forwarded.
Not established: how much of the model's .70 allowance would have been saved by restoring artifacts; no paid counterfactual was run.

## Fix

Fixed on the research branch, not on `experiments`. Commit `f938c32a0c12ad1e0a31712f89e21b5a23dc5ec5` (2026-10-31, "fix(reliable-agent-eval): return generated author snapshots") is on `research/author-cargo-artifacts-20261031` and `research/fixed-acceptance-population-20261101`. `git merge-base --is-ancestor` says it is not an ancestor of `experiments` (checked 2026-11-04), and no remote branch contains it. The code it changes, `tools/reliable-agent-eval/external_feedback/`, does not exist on `experiments`.

What the commit does, from the Codex session's memory `research/effort-steering/author-artifacts-20261031` (read 2026-11-04; I did not re-run any of it): a default-off `author_artifacts` flag in `evaluate`; only the shim opts in; it collects regular `.snap.new` files inside the same disposable container (64 files, 1 MiB each, 8 MiB total) and returns them with a patch, hash and size receipt; oracle runs reject export before any I/O. The memory reports a real-Docker RED before and GREEN after, a mutation-tested review, and a gate exit 0.

The original record of this bug, `dbe65ecebd5114ec`, was left untouched there because its worktree is sealed.

## Tests added
The unpaid artifact_probe.py is a reproduction, not a passing regression for a fix. It observes the current broken author-file visibility. A future fix needs an observed RED-to-GREEN artifact consumer test and a check that independent oracle artifacts never enter the author workspace.

## Workarounds
Manual editing from printed diagnostic text is possible but does not restore a native snapshot workflow. Increasing model budget alone does not address the tool contract.

## Resume

Nothing to do on `experiments`. If the research branches are ever merged, add the commit SHA and patch-id under `## Fix provenance` and drop `no_fix_commit`. The Codex session `01a15ca9` was told on 2026-11-04 that this record would be closed this way.

## References

- The matched workflow report exists only on the research branches (`docs/research/2026-10-31-matched-workflow.md`, commit `23f234700`); it is not on `experiments`. Read it with `git show 23f234700:docs/research/2026-10-31-matched-workflow.md`.
