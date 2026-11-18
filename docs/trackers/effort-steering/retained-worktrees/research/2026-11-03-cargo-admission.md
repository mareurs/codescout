---
id: f040268a1c713e62
kind: research-note
status: archived
title: 'Historical: Cargo operation admission — bounded controls and matched pilot handoff'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-11-03-cargo-admission.md
source_body_sha256: cf72d8bbb26094d2484161604477e5764253b5bc213a41c6a38864c54f594a89
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-11-03-cargo-admission.md
  sha256: '802bff2c13e04ab0a14dbf0e1759b78b681eeb0830065f698e76c4e5914d9c89'
  body_sha256: cf72d8bbb26094d2484161604477e5764253b5bc213a41c6a38864c54f594a89
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-11-03-cargo-admission.md
  sha256: '802bff2c13e04ab0a14dbf0e1759b78b681eeb0830065f698e76c4e5914d9c89'
  body_sha256: cf72d8bbb26094d2484161604477e5764253b5bc213a41c6a38864c54f594a89
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-11-03-cargo-admission.md

---


# Cargo operation admission — 2026-11-03

## Result

The new private supervised Cargo route passes the tested operation/cleanup/compile-diagnostic admission controls. It is not yet a paid campaign launcher. No new paid model call, low/xhigh quality estimate or general promotion follows.

Evidence root A=/home/marius/.cache/codescout-resident/codescout/cargo-admission-20261103. W/P/Q and all earlier sealed evidence remain unchanged. Worktree V=/home/marius/.codex/worktrees/615e/codescout retained; root Session-Id codex:01a15ca9-f939-7603-acb5-01c6b02f68e2. No push/merge.

## What changed

A hash-provenance copy of W adds supervisor.py. A separate child process runs the registered operation; the parent bounds its wait, kills its owned process group and reaps the direct child. Each Docker/Git invocation has a finite deadline. Four effective controller timeouts must be positive finite numbers; bool/null/zero/negative/NaN/infinity/strings are rejected before evidence or process creation.

Containers get a UUID name and invocation label before create. Cleanup requires the recorded name/label/image binding and verifies exact-ID absence. A timed-out create with no returned completion remains unresolved even if inspection reports no object. Failed/unresolved cleanup, timeout/cancellation or child binding failure cannot produce runtime_valid true.

Public build failure now returns actual build.log content and full SHA/bytes, status compile (or timeout), phase build and the actual exit code. The complete feedback retains the6000-byte UTF-8 serialized budget. No hidden cases become feedback. Valid original/public/source/container semantics remain those of W.

## Observed controls and tests

- Five missing-supervisor tests first failed assertions. Initial setup errors and failed attempts remain in A; missing-module errors are not RED evidence.
- Expanded nine real supervisor controls passed: ordinary Cargo, Cargo timeout, setup fault, cleanup uncertainty, invalid arguments, cancellation, whole-operation deadline, pending create and foreign ownership.
- Actual pinned CLI plus loopback dummy API passed success and setup-fault controls. A first1s control timed out during create instead of start and correctly returned unresolved; raising only that test's setup budget to5s reached the intended start fault.
- Actual exposed Nushell invalid source initially failed status fail!=compile after111.687s. Diagnostic selection fix passed117.338s; after the deadline fix, a fresh actual CLI compile control passed109.919s.
- Final explicit focused command passed20distinct unittest methods in38.606s. Earlier post-fix default loading executed29invocations but only20distinct IDs because an imported TestCase was rediscovered; suite-population.json records the full method-ID census. Final/mutation commands select test_fix.FixTests explicitly. The compile test is a separate real control, not an invented combined21test run.
- Project gate exited0 after784s: PYTHON_LIGHT/FMT/CLIPPY/LEAN/DEFAULT all0. Heavy Python, server-stack and two local ONNX exclusions remain; librarian is covered in DEFAULT. Gate started before the private final fix pass; no repository runtime code changed, and final private controls/replays separately validate it.

## Fresh review and one fix pass

One gpt-6-astra/high reviewer, review/review.md. Baseline13tests passed. Eight actual mutations produced3assertion-only kills,5survivors,0mixed/error-only failures and0outer timeouts. Original source hashes were restored. Review separately reproduced the null cleanup deadline defect on unchanged source with a synthetic sleeping Docker executable under a2s independent bound; no real daemon outage is claimed.

Author reapplied all five survivors to the original suite and observed all five survive BEFORE adding covering tests. The one fix pass:
- rejects invalid effective deadline configuration (author RED ValueError-not-raised then GREEN);
- tests image independently of label;
- tests successful-but-no-op removal;
- observes actual shim PID quiescence after timeout and cancellation;
- tests real Cargo changing its source;
- tests actual CLI large invalid-UTF8 output, full log provenance and parseable bounded feedback.

Exact eight deltas then replayed one at a time: **8assertion-only kills,0survivors,0test errors,0outer timeouts**, exact source restores. Each replay ran20methods. mutation-summary.json and author-after/mutations.json retain counts, diffs, commands and full logs. No second review. Selected mutations do not establish exhaustive source/property/signal-race coverage.

