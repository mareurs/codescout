---
id: 9fdf4a786f66b976
kind: bug
status: fixed
title: 'BUG: the source-file gate reads tail inside a quoted, hyphenated filename as the tail command'
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-10-06
opened: 2026-09-24
related: []
severity: low
---

# BUG: the source-file gate reads `tail` inside a quoted, hyphenated filename as the `tail` command

## Summary

`run_command`'s shell-on-source gate (IL-3 source condition) refuses a variable assignment whose
quoted value contains a hyphenated filename with `tail` as one of its words. It treats `-` as a word
boundary inside the quoted string, reads `tail` as the content-reader command, sees an in-project
source path in the same clause, and blocks the whole command.

## Symptom (Effect)

```
run_command('P="docs/issues/x-tail-y.md src/tools/mod.rs"; echo assigned')
-> {"ok": false, "error": "shell access to source files is blocked",
    "hint": "offending clause: `P=\"docs/issues/x-tail-y.md src/tools/mod.rs\"` ..."}
```

Found on a real commit command. The list of paths to stage held
`…-stderr-tail-marker-…md`, so staging was refused until the paths were written out inline.

## Reproduction

Minimal cases, run 2026-09-24:

- `P="src/tools/mod.rs"; echo assigned` -> runs (a plain assignment is fine)
- `P="docs/research/x.md src/tools/mod.rs"; echo assigned` -> runs
- `P="docs/issues/x-tail-y.md src/tools/mod.rs"; echo assigned` -> **refused**
- `git add --dry-run -- docs/issues/archive/…-stderr-tail-marker-….md src/tools/mod.rs` -> runs.
  The same word as a command argument passes, so the defect is specific to how the assignment
  clause is scanned.

## Environment

codescout `experiments` @ `74135f61`, live release binary, Linux.

## Root cause

Not read yet (unverified). The reproduction pins the trigger: a content-reader name (`tail`, and by
the same rule `cat`, `head`, `less`, `more`, `sed`, `awk`) between `-` separators inside a quoted
assignment value, plus an in-project source path in the same clause. The scanner evidently splits
on `-` or on non-word characters without first treating the quoted value as data. This is
`CLAUDE.md` § *Parsers Over a Namespace*: a construct that means "this is data" (a quoted value) is
read as command text.

## Evidence

Refusal outputs above (session 3b4fae98-500a-4fa9-8127-b16642a8c23d, 2026-09-24).

## Hypotheses tried

- Plain assignment of a source path: not the trigger (runs).
- Multiple paths in the value: not the trigger (runs).
- `tail` as a `-`-delimited word in a filename: **trigger** (refused).

## Fix

Fixed as a side effect of #29, not by a change aimed at this bug, and pinned afterwards by a test.

- `ce41048f` (PR #29, `fix(tools): Python constant ranges, IL-3 plumbing, gate reruns, and the source gate's prefix bypass`; 10 files, `src/util/path_security.rs` +454/-57 among them) introduced `executed_command` (`src/util/path_security.rs:839`) and made the source gate classify a segment by it instead of by its raw first token. `executed_command` slices the token list at `producer_index`, which skips leading `NAME=value` assignments (`is_assignment`, from `1fb66cf6`, an ancestor, `2026-09-24`). The quoted value is therefore consumed as the assignment, and a standalone assignment leaves no command at all (so nothing reads `tail`), while an assignment prefix leaves `echo` as the head.
- Of the two shapes the bug proposed, the second (match a reader only in command position) is in effect what shipped.

Not established: that `ce41048f` alone is what flipped this repro from refused to allowed. It was identified from the commit that introduced `executed_command`, the test below and the 2026-10-05 live run, not by bisecting the repro across `ce41048f`^ and `ce41048f`.

## Tests added

`5d2dc462` (`test(path_security): a reader name inside a quoted assignment value is data, not a command`, test-only, 26 insertions), `source_gate_ignores_a_reader_name_inside_a_quoted_assignment_value` (`src/util/path_security.rs:4328`). It pins:

- both assignment shapes, `P="docs/issues/x-tail-y.md src/tools/mod.rs"; echo assigned` (standalone assignment, no command left) and `P="..." echo assigned` (assignment prefix, `echo` is the head): each must be allowed;
- the control that keeps the allows a verdict rather than a blind spot: the same value followed by a real `tail src/tools/mod.rs` stays refused.

The fixtures put the reader name and the source path into ONE token, so the old raw-first-token rule would also have passed them; the test's comment says so. Whether it was RED on `ce41048f`^ was not checked here.

## Fix provenance

- **SHA:** `ce41048f` (`experiments`)
- **patch-id:** `066083d1ea88e38ed7083576925b833c14bf39d5`
- **SHA:** `5d2dc462` (`experiments`)
- **patch-id:** `56c52827e56039977fc469fe9eac26923c05d278`

## Workarounds

Write the paths inline rather than through a variable, or pass `acknowledge_risk: true`.

## Resume

Closed on 2026-10-06. Both SHAs are on `experiments` (`git branch --contains`). Live-binary evidence: a triage agent ran this bug's exact repro on the live binary on 2026-10-05 (`P="docs/issues/x-tail-y.md src/tools/mod.rs"; echo assigned`): exit 0 with `assigned`, not a refusal. Re-run on 2026-10-06 by this pass on the binary then serving it (`target/release/codescout`, dated 2026-10-06 06:03): same result, exit 0, `assigned`.

Residual follow-ups (listed, not filed):

- Bisect the repro across `ce41048f`^ and `ce41048f` to confirm which commit flipped it (the Fix section says this was not done).
- Confirm the new test is RED on `ce41048f`^.
- The `producer_index` docstring (`src/util/path_security.rs:1548`) still says a wrapper "not named here (`ionice`, a shell function)" reads as its own name, but `ionice` is handled by `98c6d798`; fix the sentence when the file is next touched.

## References

- `docs/issues/archive/2026-09-10-source-gate-joins-an-unexpanded-var-path-onto-the-project-root.md` (sibling: same gate, `$VAR` paths)
- `docs/issues/archive/2026-09-01-source-gate-refuses-the-whole-compound-command.md` (sibling: whole-command evaluation)
