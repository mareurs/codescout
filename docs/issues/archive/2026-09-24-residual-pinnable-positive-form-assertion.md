---
id: c72461fd60b38b34
kind: bug
status: fixed
title: 'RESIDUAL: Build the positive-form assertion: every name listed in the pinnable check is produced by a registered tool'
tags:
- cluster/assertion-that-cannot-fail
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-01-pinnable-assertion-vacuous-for-an-unregistered-tool.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-01-pinnable-assertion-vacuous-for-an-unregistered-tool.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Build the positive-form assertion: every name listed in the pinnable check is produced by a registered tool.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-01-pinnable-assertion-vacuous-for-an-unregistered-tool.md` (status `fixed`):

> No regression test for the vacuity itself. `tests/tool_reachability.rs` closes the enabling condition (an unregistered `impl Tool`) but not the shape — an assertion naming a string no tool produces is vacuous by the same mechanism with no unregistered type involved. The positive form (each listed name IS produced by a registered tool, then is absent from `pinnable`) is not built. The fix was also incidental: the subject was deleted, nothing diagnosed the vacuity.

## Fix

Built the positive form the parent's caveat named, in the `src/server.rs` tests only (no production code changed). `pinnable_tools_advertise_workspace_param` now asserts that each name it guards (`workspace`, `get_guide`) is a registered tool before asserting that it is absent from `pinnable`, so renaming either tool reds the test instead of leaving it vacuous. A second test, `every_pinnable_exclusion_names_a_registered_tool_or_a_named_exemption`, reads the string literals in `Tool::pinnable`'s `matches!` arm from `src/tools/core/types.rs` (a balanced-paren source scan, so a name added to the arm is checked without being restated in the test) and requires each to be a registered tool or a named exemption. The two exemptions are tied to their owning type's `name()`: `ActivateProject` (dispatched to by `Workspace`, never registered on its own) and `ProbeTool` (registered only under `CODESCOUT_PROBE=1`). The test carries controls so an empty or wrong parse cannot pass it vacuously: at least five names parsed, and `workspace`, `get_guide` and `onboarding` among them. The arm parser is textual, so a restructuring of `Tool::pinnable` away from a single `matches!` makes the parse fail the control rather than pass silently.

## Tests added

In `src/server.rs`:

- `pinnable_tools_advertise_workspace_param` (:6594) now asserts the guarded names are registered tools before their absence from `pinnable` counts.
- `every_pinnable_exclusion_names_a_registered_tool_or_a_named_exemption` (:6709) pins that every name in `Tool::pinnable`'s exclusion arm is produced by a registered tool or a named, type-tied exemption, with parse controls.
- `unaccounted_pinnable_arm_names_reports_only_the_unproduced` (:6762) is a fixture twin proving the filter reports a name no tool produces and ignores a registered one and an exempt one.
- `pinnable_arm_names_reads_every_literal_in_the_matches_arm` (:6782) is a fixture twin proving the arm parser reads every literal, including past `self.name()`'s own paren, and ignores literals outside the arm.

## Fix provenance

- **SHA:** `4c37ba0d` (`experiments`)
- **patch-id:** `a7a897d2e347c3ef2b8561a025aaf5e2a2bf2d1c`

## Resume

Closed on 2026-10-06 by `4c37ba0d` (local on `experiments`, not pushed at the time of writing). Residual follow-ups, listed and not filed: (1) the `"activate_project"` arm in `Tool::pinnable` (`src/tools/core/types.rs`) is dead for the wire surface, since no tool registers under that name; the new test exempts it by name rather than deleting it. (2) The parent `docs/issues/archive/2026-09-01-pinnable-assertion-vacuous-for-an-unregistered-tool.md` still carries its `unverified: TRACKED 60ac58939c731ae3` caveat, which the integrator decides how to retire.

## References

- `docs/issues/archive/2026-09-01-pinnable-assertion-vacuous-for-an-unregistered-tool.md` — parent
