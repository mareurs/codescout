---
id: '74479042d0911bf3'
kind: bug
status: open
title: 'RESIDUAL: Capture the operator-rule path predicate from each write tool''s actual target key (not only input[''path''])'
tags:
- cluster/declared-not-wired
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Capture the operator-rule path predicate from each write tool's actual target key (not only input['path']).

## Parent caveat, verbatim

`docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md` (status `fixed`):

> CLOSED 2026-09-01, first half: `tools::core::tests::a_real_edit_file_write_under_dot_claude_delivers_op_4` drives the REAL EditFile through call_content against an absolute path under `.claude` and asserts the OP-4 block arrives in the returned content — one call, no hand-supplied selector, no hand-fed route() call, with the negative control run FIRST so the once-per-session ledger cannot make its silence vacuous. Writable only because 30b6fc41 inverted Tool::selector_key's default, so EditFile now supplies its own selector and call_content runs the router itself. Mutation-checked: removing the annotation kills it plus two siblings (so it establishes the annotation matters, not this test's unique necessity); forcing the key to rel_path does NOT kill it, because names_path_containing scans both keys. STILL OPEN, second half: the path is captured from input["path"] specifically, so a write tool using a different key for its target gets no annotation and any rule serving it would be dead the same way. edit_file and create_file both use `path`, so no live rule is affected today.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md` — parent
