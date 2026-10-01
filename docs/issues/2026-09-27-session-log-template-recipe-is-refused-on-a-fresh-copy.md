---
id: '4d25c5b252c36a70'
kind: bug
status: open
title: 'BUG: the session-log template''s own append_entry recipe is refused on a fresh copy'
tags:
- cluster/doc-contradicted-by-code
- session-log
- template
---

# BUG: the session-log template's own append_entry recipe is refused on a fresh copy

**Valid:** dated 2026-09-27

## Summary

`docs/templates/session-log.md` ships without frontmatter and prescribes `doc(action="append_entry", …, id_prefix="F")`. On a fresh copy that call is refused (`allocate_entry_id: … does not declare an entry_prefix`) until the writer declares `entry_prefix: [F, W]` — a step the template does ask for, in its own prose, as a precondition. Five session logs in this repo never took that step and sit on the recipe gate's shrink-only exemption list (`TEMPLATE_EXEMPT` in `src/librarian/tools/append_entry.rs`). While a newly copied log is undeclared, the gate reds every session's run in this shared tree, correctly: the recipe really would be refused.

## Fix

Not started, and a decision rather than a patch. Shipping the template declared closes the window, but ends its deliberate "directly editable until declared" property, which the reconnaissance skill also states.


## Picked up 2026-10-01 (session `3e2b9cc8`) — not fixed; the decision is still the user's, and its cost changed

The fix remains a decision (ship the template declared, or keep "directly editable until declared"), so nothing here was patched. Two facts bear on it:

- **The undeclared-copy window now has a second test.** `b9cf290d16ed2b180f2c8135cc0782f14e173604` (patch-id `adc734b3971c7c8dcf5e2774a53348e375b1f87b`) gates every recipe written in `docs/trackers/*.md`. The template carries two `id_prefix="F"` calls (`docs/templates/session-log.md` lines 12 and 272) and no frontmatter, so by that check a fresh copy is refused ("does not declare an entry_prefix") until `entry_prefix` includes `F`, in addition to `template_check`. **Derived from the template text and the check, not run on a fresh copy.**
- **That test needs no exemption list.** The five `TEMPLATE_EXEMPT` logs contain no `append_entry` text (a glob over the five names finds all five files and `grep` finds the string in none), so they never reach the new check, and it adds nothing to the shrink-only list. A sixth undeclared log containing a recipe would red it.

Whichever way the decision goes, the repair for a fresh copy is the same one the refusal already names: declare `entry_prefix: [F, W]`.

## References

- `docs/superpowers/specs/2026-09-27-taxonomy-append-recipes-test-design.md` — § *Out of scope*
- `bug-fix-session-log:F-176`
