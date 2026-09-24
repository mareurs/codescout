---
id: '4cc587904a3dd44e'
kind: bug
status: open
title: 'BUG: the source-file gate reads tail inside a quoted, hyphenated filename as the tail command'
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
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

Not attempted. Likely shape: blank quoted spans before scanning for reader commands, the way the
heredoc scanners mask heredoc bodies (`mask_heredoc_bodies` in `src/util/path_security.rs`). Or
match a reader only in command position (start of a clause, or after `|`, `;`, `&&`, `$(`).

## Tests added

None yet. Owed: the refused case above must be allowed, and `tail src/tools/mod.rs` must stay
refused (the control that keeps the gate honest).

## Workarounds

Write the paths inline rather than through a variable, or pass `acknowledge_risk: true`.

## Resume

Read the reader-detection in `check_source_file_access` (`src/util/path_security.rs`), then write
the test pair first.

## References

- `docs/issues/archive/2026-09-10-source-gate-joins-an-unexpanded-var-path-onto-the-project-root.md` (sibling: same gate, `$VAR` paths)
- `docs/issues/archive/2026-09-01-source-gate-refuses-the-whole-compound-command.md` (sibling: whole-command evaluation)
