---
id: '46099dbdb68e0d7b'
kind: bug
status: open
title: 'BUG: event_create writes source.uri and author into committed text unchanged'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-14
severity: low
unverified: Read from the code and the review, not reproduced.
---

# BUG: event_create writes source.uri and author into committed text unchanged

## Summary
The event section that `event_create` writes (`src/librarian/catalog/event_text.rs`) puts the caller's `author` in the section heading and the caller's `source.uri` in the comment JSON, both unchanged. Task 1 turns control characters in the heading into spaces, but it does not touch the text. An absolute path in a `source.uri` is committed. A `[[x]]` in an `author` becomes a derived citation of the document, because a wiki link in a heading is read by the grammar.

## Class fit
cluster/unclassified: caller text crosses into a committed file without a rule for what it may hold. The spec says no tool output holds an absolute path as a document reference.

## Symptom (Effect)
A committed `## Events` section can hold a machine path (`/home/<user>/...`) or an unintended citation edge.

## Reproduction
Not reproduced. Read path: `event_create` on a document with `single_writer: true`, with `author` set to `[[some-key]]` or `source.uri` set to an absolute file path. Read the document and the derived edges.

## Environment
experiments at 5fc3680f4.

## Root cause
No scrubbing step between the argument and `render_event_section`, apart from control characters in the heading.

## Fix
Decide the rule (for example: escape `[[` in the heading text, and write a `source.uri` that is a path as a repo-relative path or refuse it). Add tests for both inputs. Also see bug `3f0dfd4524d7164f` (paths in refusal texts).

## Resume
Open. Not started. Found by the Plan C2 whole-branch review (M-8).

## References
- `src/librarian/catalog/event_text.rs` (`render_event_section`, `one_line`)
- `src/librarian/tools/event_create.rs`
- `docs/research/2026-11-14-artifact-identity-plan-c2-execution-state.md`
