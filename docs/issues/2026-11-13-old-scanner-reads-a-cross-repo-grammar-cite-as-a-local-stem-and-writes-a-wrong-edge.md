---
id: fe65babc93a25084
kind: bug
status: open
title: The old scanner reads a cross-repo grammar cite as a local file stem and writes a wrong edge
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-13
severity: medium
unverified: 'The wrong edge is shown by a test in a temp repository, not observed in the machine-global catalog. Not established: how many real ledgers cite another repository through append_entry on a params ledger (none did before Plan C1 Task 7).'
---

# BUG: The old scanner reads a cross-repo grammar cite as a local file stem and writes a wrong edge

## Summary

`link_scan` reads `key:TOKEN` inside a written `[[repo/key:TOKEN]]` cite as `<local file stem>:TOKEN`. When a different file in the scanned repository has the stem `key` and defines `TOKEN`, a scan with `write=true` records an `origin='scan'` `entry_cite` row to that local file, beside the correct `origin='write'` row. The cite was written by `append_entry` on the params path (Plan C1, Task 7), the first server code that writes `[[repo/key:TOKEN]]` text.

## Class fit

cluster/unclassified: a closed grammar (`[[repo/key:TOKEN]]`) is read by a scanner that predates it and has its own pattern. The scanner is the old indexer's `link_scan`, which stays unchanged until Plan E retires it, so the bug is a known gap, not a defect to fix inside Plan C1.

## Symptom (Effect)

After `append_entry` writes `**Cites:** [[other-repo/notes:F-3]]` into a ledger of repository R, `link_scan` with `write=true` on R:

- records an `entry_cite` row from the citing entry to the file `notes.md` of R, if R has a file with the stem `notes` that defines `F-3`, in addition to the write row to the other repository;
- records nothing, and reports `CrossRepo`, if R has no file with the stem `notes`.

The first case is a wrong edge in the old catalog. Session-log style files in every repository define `F-1`..`F-5`, so a matching token is likely.

## Reproduction

Two tests in `src/librarian/catalog/augmentation.rs` (Plan C1 Task 7) build a real second repository and run the scan:

- `a_write_scan_of_a_cross_repository_cite_makes_no_edge_when_no_local_file_has_the_stem`;
- `a_write_scan_of_a_cross_repository_cite_edges_to_a_local_file_that_shares_the_stem` pins the current wrong behaviour.

## Environment

Any repository with a ledger that cites another repository through `append_entry` on a params ledger, plus a local file whose stem equals the foreign key.

## Root cause

`cross_repo_re` in `src/librarian/tools/link_scan/extract.rs` (~287-290) is `\b[a-z][a-z0-9_-]{1,119}:[A-Z]{1,3}-\d+\b`. It has no knowledge of `[[` or of a `repo/` prefix, so in `[[other-repo/notes:F-3]]` the word boundary after `/` lets it match `notes:F-3`. `resolve.rs` (~335-375) then looks the qualifier up as a file stem (`corpus.by_stem.get(qualifier)`), and a single definer of the token gives `Outcome::Edge` to the local file. The same-repository case is not affected: the `entry_cite` primary key is `(src_slug, src_local, dst_ref, rel)` and leaves `origin` out, so the scan's row for the same edge is ignored (`skipped_existing: 1`).

Verified by two Opus reviews of Plan C1 Task 7 reading both files, and by the two tests above.

## Evidence

- `src/librarian/tools/link_scan/extract.rs` ~287-290 and `src/librarian/tools/link_scan/resolve.rs` ~335-375, read in the Task 7 review.
- The two tests named above; the second passes today and pins the wrong edge.

## Hypotheses tried

None. The cause is a direct reading of the pattern.

## Fix

Not made. Options, in order of cost:

1. Plan E retires the old scanner; the derived index reads the grammar from `identity::grammar::extract`, which parses `[[repo/key:TOKEN]]` as a unit. Nothing to fix before then.
2. Teach `cross_repo_re` to skip a match that sits inside `[[ … ]]`.
3. Make `append_entry` refuse a cross-repo entry-grain cite until the scanner is retired.

Controller recommendation: option 1, and leave the pinning test as the marker. When the scanner changes, the second test should flip to `written: 0`.

## Tests added

The two tests named above, added with Plan C1 Task 7.

## Workarounds

Cite a cross-repo document with `[[repo/key]]` (file grain) instead of an entry token, or run `link_scan` without `write=true`.

## Resume

If the old scanner is still in use after Plan E's read flip, do option 2.

## References

- docs/superpowers/plans/2026-11-12-artifact-identity-plan-c1-dual-write-entries-and-sidecars.md (Task 7)
- session log `docs/trackers/artifact-identity-session-log.md`
