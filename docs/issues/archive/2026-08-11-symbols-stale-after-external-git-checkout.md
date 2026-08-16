---
id: '000ed425ab6cc63f'
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

Shipped in **two** commits, and the second exists because the first did nothing.

> **`eedb308c` was INERT on the mux path** — the path this project actually uses
> for Rust. Live verification on the rebuilt server reproduced the original bug
> unchanged. Really fixed in **`7c8863f0`**; see § Resume, where the mechanism is
> worth more than the fix.

The approach is the candidate this file proposed — mtime/hash drift detection with
an auto-flush — with the hash dropped for a reason worth recording.

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

**That end-to-end test was necessary and not sufficient**, which is this file's
real lesson. It drives `LspClient::start` with `mux: false` — the **stdio**
transport — and the defect exists only on **socket**. It passed against a real
rust-analyzer while the shipped binary stayed broken.

Two transport-independent tests landed in `7c8863f0`:

- `a_repeat_did_open_must_not_overwrite_the_recorded_signature` — asserts both
  that the mark holds *and* that drift is still visible after a repeat `didOpen`.
  Mutation-verified against the real defect, not a hypothetical: restoring the
  unconditional overwrite fails it with `(20, t=1600000060)` where
  `(10, t=1600000000)` was expected.
- `an_unrecorded_file_has_not_drifted` — a never-sent file must not fire a
  redundant `didChange` on first navigation.

Both assert on `SyncedSignatures` rather than through a transport. That is what
makes the invariant testable without standing up a mux — the reason it had no test
before.

**Live-verified on the mux path**, 2026-08-15, after `cargo rb` + `/mcp`: an
external `cp` prepended three lines to a probe file; `symbols()` with no flush
reported `10-12`, having reported `7-9` before. The identical sequence against the
`eedb308c` binary returned the pre-write range.

Gate: `cargo test --workspace` → 3817 passed / 0 failed / 50 ignored; clippy clean.
## Workarounds

After any edit made outside codescout's own tools (external `git checkout`, `git stash`, another editor, a build step that rewrites generated code), call `workspace(action="status", post_compact=true)` before trusting `symbols()`/`edit_code` output for the affected file(s).

## Resume

Closed — for real this time, and the two-step is the useful part.

**Why `eedb308c` shipped inert.** `did_open` recorded the on-disk signature. On
**socket** transport it never takes its already-open early return, because the mux
owns document dedup — so every `document_symbols` re-recorded the *current* disk
state, and the drift check running afterwards compared the file against itself.
Meanwhile the mux deduped the `didOpen`, so the server kept its stale copy. Two
halves silently cancelling.

**Why no test caught it.** The end-to-end test used `mux: false`. **It exercised
the one transport on which the defect cannot appear** — and Rust in this project
runs through the mux.

**What actually fixed it.** Not the reordering, though the check now runs before
`did_open`. The repair that matters is making the rule explicit rather than
positional: **`did_open` records vacant-only, `did_change` overwrites — a
notification that may be deduped must not claim delivery.** That turns ordering
from load-bearing into irrelevant, which is the difference between fixing this
instance and being able to regress it silently again.

Two lessons, both generalised:

1. **A test that exercises only the simplest transport or deployment mode is not
   verification of a fix whose failure mode is mode-specific** — R-86, F-49.
2. The earlier lesson still stands: `did_change_refreshes_stale_symbol_positions`
   documented a *defect*, so rewriting it was right (R-85). Note this one bug
   required both calls, in opposite directions, one commit apart — a test yielded
   to the code, then the code yielded to reality.
## References

- `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/task-5-report.md` § "A real tool hazard hit mid-fix (worth recording)"
- Related but a different mechanism (stale `range_start_line` inside a single session's own prior edits, not staleness from an external write): `docs/issues/2026-07-28-edit-code-target-base-from-stale-lsp-range.md`
