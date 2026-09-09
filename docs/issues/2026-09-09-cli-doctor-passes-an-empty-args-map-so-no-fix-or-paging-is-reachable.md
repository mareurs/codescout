---
id: a06de4dfc30c2e8d
kind: bug
status: open
title: 'BUG: cli doctor passes an empty args map, so no fix mode or sample paging is reachable — and its doc comment says the scanner takes no input'
tags:
- cli
- doctor
- librarian
- cluster/doc-contradicted-by-code
closed: null
opened: 2026-09-09
owner: marius
related:
- d4b61746950b86b7
severity: low
---

> **Cluster:** `cluster/doc-contradicted-by-code` (`IC-11`, n=32, verdict *clears both
> bars as of 2026-09-04*).

## Summary

`codescout doctor` passes an **empty** args map to `doctor::call`, so none of the
scanner's seven parameters are reachable from the CLI — no `limit`/`offset` paging of the
`abs_path_outside_managed_roots` sample, and no `fix=` repair mode. The module's own doc
comment states the opposite reason for this: *"no doctor-specific args yet because the
scanner takes no input."*

## Symptom (Effect)

`src/cli/doctor.rs:57`:

```rust
let v = crate::librarian::tools::doctor::call(&ctx, Value::Object(Map::new())).await?;
```

`DoctorArgs` (`:14-27`) carries only `common` and `--fail-on-violations`. So every repair
documented in `get_guide("librarian")` § *doctor repairs* — `prune_missing`,
`reseat_worktree`, `rehome`, `repair_frontmatter_id`, `mint_slugs`,
`export_augmentations` — is MCP-only, with nothing at the CLI saying so.

The outside-roots sample is also frozen at its default window of 10
(`OUTSIDE_ROOTS_SAMPLE_DEFAULT`, `doctor.rs:751`) with no way to page past it, even though
that constant's own comment calls the window "caller-controlled" because *"a sample nobody
can look past is a report that names findings it cannot produce."* From the CLI, nobody
can look past it.

## Reproduction

```
git rev-parse HEAD                  # 4d928f2d (experiments)
cargo run -- doctor --help          # no --limit, --offset, --fix, --root
```

Then read `src/cli/doctor.rs:5` and `:57`.

## Environment

Linux 7.2.3-zen1-3-zen · branch `experiments` @ `4d928f2d` · CLI surface (not MCP) ·
project `codescout`.

## Root cause

Two facts, both current:

- `run` constructs the args inline as an empty `serde_json::Map` (`src/cli/doctor.rs:57`)
  rather than projecting `DoctorArgs` into it.
- The module doc comment (`:1-5`) asserts *"the scanner takes no input"*. `doctor::call`
  reads seven arguments: `fix`, `confirm`, `old_root`, `root`, `new_root`
  (`src/librarian/tools/doctor.rs:329-350`), `limit` and `offset` (`:757-766`).

The comment was true when written — the scanner genuinely took no input at that point —
and the arguments were added on the MCP side without the CLI wrapper being revisited. So
this is drift, not a mistake: the sentence records a fact that expired.

*measured 2026-09-09: read at `src/cli/doctor.rs:5,57` and
`src/librarian/tools/doctor.rs:329-350,757-766`. Not observed at runtime — the claim here
is about which arguments the wrapper can pass, which the source settles.*

## Evidence

`src/cli/doctor.rs:1-6`:

```
//! `cargo run -- doctor` — invoke the librarian catalog drift scanner.
//!
//! Thin CLI wrapper over `crate::librarian::tools::doctor::call`. Identical
//! discovery surface (project override, --json, --no-color); no
//! doctor-specific args yet because the scanner takes no input.
```

"Identical discovery surface" is accurate for project/format flags and misleading as a
whole, since the scanner's own surface is not identical at all.

## Hypotheses tried

1. **Hypothesis:** the fixes are deliberately MCP-only, for safety — they mutate the
   catalog, and a CLI invocation has no conversational confirmation step.
   **Verdict:** deferred, and it is a *reasonable* design. But it is not what the code
   says: every `fix=` mode is already a dry run until `confirm=true`, which is exactly the
   guard a CLI would need. If MCP-only is intended, the doc comment should say so instead
   of citing an absent input surface.

## Fix

Not yet fixed. Two candidate directions, and the choice is a design call, not a bug call:

- **Wire it:** add `--limit`, `--offset`, `--fix`, `--root`/`--old-root`, `--new-root`,
  `--confirm` to `DoctorArgs` and project them into the args map. Makes the CLI a real
  peer of the MCP surface and gives `prune_missing` a scriptable home — useful, since
  pruning a dead root is exactly the kind of maintenance run outside a session.
- **Say so:** keep the CLI read-only and rewrite `:5` to state that the repair and paging
  surfaces are MCP-only, and why.

Either way the doc comment must stop asserting the scanner takes no input. **Do not fix
this in the same commit as `d4b61746950b86b7`** (the `scope` bug) — that one adds a typed
`Args` struct to `doctor`, which changes the projection this wrapper would target, so
sequencing it second avoids rewriting the same lines twice.

**SHA:** N/A — not yet fixed.
**patch-id:** N/A — not yet fixed.

## Tests added

None. A regression test is only meaningful once the direction is chosen: wiring it makes
`doctor_cli_forwards_limit_to_the_scanner` the guard; documenting it makes the guard a
prose assertion no test should pin (per § *Testing Discipline* — pinning sentences reds on
every rewording).

Note the existing test in this file, `an_informational_only_report_does_not_trip_the_exit_1_path`
(`src/cli/doctor.rs:76`), is unaffected either way — it tests `fails_the_gate`, not
argument projection.

## Workarounds

Use the MCP surface for anything beyond a bare scan:
`librarian(action="doctor", limit=50, offset=10)` to page the outside-roots sample, and
`librarian(action="doctor", fix="…", confirm=true)` for repairs. `codescout doctor
--fail-on-violations` remains correct for its one job — CI gating on `summary.defects`.

## Resume

Ask the user which direction they want before writing either. If wiring: extend
`DoctorArgs` in `src/cli/doctor.rs:14`, build the map in `run` at `:57`, and note that
`--fix` there wants the same `enum` values as the MCP schema
(`src/librarian/tools/librarian.rs:108`) so the two surfaces cannot drift. If documenting:
rewrite `src/cli/doctor.rs:3-5` only — do not add a test.

## References

- `src/cli/doctor.rs:1-6` (the stale claim), `:14-27` (`DoctorArgs`), `:57` (the empty map)
- `src/librarian/tools/doctor.rs:329-350`, `:757-766` (the seven arguments),
  `:751` (`OUTSIDE_ROOTS_SAMPLE_DEFAULT` and its caller-controlled rationale)
- `src/librarian/tools/librarian.rs:108` (the `fix` enum), `:120` (`limit`), `:105` (`offset`)
- Sequenced after `d4b61746950b86b7`
  (`docs/issues/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md`)
