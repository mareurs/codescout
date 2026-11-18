---
id: '8965d0be731c902c'
kind: spec
status: approved
title: Executable temporary storage for native repair workflows
---

# Executable temporary storage for native repair workflows

User authorized fixing /tmp execution and increasing resource limits2026-11-06. Existing autonomous execution authorization applies. MAIN bug4d66b80cca2e6390 holds reproduced evidence.

## Design

Verified native argv must contain exactly one /tmp tmpfs with rw,exec,size=4g,uid=1000,gid=1000. Normalize inherited /tmp flags, preserve unrelated tmpfs/image/payload, reject duplicate /tmp definitions. New external entry requires32GiB memory, optional swap cap32GiB,4CPUs/init and that exact temporary policy; internal default12GiB unchanged. Extra RAM is headroom, not demonstrated OOM remediation. No host or active actor changes.

## Approaches

Explicit exec on isolated container tmpfs addresses the reproduced mechanism. RAM alone cannot fix EACCES. Excluding tests would conceal environment failures. Original frozen cohort stays on Vc1d51c446; new implementation uses /tmp/codescout-tmp-exec-01a15ca9.

## Validation

RED/GREEN tests, full lifecycle/pilot/workflow Python suites, actual generated-argv executable/doctest controls, applied mutations, required gate and independent review. Sequential unchanged-source public qualification with isolated writable state before any new paid profile. Keep unrelated failures visible and retain all old attempts/costs. No merge/push.
