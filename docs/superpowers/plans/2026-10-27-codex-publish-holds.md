---
id: '002eb5d7b4ca73ea'
kind: plan
status: done
title: Codex publish holds implementation plan
---

# Codex Publish Holds Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Native execution is already authorized.

**Goal:** Let a Codex root session set and honor its own publish hold across commit and push.

**Architecture:** Share one validated identity adapter over session_context.py; retain Claude legacy tokens and map qualified Codex tokens to a separate ref subtree. Shell entry points share the adapter and preserve existing messages/range policy.

**Tech Stack:** Python standard library, Bash, Git, pytest.

**Spec:** docs/superpowers/specs/2026-10-27-codex-publish-holds.md

## Global Constraints

- Preserve Claude refs/holds/<sid>, trailer replay and no-identity manual behavior.
- Never change shared checkout, global config, installed hooks or real hold refs.
- Codex canonical principal is codex:<sid>; its ref is refs/holds/_codex/<sid>.
- Root session identity is separate from child thread metadata.

## Review Focus

- Same UUID in both harnesses: independent hold, release, foreign ownership and liveness.
- Both native identities or explicit/native mismatch: refuse before commit, push or hold mutation.
- Held author under wildcard acknowledgement: every branch/tag/raw-SHA path refuses.
- Ref failures and invalid ref tokens: refuse safely without interpreting absence.
- Trailer replay/coauthors: preserve foreign attribution and avoid duplicate stamps.

### Task 1: Complete the identity-to-hold path

**Files:** new scripts/publish-identity.py and scripts/publish-identity.sh; modify scripts/hold-publish.sh, scripts/pre-commit-run.sh, scripts/prepare-commit-msg-session-id.sh, scripts/pre-push-foreign-session-guard.sh and scripts/resolve-sids.sh; new tests/test_codex_publish_hold.py.

**Interfaces:** Python current identity adapter consumes resolve_context(workspace,environ), emits legacy Claude token or canonical Codex token, empty for missing/manual, nonzero for conflicts/unsupported. Shell publish_hold_ref(token) validates a legacy/qualified principal and emits its exact ref. publish_identity() resolves caller once through Python.

- [x] Add isolated end-to-end fixtures with sanitized environment; test set/refresh/list/release, held/unheld commits, native/explicit conflicts, child identity, stamping, same UUID independence, push hold/foreign acknowledgements, tags and no identity.
- [x] Run pytest -q tests/test_codex_publish_hold.py and observe missing Codex enforcement/stamps fail before implementation.
- [x] Add the adapter and wire each shell path to it; adapt lookup/list to namespaced refs and protect qualified identities from Claude registry lookup.
- [x] Rerun new pytest cases, existing pre-commit pytest and hold/push/trailer shell discriminations; fix regressions.
- [x] Request an independent strong-model reviewer to APPLY candidate mutations in scratch and report observed survivor counts before adding closure tests.
- [x] Close observed review gaps with failing tests first, then rerun affected lanes.
- [x] Run ./scripts/gate.sh --check, update user convention and existing subject issue with evidence, and commit only owned paths on this isolated branch.

## Reconnaissance

Actual session_context.py native resolver rejects simultaneous Claude/Codex roots, preserves Codex root under child threads and verifies explicit workspace identity. Existing hold refs use blobs with three fixed lines; no new blob fields are necessary. Existing Session-Id parser reads final trailers, and preserving its key means downstream foreign/coauthor scanning needs no parallel trailer protocol. Existing baseline: 16 pre-commit pytest cases and shell hold/push suites green before edits.

Ruling: use `_codex` rather than `codex` as the ref directory: a valid legacy Claude id can be the word `codex`, whereas underscore lies outside its grammar. This prevents a file/directory ref collision without migrating any existing hold.

Progress: initial new integration suite observed 33 failures before implementation. Adapter/wiring then reached 49 passing new/existing pre-commit cases. Existing shell suites require sanitizing inherited native Codex context; copied-guard fixtures must copy mandatory identity dependencies while still omitting their optional liveness/age dependencies.

## Review closure log

