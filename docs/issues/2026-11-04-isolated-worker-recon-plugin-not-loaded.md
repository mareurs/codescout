---
id: '0c1fe928735db6dd'
kind: bug
status: mitigated
title: Isolated Claude worker configuration cannot invoke the requested Recon skill
tags:
- cluster/unclassified
- effort-steering
closed: 2026-11-04
opened: 2026-11-04
owner: marius
severity: medium
unverified: STANDING — Private diagnostic transport workaround only; no full worker/task admission, real model adherence or universal isolation guarantee.
---

# BUG: Isolated worker lacks the requested Recon skill

## Summary
A source clone and isolated Claude config do not install or load the Codescout companion plugin. The actual Skill call returns Unknown skill: codescout-companion:reconnaissance.

## Class fit
Configured intent and effective worker capability differ at plugin discovery. This is experimental worker setup, not evidence that Recon's content reduces model quality or that every host session lacks the plugin.

## Symptom (Effect)
client-auto/request-1.json includes the actual failed Skill tool result. The emitted Skill request came from a local scripted responder; no real model decision/adherence was measured.

## Reproduction
W=/home/marius/.cache/codescout-resident/codescout/worker-context-verification-20261104; client_context_capture.py client-auto 0 low. Actual pinned Claude with isolated HOME/config, no plugin-dir. Complete request/reply/debug/child receipts retained.

## Environment
Claude2.1.309.582; loopback API responses and usage fields are fixtures, no real model or credential. Native MCP also failed separately; Skill failure is independently explicit.

## Root cause
No companion plugin was loaded into the isolated worker configuration. Files existing on the host or skills available to the root agent do not prove worker registration.

## Evidence
Actual Unknown skill result, available skill catalog, launch arguments and private settings.

## Hypotheses tried
Explicit private --plugin-dir points at the pinned companion1.20.26 package in a separate v2 replay; pending actual result.

## Fix

Explicit private --plugin-dir loads pinned companion1.20.26. Both final efforts successfully invoke Skill, and the COMPLETE Recon body, excluding YAML metadata, appears unchanged in subsequent actual serialized requests. No global profile changed; plugin hooks deliberately disabled, so hook behavior and model skill adherence remain unqualified.

## Tests added

Actual no-plugin negative Unknown skill, then two actual registered-skill/full-body positive controls. All responses scripted locally; this establishes transport only.

## Workarounds
Pin and explicitly load the intended plugin for every evaluated arm; witness actual Skill success and the body in the next serialized request.

## Resume
Verify the positive counterpart and keep equal common plugin versions across low/xhigh.

## References

[Actual client context verification and preserved evidence](../research/2026-11-04-worker-context-verification.md). The full continuation report and bundle are in the main checkout.
