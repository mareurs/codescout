---
id: cc73e99dcbe70a2a
kind: bug
status: open
title: classify()/unclassified_decls() conflate two defect shapes under one MalformedReason message
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-09-03
severity: low
---

## Summary

The `result-cap-marker-gate` gate's `classify()` function (in the SDD worktree's `tests/result_caps.rs`, feeding `src/tools/core/cap_probe.rs`) returns the same `MalformedReason` variant for two structurally different defects: a `NOT_A_CAP` annotation with no stated reason, and a `RESULT_CAP` annotation whose id fails the grammar `[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*`. `unclassified_decls` renders both under one message — `"— NOT_A_CAP with no reason"` — so the second case's error text names a token that is nowhere on the annotated line and prescribes the wrong repair.

## Symptom (Effect)

Given a source line `// cap-class: RESULT_CAP probed` (a malformed id — no dot), the gate reports:

```
src/tools/grep.rs:782 MAX_MATCH_BYTES — NOT_A_CAP with no reason
```

The constant is annotated `RESULT_CAP`, not `NOT_A_CAP`, and the fix is to correct the id's grammar, not to add a reason to a `NOT_A_CAP` token that was never written.

## Reproduction

1. Check out the `result-cap-marker-gate` worktree at commit `2a32c043` (or later, once merged).
2. In any tracked `src/` file, change a `cap-class: RESULT_CAP <valid.id>` annotation to `cap-class: RESULT_CAP probed` (an id with no `.`).
3. Run `cargo test --test result_caps unclassified_decls` (or the whole-file `cargo test --test result_caps`).
4. Observe the reported message names `NOT_A_CAP with no reason` for a line that says `RESULT_CAP`.
5. `git checkout -- <file>` to revert.

Reproduced live during Task 7 of the `result-cap-marker-gate` SDD run (2026-09-03), driving the real gate: `src/tools/grep.rs:781` mutated `RESULT_CAP` → `RESULT_CAP probed`, observed message `src/tools/grep.rs:782 MAX_MATCH_BYTES — NOT_A_CAP with no reason`.

## Environment

`result-cap-marker-gate` branch/worktree, `tests/result_caps.rs`'s `classify()` and `unclassified_decls()` functions.

## Root cause

`classify()` returns `CapClass::MalformedReason` for two distinct input shapes: (a) a `NOT_A_CAP` token with an empty or absent reason string, and (b) a `RESULT_CAP` token whose id does not satisfy `is_valid_cap_id`. `unclassified_decls()` renders every `MalformedReason` value under one fixed message string, `"NOT_A_CAP with no reason"`, with no branch on which shape produced it.

*Read from `tests/result_caps.rs` at `result-cap-marker-gate` HEAD `2a32c043` — not yet cross-checked against an even later commit if the branch has moved since filing.*

## Evidence

Observed live gate output (Task 7, 2026-09-03), quoted verbatim by the implementing agent:

```
src/tools/grep.rs:782 MAX_MATCH_BYTES — NOT_A_CAP with no reason
```

for a mutation that read `// cap-class: RESULT_CAP probed` above `MAX_MATCH_BYTES` at `src/tools/grep.rs:781`.

## Hypotheses tried

1. **Hypothesis:** the two cases are rare enough in practice that a shared message is harmless. **Test:** none run — this is a message-quality bug, not a correctness bug (the gate still reds in both cases; it just misdirects the reader). **Verdict:** not tested; filed as low-severity on that basis.

## Fix

Not designed. `classify()` should return distinguishable variants (or `MalformedReason` should carry which sub-case fired), and `unclassified_decls()` should render a message naming the actual annotation token found (`NOT_A_CAP` vs `RESULT_CAP`) and the actual defect (missing reason vs malformed id).

## Tests added

None yet. A fix should add a case distinguishing the two `MalformedReason` sub-shapes and asserting the message names the correct token in each.

## Workarounds

A reader hitting this message should check the actual annotation line rather than trusting the message's stated token.

## Resume

1. Once `result-cap-marker-gate` merges, locate `classify()` / `unclassified_decls()` in `tests/result_caps.rs` and split the shared variant/message.
2. Add a regression test per sub-case.

## References

- `tests/result_caps.rs` (`classify`, `unclassified_decls`) at `result-cap-marker-gate` branch, worktree `.worktrees/result-cap-marker-gate`
- Found during Task 7 of the branch's SDD run, session ledger `.superpowers/sdd/2026-09-02-result-cap-marker-gate/progress.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` (the gate this bug lives inside)

