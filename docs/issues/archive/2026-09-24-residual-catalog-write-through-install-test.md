---
id: 8d578fe8206a10be
kind: bug
status: fixed
title: 'RESIDUAL: Add a test that reds when the server-side catalog write-through install (src/server.rs:374 at filing) is removed'
tags:
- cluster/unclassified
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Add a test that reds when the server-side catalog write-through install (src/server.rs:374 at filing) is removed.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md` (status `fixed`):

> Liveness caveat CLEARED 2026-08-30 13:2x: re-verified after the rebuild this field asked for, on a fresh server (PID 1899149, /proc/<pid>/exe live, binary inode 6442149 built 13:25:19). This very frontmatter write is the probe — it sets a CATALOG COLUMN (owners), and the catalog reflected it with no reindex. Remaining and unchanged: the server-side install at src/server.rs:374 is covered by no test; deleting that line leaves all 8 green.

## Fix

A new integration-test binary, `tests/server_installs_librarian_hooks.rs`, registered in `Cargo.toml` as a `[[test]]` with `required-features = ["librarian"]` like the other librarian test binaries. The install slots are process-global and last-writer-wins, and `call_tool_inner` is private, so the tests spawn the real `codescout` binary (its own process, its own slots) against a scratch librarian workspace and catalog rather than calling `from_parts_with_env`. Artifacts are created through `doc`, then the stamped `id:` line is stripped from the file so it is the plain catalogued kind the hooks serve (a stamped file is refused by the guard before either hook matters). No change to `src/`. This pins both hooks installed in `CodeScoutServer::from_parts_with_env`: the catalog frontmatter write-through this file was filed for, and the augmentation oracle that sits beside it.

## Tests added

Both in `tests/server_installs_librarian_hooks.rs`:

- `the_server_wires_a_frontmatter_write_through_to_the_catalog_row` (BL-48 write-through): `edit_file` sets `status: fixed` in an id-less artifact's frontmatter, and the catalog row, read straight from the SQLite file over a second connection, must follow. A baseline read of `open` first proves the row exists and the observer reads.
- `the_server_wires_the_augmentation_oracle_into_the_markdown_guard` (BL-33): a direct `edit_file` on an id-less augmented tracker must be refused with a reply containing "augmented" and leave the file unchanged; the same edit on a plain id-less catalogued spec succeeds, as the positive twin.

Mutation check, recorded in the commit message: deleting both install lines in `src/server.rs` turns both tests red at the intended assertions, and restoring them turns both green. Both lines were removed in one build, not one at a time, so which single install each test discriminates was not separately measured.

## Fix provenance

- **SHA:** `cbe8f6c6` (`experiments`)
- **patch-id:** `d61fb4777eb9a18aabaaa7cd6e69dd05a066df35`

## Resume

Closed on 2026-10-06. Residual follow-ups, listed and not filed:

- Delete each install line separately and confirm each test reds on its own hook only (test 1 for the write-through, test 2 for the oracle); the recorded mutation removed both together.
- The tests need the `librarian` feature and spawn the real binary over stdio (Cargo builds it via `CARGO_BIN_EXE_codescout`), so they exercise the production path rather than `from_parts_with_env` directly; a lower-level seam would need `call_tool_inner` or the tool list to become visible.

## References

- `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md` — parent
