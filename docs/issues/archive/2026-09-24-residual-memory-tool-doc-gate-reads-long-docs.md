---
id: 52926c4f251df5c2
kind: bug
status: fixed
title: 'RESIDUAL: Extend the memory tool-doc gate to check long_docs() as well as description()'
tags:
- cluster/doc-contradicted-by-code
closed: 2026-09-26
fix_patch_id: 18a55c33641f988e37aeab08c9500be739952268
fix_sha: b990f177df31d8cff5f00bd54085f72f4b827a5b
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Extend the memory tool-doc gate to check long_docs() as well as description().

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md` (status `fixed`):

> the long_docs() half has no regression test — the gate reads description() only, so a future long_docs drift is uncaught

## Fix

Extended `tool_descriptions_name_every_action_they_claim_to_enumerate` (`src/server.rs`) to also compute a missing-actions check against `t.long_docs()` when it is `Some`, scoped to tools declaring `ActionContract::Inventory` (only `memory` currently has both). Added `under_reported_long_docs` alongside the existing `under_reported` and a fourth `assert!` naming the omissions. `memory`'s `long_docs()` already names all 8 actions, so the gate is currently green with no drift to fix — this closes the residual regression-test gap, not a live defect.

Fixed in `b990f177` (`git show b990f177 | git patch-id --stable` → `18a55c33641f988e37aeab08c9500be739952268`).

## Tests added

No new test function — the existing gate test is the regression test; that is the point (one site, extended, not a duplicate). Verified red: temporarily renamed `action="refresh_anchors"` to `action="refresh-anchors-MUTATED"` in `memory`'s `long_docs()`, re-ran `cargo test --lib tool_descriptions_name_every_action_they_claim_to_enumerate` under `scripts/with-slot.sh`, observed failure naming `memory long_docs() omits ["refresh_anchors"] of 8` at the new assertion's line, then reverted (`git diff` clean) and re-ran green.

## References

- `docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md` — parent
