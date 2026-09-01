---
id: b26474f568eaf5e3
kind: bug
status: fixed
title: run_command/read_file buffer output recurses into meta-wrappers instead of resolving to raw text
tags:
- run_command
- read_file
- buffering
- progressive-disclosure
- friction
closed: 2026-08-25
---

## Summary

When `run_command`'s stdout exceeds the inline budget (~9-10KB) and gets
buffered as `@tool_*`/`@cmd_*`, following the documented recovery path
(`read_file` on the buffer, `json_path=$.stdout`, then `start_line`/`end_line`
slicing) does not reliably converge to raw text. Instead, slicing a derived
buffer frequently returns *another* meta-wrapper object — a small JSON blob
whose own `content` field just points at yet another `@file_*` buffer id —
rather than the actual log lines. This can chain 4-6 calls deep before
(sometimes) reaching real text, and in this session several chains never
converged at all inside a reasonable number of calls.

## Environment

- **Project:** `system` (`/home/marius/agents/system`), codescout MCP server
- **Date/time observed:** 2026-08-25, ~20:00-20:15 local (session timestamps
  in the transcript around `19:52`-`20:08` remote-host time, which is UTC+1
  BST — same incident)
- **Trigger context:** SSHing from the `system` project host into a remote
  Raspberry Pi (`192.168.1.139`) to `grep`/`tail` a Kodi log file
  (`~/.kodi/temp/kodi.log`), then trying to read the (grep-filtered, small —
  under 30 lines) result back through `run_command` + `read_file`.

## Steps to reproduce

1. `run_command(command="ssh user@host \"grep -i 'foo' /path/to/log | tail -25\"")`
   — output is ~15-20KB (a Kodi log's timestamp-prefixed lines add up fast
   even for a couple dozen matches), so it comes back as an overflow envelope:
   `{"output_id": "@tool_XXXX", "summary": "...", "hint": "...", "buffered_bytes": ~17000}`.
2. Follow the documented pattern: `read_file(path="@tool_XXXX", json_path="$.stdout")`.
   This step works — returns something like `"12 lines\n\nBuffer: @file_YYYY\n..."`.
3. `read_file(path="@file_YYYY", start_line=1, end_line=12)` to get the actual
   lines. **This is where it breaks**: instead of the 12 lines, the response
   is again a JSON meta-object (`{"type": "generic", "exit_code": 0,
   "output_id": "@cmd_ZZZZ", ...}` — the *original run_command's wrapper
   shape*, apparently re-serialized) with only 4 of "6 lines" shown, ending in
   `Buffer: @file_WWWW` pointing at yet another buffer.
4. Repeating `read_file` on `@file_WWWW` with `start_line`/`end_line` produces
   the same pattern again — a `{"output_id": "@tool_...", "summary": "N
   lines\n\n(truncated)", "hint": "..."}` shape — sometimes requiring a
   `json_path="$.content"` extraction to peel one more layer, which *itself*
   then returns another `@file_*` pointer instead of content.
5. Observed chains: `@tool_A` -> (`json_path=$.stdout`) -> `@file_B` ->
   (`start_line/end_line`) -> `@tool_C` -> (`json_path=$.content`) -> `@file_D`
   -> (`start_line/end_line`) -> `@tool_E` (still not raw text). At this
   depth the content being chased was under 30 short log lines (~2KB
   uncompressed) — nowhere near large enough to justify this many hops.

## Impact

- Multiple user-visible turns spent purely on buffer chasing, with zero
  forward progress on the actual task (reading a handful of Kodi debug log
  lines to diagnose a Premiumize/Umbrella "no streams found" issue).
- Eventually had to abandon `run_command`/`read_file` entirely for this
  sub-task and fall back to `scp`-ing the (already tiny, pre-filtered) file
  to local disk and reading it through `mcp__codescout__read_file` on the
  *local path* directly — which worked in one call. That workaround isn't
  always available (e.g. when the content only exists as command *stdout*,
  not a file you can point a path-based reader at).
