---
kind: bug
status: fixed
tags:
- read_file
- output_buffer
- buffering
- progressive-disclosure
closed: 2026-08-25
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

### Scope was wider than this file claimed

The filed report named one call site. Enumerating every `store_file` caller
(`grep`, after `references` returned 0 with a warming-index warning) found
**four** production sites that mint a handle from a derived subset under a real
path, all of which refresh into the whole file:

| Site | What it stores |
|---|---|
| `read_file.rs` — `read_with_line_range` | an oversized line range |
| `read_markdown.rs` — `read_markdown_multi_heading` | several joined sections |
| `read_markdown.rs` — `read_markdown_single_heading` | one oversized section |
| `read_markdown.rs` — `read_markdown_line_range` | an oversized line range |

Two further sites store a file's WHOLE content (`read_full_file`,
`read_markdown_default_tiers`); for those the refresh is the intended freshness
guarantee and is left alone.

A fifth site, `memory/mod.rs` `apply_sections_filter`, had already hand-rolled a
workaround — a `@`-prefixed *synthetic* path, with a comment saying it exists to
stop `get_with_refresh_flag` stat-ing a non-existent file. Someone hit this trap
before and routed around it locally.

### The design flaw, stated

`source_path` conflates two questions: **"where did this come from?"**
(diagnostics) and **"may I re-read the whole file into this entry?"** (refresh
policy). The `path.starts_with('@')` check answers neither — it just happens to
disable both, which is why the memory workaround takes the shape it does.

### Decision: excerpts are snapshots

The two candidate directions this file originally listed were
(a) record the range and re-extract on refresh, or (b) mint excerpts with
`source_path = None`.

**(a) was rejected on evidence, not preference.** Two of the four sites extract
by HEADING, and a heading's line range moves when text above it changes — so no
stored range reproduces "the `## Foo` section" after an edit. Storing the
extraction *query* instead would mean re-running the tool, at which point the
entry is not a buffer.

**(b) generalizes across all four sites, and is consistent with the rest of the
system:** every other buffer kind is already a snapshot. `@cmd_*` and `@tool_*`
never re-run their source. The auto-refreshing whole-file handle is the special
case, and it is justified precisely because there the path fully determines the
content.

### The change

`src/tools/output_buffer.rs` — refresh policy is now chosen explicitly at the
call site instead of inferred from the path's first character:

- `store_file(path, content)` — unchanged contract, now documented as
  **whole-file only**: the entry auto-refreshes because the stored content IS
  the file.
- `store_file_excerpt(path, content)` — new. A derived subset. `path` is still
  recorded in `command` for diagnostics; `source_path` is deliberately unset, so
  the entry is a snapshot.
- `store_file_inner(path, content, source_path)` — shared mint/insert behind
  both.

The four excerpt sites now call `store_file_excerpt`. The two whole-file sites
are untouched, so every existing refresh test still pins the behavior it always
did.

`read_from_buffer`'s two sites (`@tool_x:$.jp`, `@file_x[13-24]`) are excerpts
too, but already get `source_path = None` via the `@` prefix. They were left
alone rather than bundled — they exhibit no defect today. The guardrail against
them regressing is `store_file`'s doc comment, which now names
`store_file_excerpt` as the required call for any derived subset.
**Fix commit:** `bde47270498c17d7f1edac704dcae0e007708715`, on **`experiments`**.

**patch-id:** `24ebd495d2f1e5e87ca8f9b4ddf211a4ae33b580`
(`git show <sha> | git patch-id --stable`).

The SHA is positional and dies when `experiments` is rebased; the patch-id is a
content hash of the diff and survives rebase and cherry-pick alike. Both
recorded once — nothing owed later, whichever way the fix reaches `master`.
## Tests added

Three, each RED before the change:

- `excerpt_handle_does_not_refresh_into_the_whole_file`
  (`src/tools/output_buffer.rs`) — the API-level contract, written as the
  explicit twin of the existing `get_file_handle_refreshes_when_file_modified`
  so the two policies sit side by side. Asserts the refresh flag stays false and
  that `command` still carries the path.
- `ranged_read_handle_stays_the_range_after_the_file_changes`
  (`src/tools/read_file.rs`) — the measured defect path, end to end through
  `ReadFile::call`. Before the fix the handle's content was `"REPLACED\n"`.
- `heading_excerpt_handle_stays_the_section_after_the_file_changes`
  (`src/tools/markdown/tests.rs`) — the heading-shaped extraction, which is the
  shape that ruled out the store-a-range design.

All three drive the mtime trigger deterministically with
`filetime::set_file_mtime`, following the pattern of the existing refresh tests
rather than sleeping.

Gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
— 4455 passed, 0 failed.

Not re-verified through a reconnected MCP server, and deliberately so: unlike
the sibling nested-buffer bug — whose first mechanism only appeared through
`call_content`'s serialization and was invisible to unit tests — this defect
lives entirely inside `OutputBuffer`, and the tests above exercise the real
`ReadFile::call` / `output_buffer.get` path directly. There is no MCP-layer
component for a live run to add.
## Workarounds

Re-issue the ranged `read_file` against the original path rather than reusing a
`file_id` across an edit. After
`docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md` was
fixed,
`next` already points at the original path, so ordinary pagination no longer
routes through the stale handle; the exposure is `grep`-ing a `file_id` that was
minted before an edit.

## Resume

N/A — fixed.
## References

- `docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md` — the sibling
  bug in the same code block; found while fixing it.
- `src/tools/read_file.rs` — `read_with_line_range` oversized branch.
- `src/tools/output_buffer.rs:188-296` — `get_with_refresh_flag`, `store_file`.

## Found while fixing, filed separately

`docs/issues/2026-08-25-read-markdown-next-actions-uses-file-line-numbers.md` —
`read_markdown`'s oversized-section error builds `next_actions` from the
section's line numbers **in the file** while addressing the excerpt handle,
which holds only the section. Following the tool's own suggestion returns
`start_line 304 exceeds file length 202`. Same defect class as the sibling
nested-buffer bug (mixed coordinate frames), different mechanism — so fixing
that one did not touch it. Not bundled here: this file is about refresh policy.
