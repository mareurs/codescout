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

## References

- `docs/superpowers/specs/2026-09-27-taxonomy-append-recipes-test-design.md` — § *Out of scope*
- `bug-fix-session-log:F-176`
