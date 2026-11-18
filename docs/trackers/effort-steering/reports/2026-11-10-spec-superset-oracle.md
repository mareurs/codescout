---
id: '3bda14aee18496da'
kind: report
status: active
title: Private specification oracle — Lean cache status RED/GREEN
tags:
- effort-steering
- oracle-admission
- evaluation
topic: effort-steering
---

# Private specification oracle

## Current result — 2026-11-10

The same new private behavioral oracle compiles on both frozen Lean1168 base and known reference. Base: the successful200 response/body/cache case passes;400,429,500 each fail because the second identical request returns200. Reference: all4cases pass. These are actual native assertions, not source-text matches or compilation failures. Original control durations:133.1s and122.1s. The independent review reproduced1pass/3fail and4pass respectively and returned scoped PASS:8distinct native semantic mutants killed,0survivors/invalids/timeouts. A base-derived alternative that caches successful responses only passes4/4.

## Acceptance contract

This is a **new prospective acceptance superset**, preregistered before any model repair on these tasks. Every original selected expected test remains required AND the private specification oracle must pass. Original dataset, task pair, expected arrays and failed historical admission are unchanged. The original sole Lean F2P already passes base and stays documented as nondiscriminating; this report does not relabel it or rewrite its score. Filtered oracle success alone admits neither task nor any paid pilot.

The [plan](../plans/2026-11-10-spec-superset-oracle.md) derives400/429/500 status preservation from the public cache-error requirement. Each case sends2real loopback HTTP requests through forward_request and actual OCLA cache, independently asserting literal status/body for both responses. The200case additionally checks one real upstream request. Error-only cache bypass is permitted; global cache bypass must fail. No gold-specific API shim is required. Private tests are installed only in evaluator containers after candidate source freeze; nothing is added to actor onboarding, memories or task prompts.

## Runtime and integrity

[Registration](../evidence/spec-superset-oracle-v1-20261110/registration.json) freezes oracle, helper, image and production grader source hashes. The supplemental test paths are disjoint from gold production paths, avoiding candidate erasure by test restoration. Both native controls use fresh private image layers, UID0,4CPU/8GiB,total memory+swap8GiB,capabilities dropped,init,pids4096,network none. No shared writable build cache, token or host-home mount. Exact stream capture, pre/final inspect and cgroup observations, OOM0 and actual container absence were verified. Host resource-floor qualification remains open.


## Checked oracle custody — 2026-11-10

[Closure](../evidence/spec-superset-oracle-v1-20261110/closure.json) preserves the oracle/helper/source, original RED/GREEN controls, exact90file review input,171file independent evidence manifest, all native mutation/control logs, runtime/source bindings and the17bytecode-cache additions. verification.tar.gz contains374regular members,1494519bytes,SHA256adfcced3ce816b094584874178e58ed91a6438a311b0129014c89dcd8a429f07; every member reopened byte-equal and original files stayed unchanged during sealing. Root audit independently reparsed all13native result logs, requalified13complete stream contracts, checked all171review manifest files and90registered snapshot files and freshly confirmed13container absences. This audit adds no new native model trial and does not qualify the full selected population; paid_readyfalse.

## Review and limits

One fresh strong reviewer applied8distinct production-behavior mutations: warm200, warm500, only429changed to400, corrupt warmbody, empty warmbody, global cache bypass, forced cold200 and empty coldbody. All compiled and were killed by native runtime assertions; no setup/hash refusal was counted as a kill. Global bypass passed all3error cases but failed the200upstream counter. A valid base-derived success-only-cache alternative passed4/4, proving the oracle accepts a repair strategy different from gold. Two unchanged-mutant repeats complete source-byte binding and are not additional distinct mutants. Thirteen actual executions had at most2overlap, no host binds, unique image-provided anonymous/mnt_tmp volumes, complete streams, no OOM and confirmed container absence. Volume reclamation is not claimed.

The reviewer found no new blocking, important or minor source finding in this slice. One review-hygiene deviation is preserved: initial Python imports generated17bytecode-cache files inside the review snapshot. All90registered source/evidence files stayed byte-identical; bytecode writes were then disabled. No oracle correction or second review round was needed.

Rulings on declined scopes: broader valid implementations, headers, arbitrary statuses, streaming,TTL/eviction/cache-key separation are outside the4declared fixtures and no coverage claim is made. Deliberate upstream hangs/timeout recovery remain unqualified; success-path server abort and external900sbound do not prove that recovery. The full old expected population and stable reference-repeat are the immediate next admission controls. Obscura external fixture, host resource floor,F6linker classification and exact all-arm context/runtime/controller/accounting remain blocking work under the parent plan. The existing grader stream dependency and prior product gate were not re-reviewed here; no product source changed. OriginalF2Plabel and failed old admission remain intact.

No new experiment model/provider repair was launched. Known historical charges and unresolved4USDhold remain carried by the experiment ledger. Paid admission remains false.
