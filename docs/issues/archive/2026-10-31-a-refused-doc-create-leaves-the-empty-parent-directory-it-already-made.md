---
id: 2570eea4939d4af4
kind: bug
status: fixed
title: a refused doc(create) leaves the empty parent directory it already made
tags:
- cluster/unclassified
closed: 2026-10-31
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

Fixed in `343d77d9b`. `create_dir_all(parent)` in `src/librarian/tools/create.rs` `call` now runs just before `fs::write`, after every validator and after the catalog upsert. The symlink guards (`refuse_write_through_symlink`, `refuse_catalogued_path_outside_repo`) still run before everything, because they judge the path and not the directory. A comment at the new site names this record.

## Tests added

`a_refused_create_does_not_leave_the_parent_directory_it_would_have_made` in `src/librarian/tools/create.rs`. Three cases, each writing under a directory that does not exist yet: a bad `opened` on a bug, a body that opens its own frontmatter, and an out-of-vocabulary bug status. Each must be refused and must leave no top directory.

Red before the fix (`bad_opened: a refused create left its new parent directory on disk`). Mutation: putting the early `create_dir_all` back turned it red again, on the first case. The loop stops at the first failure, so the other two cases were not shown red by the mutant. The requirements-file refusal (`enforce_on_create`) and the taken-prefix refusal are not in the test.


## Fix provenance

- **SHA:** `343d77d9b` (`experiments`)
- **patch-id:** `6c283949e15e3cb3ff045e48219dd414d5bcfa49`

## Workarounds

`rmdir` the empty directory by hand.

## Resume

Fixed. Full gate: `CLIPPY=0 LEAN=0 DEFAULT=0`, `PYTHON_LIGHT=0`; `FMT=1` was `ownership-refused` (peer-owned dirty files), so I ran `rustfmt --edition 2021` on `create.rs` alone (one stray blank line) and the commit hook's `rustfmt --check` passed. Heavy Python suites not run.

**Seen live 2026-10-31** on a server rebuilt after `343d77d9b`: `doc(action="create", kind="bug", rel_path="docs/issues/zz-live-dir/deeper/probe.md", extra={"opened": "yesterday"})` was refused with `extra.opened "yesterday" is not a YYYY-MM-DD date`, and `docs/issues/zz-live-dir` did not exist before or after. Only the bad-`opened` refusal was run live. The leading-frontmatter and bad-status refusals are covered by the unit test, not by a live call.

## References

- `src/librarian/tools/create.rs` `call`
- `docs/issues/archive/2026-10-31-doc-create-accepts-a-malformed-opened-on-a-bug-so-the-frontmatter-gate-fails-in-another-session.md`
