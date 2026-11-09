---
id: '9f27965f08caa909'
kind: tracker
status: active
title: ECC integration evaluation — index
tags:
- ecc
- codex
- evaluation
topic: ecc-integration
---

# ECC integration evaluation

Evaluate whether the Codex ECC adapter adds useful behavior to the existing codescout setup.

## Current state — 2026-11-09

**Navigation cycle completed and integrated:** [report](reports/2026-11-09-navigation-dogfood.md), [observed draft](reports/2026-11-09-navigation-observed.md). Recoverable call_graph hop failures now reach the consumer as scoped completeness warnings while partial edges and quiet genuine negatives remain. Integration `3fe613bddca6ddd344164f6ece3278be56186c1a`, stable patch-id `4b7cad6bf7b5eda6f600304f4969fb13929b5eeb`; focused36/0, real MCP, applied mutation review and frozen-tree full gate passed. Peer research-only tree drift is explicit in the report. Next: user-requested active prompt-surface audit for current, visible call_graph guidance. No no-ECC replay or shared-server rollout.

**Dogfood cycle verified in an isolated worktree:** [report](reports/2026-11-09-doc-create-dogfood.md), [plan/ledger](plans/2026-11-09-doc-create-dogfood.md). The known doc(create) diagnostic defect was reproduced RED→GREEN; private real-MCP checks passed, both applied mutations were KILLED, independent review found no issues, and the standard full gate passed. Correction committed as `561c945d72a3997c972d5ef055e2c9d38e922e67` and integrated locally on experiments as `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7`, stable patch-id `3e8ba9084781d79228efe0f163c04ce19ffdd203`; the full post-integration gate passed. No push or shared-server rollout. The [next navigation cycle](plans/2026-11-09-navigation-dogfood.md) starts at that frozen integrated commit. [Exact baseline and future replay capture](notes/2026-11-09-doc-create-replay.md): commit `fe965888377151110f7fd10ab7c20469d6ea2f1c`, source archive and input/runtime snapshots. No replay ran. The comparison below remains historical evidence for its earlier scope.

The optional pilot contains `ecc-spec-miner` and `ecc-silent-failure-review` only. Native installation, fixture navigation and librarian persistence have been exercised. The pilot is enabled in the default /home/marius/.codex profile. Real-code evaluation of doc(create), including temporary-workspace admission and error outcomes, has completed. Both arms identified the same conditional cleanup-reporting defect; no additional confirmed finding was established by ECC in this pair. See the [dated comparison report](reports/2026-11-09-doc-create-comparison.md). The comparison is explicit use versus non-use with metadata installed in both arms; scoped enforcement files were stable, while two other source files changed. The controller's gate --check exited 0.

Loading and fixture success establish integration wiring, not better task quality or lower cost. See [evidence](evidence.md) for the observed checks and [issues](issues.md) for limits.

## Where to find things

| Location | Purpose |
|---|---|
| [plans/README.md](plans/README.md) | Evaluation protocols, acceptance criteria and next work |
| [reports/README.md](reports/README.md) | Dated results and adjudicated conclusions |
| [notes/README.md](notes/README.md) | Setup, provenance and historical decisions |
| [evidence.md](evidence.md) | Evidence inventory, reproduction paths and what each check measures |
| [issues.md](issues.md) | Integration friction, evaluation limitations and links to code bugs |

This folder is the canonical evaluation index. The adapter implementation remains in /home/marius/work/claude/claude-plugins.worktrees/ecc-codex-codescout, branch feat/ecc-codex-codescout; upstream ECC is /home/marius/work/claude/ecc. This index does not replace the adapter's source/provenance records.

## Evaluation questions

- Are both native skills discovered and used alongside the existing plugins?
- Do outputs trace real enforcement/callers and distinguish inspected tests from executed evidence?
- Does review separate confirmed defects from permitted fallbacks and unknown contracts?
- What useful findings or omissions differ from the existing workflow on the same scope?
- What native usage counters and elapsed times were observed, under what cache/order/load conditions?

## Decision boundary

Keep the user-enabled two-skill pilot available for explicit optional use. This first real-code pair showed the same confirmed diagnostic finding in both arms, with larger native input/output counters in the ECC run; no incremental effectiveness or cost saving was established. Future cases need a qualified control and stable source snapshot. Full ECC integration remains a separate scope.

## Maintaining this folder

Read linked primary evidence before changing a conclusion. Update the dated current state and next actions after each experiment. Preserve failed controls, inconclusive checks and unknowns. New reports/plans/notes live here; project memory should point to this index instead of accumulating another experiment journal. Artifact changes go through librarian.