- The interactive user explicitly flagged this as unacceptable friction
  mid-session ("wait! WTF is with this codescout friction?!? you never seem
  to read what you want").

## What I'd expect instead

- `read_file` on a `@file_*`/`@tool_*` buffer with explicit `start_line`/
  `end_line` should return the literal text of those lines directly, full
  stop — not a re-wrapped copy of the *originating* tool's JSON envelope.
- If a slice is still "too big" to inline (implausible for a 1-3 line slice
  of a 12-line, ~2KB buffer), the overflow envelope should say so
  explicitly and by how much, rather than silently nesting another buffer
  of the same apparent size as the parent.
- A `json_path="$.stdout"` or `$.content` extraction that itself resolves to
  another opaque buffer reference (rather than the extracted string) seems
  like the core defect — the "extract a field" operation should terminate at
  the field's value, not re-buffer it as a fresh indirection.

## Substrate check (2026-08-25)

Not verified against codescout source directly during this session — this
report is written from the *client-observed* behavior only (tool call/response
pairs visible in the transcript). Whoever picks this up should reproduce
against `src/` directly (likely somewhere in the progressive-disclosure /
`@tool_*`/`@file_*` buffer machinery referenced by
`get_guide("progressive-disclosure")`) rather than trusting this
transcript-only account.

**Valid:** dated 2026-08-25


## Root cause (2026-08-25, verified against source)

Two independent mechanisms, both in the oversized-chunk branch that
`read_file` and `read_markdown` each carry a copy of. Reproduced locally
first — the SSH/Kodi transport is incidental, and a local `awk` emitting the
same byte volume reproduces both.

### 1. The nesting — a chunk budgeted in raw bytes, measured after escaping

`INLINE_BYTE_BUDGET` caps the inlined chunk at 9000 raw bytes, 90% of
`TOOL_OUTPUT_BUFFER_THRESHOLD`; its doc comment budgets the remaining 10% for
"the JSON envelope overhead (~500-1000 bytes for content/complete/next/
shown_lines keys)". But the threshold `call_content` measures is the
**serialized** response, and inside a JSON string every `\n` costs two bytes.
The escaping charge scales with line COUNT — a variable the constant never
accounted for — so a line-dense chunk overshoots before a single key name is
written.

measured 2026-08-25, live server then reproduced as a unit test byte for byte:
a 1200-line buffer read as one range produced a **10169-byte** response against
the 10000-byte threshold. `call_content` re-wrapped it as `@tool_*`, and that
envelope's hint reads `json_path="$.content"` — which extracts a value too
large to inline, so it becomes a `@file_*`, which slices to another `@tool_*`.
That is the reported chain, `$.content` peel and all.

### 2. Two coordinate frames in one response

The same branch reported `shown_lines` / `total_lines` in the ORIGINAL ref's
line numbers while phrasing `next` in the freshly-minted `@file_*` slice's own
1-based frame. The two differ by `start - 1`, so following `next` after a
mid-range read re-serves lines already seen — on a new handle each time, which
is what makes the chain look like it is going backwards.

measured 2026-08-25: `read_file("@file_…", start_line=13, end_line=24)` returned
original lines 13-21 alongside `Next: … start_line=9`.

The invariant was already pinned in words by
`format_read_file_auto_chunked_mid_file` (`shown_lines: [50,51]` → `next
start_line=52`) and contradicted by `read_file_buffer_ref_range_auto_chunks`
("next should reference the file_id and use sub-buffer-relative line numbers").
The latter reads from line 1, where the two frames coincide, so it never
exercised the defect it pinned.

### What the report got right, and what it got wrong

**Right:** the chains are real, and they do not converge.

**Wrong:** "a `json_path="$.stdout"` … extraction that itself resolves to
another opaque buffer reference … seems like the core defect". It is not a
defect — the extracted value in the reproduction was 27KB, and no amount of
correctness makes 27KB inline. Chasing it would have produced no fix. Running
the reproduction before reading a fix plan is exactly what separates the two.

**Already true:** the expectation that an overflow envelope "should say so
explicitly and by how much" is satisfied today — the envelope carries
`buffered_bytes`, and it was `buffered_bytes: 10169` that identified mechanism 1.

## Fix

`src/util/text.rs` — new `extract_lines_to_json_budget()`, charging each line
its post-escaping cost. `extract_lines_to_budget()` keeps its raw-byte contract
for callers whose budget really is raw bytes; both now share
`extract_lines_with_cost()`, and the safety valve (always ≥ 1 line) moves there
intact.

