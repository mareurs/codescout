---
id: d5a3e9b6e9801fcc
kind: bug
status: fixed
title: doc(create) claims catalog rollback after best-effort cleanup can fail
tags:
- cluster/unclassified
- librarian
- diagnostics
topic: ecc-integration
closed: 2026-11-09
fix_commit: b3d02d482eaeb3cabf329f4f37a2d3649a6057b7
fix_patch_id: 3e8ba9084781d79228efe0f163c04ce19ffdd203
opened: 2026-11-09
related:
- '22ae2baefdfdd73a'
severity: medium
unverified: 'CLEARED 2026-11-09: committed and verified on experiments at b3d02d482eaeb3cabf329f4f37a2d3649a6057b7 with full post-integration gate; shared-server rollout is outside this correction.'
---

# BUG: doc(create) claims catalog rollback after best-effort cleanup can fail

## Summary

When the directory/file write fails, create invokes best-effort cleanup and then unconditionally returns an error context saying the catalog row was taken back. If the delete fails, that assurance is false. Create still returns failure; this is inaccurate recovery reporting, not a successful operation hiding its original failure.

## Class fit

Unclassified diagnostic-integrity defect at a recovery boundary: the error response claims a cleanup effect that its callee does not guarantee. Unlike the related partial-filesystem-create incident, this record concerns truthful reporting when the attempted mitigation itself fails, rather than adding the mitigation.

## Symptom (Effect)

Conditional path: disk operation fails → artifact deletion fails → undo_failed_create logs a warning and returns unit → create returns “could not write …; the catalog row was taken back”. The main MCP fatal renderer exposes the outermost context, while the delete warning remains server-side.

## Reproduction

Reproduced during the authorized ECC dogfood cycle on baseline `fe965888377151110f7fd10ab7c20469d6ea2f1c`. A real regular file occupies the intended parent directory; create inserts the artifact/augmentation before create_dir_all fails. A BEFORE DELETE SQL abort trigger in a fresh in-memory catalog makes cleanup fail. The original I/O error and retained artifact/augmentation were asserted before the new test failed specifically on the false rollback assurance. This deterministic injection works under root without quota exhaustion or permission-dependent skips. Actual stdio MCP against a private catalog also confirmed residual rows and the corrected outer diagnostic. [Report](../../trackers/ecc-integration/reports/2026-11-09-doc-create-dogfood.md).

## Environment

Observed during the 2026-11-09 ECC pilot comparison in the codescout checkout. The run recorded HEAD 6043fb27aeb21eb03c52238f2af3c189a97c16e9 and a dirty shared checkout. Exact source hashes and native outputs are retained by the linked report; concurrent changes elsewhere prevent treating HEAD alone as a full snapshot.

## Root cause

undo_failed_create returns no cleanup result. artifact::delete propagates SQL errors; the helper logs such an error and proceeds. The caller constructs an unconditional past-tense rollback assurance after the helper returns. The helper's comment explicitly calls cleanup best effort, so no stronger atomic-create contract is needed to establish this message mismatch.

## Evidence

Both the existing-workflow baseline and explicit ECC review independently reported this same conditional defect. Controller inspection confirmed the relevant bodies and fatal-rendering policy. At the initial read-only comparison, existing cleanup assertions covered successful removal, not delete failure; the later dogfood regression below adds the missing failure case. The related issue 22ae2baefdfdd73a records the earlier filesystem-failure mitigation; it does not verify truthful reporting when rollback fails.


The later authorized [dogfood report](../../trackers/ecc-integration/reports/2026-11-09-doc-create-dogfood.md) supersedes the initial static-only verification limit. It records deterministic RED→GREEN, actual MCP responses, independent review, applied mutation verdicts and a passing standard-environment gate. The earlier nested-TMPDIR gate failure is preserved too. No actual disk exhaustion or partial-file unlink failure was injected.

## Hypotheses tried

- False successful create: rejected; the primary write failure remains an error.
- Guaranteed row deletion: rejected by the helper's inspected error branch.
- General all-or-nothing create guarantee: unnecessary for this finding and otherwise unresolved.

## Fix

Committed in the isolated worktree as `561c945d72a3997c972d5ef055e2c9d38e922e67`, integrated and verified on **experiments** as `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7`; stable patch-id `3e8ba9084781d79228efe0f163c04ce19ffdd203`. The context now states that cleanup was attempted and catalog/file state may remain; original I/O cause and actual cleanup are preserved. The conservative warning also appears after successful cleanup. Full post-integration gate passed on the frozen integrated commit with the optional /tmp nextest override disabled after quota failures. [Report](../../trackers/ecc-integration/reports/2026-11-09-doc-create-dogfood.md). No master promotion or shared-server rollout was performed.

## Fix provenance

- **SHA:** `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7` (`experiments`)
- **patch-id:** `3e8ba9084781d79228efe0f163c04ce19ffdd203`

## Tests added

`librarian::tools::create::tests::failed_disk_write_reports_best_effort_cleanup_without_promising_rollback`: observed RED before production edits, then GREEN. Two subcases assert retained artifact/augmentation under aborted deletion and removal under successful deletion, original I/O cause, preserved blocker and truthful outer Display. Module: 48 passed. Real private-catalog MCP: both error cases and normal create passed. Applied old-diagnostic and missing-cleanup mutations were both KILLED by executed tests; exact source restoration verified. Standard full gate --check passed, including this regression in DEFAULT. Heavy Python/server-stack/local ONNX execution remain outside that gate's coverage.

## Workarounds

After a create write failure, inspect catalog/file state rather than relying on the rollback assurance alone. Do not assume a retry is safe solely from that message.

## Resume — 2026-11-09

Fixed and verified on experiments at `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7`, patch-id `3e8ba9084781d79228efe0f163c04ce19ffdd203`. Archived after passing gate plus regression; master promotion is not required. [Replay capture](../../trackers/ecc-integration/notes/2026-11-09-doc-create-replay.md) preserves the original baseline and inputs; no replay has run.

## References

- [Related partial-filesystem incident](../2026-10-29-artifact-create-partial-filesystem-failure.md)
- [ECC comparison](../../trackers/ecc-integration/reports/2026-11-09-doc-create-comparison.md)
- [Create implementation](../../../src/librarian/tools/create.rs)
- [Fatal error rendering](../../../src/server.rs)
