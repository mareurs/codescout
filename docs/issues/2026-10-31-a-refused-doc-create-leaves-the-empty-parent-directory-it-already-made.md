---
id: '4b53fc372ed9a3a2'
kind: bug
status: open
title: a refused doc(create) leaves the empty parent directory it already made
tags:
- cluster/unclassified
opened: 2026-10-31
owner: marius
severity: low
---

# BUG: a refused doc(create) leaves the empty parent directory it already made

## Summary

`doc(create)` calls `create_dir_all(parent)` before it runs the validators that can refuse the call. A refused create writes no file and no catalog row, but the new parent directory stays on disk, empty.

## Class fit

A refusal path that does part of its side effect before it decides. The write is guarded, but the directory creation that comes first is not.

## Symptom (Effect)

After a refused create, `ls <parent>` succeeds and shows an empty directory that did not exist before the call.

## Reproduction

Observed on 2026-10-31 on a rebuilt server. `doc(action="create", kind="bug", rel_path="zz-probe/opened-probe.md", extra={"opened": "yesterday", ...})` was refused with `extra.opened "yesterday" is not a YYYY-MM-DD date`. Then `ls -ld zz-probe` showed a directory with mtime 12:55:55, the moment of the call. I removed it with `rmdir`.

## Environment

codescout `experiments`, tip after `30721b865`. Linux.

## Root cause

Read in `src/librarian/tools/create.rs`, function `call`: `std::fs::create_dir_all(parent)` is at line 422. `reject_body_leading_frontmatter` is at line 433, `enforce_on_create` at line 436 and the `opened` check at line 448. Each of them can refuse after the directory exists. The comment at lines 410-411 shows the symlink guard was placed before `create_dir_all` on purpose, so that ordering is known for one validator and not for the rest.

## Evidence

The live observation above, and the line numbers read from the file. I did not run the requirements or body-frontmatter refusals to see the same directory. Their position after line 422 is a code read.

## Hypotheses tried

None beyond the code read.

## Fix

Not started. Two candidates: run the content validators before `create_dir_all`, or remove the directories this call created when it refuses. The first must keep the symlink guard ahead of `create_dir_all`.

## Tests added

None yet.

## Workarounds

`rmdir` the empty directory by hand.

## Resume

Write a test that creates a bug with a bad `opened` under a new subdirectory, expects the refusal, and asserts the subdirectory does not exist. The existing test `a_bug_that_breaks_the_project_requirements_is_refused_before_anything_is_written` only checks that the file is absent.

## References

- `src/librarian/tools/create.rs` `call`
- `docs/issues/2026-10-31-doc-create-accepts-a-malformed-opened-on-a-bug-so-the-frontmatter-gate-fails-in-another-session.md`