Adopted at all four sites that inline a chunk into a JSON response:
`src/tools/read_file.rs` (`read_from_buffer`'s line-range and full-buffer
branches, and `read_with_line_range`) and
`src/tools/markdown/read_markdown.rs` (`read_markdown_line_range`).

Those branches now emit `next` against the ORIGINAL path/ref at
`shown_lines[1] + 1`, with `total_lines` the ref's own total so `shown_lines`
reads against it. The slice is still buffered under a `@file_*` handle — that is
what keeps the response small enough to escape a `@tool_*` re-wrap (BUG-026,
`docs/archive/bug-reports/2026-03-to-2026-04-tool-misbehaviors.md`) and keeps it
greppable — it is just no longer the address navigation runs through. On source
files the continuation carries `force=true`, because the head-read exemption
(`start == 1`) does not survive into a follow-up.

`read_markdown_line_range` gained a `path: &str` parameter — one caller, same
file, verified with `references` — so it can name the file in its `next`
instead of a handle.

**Fix commit:** `7712d8e67565b90181bb215c31d48fb3b7c0e091`, on **`experiments`**.

**patch-id:** `4a8809ba4b8fd26dacedd430ae11f16d5afad348`
(`git show <sha> | git patch-id --stable`).

The SHA is positional and dies when `experiments` is rebased, which happens
after every ship; the patch-id is a content hash of the diff and survives both
rebase and cherry-pick. Both are recorded here once — there is no promotion path
to check and nothing owed later, whichever way the fix reaches `master`.
## Tests added

All in-crate, all RED before the change:

- `read_file_buffer_range_chunk_fits_the_threshold_it_is_measured_against`
  (`src/tools/read_file.rs`) — reproduced the live `10169` byte-for-byte.
- `read_file_buffer_full_chunk_fits_the_threshold_it_is_measured_against`
  (`src/tools/read_file.rs`) — the whole-buffer branch, same arithmetic.
- `read_file_buffer_oversized_slice_next_continues_from_shown_lines`
  (`src/tools/read_file.rs`) — the buffer-ref frame.
- `read_file_oversized_range_next_continues_from_shown_lines`
  (`src/tools/read_file.rs`) — the real-file frame.

`read_file_buffer_ref_range_auto_chunks` (`src/tools/edit_file/tests.rs`) was
re-pointed rather than deleted: it pinned the old contract in prose, so it now
pins the continuation invariant and records why reading from line 1 hid the
defect.

Gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
— 4452 passed, 0 failed.

## Live verification (2026-08-25, reconnected MCP server)

Both reproductions re-run against the rebuilt binary (`cargo rb`, then `/mcp`).
Every other live measurement in this file was taken against the pre-fix binary;
these two were not.

| Check | Pre-fix | Post-fix |
|---|---|---|
| `read_file("@file_…", start_line=13, end_line=24)` | lines 13-20 with `Next: … start_line=9`, on a freshly minted handle | lines 13-20 with `Next: read_file("@file_3aa83a83", start_line=21, end_line=24)` — the original ref |
| 1200-line ranged read of a `@cmd_*` buffer | `@tool_*` envelope, `buffered_bytes: 10169`, **no content returned** | 900 lines inline, `[900 of 1200 lines shown]`, `Next: read_file("@cmd_3aa87fb7", start_line=901, end_line=1200)` |

The chunk shrinking from ~1000 lines to 900 is mechanism 1's fix visible in the
output: escaping is now charged, so the serialized response stays under the
threshold it is actually measured against.

Chain depth to reach raw text: 4-6 hops, sometimes never → **1**.
## Found while fixing, filed separately

`docs/issues/2026-08-25-file-slice-handle-refreshes-to-whole-file.md` — a
`@file_*` handle minted from an oversized *slice* of a real file is created with
`source_path` pointing at the whole file, so the first `get()` after an mtime
bump replaces the slice with the entire file. Confirmed live: a handle minted as
12 lines reported 41 and served the file's line 1. Not bundled here — it needs a
view-vs-snapshot decision this fix does not force.
