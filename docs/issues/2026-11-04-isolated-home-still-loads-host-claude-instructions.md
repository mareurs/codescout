---
id: a68a473b0d74dd0d
kind: bug
status: mitigated
title: Isolated HOME does not isolate Claude instruction discovery from host profile
tags:
- cluster/unclassified
- effort-steering
closed: 2026-11-04
opened: 2026-11-04
owner: marius
severity: medium
unverified: STANDING — Private diagnostic transport workaround only; no full worker/task admission, real model adherence or universal isolation guarantee.
---

# BUG: Isolated HOME still loads host Claude instructions

## Summary
The actual client request contains an additional Contents of /home/marius/.claude/CLAUDE.md header despite HOME and CLAUDE_CONFIG_DIR pointing to a private diagnostic profile.

## Class fit
An environment-variable boundary is mistaken for complete prompt-source isolation. This demonstrated instruction-source inheritance is separate from credential access or an actual oracle leak, neither of which is established by the observation.

## Symptom (Effect)
client-auto/request-0.json has three instruction-source headers: isolated global snapshot, host /home/marius/.claude/CLAUDE.md, and actor/project/CLAUDE.md. Host settings, skills and instruction bytes must be treated as effective inputs until isolated or pinned.

## Reproduction
W=/home/marius/.cache/codescout-resident/codescout/worker-context-verification-20261104. client-auto/launch.json pins environment/cwd; request-0.json carries actual serialized context. Diagnostic uses no real credential/model.

## Environment
Pinned Claude2.1.309.582 on host, cwd under /home/marius/.cache. Built-in client modules still run when disableAllHooks=true. No change to any real profile.

## Root cause
Exact loader cause remains untraced. Measured fact: private HOME/config alone does not exclude host-global instruction discovery in this launch. Do not infer that removing only one ancestor path fixes every source.

## Evidence
Complete actual request with file-source headers and declared child environment. Visible global instruction lines are present after HTML comments are stripped; a separate audit false negative is not missing policy.

## Hypotheses tried
None yet. A container-isolated evaluated client or an explicit complete effective-source policy needs actual controls.

## Fix

Final diagnostic client CWDs are fresh /tmp Git-bound clones outside the host-home ancestor chain. In BOTH actual initial request payloads, the entire instruction-source header population has exactly two members: the private global snapshot and that clone's project CLAUDE.md. The extra /home/marius/.claude/CLAUDE.md header is absent. Bounded launch/config workaround only; no all-version/adversarial host isolation guarantee.

## Tests added

Original inherited-host negative plus two actual final source-header populations, visible global/project body checks and an existing but unmounted host-canary read failure. Native host Read/Bash tools are not advertised. This is not a complete runtime authority or oracle-isolation audit.

## Workarounds
Treat existing host-run diagnostic as context characterization, not admitted worker isolation. Do not start quality trials from private HOME alone.

## Resume
Bound inherited sources and compare positive/negative isolation controls before paid comparison.

## References

[Actual client context verification and preserved evidence](../research/2026-11-04-worker-context-verification.md). The full continuation report and bundle are in the main checkout.
