---
kind: bug
status: open
tags:
- read_file
- output_buffer
- buffering
- progressive-disclosure
closed: null
opened: 2026-08-25
owner: marius
related:
- docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md
severity: medium
---

# BUG: a `@file_*` handle minted from an oversized *slice* silently refreshes into the whole file

## Summary

`read_file(path, start_line=N, end_line=M)` on a real file, where the extracted
range exceeds the inline budget, parks that range under a `@file_*` handle and
hands it back as `file_id`. The handle is created with `source_path` set to the
**whole file**, so the first `get()` after any mtime bump replaces the stored
slice with the entire file's contents. The handle keeps its identity and its
advertised meaning ("the range you asked for") while silently changing what it
holds.

## Symptom (Effect)

A handle minted as 12 lines reports 41 lines, and its line 1 is the file's line
1 rather than the slice's first line.

## Reproduction

Commit `2bba5309f8d855f34767b66315e0e846fa067bdc`, branch `experiments`, live
MCP server (`cargo rb` + `/mcp`).

1. Build a 40-line file whose lines are ~900 bytes, so a 12-line slice clears
   the 9000-byte inline budget:

   ```
   awk 'BEGIN{for(i=1;i<=40;i++){printf "line %04d ", i; for(j=1;j<=900;j++) printf "x"; printf "\n"}}' > /tmp/slice-refresh.txt
   ```

2. `read_file(path="/tmp/slice-refresh.txt", start_line=13, end_line=24)`
   → returns a chunk plus `Buffer: @file_XXXX`. That handle holds the 12-line
   slice, whose line 1 is `line 0013`.

3. Bump the file's mtime: `printf 'appended line\n' >> /tmp/slice-refresh.txt`

4. `read_file(path="@file_XXXX", start_line=1, end_line=1)`

**Expected:** `line 0013` (the slice's first line), or a stale-handle error.
**Got:** a `41 lines` header and `line 0001` — the handle now holds the whole
file, including the appended line.

## Environment

- codescout `0.15.0`, branch `experiments`, Linux, stdio MCP transport.
- Observed 2026-08-25 against the running server.

## Root cause

Two correct-in-isolation behaviors compose into a wrong one.

- `read_with_line_range` (`src/tools/read_file.rs`, oversized branch) stores the
  **slice** but passes the **whole file's** path as the buffer's source:
  `store_file(resolved.to_string_lossy().to_string(), content.clone())`, where
  `content` is `extract_lines(text, start, end)`.
- `store_file` (`src/tools/output_buffer.rs:266-296`) treats a non-`@` path as a
  real filesystem path and sets `source_path = Some(path)`.
- `get_with_refresh_flag` (`src/tools/output_buffer.rs:188-255`) then honours
  that source: when `mtime_ms > entry.timestamp` it does
  `entry.stdout = std::fs::read_to_string(&path)` — the **whole file**, with no
  memory that the entry was only ever a range of it.

The refresh mechanism is right for a handle minted by a *whole-file* read
(`read_full_file`, same `store_file` call with the same path) — there the stored
content and the file are the same thing, so re-reading is exactly the intended
freshness guarantee. It is wrong for a handle minted from a range, because the
entry does not record the range it came from.

measured 2026-08-25: the reproduction above, run against the live server —
`read_file("@file_3a625da7", start_line=1, end_line=1)` returned `41 lines` and
`line 0001` after an append, for a handle minted as lines 13-24.

## Evidence

Handle minted from `start_line=13, end_line=24` (note the slice's own first line):

```
line 0013 xxxxxxxx…
…
  Buffer: @file_3a625da7
  [9 of 12 lines shown]
```

After `printf 'appended line\n' >> …`:

```
read_file("@file_3a625da7", start_line=1, end_line=1)
→ 41 lines

  line 0001 xxxxxxxx…
```

## Hypotheses tried

1. **Hypothesis:** `store_file` guards against this the way it guards buffer-ref
   paths. **Test:** read `store_file`'s body. **Verdict:** rejected — the guard
   is `path.starts_with('@')`, which only covers handles-as-paths (the
   `store_file_with_buffer_ref_path_survives_get` case). A real path from a
   ranged read passes straight through.

## Fix

Not yet fixed. Two candidate directions, unranked:

- Record the range on the entry (e.g. `source_range: Option<(usize, usize)>`) and
  have `get_with_refresh_flag` re-extract that range after re-reading, so the
  handle keeps meaning what it said it meant.
- Or mint ranged slices with `source_path = None`, making them immutable
  snapshots like the buffer-ref case, and accept that they go stale rather than
  silently widen.

The first preserves the freshness guarantee; the second is smaller. Deciding
between them needs a call on whether a ranged handle is a *view* or a *snapshot*
— which is the question the current code never answers.

## Tests added

None yet — this file is the capture, not the fix.

## Workarounds

Re-issue the ranged `read_file` against the original path rather than reusing a
`file_id` across an edit. After
`docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md` was
fixed,
`next` already points at the original path, so ordinary pagination no longer
routes through the stale handle; the exposure is `grep`-ing a `file_id` that was
minted before an edit.

## Resume

Decide view-vs-snapshot (above), then implement in
`src/tools/output_buffer.rs`. A regression test belongs next to
`store_file_with_buffer_ref_path_survives_get` in that file's `tests` module:
mint a ranged handle, bump the source's mtime, assert the handle still yields
the range (or errors) rather than the whole file.

## References

- `docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md` — the sibling
  bug in the same code block; found while fixing it.
- `src/tools/read_file.rs` — `read_with_line_range` oversized branch.
- `src/tools/output_buffer.rs:188-296` — `get_with_refresh_flag`, `store_file`.
