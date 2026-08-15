---
id: '197843bd0a5c4fd5'
kind: bug
status: fixed
title: 'BUG: symbols() keeps reporting pre-checkout content after an external git checkout, until an explicit LSP flush'
tags:
- symbols
- lsp
- staleness
- tool-friction
closed: 2026-08-15
opened: 2026-08-11
owner: marius
related:
- '9e649674c95cd7bd'
severity: medium
---

## Summary

After running `git checkout -- <file>` from outside codescout's own edit tools (to revert an in-progress edit that had produced a duplicate symbol — see `docs/issues/2026-08-11-edit-code-no-disambiguator-for-duplicate-name-path.md`), `symbols()` continued to report the *stale*, pre-checkout (already-duplicated) file content for several subsequent calls — even though `git diff` and `read_file(force=true)` both confirmed the on-disk content was correctly reverted. `workspace(action="status", post_compact=true)` ("flush all LSP clients") repaired it; `symbols()` matched disk again immediately after.

## Symptom (Effect)

```
git checkout -- src/retrieval/embedder.rs      # reverts a duplicated-impl mistake
git diff -- src/retrieval/embedder.rs           # clean — file matches HEAD
symbols(path="src/retrieval/embedder.rs")       # still lists the pre-checkout duplicated impl blocks
...
workspace(action="status", post_compact=true)   # "flushed": true
symbols(path="src/retrieval/embedder.rs")       # now matches disk
```

## Reproduction

1. Make an `edit_code` edit that grows or duplicates a symbol in a file.
2. `git checkout -- <file>` to revert, from outside codescout's own tools.
3. `symbols(path=<file>)` — reports the pre-checkout (stale) shape.
4. `workspace(action="status", post_compact=true)` — flushes all LSP clients.
5. `symbols(path=<file>)` — now matches disk.

Branch: `feat/local-onnx-query-path`, Task 5.

## Environment

codescout `feat/local-onnx-query-path`, Linux, MCP stdio, rust-analyzer LSP mux.

## Root cause

`LspClient::document_symbols` opens the file with `did_open` before querying — and
`did_open` **returns early when the file is already open** (`src/lsp/client.rs`,
the `open_files.contains_key(&canonical)` guard). That early return is the whole
hole.

codescout's own writes notify the server (`did_change` after every edit). An
external `git checkout`, `git stash`, rebase, or plain editor save does not.
Nothing between those two facts ever re-read the disk, so the server kept
answering from whatever content it was last handed — indefinitely, since nothing
expires.
## Evidence

Quoted from `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/task-5-report.md` § "A real tool hazard hit mid-fix (worth recording)":

> Second hazard from the same revert: after the external `git checkout`, `symbols()` kept reporting the *stale* (duplicated) state for several calls even though `git diff`/`read_file(force=true)` confirmed the on-disk content was correctly reverted — the LSP's in-memory document model doesn't observe filesystem changes made outside codescout's own edit tools. Fixed by calling `workspace(action="status", post_compact=true)` ("flush all LSP clients"), after which `symbols()` matched disk again.

## Hypotheses tried

None — the mitigation (`post_compact=true`) was applied on notice, without reading the LSP sync path.

## Fix

Shipped in `eedb308c`, and it is the candidate this file proposed — mtime/hash
drift detection with an auto-flush — with the hash dropped for a reason worth
recording.

`LspClient` gains `synced_sigs`: the on-disk signature of the content last **sent**
to the server, per file. Recorded in `did_open` and `did_change` (both already read
the file), checked by `resync_if_drifted` before the `documentSymbol` request goes
out. On drift it calls `did_change`, which was already the correct repair — it
re-reads from disk and sends full content, on both stdio and mux transports.

**Deliberately separate from `open_files`.** That map tracks LSP document versions
and answers *"does the server know this file"*; this one answers *"does the server
have its current bytes"*. Conflating the two questions is how the gap survived —
the file was open, so everything looked fine.

**The signature is `(len, mtime)`, and its blind spot is documented and asserted.**
`len` alone misses a same-size change, which is exactly what reverting one
character produces; `mtime` alone has filesystem-dependent granularity. The pair
narrows the window but does not close it: a same-length change *inside one mtime
tick* still reads as unchanged. Accepted — closing it means hashing every file on
every navigation call, to catch a case needing coarse mtime AND a length-preserving
edit AND sub-tick timing, whose cost is one stale answer, i.e. the behaviour before
this fix. The asymmetry is the argument: a false negative costs what we already
had; a false positive costs one redundant `didChange`.
## Tests added

- `disk_signature_detects_a_same_length_change` — the `git checkout` shape
  specifically: same byte count, different content. mtime is forced with
  `filetime` rather than left to wall-clock, or the test would be flaky on exactly
  the coarse-granularity filesystems the pair exists for. It also asserts the
  **documented blind spot** (same length + same mtime → identical signature), so
  the limitation is pinned rather than implied.
- `disk_signature_is_none_for_a_missing_file` — drives `resync_if_drifted`'s early
  return; an unstattable path must not read as "changed" and loop.
- `document_symbols_reflects_a_write_made_outside_codescout` — **rewritten from**
  `did_change_refreshes_stale_symbol_positions`, whose step 3 asserted the
  opposite. See § Resume.

**Verified end to end against a real rust-analyzer**, not only by unit test: the
rewritten test prepends three lines on disk with no flush and asserts the shift is
picked up. Gate: `cargo test --workspace` → 3814 passed / 0 failed / 50 ignored;
clippy clean.
## Workarounds

After any edit made outside codescout's own tools (external `git checkout`, `git stash`, another editor, a build step that rewrites generated code), call `workspace(action="status", post_compact=true)` before trusting `symbols()`/`edit_code` output for the affected file(s).

## Resume

Closed. The part worth carrying forward is the test that had to be rewritten.

`did_change_refreshes_stale_symbol_positions` asserted, at its step 3, that a
query after an external write returns **stale** positions — as a demonstration of
why a caller had to send `did_change` themselves. **That demonstration was the
bug.** Fixing it necessarily broke the test.

The distinction that decided what to do: **the test's subject was a defect, not a
contract**, so it became obsolete the moment the defect closed, and rewriting it
was correct. Contrast `9b902e0a` the same day — there a failing test pinned a real
contract with purpose-built support behind it, and the *code* was wrong, so the
change was reverted. Same surface (a test blocking a change), opposite verdicts.

**Ask what a blocking test is a statement about before deciding which side
yields.** A test that documents "this is broken" and a test that documents "this
is the agreement" look identical from the failure message.
## References

- `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/task-5-report.md` § "A real tool hazard hit mid-fix (worth recording)"
- Related but a different mechanism (stale `range_start_line` inside a single session's own prior edits, not staleness from an external write): `docs/issues/2026-07-28-edit-code-target-base-from-stale-lsp-range.md`
