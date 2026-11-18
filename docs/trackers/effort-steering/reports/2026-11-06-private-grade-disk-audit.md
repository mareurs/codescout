---
id: ec45f528302fb06b
kind: research
status: done
title: External private grade and disk-pressure validity audit
tags:
- effort-steering
- evaluation
- infrastructure
---

# External private grade and disk-pressure validity audit

**Valid:** dated 2026-11-06

## Conclusion

The original four raw booleans are reproducible but cannot be interpreted as two valid passes and two model failures. Original Nushell/low did not run its tests: its linker crashed during a confirmed host ENOSPC window. A registered same-patch replay has since run all selected tests and found both acceptance failures. Nushell/xhigh has one genuine selected-suite regression and one parser artifact. Both rust-analyzer candidates pass every selected expected test. All four original public workflows remain incomplete. This cohort does not establish a generic low-to-xhigh mechanism.

## Audited outcomes

| Task and effort | Selected expected outcomes | Correct interpretation |
|---|---|---|
| Nushell low | 8622 absent statuses; zero observed test statuses; linker SIGBUS | Original infrastructure-invalid; same-patch diagnostic now observes both F2P fail and all8620P2P pass |
| Nushell xhigh | 8620 parsed expected passes, one expected assertion failure, one parser-missed actual pass; both F2P pass | Genuine regression remains after parsing artifact is accounted for |
| Rust-analyzer low | All8182 expected names observed pass | Supported selected-suite patch pass |
| Rust-analyzer xhigh | All8182 expected names observed pass | Supported selected-suite patch pass |

Numbers above enumerate each frozen expected-name population against actual record-parser outputs. Extra unregistered/skipped names are not substituted for expected coverage. No original output was rewritten. All four raw booleans/counts reparse exactly; all recorded grade steps have no truncation flags.

## Disk and environment evidence

System journal reports host ENOSPC and simultaneous rust-lld SIGBUS at09:51:20 local. The Rust1.95.0 crashed linker PID434016 belongs to Docker cgroup0cdda52eccee77301892550ddae51c6df0b6e794f8fdeff8e4930d414de2b0a7; its09:38–09:52 lifecycle aligns with the exact named Nushell/low grader and09:52:44/47 grade artifact mtimes. The scope also reached8GiB memory peak. This establishes overlap with actual storage exhaustion, not a disk-only causal isolation. No literal ENOSPC in a test log is insufficient to exclude it.

The actor's temporary-execution failure is separately established by same-image noexec/exec paired executable and doctest controls: issue4d66b80cca2e6390. Original private grading used a different profile, without the actor tmpfs. Regrading a patch cannot retroactively repair the original public workflow or its wasted continuations.

## Independent verification

Reviewer /root/tmp_exec_review reproduced all four verdicts, enumerated complete F2P/P2P populations, and ran14 actual parser input perturbations plus a copied-whole-log newline control. Inserting only the missing newline in the Nushell/xhigh copied log changes missing1→0/not_passing2→1 while pass remainsfalse. The genuine mixed_str_length_result_does_replace_last assertion is String("ab") vs Int(2). Both acceptance tests pass. Parser weakening is not used to override raw results.

## Journal and replay review closure

Separate independent supplements rechecked both raw acceptance assertions and all 70 resource samples. All 53 registered source hashes, candidate, bundle, record, original results and arm receipt match. The reviewer initially compared the registered arm receipt hash against a different nested grade file; its incorrect intermediate result and explicit path correction are both preserved. Final corrected integrity checks pass.

Independent journal queries additionally link the ENOSPC compression error to the same coredump handler that recorded the Rust1.95.0 SIGBUS. Association of Docker scope ID with the original named grader is an inference from adjacent start/name events and matching runtime/artifact times, not a direct single-field mapping. The exact cause of the linker SIGBUS remains unisolated.

## Diagnostic replay

One preregistered zero-provider replay CLOSED: @bg_000000f7 exited0,704.5s. Exact same captured Nushell/low patch, original pinned image/record/parser/source,4CPU/8GiB. It now links and emits every selected expected result: both2F2P fail; all8620P2P pass; missing0. This supplies a real rejection of the captured patch separately from the original infrastructure-invalid receipt. It does not relabel the original failed execution.

The70 resource samples (68 successful container probes, every~10s) show minimum sampled root availability322369671168bytes (~300GiB), no sampled cgroupOOM/oom_kill, and memory peak8589938688bytes at the8GiB limit. The max memory-limit event counter857457 indicates limit pressure, not857457OOMs. Full raw log has zero matches for the declared disk/linker-crash signature scan. The raw assertions, all expected identities, cgroup samples and audited-outcome.json are preserved. Source/candidate/original receipt hashes unchanged; removed container independently absent. No provider/model calls.

