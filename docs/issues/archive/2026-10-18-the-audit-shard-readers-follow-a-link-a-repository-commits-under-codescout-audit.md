---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: 2026-10-24
opened: 2026-10-18
owner: marius
related:
- docs/issues/2026-10-17-the-librarian-applies-no-read-deny-list-to-a-catalogued-outward-link.md
- docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md
severity: low
unverified: 'The read itself is closed (content is refused at the read door; the new tests were red before the fix). Still open: `read_dir` follows a linked `.codescout/audit` directory, so counts of an outside directory (`unreadable_files`, `files_skipped_by_window`) can reach the caller; the Windows and macOS read-door paths were not run.'
---

# BUG: the audit shard readers follow a link that a repository commits under `.codescout/audit/`

## Summary

`src/librarian/catalog/audit/shard.rs` reads committed files by plain `std::fs::read_to_string`: every
`.codescout/audit/<host>-<month>.jsonl` shard (`read_shards`) and the repository's `.gitattributes`
(`gitattributes_declares_shard_union`). Both are repository content, so a repository can commit a shard name as a
link to any file the user can read, and the reader follows it. The read door added by the read-deny-list fix
(`src/librarian/read_door.rs`) is not used here, because audit shard I/O was out of that fix's scope; these reads
are listed with that reason in the source gate's allow-list (`no_librarian_production_read_bypasses_the_read_door`).

## Class fit

The same absence as the read-door issue (a containment check missing from a read whose path a repository
controls), in the one librarian subsystem that fix excluded by scope. Filed unclassified like its parent.

## Symptom (Effect)

Not established. A target that is not JSON lines is counted under `malformed`; a target that IS JSON lines
(another tool's log) would be merged into `audit_log` results as rows of a foreign host.

## Reproduction

Read, not run.

## Environment

Any repository that commits `.codescout/audit/`.

## Root cause

The shard reader predates the read door and was excluded from it by the fix's brief.

## Evidence

`shard.rs` `read_shards` (`std::fs::read_to_string(e.path())`) and `gitattributes_declares_shard_union`;
the writer side of the same file (`OpenOptions::append`) is the write-through row already filed as NOT fixed in
`2026-10-17-doc-update-writes-through-a-catalogued-symlink-...` (its per-site table).

## Hypotheses tried

N/A.

## Fix

Fixed. `read_shards` and `gitattributes_declares_shard_union` (`src/librarian/catalog/audit/shard.rs`) read through `read_door::for_project(repo_root)`. A refused shard counts into the existing `unreadable_files` counter, so it is reported and its rows never reach `rows`, `hosts` or `files_read`. `.gitattributes` is read with `text_for_write`, which refuses ANY leaf link, because git itself ignores a symlinked `.gitattributes`; an in-repository link therefore no longer counts as a union-merge opt-in. The two content-read entries left the `ALLOWED` list in `src/librarian/read_door.rs`, so the source gate `no_librarian_production_read_bypasses_the_read_door` now enforces these reads (the issue said three; only two content-read entries existed).

- Fix SHA (on `experiments`; merged 2026-10-24 by fast-forward, tip `e0255583b`): `3ea841fc241163e7dc7aa606d0bd7143c29f4161`
- patch-id: `0d7448249aa8f6de5f3970ce515f5abfda903feb`

An independent Opus review found no blocker; its two SHOULD-FIX items (a symlinked in-repo `.gitattributes` still counted as an opt-in; the refusal fixtures depended on whether an ancestor of TMPDIR is a git repository) are in the commit.

## Fix provenance

- **SHA:** `3ea841fc241163e7dc7aa606d0bd7143c29f4161` (`experiments`)
- **patch-id:** `0d7448249aa8f6de5f3970ce515f5abfda903feb`

## Tests added

In `src/librarian/catalog/audit/shard.rs`, all `#[cfg(unix)]`, links made only inside tempdirs, each fixture creating `repo/.git`:

- `a_shard_that_is_a_link_out_of_the_repository_is_refused_and_counted`
- `a_shard_directory_that_is_a_link_out_of_the_repository_is_refused_and_counted`
- `a_gitattributes_that_is_a_link_out_of_the_repository_is_not_an_opt_in`
- `a_gitattributes_that_is_a_link_inside_the_repository_is_not_an_opt_in`
- `a_shard_that_is_a_link_inside_the_repository_is_still_read` (control)

Red before the fix (the outside row `leaked` was merged in, `files_read: 1`, and the link counted as an opt-in); green after. The source gate now covers these reads. Gate on the merged tree: exit 0.

## Workarounds

Do not run `librarian(audit_log)` in a repository you do not trust.

## Resume

Done. Archive into `docs/issues/archive/` once the fix is on `experiments`. Not done: the `read_dir` residual in the `unverified:` caveat; a layout row for `.gitattributes` in `repository_config_files_are_read_only_through_the_door` was skipped because that table expects an in-repository link to be readable.

## References

- `src/librarian/catalog/audit/shard.rs`
- `src/librarian/read_door.rs` (`ALLOWED`)
