---
kind: bug
status: fixed
tags:
- cluster/declared-not-wired
- cli
- librarian
- doctor
- parity
- cross-machine
closed: 2026-09-09
opened: 2026-08-30
owner: marius
related: []
severity: medium
unverified: '`--scope` deliberately not exposed (declared omission) while d4b61746950b86b7 is open. `to_tool_args` is doctor-local, so marshalling is still duplicated per subcommand — only the guard generalises. Tests are librarian-gated and absent from the lean lane.'
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

Fixed, and by the direction this file argued for rather than the one it opened with.

`--fix`, `--confirm`, `--root`, `--old-root`, `--new-root`, `--limit` and `--offset` are
now flags on `DoctorArgs`, marshalled by a `to_tool_args` extracted for the same reason
`fails_the_gate` was: `run` opens a catalog and can end in `std::process::exit`, so nothing
about the marshalling was reachable from a test while it was inlined — and inlined, it was
`Map::new()`.

**The key-set test is what shipped, not three more flags plus a per-flag test.** This
file's second paragraph and `BL-60`'s Resume both argued for it, and the argument holds up:
a per-flag test reds when a flag breaks, whereas every one of the four instances failed in
the *other* direction — a param added to the tool and never to the CLI.
`every_scanner_param_is_reachable_from_the_cli_or_named_as_omitted` reads the field names
out of the scanner's own `struct Args` source, so the list cannot drift from the thing it
describes; a second hand-written copy would have been one more thing to drift.

**The one param left out is declared, not dropped.** `--scope` is absent because the
scanner accepts, validates and echoes `scope` while never widening the scanned population
with it (`d4b61746950b86b7`, open) — so the flag's only observable effect would be making
the report *assert* a scope it did not apply. `SCANNER_PARAMS_THE_CLI_OMITS` carries the
entry and the reason, and a test refuses an entry naming a param that no longer exists.
That list is an admission, never a pass.

**Not addressed, and named because this file is the right place for it:** `to_tool_args` is
doctor-local, so the marshalling helper this file compared against (`build_update_tool_args`
for `--force`) is still a separate implementation. The *guard* now generalises; the
marshalling does not. If a fifth instance appears on another subcommand, the key-set test
is the thing to copy — per-site, per § *Testing Discipline*'s "mutate once per guarded
SITE".

**SHA:** `953c98f3a9b16c5e9165537d9521b58c359e0b4a` (**experiments**)
**patch-id:** `8de7522768dd6dacacd293eae5d881442470422a`
## Tests added

Four, in `src/cli/doctor.rs`, all watched red first:

- `every_scanner_param_is_reachable_from_the_cli_or_named_as_omitted` — the mechanism.
  Red naming all seven: `["fix", "confirm", "root", "old_root", "new_root", "limit",
  "offset"]`, with `scope` correctly absent because its omission was already declared.
- `the_scanner_field_scan_is_not_vacuous` — rename `struct Args` and the source scan finds
  no fields, so the coverage test passes while checking nothing. This is the failure mode a
  source-scanning guard owes an answer to.
- `every_declared_omission_names_a_real_param_and_gives_a_reason` — stops the admission
  list going stale in the other direction.
- `a_set_flag_reaches_the_args_map_with_its_value` — the translation test this file asked
  for by name: "the dangerous half is a flag that parses and is dropped, because the tool
  defaults the missing key and reports success." Also pins that unset flags are ABSENT
  rather than defaulted at the CLI layer.

And the end-to-end check, because the defect was a wrapper that read correctly in source:
`--limit 1` → 1 outside row, `--limit 3` → 3, default → 10, `summary.total` steady at 166
while `shown` moves 157/159/166.

**These tests do NOT run in the lean lane** — `cli/doctor.rs` is behind
`feature = "librarian"`. Recorded so nobody reads a two-lane green as two-lane coverage.
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
- `docs/issues/archive/2026-08-28-augmentation-declaration-records-existence-not-shape.md` — BL-50,
  whose `export_augmentations` is the fix this gap strands
- `src/librarian/tools/librarian.rs` — the `fix` enum the CLI does not expose