Corrected conclusion: the old Nushell/low grade was invalid during disk pressure, and the independent same-patch diagnostic still rejects its repair. The two original passes remain supported; the two rejected candidates fail for different observed reasons (low leaves both acceptance failures; xhigh repairs them but introduces a selected regression). This coincidence in aggregate pass/fail counts does not validate the old measurement process.

## Evidence and issues

Frozen original cohort: /home/marius/.cache/codescout-resident/codescout/external-matched-repairs-20261105. Derived audit/replay: sibling external-grade-audit-20261106. Independent audits were closed before archival; original reports and receipts remain unchanged.

Durable closure archive: [2026-11-06-private-grade-disk-audit-evidence.tar.gz](../../../research/data/2026-11-06-private-grade-disk-audit-evidence.tar.gz), SHA256 `89dfc97b0d7a901d4da3f37a9beb5576e7fe7d36f319cfdce8ddbf986367f518`, 33,232,147 bytes, 1,600 payload files plus manifest. Every archived file was reopened and checked against its exact source hash. It includes the four original terminal receipts/candidates/model streams/public/background/private logs; independent parser, replay, source-guard and journal reviews; all 53 bound grader source files; replay telemetry; and helpers. Writable homes, semantic/model caches, image layers and unfinished v3 qualification are excluded. A documented synthetic NOTAREALTOKEN test constant is retained byte-for-byte. Original results.json remains SHA256 `e28e9eac4c326d6bebe4109336cd1594a712cd3374b041d41ad299ff602059da`.

Filed immediately in MAIN: [linker/no-test conflated with candidate failure](../../../issues/2026-11-06-private-grader-linker-crash-counted-as-model-failure.md), [concatenated Rust status missed](../../../issues/2026-11-06-rust-grader-parser-misses-concatenated-test-status.md). Source-guard issue1720ba9339aa9ad6 and prefix-control coverage issue5d69759479e75de7 concern prospective environment qualification. The grader-classification and parser production defects remain open; this audit corrects interpretation rather than silently changing historical grading.

## Roadmap

1. DONE: closed the one same-patch replay and separated semantic rejection from the original infrastructure-invalid grade.
2. DONE: preserve original cohort closure, independent audit and replay with hashes; update current research interpretation.
3. DONE: zero-provider unchanged-baseline public checks @bg_000000fd, ROOT external-exec-baseline-20261106-v3, corrected32GiB RAM/executable4GiBtmpfs. Both projects pass all registered public commands. Exact startup.gitignore append is admitted by a narrow tested guard; other tracked/index/mode/HEAD changes are rejected.
4. Before admitting new paid repairs, enforce the documented grader distinction between complete semantic outcomes, linker/OOM/storage infrastructure faults and parser-missing outcomes; requalify its positive/negative controls. Environment qualification alone does not close these two grader bugs. Reuse existing once-per-project onboarding. New task claims require additional matched evidence; this audit does not supply it. These two task outcomes and private assertions are now visible to the controller: further tuning on them is development evidence, not fresh held-out validation.

## Corrected native environment checkpoint

Zero-provider v3 qualification CLOSED successfully: @bg_000000fd exited0. Both exact unchanged image baselines pass their original public build and test commands, with no new skip:

| Task | Build | Public test command | Test wall time |
|---|---|---|---|
| Nushell18863 | exit0 | Original workspace command, retaining only its preregistered plugin-persistence skip: exit0 | 518.59s |
| Rust-analyzer22992 | exit0 | Original RUN_SLOW_TESTS=1 command: exit0 | 176.00s |

Both generated runtime executable/doctest probes passed. Actual containers use32GiBRAM and executable4GiBtmpfs. Before/after guards are identical on each task: originalHEAD, no staged/mode/othertracked changes, sole exact startup.gitignore append. Both containers removed and independently absent again at closure. Commands/population and source/helper hashes verified against registration. This qualifies the corrected environment profile; it does not retrofit the old model workflows or isolate RAM versus tmpfs effects.

Durable qualification archive: [2026-11-06-executable-native-baseline-evidence.tar.gz](../../../research/data/2026-11-06-executable-native-baseline-evidence.tar.gz), SHA256 `a6f224468b952c814988dc784f1bf036d02ad71aea1ba22ab3408b0bfc405f38`, 211,918 bytes, 44 payload files plus manifest; all reopened/hash-verified. Includes full public logs, guards/resources/cleanup, helper registrations and all five proposed lifecycle source/test/doc files. No new LLM/provider calls, commit, merge or push.