- Independent reviewer observed corrupt Codex refs bypassing push and making release report absence. Implementer reproduced both as failing push/release pytest cases before changing code. Codex lookups now distinguish show-ref --exists missing/error; existing Claude policy stays compatible.
- Reviewer applied liveness mutation M02 that stripped codex: and borrowed a same-UUID Claude registry row; the original 49-test suite survived. Added a registry-bearing discrimination, reapplied exactly that mutation in scratch and observed failure: Codex incorrectly printed gone instead of ?. Production liveness logic already rejected the alias and passes the new case.
- Added a legacy Claude ID literally codex alongside a Codex hold to pin the namespace collision ruling. All review closure tests run solely on disposable repositories.

## Applied mutation results

The independent reviewer applied exactly 10 isolated candidate mutations against a clean 49-case pytest baseline: 8 were killed and 2 survived. M02 stripped the Codex prefix during Claude-registry lookup; M10 routed the explicit claude-code alias into the Codex hold namespace. Both survivors were observed before adding their closure tests. Implementer reapplied each exact mutation in separate disposable snapshots with the new registry/alias cases and observed one failure for each; the corresponding production cases pass. These are observed pytest outcomes, not inferred coverage arguments. Review logs: /tmp/codex-hold-review-20261027/logs, including results.json. A durable result table and review-boundary rulings are retained in docs/superpowers/reviews/2026-10-27-codex-publish-holds.md.

The actual corrupt-ref failures were separately reproduced as two failing push/release cases before production fixes; the focused production closure run then passed all five cases covering liveness, namespace collision, alias discrimination and the two corrupt-ref operations. Broader direct verification after the fixes reached 53 passing new/existing pre-commit pytest cases and reran hold/push shells; the final alias case was added afterwards. Final light lane is being rerun.

## Final verification

Full ./scripts/gate.sh --check exited 0 in the run-leased slot-0 target: PYTHON_LIGHT=0, FMT=0, CLIPPY=0, LEAN=0, DEFAULT=0, FMT_STATE=passed. Initial Python lane before review closures reported 1402 passed plus 445 subtests; final light-lane rerun after all five added closure cases reported 1407 passed plus 445 subtests. No Rust source changed. A later help-message wording adjustment was followed by the 91-assertion hold suite, green.

Direct shell results: hold 91 passed/0 failed, pre-push 412 passed/0 failed, commit-trailer 21 passed/0 failed, index-hook discrimination 298 passed/0 failed. Bash syntax checks and git diff --check passed. The native identity adapter was invoked in the actual Codex worktree and returned a canonical codex root token; no Claude identity was supplied.

Coverage limits printed by the gate: three heavy Python suites are excluded (test_phase1b_step4.py, test_phase1b_step5.py, test_phase1b_training.py); server-stack is not compiled by these lanes; librarian code runs only in DEFAULT; two local ONNX weight tests report ok while skipped without their weights. Existing configured ignored Rust/doc tests remain ignored. No model CLI/API calls, remote publication, hook installation, profile changes or real hold mutations were performed.

Delivery: preserve the isolated feature branch/worktree and its catalog overlay. Integration and librarian merge_worktree reconciliation require a separate explicit decision; no merge, push, global enablement or catalog reconciliation is performed here. Feature commit: f7d6c797e733275dce6f7455bb30de797e7dcb09; stable patch-id f4cbd8641d2bd78440c5078254b264af8a6ffb5e. Actual native commit attribution was read back as a canonical codex root Session-Id. The isolated branch is feat/codex-publish-holds-20261027. Raw gate, final Python and mutation logs are retained under the Git-ignored .codescout/resident-private/codex-publish-holds-20261027/ directory; original reviewer snapshots and closure scripts remain under /tmp. The implementation commit's dead-artifact-id hook reported a documented skip because no default binary path existed in this worktree; a separate explicit gate-binary audit was then run over all seven changed Markdown paths, with its report retained beside the logs. Its high-severity artifact_missing/artifact_id selection was empty; historical missing-path and unknown-symbol findings remain in the raw report and were not represented as a clean all-reference audit.
