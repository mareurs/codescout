---
id: ad88e91e7dac43a7
kind: bug
status: mitigated
title: Native worker bootstrap omits Codescout guidance trimmed from initialize instructions
tags:
- cluster/unclassified
- effort-steering
closed: 2026-11-04
opened: 2026-11-04
owner: marius
severity: medium
unverified: STANDING — Private diagnostic transport workaround only; no full worker/task admission, real model adherence or universal isolation guarantee.
---

# BUG: Worker bootstrap omits trimmed Codescout guidance

## Summary
The onboarding-generated .codescout/system-prompt.md exists, but the pinned native initialize response replaces it with a custom-instructions trim note. The recorded status/search/onboarding/memory sequence never explicitly reads it, so file presence did not establish delivery to a worker.

## Class fit
A private bootstrap relies on a bounded automatic channel to carry unbounded onboarding content. The underlying channel limit is already documented by e3437bd1ec116dec; this issue tracks the experimental worker's missing explicit delivery step, not a new claim that the known limit has been fixed.

## Symptom (Effect)
The full bindgen query-bindgen-git-resumed-project MCP response population1through7 contains neither the Code Explorer Guidance title nor the inspected MonotoneFramework section anchor. Initialize explicitly says status trimmed: custom instructions. The original real-client attempt also has no guidance content, but its MCP connection failed separately.

## Reproduction
E=/home/marius/.cache/codescout-resident/codescout/native-env-admission-20261104. S=E/semantic-minimal-20261104; immutable source/knowledge snapshot contains the complete47-line guidance. The complete real native responses are S/query-bindgen-git-resumed-project/mcp/. W=E.parent/worker-context-verification-20261104 holds actual client serialization.

## Environment
Pinned native Codescout private package and bindgen common source snapshot. Original full native bootstrap used direct stdio controls; actual client transcript is a separate transport instrument.

## Root cause
Custom instructions remain in the bounded initialize channel, are dropped when oversized, and build_status_response_block emits only Substitutable segments. A pointer in project CLAUDE.md is not proof a worker read the pointed-to file.

## Evidence
Actual complete initialize and follow-up response files; source read of prompt tier selection/post_process. No model adherence or quality effect inferred.

## Hypotheses tried
Add an explicit native read of the common guidance and inspect the subsequent actual serialized request. Positive replay pending.

## Fix

An explicit native MCP read delivers the complete common .codescout/system-prompt.md to the client; the full47-line body appears in later actual serialized requests for both final efforts. Original automatic transport remains trimmed, and the underlying product cap is unchanged. This mitigates the private bootstrap's omission; model adherence is unmeasured.

## Tests added

Complete original response population lacks guidance; complete final request payloads contain its exact full body after explicit read. Source/knowledge snapshot unchanged; no repeat onboarding.

## Workarounds
Make reading the frozen common guidance an explicit bootstrap step and witness full body transport before the task. Preserve equal bytes across arms; do not regenerate onboarding.

## Resume
Verify direct common-guidance delivery alongside global/project rules and registered skills; distinguish loading from model adherence.

## References

[Actual client context verification and preserved evidence](../research/2026-11-04-worker-context-verification.md). The full continuation report and bundle are in the main checkout.
