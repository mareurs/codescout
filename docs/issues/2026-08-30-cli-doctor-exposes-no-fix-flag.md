---
status: open
opened: 2026-08-30
closed:
severity: medium
owner: marius
related: []
tags: [cli, librarian, doctor, parity, cross-machine]
kind: bug
---

# BUG: the CLI's `doctor` exposes no `--fix`, so every repair is MCP-only

## Summary

`librarian(action="doctor", fix=…)` offers six repairs. `codescout doctor` offers
none — its clap args are `--project`, `--json`, `--no-color`,
`--fail-on-violations`. Every repair in the tool is unreachable from the command
line.

This is the third instance today of one mechanism: the CLI keeps its own clap
structs and hand-marshals into the MCP tool's JSON, so a param exists on one
surface and silently not the other. See
`docs/issues/archive/2026-08-30-cli-artifact-update-has-no-force-escape-for-the-shrink-guard.md`
(`--force`) and BL-60 (`--time-scope` / `--extra`).

## Symptom (Effect)

```
$ codescout doctor --help
Options:
      --project <PROJECT>   Optional project root override (defaults to cwd)
      --json                Emit JSON to stdout
      --no-color            Force no color
      --fail-on-violations  Exit with code 1 when the scanner reports any violation
```

No `--fix`, no `--confirm`, no `--root`, no `--new-root`. The subcommand's own
help text calls it a "Read-only scan", which is accurate about the CLI and
describes only half of what `doctor` is.

## Reproduction

`codescout doctor --help` against the debug binary at `e799f29d`. Measured
2026-08-30 — run rather than read off the clap struct, on the BL-60 lesson that
"nothing to run" in a bug file is a claim rather than an instruction.

## Environment

Linux, `experiments`, codescout 0.15.0.

## Root cause

Not read at the source. **Inferred** from `--help` output and from the two
confirmed siblings: `src/cli/doctor.rs` defines `DoctorArgs` with no fix-related
fields, so the values never reach `run_fix`. The enforcement side is
surface-agnostic — `run_fix` is dispatched from the shared tool `call` — as it
was for `--force`.

## Evidence

`src/librarian/tools/librarian.rs` advertises
`fix: prune_missing | reseat_worktree | rehome | repair_frontmatter_id |
mint_slugs | export_augmentations`, each with scope and `confirm` semantics.
None of those names appears in `codescout doctor --help`.

## Hypotheses tried

None — noticed while verifying `export_augmentations` (BL-50), not investigated.

## Fix

Not implemented. Add `--fix`, `--confirm`, `--root`, `--new-root` to
`DoctorArgs` and marshal them the way `build_update_tool_args` marshals
`--force`, with a test on the translation rather than only on the parser — the
dangerous half is a flag that parses and is dropped, because the tool defaults
the missing key and reports success.

**Better than three more flags, and the reason this is worth more than its own
size:** a test asserting the CLI's marshalled key set covers the tool's `Args`
field set. That closes the mechanism instead of its fourth instance. BL-60's
Resume argues the same; two independent discoveries now point at it.

## Tests added

None — not fixed.

## Workarounds

Use the MCP tool: `librarian(action="doctor", fix=…, root=…, confirm=true)`.

## Why it matters more than a normal parity gap

`export_augmentations` (BL-50, `e799f29d`) exists to be run **on a different
machine** — the one whose catalog still holds augmentations this one lost. The
natural interface there is the shell. It is still reachable, because that
machine runs codescout as an MCP server too, so this is a usability gap and not
a dead end. But a repair designed for another host, offered only through the
host's editor session, is a fix pointed away from its own use case.

## Resume

Read `src/cli/doctor.rs`'s `DoctorArgs` and confirm the inferred cause before
building. Then decide between the four flags and the key-set coverage test
under **Fix** — they are different bets and the test is the one that scales.

## References

- `docs/issues/archive/2026-08-30-cli-artifact-update-has-no-force-escape-for-the-shrink-guard.md`
  — same mechanism, fixed at `19289b1f`; its `build_update_tool_args` is the seam to copy
- `docs/issues/2026-08-28-augmentation-declaration-records-existence-not-shape.md` — BL-50,
  whose `export_augmentations` is the fix this gap strands
- `src/librarian/tools/librarian.rs` — the `fix` enum the CLI does not expose
