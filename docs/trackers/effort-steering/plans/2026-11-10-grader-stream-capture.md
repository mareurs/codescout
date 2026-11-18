---
id: '8d672ef0b425f5c8'
kind: plan
status: active
title: Prospective grader command stream capture implementation
topic: effort-steering
---

# Prospective Grader Command Stream Capture Implementation Plan

> For agentic workers: use superpowers:executing-plans inline, task by task. The user delegated autonomous execution. Do not replace the main research objective with this instrument correction.

**Goal:** Preserve the command output needed to distinguish captured test text from measurement infrastructure faults.

**Architecture:** Opt-in complete capture for prospective live grading; preserve historical tail-only path. Strict versioned JSON/base64 evidence per command and independent print output. Policy admission follows production wiring and native qualification.

**Tech Stack:** Python stdlib, bash/coreutils in existing Rust task images, pytest.

**Spec:** [Stream provenance report](../reports/2026-11-10-grader-stream-provenance.md), section Prospective capture contract.

## Global constraints

No LLM/provider calls, private candidate regrade, F3 policy change, mutation of frozen evidence, merge or push. Worktree .worktrees/grader-stream-provenance-01a15ca9 starts at f586d35f4. Every meaningful test must first fail against absent/broken behavior; apply mutation probes on independent copies.


Python3 must already be present in every admitted task image. The actual Obscura and Lean generated-crate native controls reached real test events with the new reader; no install/global config change. Semicolon-joined rebuild evidence remains one aggregate command as in the original harness; it does not attest individual rebuild subcommand exits.

## Review focus

Raw binary/ANSI/UTF-8 data, a failed command with complete output, capture-write failure, a background child retaining output descriptors, exact-cap versus excess output, and evidence consistency must be measured through actual shell execution. A successful capture is not successful grading. Keep independent stream boundaries and do not reconstruct scopes from tails.

## Task 1 — Strict bounded command capture

Files: create tools/setup-ab-eval/command_capture.py and test_command_capture.py.

Interface: script(command, limit_bytes) returns bash source; decode(payload, returncode, limit_bytes) returns a step-capture mapping, with capture_version, stdout/stderr base64, lengths, SHA256, complete and tail. read_streams(capture) validates and returns exact stdout/stderr bytes. Malformed envelopes raise CaptureError. Version command-stream-v1. Limit positive integer; at most64MiB per stream for prospective use. The wrapper uses Python3 selectors in the task image, keeps stdout/stderr separate, bounds retained pipe bytes without limiting command artifacts, records EOF/open streams and both process/transport returncodes, and cleans its own scratch directory.

- [x] Write real-bash regressions for separated binary output and exit7; output beyond4096bytes; exact cap and over-cap; bad/missing/contradictory envelopes; scratch failure; bounded background-writer behavior.
- [x] Observe expected RED; implement minimal script/decode/read_streams.
- [x] Run the new file, then actual guard mutations and fix observed gaps.
- [x] Preserve commands, red/green outputs and source in the checked complete-capture capsule. Scoped review and root gate passed. Source remains dirty with a binary patch because publication is withheld; no commit is required to close this preservation step.

## Task 2 — Opt-in production wiring

Files: multilang_control.py, multilang_tasks.py and their tests.

Interfaces: rebuild(...,capture_streams=False), run_tests(...,capture_streams=False), live_grader(...,capture_streams=False). The true path stores complete capture for every rebuild/test/print command, preserves exact independent print parts and writes full grade evidence. False path remains historical replay behavior.

- [x] Pin consumer-visible source behavior: real command output reaches full grade.json; cap or malformed data produces error; ordering and per-print boundaries survive.
- [x] Observe RED, wire Task1 at control boundary, keep old suites green.
- [x] Qualify actual nextest 0.9.128 generated-crate capture without model calls; also measure ordinary cargo/libtest on the actual Lean image. Both owned containers removed and absence checked. This does not qualify Lean's exact dataset command.

## Task 3 — Prospective policy admission

**New observed boundary:** actual Lean metadata uses cargo/libtest, not nextest, and redirects cargo stderr to a literal file1 (`2>1`) that print_cmds does not collect. Its controlled shell probe returned0 with stdout in the report and stderr solely in file1; escaped quotes were not shown to break the exit check. Native ordinary cargo/libtest generated-crate capture is a separate format control, not qualification of this dataset command. Task3 must explicitly retain declared diagnostic artifacts with freshness/provenance, or refuse an insufficient contract; no silent command edits or candidate-selection change. Keep the frozen record and prepare identical evaluation inputs for all arms.

Files: grade_policy.py, test_grade_policy.py, measured runner fixture metadata and prospective controller integration where needed.

- [x] Derive policy inputs from complete stream evidence; refuse missing/contradictory stream proof. Tails cannot award/revoke a verdict.
- [x] Pin runner version and accepted framing; explicitly refuse ambiguous legacy/nested/unclosed provenance rather than silently pass. Preserve selected statuses and F3 decision.
- [x] Re-run actual native complete-versus-cut control, multiple commands, top-level faults after captures, UTF-8/ANSI and second-review guard cases.
- [x] Fresh strong independent review must actually apply mutations and report observed outcomes; then run the prescribed full gate and preserve all evidence.
- [ ] Update tracker readiness and return to the original cohort/controller qualification. No paid admission inferred solely from this correction.


## Task3 implementation boundary — 2026-11-10

Use a separate strict stream_evidence module and a prospective adjudicate_streams entry point. Historical adjudicate_private remains replay-only. Registration binds the exact record command lists and gives every test command a measured runner profile plus its independent print-report indices. Complete capture, exact print-to-log correspondence and matching returncodes are prerequisites. No tail reconstruction is accepted.

For measured nextest0.9.128 combined-output framing, remove a displayed test payload only when the complete structured JSON event authenticates its exact rendered content and adjacent measured header. Reject unmatched, ambiguous, legacy or unclosed capture framing. Remaining diagnostics retain their original step identity. For measured cargo/libtest --lib, captured test data is stdout/report data; compiler/host diagnostics are stderr. Do not apply this profile to doctests or --nocapture. Rustc1.97.1 is measured in the Lean generated control.

Initial boundary, now implemented locally: the exact Lean record needs file1 collection with a fresh-artifact receipt. A missing, contradictory or incomplete artifact contract is refused. This temporary refusal is not cohort admission or a substitute for finishing artifact collection. Preserve F3 and all existing frozen results. This is an approved autonomous implementation choice within Task3; no paid work follows from a module's green tests.


## Local verification and remaining handoff — 2026-11-10

Task3 implementation is locally verified:508 related tests/282subtests; independent original20 plus5new mutations all25semantically killed,0survivors/invalid; source hashes exactly match the reviewed snapshot. Stage-visible root gate exits0 in415s. Actual generated-code v4 uses literal record commands on both images, with only Lean's owned project-root substitution. Complete/cut replay and selected-pass/selected-failure/top-level-storage controls pass. The stream-policy capsule preserves source, binary patch, original failures, both review populations and all checks. No commit/merge/push or paid call.

The final handoff checkbox stays open until this source is bound into real-task base/reference/repeat evaluator qualification and the original campaign admission. F6 ordinary linker classification and the all-arm context/runtime/controller audit remain explicit. F3 is unchanged. This instrument plan does not substitute for the research objective.
