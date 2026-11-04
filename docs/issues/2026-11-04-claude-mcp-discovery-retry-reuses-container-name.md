---
id: '2d04c8e1d89e0089'
kind: bug
status: mitigated
title: Claude MCP discovery retry fails on a retained fixed container name
tags:
- cluster/unclassified
- effort-steering
closed: 2026-11-04
opened: 2026-11-04
owner: marius
severity: medium
unverified: STANDING — Private diagnostic transport workaround only; no full worker/task admission, real model adherence or universal isolation guarantee.
---

# BUG: Claude MCP discovery retry reuses an occupied Docker container name

## Summary
The actual pinned Claude client sends server/discover before legacy MCP initialization. rmcp rejects that request and closes; the client respawns the configured command in legacy mode. A fixed Docker name without auto-removal remains occupied, so the retry fails and the model receives no native MCP tools.

## Class fit
A reconnect reruns a command whose resource identity was valid only once. This private experimental launcher fails at the client/server lifecycle boundary, unlike the separate old author cleanup-receipt race; direct handcrafted MCP initialization does not exercise this client negotiation.

## Symptom (Effect)
client-auto/debug.log records server-discover-probe-1, rmcp expect-initialized failure, automatic legacy retry, Docker Conflict, then CONNECTION_CLOSED. Actual serialized request advertises only Skill and says Codescout failed to connect. The CLI exits0 because a scripted local responder ends the diagnostic; that exit is not admission.

## Reproduction
W=/home/marius/.cache/codescout-resident/codescout/worker-context-verification-20261104. Original client_context_capture.py, client-auto/mcp.json, debug.log, request-0.json, child-process.json and cleanup.json preserve the failure. No real API credential or model invocation.

## Environment
Pinned Claude2.1.309.582, private packaged Codescout58c33c3064c3e08b19ff26a2b84d96f447746a09ca1547ad4867445a77dd8028, same bindgen image/runtime. Local scripted API only.

## Root cause
Static --name plus retained stopped first-attempt container conflicts with the client's supported reconnect path. Direct MCP tests never sent discovery, so did not cover it.

## Evidence
Actual complete client debug log and transmitted tool list. Original owned container checked by label/image and removed; receipt retained.

## Hypotheses tried
A separate v2 uses a unique name per command invocation and auto-removes owned stdio containers. Actual client replay is pending; source reasoning alone is not a fix verdict.

## Fix

Unique connection-specific names and --rm stdio containers now pass actual pinned-client discovery/retry in both final low and xhigh transport controls. Actual native index and memory results arrive in later requests. Two exact owned names per final client (discovery plus legacy connection) are independently absent. Original fixed-name failure remains retained; only private diagnostic configuration is mitigated.

## Tests added

Pinned real-client replay on both efforts,9serialized request records each; successful MCP results inspected, not just CLIexit0. No generic paid controller or product protocol fix.

## Workarounds
Generate connection-specific resource identities and verify every attempted owned name absent after the actual client exits.

## Resume
Run real-client replay; require native tools and actual results in subsequent serialized requests before declaring mitigation.

## References

[Actual client context verification and preserved evidence](../research/2026-11-04-worker-context-verification.md). The full continuation report and bundle are in the main checkout.