## Independent receipt and resource audit

audit.py reparses actual API declarations/tool identity/input/result, MCP request/response correlation, successful CLI final envelope, controller result, source/image/network/mount binding, phase/exit/log SHA/length and inline wire bytes. Three named controls agree. Selected build failure:12142full-log bytes,4226model-text bytes, exit101/phasebuild. Selected success is the invalid-UTF8 control:18529raw bytes,5935model-text bytes. Two separate actual receipt-corruption controls reject an added Bash declaration and changed MCP result by assertion.

resources.py checks17declared external file bindings, immutable image,431distinct IDs from retained physical identity receipts and421recorded invocation names, all absent at final check. This is a declared subset/current absence check, not a complete transitive/platform/global resource census.

The final name census found the container from the initially uncertain create: initially absent on cleanup inspection, later present. Exact label/image/ID binding was checked; follow-up removed only85ecb2a209382373d161419dc392c2c512061b4a01423637a61ef7cf7f82c518 and verified its absence. Original invocation stays infra/runtime_valid false/unresolved. See resource-first-attempt.json and resource-followup; session log F-9.

Historical resource limitation: the reviewer explicitly identifies nine early foreign-fixture executions whose IDs were not persisted; some early author fixtures also predate ID recording. Successful teardown or an empty prefix listing is not retrospective exact-ID proof. New fixtures persist IDs before assertions. No broad prune, peer deletion or image deletion occurred.

## Prospective experiment handoff

[Matched pilot draft](../superpowers/specs/2026-11-03-matched-cargo-pilot.md): two external tasks, low/xhigh with identical public checking and at most one repair each, order LX/XL, complete initial/repair/failed-call cost, independent hidden acceptance and at most$8reserved for four workflows. It requires actual settled-ledger/cap enforcement and request-overrun reserve before payment; the draft is not a launch registration.

The calibrated metadata census has29accessible bundle paths versus22nofollow paths; GNU find -L equals the Python accessible set. Seven linked task directories explain the difference (F-8). No count of unseen tasks is inferred. Named historical-manifest scan finds a Ruff15309 exposure/registration lead that the narrow literal-memory scan missed. Conservative metadata shortlist: Obscura477/414/412, rust-analyzer22789, Obscura447 in fixed hash order. All five remain candidates, not admitted tasks. No new gold/model outcomes were inspected to choose them.

Next: resolve exposure provenance, calibrate two eligible independent oracles/offline builds, generalize task-bound public validation beyond exposed N, integrate exact launcher/effort/tool audit and serialized budget reservation, then freeze/preflight the four slots. Do not replay completed Cargo/N controls unless a new change or concrete failure requires it. Stop before payment if fewer than two eligible cases remain.

Client cumulative25.340899699999999810 of40; remaining14.659100300000000190. All new CLI calls used loopback/dummy credentials; synthetic usage excluded. Native/review/CPU/invoices and global provider/network population are unmeasured.

## Rulings and limits

- Retain native ledger/external evidence rather than delete: research preservation; cost metadata/disk.
- Test setup budget1→5s only after unintended real create timeout: reaches the intended start fault; cost a few test seconds, no production deadline relaxation.
- Grade copied post-source/wire-budget gaps Important despite review's Minor label: regressions compromise the new campaign; cost two focused controls. No deferred Minor remains.
- Review set aside efficacy/spend/Task3: runtime success grants none; draft remains gated. Full repository gate and sealed roots are separately scoped. Existing compile receipts were audited by reviewer; author later ran the fresh final compile. Arbitrary OS/real indefinite daemon/signal races beyond these controls remain unmeasured.
- Missing historical foreign IDs cannot be reconstructed from an evicted event ring. Retain that limitation; do not replace it with a global cleanup claim.
- This plan's native workspace remains retained by standing user authorization. The publication will bind historical document bytes; later roadmap/metadata edits do not rewrite the evidence seal.

## Publication and closure

Local publication bd8f83b524e8b111f0ab1c4d6aebc4dc71c1b2d5, patch-id7636c66520ad6389ff13e6dd92f4bf8cec5f2b71 binds10historical document paths. Complete calibrated evidence seal SHA256 b91e71ca60af994ec92a91e03b4a0372caa67e4cc9a5600df1fbadf951d6a68a covers23054regular file paths/0literal symlinks, Python inventory equal to GNU find/sha256sum. close.py record returned verified-closed after historical publication, selected API/MCP/process audit and declared resource checks.

Canonical: python3 -B /home/marius/.cache/codescout-resident/codescout/cargo-admission-20261103/close.py verify. Only repeat-verification.log is mutable. No CLI/test/mutation writers or LSP in A or older sealed roots. Later document completion/roadmap edits do not change the historical publication binding. Native V ledger retained; A/native-ledger is its preclosure checkpoint. Precommit dead-artifact-ids explicitly skipped because no runnable codescout binary was found in V's hook lookup paths; other refusing hooks passed. No push/merge or new paid job.
