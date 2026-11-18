---
id: ca13bb39b05e0bed
kind: research
status: done
title: Public ledger nondeterminism — fixed baseline and candidate matrix
tags:
- effort-steering
- public-test-validity
- offline-diagnostic
topic: effort-steering
---

# Public ledger nondeterminism — 2026-11-11

The fixed public-only matrix reproduces the exact recorded ledger assertion on the frozen P candidate. The same candidate also passes other default-mode processes. Both serial candidate processes pass. This establishes nondeterminism in the measured candidate/public-test setup; it does not establish a deterministic functional regression, a baseline failure rate or a permanent source fix. The primary eight-attempt results stay unchanged.

## Fixed experiment and actual outcomes

Registered before these outcomes: baseline order default/serial/default/serial/default; candidate order serial/default/serial/default/default. Each source gets3default and2serial suite processes; no additional retries. Same immutable b72f902d image and cargo-v2 profile(4CPU/8GiB, executable scratch, networknone, read-only source). Compilation is reused within each source container; all test suites run as separate processes. Default means the original libtest command without an assumed thread count. Serial adds only -- --test-threads=1.

| Source | Mode | Suite processes passed | Suite processes failed | Tests per suite |
|---|---|---:|---:|---:|
| Baseline | default | 3 | 0 | 222 |
| Baseline | serial | 2 | 0 | 222 |
| Frozen P candidate | default | 2 | 1 | 223 |
| Frozen P candidate | serial | 2 | 0 | 223 |

The single failure is delegates_to_verified_ledger, observed event.tool proxy_response_optimizer versus expected ocla_savings. That process returns101 and reports222pass/1fail. Both source-level diagnostic containers return0 because they successfully collect all scheduled inner outcomes, including the failed suite. Outer success does not overwrite an inner failure. Raw logs, process commands/exit codes, container inspections/resources and verified cleanup are retained. There were no model/provider calls; root/CPU/storage costs are not claimed zero.

An independent audit applies the already qualified frozen record parser to every raw stdout. All10status maps agree with the producer's reported pass/fail/skip counts and failed-test names. This calibration is on these actual logs, not a claim that either parser handles arbitrary output. Both original source digests, all192bound campaign inputs and all eight captured candidates are unchanged before and after. The paid controller is not rerun and no private verdict is altered.

## Source mechanism and attribution limit

The ledger test file is byte-identical in baseline and P candidate(SHA25625647e01c120d763dd3a4369064bc75b3a7908d6730e6a05dc6d718e98ea38b8). It uses isolated_data_dir, which holds test_env_lock while temporarily changing process-global data-dir environment, then reads the first ledger line and assumes it belongs to its own ocla_savings write.

Three builtin response-optimizer tests call optimize_response without acquiring that lock. The adapter delegates to proxy::response_optimizer::optimize_response, whose record_response_measurement obtains the ledger path from the same process-global configuration and appends proxy_response_optimizer. The correctly locked proxy-specific measurement test does not protect those other callers. This is a concrete noncooperating-writer path consistent with the observed foreign event.

No instrumentation identifies the exact concurrent test invocation responsible for the failed historical write. The baseline passes all five bounded controls. Therefore do not claim a reproduced baseline failure, prove the candidate cannot affect scheduling, or infer a failure probability from1/3. A forced baseline interleaving would be additional causal evidence, not another favorable retry of the primary candidate. Serial execution is a measured prospective workaround on this source, not proof of universal race absence. A future experiment must freeze any common serialization policy for every arm and requalify that changed runtime boundary before paid calls.

## Evidence and closure

Original diagnostic: /var/tmp/ep11-01a15/public-ledger-diagnosis-v1. Jobs bg77(matrix) and bg7c(audit/archive) both exit0. [Independent audit](../evidence/public-ledger-diagnosis-20261111/audit.json), SHA256b31008d066d87b0d1cdc869cf366baa11118eace8588b45211aff0d7ab5ba1eb.

Capsule ../evidence/public-ledger-diagnosis-20261111/evidence.tar.gz contains both complete actual input snapshots, all10inner stdout/stderr logs, registration, requests, two runtime receipts/inspections/samples, qualified parser, audit and exact helper sources. All6072selected payload entries were reopened/hash-checked, and originals matched at closure. Size29137865bytes, SHA25621eb2a0a69fd1b199f994425c7140201c8bc9cee6905b3f2574a9a8c4150e5f6; independent GNU digest agrees. Compiler caches and duplicate staging sources are omitted explicitly and remain untouched. External image/earlier original-source custody stays referenced; symlink targets remain literal. No deletion, paid run, source fix or grade reclassification.

Issue9e4d2a12f64644cc retains the original failure and attribution caveats. [Closed primary pilot](2026-11-11-closed-generic-repair-pilot.md) remains NO-GO. This investigation resolves why an all-pass declaration can coexist with a later real failing replay without fabrication, but does not turn the evidence-gated mechanism into an effective repair intervention.
