---
id: b64e425fddb86e4d
kind: bug
status: fixed
title: memory(action="read") returns a buffered handle with no navigational hint
owners:
- marius
tags:
- cluster/unclassified
---

## Summary

`memory(action="read", topic=...)` on a topic whose content overflows the inline
budget returns `{"file_id": "@file_...", "total_lines": N}` with **no `hint`
field** — every other buffering call site in this codebase (`read_markdown.rs`'s
three tiers, `read_file.rs`'s slice buffering, `grep`, `run_command`) pairs the
handle with a `hint` naming the concrete follow-up call. Without it, an agent has
no cue that the buffer needs to be dereferenced at all.

## Symptom (Effect)

Observed live in a peer session's transcript, relayed by the user:

```
● codescout - memory (MCP)(action: "read", topic: "architecture")
{
  "file_id": "@file_b66b96a3",
  "total_lines": 217
}

∴ I should start by identifying the actual population count first.
```

The agent called `memory(action="read", topic="architecture")` to load project
context, got back a bare two-field envelope, and — with nothing in the response
pointing at how to read the buffer — moved straight to an unrelated subtask.
The memory content it asked for was never actually read.

## Reproduction

```
memory(action="write", topic="large-topic", content=<>10KB of markdown>)
memory(action="read", topic="large-topic")
```

Commit at time of filing: `bfe69f78` (branch `experiments`).

Response:
```json
{"file_id": "@file_xxxxxxxx", "total_lines": 300}
```

No `hint` field, contrast with e.g. `read_file` on an oversized markdown file:
```json
{"lines": 300, "headings": [...], "file_id": "@file_...", "hint": "use \"@file_...\" — heading=\"## Section\" or start_line/end_line"}
```

## Environment

codescout MCP server, any transport. Affects both `memory(action="read")`
call sites that route through `apply_sections_filter` — plain topic reads and
the workspace-merged read path. Not specific to OS/runtime.

## Root cause

`apply_sections_filter` (`src/tools/memory/mod.rs:653-696`) builds the buffered
overflow envelope directly:

```rust
json!({ "file_id": file_id, "total_lines": total_lines })
```

with no `hint` key, unlike every sibling buffering site:

- `src/tools/markdown/read_markdown.rs:124-149` (oversized multi-heading join)
- `src/tools/markdown/read_markdown.rs:228-291` (oversized single-section match)
- `src/tools/markdown/read_markdown.rs:353-375` (oversized line-range extract)
- `src/tools/markdown/read_markdown.rs:424-448` (whole-file heading-map tier)
- `src/tools/read_file.rs` slice buffering (`extract_lines_to_json_budget` site)

all of which construct a `hint` naming the concrete `read_file("@file_id", ...)`
follow-up (and often `next_actions`). `docs/PROGRESSIVE_DISCLOSURE.md`'s own
contract states the overflow envelope's typical fields as `{output_id, summary,
hint, ...}` — this call site is the one place in the tool layer that mints a
buffer handle without the hint half of that contract.

Two call sites reach this function unguarded
(`references(apply_sections_filter)` → `src/tools/memory/mod.rs:951,966`), so
the gap affects both the single-topic read and the workspace-union read.

Note on capability, checked while designing the fix: the buffer this function
mints uses a synthetic `@memory:{topic}:filtered` path, and `store_file`
(`src/tools/output_buffer.rs:517-527`) unconditionally sets `source_path = None`
for any `@`-prefixed path. `is_markdown_target` (`src/tools/markdown/read_markdown.rs:577-590`)
resolves an `@file_*` buffer's markdown-ness from that same `source_path`, so it
is always `false` here regardless of the topic's actual markdown content —
`heading=` addressing genuinely does not work on this buffer today. The hint
must say `start_line`/`end_line` only, not `heading=`, or it would advertise a
capability the buffer doesn't have.

*Measured 2026-09-19: read the two call sites via `references`, and grepped
every `store_file`/`store_file_excerpt` call site in the tool layer for an
accompanying `hint` key — `apply_sections_filter` is the only one missing it.*

## Evidence

Sibling site (`src/tools/markdown/read_markdown.rs:424-436`), for contrast:

```rust
let hint = if all_headings.is_empty() {
    format!("use {:?} — start_line/end_line", file_id)
} else {
    format!(
        "use {:?} — heading=\"## Section\" or start_line/end_line",
        file_id
    )
};
let mut result = json!({
    "lines": total_lines,
    "headings": headings_json,
    "file_id": file_id,
    "hint": hint,
});
```

Buggy site (`src/tools/memory/mod.rs:677-688`):

```rust
let value = if crate::tools::exceeds_inline_limit(&content) {
    let total_lines = content.lines().count();
    let synthetic_path = format!("@memory:{topic}:filtered");
    let file_id = output_buffer.store_file(synthetic_path, content);
    if missing.is_empty() {
        json!({ "file_id": file_id, "total_lines": total_lines })
    } else {
        json!({ "file_id": file_id, "total_lines": total_lines, "missing": missing })
    }
}
```

Existing regression test `memory_large_read_buffers_as_file_ref`
(`src/tools/memory/tests.rs:1577-1631`) asserts `file_id` and `total_lines` are
present and that the handle is line-navigable, but never asserts a `hint` key
exists — so this gap shipped a green suite.

## Hypotheses tried

1. **Hypothesis:** the missing hint is specific to the workspace-union read
   path, not the plain topic read. **Test:** read both call sites
   (`src/tools/memory/mod.rs:951` and `:966`) — both call the same
   `apply_sections_filter`. **Verdict:** rejected — both paths share the one
   function, so both are affected identically. **Evidence link:** Root cause.
2. **Hypothesis:** the buffer supports `heading=` addressing on the stored
   markdown content, so the fix could recommend it. **Test:** traced
   `is_markdown_target` against `store_file`'s `source_path = None` behavior for
   `@`-prefixed paths. **Verdict:** rejected — confirmed unconditional, so the
   hint must not offer `heading=`. **Evidence link:** Root cause.

## Fix

Add a `hint` field to both branches of `apply_sections_filter`'s buffered
envelope, matching the sibling idiom but truthful about what this buffer
actually supports (`start_line`/`end_line` only — see Root cause):

```rust
let hint = format!("use {file_id:?} — start_line/end_line to browse the full content");
```

merged into both the `missing.is_empty()` and non-empty branches.
Change lives in `src/tools/memory/mod.rs` (`apply_sections_filter`).

Fixed in `36f0049b78c337ef134301f1c782eeba58c5d3a6` on `experiments` (patch-id `0aa25506122c203e129f8bd40334a0ebe1b26df6`). `apply_sections_filter` (`src/tools/memory/mod.rs`) now attaches `hint = format!("use {file_id:?} — start_line/end_line to browse the full content")` to both branches of its buffered envelope.

## Tests added

Extended `memory_large_read_buffers_as_file_ref`
(`src/tools/memory/tests.rs:1577`) to assert `result["hint"]` is present,
names the returned `file_id`, and does not mention `heading=`. Mutation-probed
in an isolated worktree 3 ways before commit — blank the hint value, add
`heading=` to it, drop the `hint` key entirely — each KILLED by a distinct
assertion. Full gate (`fmt-mine`, `clippy --features local-embed`,
`test --no-default-features`, `test`) green before commit.

## Workarounds

None needed by users — the buffer itself is fully readable via
`read_file("@file_id", start_line=N, end_line=M)`; the defect is purely the
missing discoverability cue, which any caller who already knows the `@file_*`
convention can route around by guessing.

## Resume

N/A — fixed.

## References

- `docs/PROGRESSIVE_DISCLOSURE.md` — the overflow-envelope contract this call
  site violates.
- `src/tools/markdown/read_markdown.rs` — four sibling sites implementing the
  contract correctly.
- Reported via a cross-session message relaying a peer's observed transcript,
  2026-09-19.
