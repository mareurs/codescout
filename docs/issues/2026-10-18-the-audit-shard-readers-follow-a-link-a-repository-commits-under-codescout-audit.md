---
status: open
opened: 2026-10-18
closed:
severity: low
owner: marius
related: [docs/issues/2026-10-17-the-librarian-applies-no-read-deny-list-to-a-catalogued-outward-link.md, docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'Read, not run. Whether any field of a parsed line, or a count, reaches a caller in a way that discloses content of a file outside the repository was not traced; the parse is strict JSON per line, so a non-JSON target is counted as malformed.'
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

Not applied. Read each shard and `.gitattributes` through `read_door::for_project(repo_root)`, and drop the
three entries from the gate's allow-list (the gate then enforces it).

## Tests added

N/A: filed, not fixed.

## Workarounds

Do not run `librarian(audit_log)` in a repository you do not trust.

## Resume

Route the reads through the door; add a layout row with a shard that is a link out of the repository.

## References

- `src/librarian/catalog/audit/shard.rs`
- `src/librarian/read_door.rs` (`ALLOWED`)
