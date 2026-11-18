---
id: '7b0356b28ee5e3ed'
kind: report
status: active
title: Full Lean original population plus private specification oracle controls
tags:
- effort-steering
- evaluation
- oracle-admission
topic: effort-steering
---

# Full Lean acceptance-superset controls

## Current native observations — 2026-11-10

The unchanged full original selected Lean population now runs alongside the4reviewed private specification tests under the common init runtime. Base:8591expected,3notpassing,0missing,433.7s; the only failures are new400/429/500warm-status checks. All8587original selected statuses and new200cachepositive pass. Reference:8591expected,0notpassing,0missing,435.3s; rebuild,test,print all0. Both complete-stream qualifications and exact final original-reference/base production+oracle source hash checks pass. Containers removed and fresh absence observed. Reference-repeat:8591/8591pass,0missing,414.2s; selected status map is byte-for-byte identical as parsed to the first reference. All3controls have complete streams,correct final source bytes,no OOM,registered resource/init/networklimits and fresh container absence.

[Registration](../evidence/full-lean-superset-controls-v2-20261110/registration.json) preserves all8587original names/labels and literal rebuild/test/print commands. The4new checks append to the old population; original dataset bytes and failed earlier admission remain immutable. A base must pass original tests and200cachepositive but fail400/429/500warm statuses; reference/repeat must pass all8591with identical maps. UID0,4CPU/8GiB/swap8GiB,init,networknone,capdropALL,pids4096,900sperstep. Exact original redirected rust/1 is captured as diagnostic evidence. Private tests are evaluator-only.

## Binding correction before controls

The first registration attempt stopped before any native run because reviewer-reconstructed reference patch bytes differ from the original benchmark patch. [Issue](../../../issues/2026-11-10-full-control-reference-patch-byte-identity-assumption.md) retains the failed helper and local correction. V2 computes expected source hashes by applying the ORIGINAL reference patch to isolated copies, then compares actual final bytes inside every evaluator before removal. Both distinct patch digests are recorded; no identity is inferred from passing behavior.

## Checked closure and next action

[Closure](../evidence/full-lean-superset-controls-v2-20261110/closure.json) confirms every original expected name,unchanged literal commands,3newerrorfailures onbase and all8591pass onreference/repeat. Root audit reparsed every log,requalified complete streams,checked actual source/limits/init/no-host-binds/OOM0 and fresh absence for all3containers. verification.tar.gz has99regular members,2844515bytes,SHA256a052683ec061ada7d6f283b9d47e215cf322f0b3cbd6709a45a41d6f4d403fde; all members reopened byte-equal and original files unchanged during seal. It includes the refused v1registration and exact original-reference host binding.

Lean's native negative/positive/repeat predicates are satisfied under this new acceptance version. This does not qualify the Obscura external dependency and newly observed source-image mismatch,host resource floor,F6linker classification or exact all-arm/context/controller/accounting. No new experiment model repair or paid admission; parent Task4 remains open. The original label mismatch and old failed admission remain preserved; no score is rewritten.
