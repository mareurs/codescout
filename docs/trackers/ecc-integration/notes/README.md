---
id: e0c447b3e19f2fba
kind: tracker
status: active
title: ECC integration notes and provenance
tags:
- ecc
- codex
- evaluation
topic: ecc-integration
---

# ECC integration notes

[Create finalization replay provenance — 2026-11-09](2026-11-09-create-finalization-replay.md): exact038fffb3 Git pin, neutral brief/source hashes, private pre-action host history and conditioning/runtime limits. No replay ran.

[doc(create) replay capture — 2026-11-09](2026-11-09-doc-create-replay.md): exact base commit/tree, validated source archive, task inputs, runtime/skills/server snapshots and checklist for a future no-ECC condition. No replay executed.

## Pilot provenance — 2026-11-09

Upstream clone: /home/marius/work/claude/ecc, revision d2ef2680594f9a17526a6e1a2e18b3d242c5d17b (declared 2.2.3). Adapter worktree: /home/marius/work/claude/claude-plugins.worktrees/ecc-codex-codescout, branch feat/ecc-codex-codescout, based on 85720782f0efbaba3a2fc607f7a44317fc95adec. Adapter changes remain uncommitted.

The native ecc-codescout 0.1.0 package adapts two workflows to codescout source navigation and librarian artifact persistence. It retains upstream hashes/license and adds no hooks, bundled MCP servers, second memory backend or delegation policy.

## Default-profile activation — 2026-11-09

At the user's request, the pilot was installed in /home/marius/.codex. Installed cache: /home/marius/.codex/plugins/cache/ecc-codescout-pilot/ecc-codescout/0.1.0. The user restarted Codex afterward; native discovery was checked again.

Backup and revert instructions: /home/marius/.codex/backups/ecc-codescout-20261109T105201Z/README.md. Surgical revert: `bash /home/marius/.codex/backups/ecc-codescout-20261109T105201Z/revert.sh`. The pre-install config hash and script syntax were verified; revert was not executed. Only ~/.codex was found on this machine; other profile names in the general instructions were checked but not created.

## Historical evidence locations

The adapter worktree's ecc-codescout/evidence/verification.md, native-report.json, native-retry-observed.md and docs/trackers/ecc-codex-codescout-session-log.md retain original packaging, fixture and setup observations. Treat these machine-local paths as external provenance, not files guaranteed to exist after cloning codescout elsewhere.

[Retained setup history](2026-11-09-pilot-setup-history.md) preserves the former research/ecc-codex-pilot-20261109 memory in full; that memory now points to this index. The research/ecc-integration-20261109 topic retains earlier upstream research. New evaluation journals belong in this folder; memory should retain the index pointer.

Return to the [evaluation index](../README.md).
