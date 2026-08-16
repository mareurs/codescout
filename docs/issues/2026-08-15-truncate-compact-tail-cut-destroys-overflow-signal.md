---
status: open
opened: 2026-08-15
closed:
severity: high
owner: marius
related: []
tags: [progressive-disclosure, symbols, output-buffer, external-report]
kind: bug
---

# BUG: `truncate_compact` cuts from the tail, destroying the overflow signal it should preserve

## Summary

Every buffered tool response is summarised by the tool's `format_compact`, then cut to
budget by `truncate_compact` — which trims from the **tail**. Surfaces that append
their overflow line at the tail therefore lose the incompleteness signal precisely when
the result is incomplete. One root cause, one choke point, several affected surfaces.

## Symptom (Effect)

Reported by an external user, runtime-observed on `symbols`: the rendered header
printed a count while the buffer's own `$.overflow` reported
`{"shown":149,"total":273}` — 124 symbols unaccounted for in the visible text. He
further reports that the advertised recovery does not recover them, because the handle
holds the already-truncated result.

He rated this *"far worse than I told you — and it's systemic."*

## Reproduction

Not yet reproduced end-to-end on this host. To reproduce: call `symbols` on a directory
large enough that `format_overview_symbols` output exceeds
`COMPACT_SUMMARY_HARD_MAX_BYTES` (3,000), then compare the rendered summary against
`read_file("@tool_…", json_path="$.overflow")`.

## Environment

Reported on macOS against `experiments @ d7988aca`. Mechanism verified on Linux at
`821f9d0d`; the runtime count discrepancy is not yet re-measured here.

## Root cause

Two halves, both verified from source 2026-08-15.

**The cut is from the tail.** `truncate_compact` (`src/tools/core/types.rs:322-336`)
returns `&text[..nl_pos]` at the last newline within `hard_max`, appending
`"\n… (truncated)"`. Everything after that byte is discarded.

**The signal is at the tail.** The single production call site is
`src/tools/core/types.rs:641`, on the overflow path:

```rust
let raw_summary = self.format_compact(&val)
    .unwrap_or_else(|| format!("Result stored in {} ({} bytes)", ref_id, json_len));
let summary = truncate_compact(&raw_summary,
    COMPACT_SUMMARY_MAX_BYTES, COMPACT_SUMMARY_HARD_MAX_BYTES);
```

So whatever a tool's `format_compact` places last is cut first. `src/tools/symbol/display.rs:225-229`
does exactly that:

```rust
let mut out = render_grouped(&groups, total, files, noun, render_item);
if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
    out.push('\n');
    out.push_str(&format_overflow(overflow));
}
if let Some(w) = warning { out.push_str("\n\nwarning: "); out.push_str(w); }
```

The overflow note **and** the warning are appended after the rows — so on any result
big enough to be cut, both are the first casualties.

*Verified 2026-08-15: `references(truncate_compact)` → exactly one production caller
(`types.rs:641`), read directly; `display.rs:220-234` read directly. Inferred from
source — the 149-vs-273 runtime discrepancy is the reporter's measurement, not ours.*

## Evidence

### The fix already exists on one surface — and documents this exact bug

`format_semantic_search` (`src/tools/semantic/semantic_search.rs:783-897`) carries a
comment naming the mechanism and the remedy:

> `truncate_compact` (src/tools/core/types.rs) cuts everything AFTER its budget from
> the string's TAIL, keeping only the prefix — and `call_content`'s overflow path uses
> this function's OUTPUT, verbatim, as the only summary the caller ever sees. Placed
> ahead of the result rows, these fields survive that cut regardless of how many/how
> large the rows are; placed after them, a summary that merely exceeds the hard cap
> silently drops every one of them along with the rows.

It builds `hint`, `worktree_state_warning`, `main_never_indexed_note`, `drift_note`,
and `truncated_hint` **first**, and has a regression test:
`format_semantic_search_keeps_state_fields_above_the_truncation_cap`.

**This is the load-bearing finding: the bug is understood and fixed on exactly one
surface, and the fix was never generalised.** Every other `format_compact` implementation
is unaudited against it.

## Hypotheses tried

1. **Hypothesis:** the tail-cut is itself the bug, and `truncate_compact` should
   preserve trailing lines. **Test:** read the function and its call site.
   **Verdict:** rejected — a tail cut is the correct default for prose, and
   `semantic_search` shows the working remedy is ordering at the producer, not changing
   the cutter. Fixing `truncate_compact` would be the wrong layer.

## Fix

Not yet implemented. The remedy is known and proven on one surface: **state/overflow
fields go at the head of `format_compact` output, above the variable-length rows.**

The audit is already done. `format_overflow` is tail-appended at **nine call sites
across five surfaces** (measured 2026-08-15 at `821f9d0d`, via
`grep("push_str\(&format_overflow")`):

| Site | Function | Surface |
|---|---|---|
| `src/tools/read_file.rs:874` | `format_read_file` | `read_file` |
| `src/tools/read_file.rs:888` | `format_read_file` | `read_file` |
| `src/tools/symbol/display.rs:227` | `format_search_symbols` | `symbols` |
| `src/tools/symbol/display.rs:252` | `format_overview_symbols` | `symbols` |
| `src/tools/symbol/display.rs:288` | `format_overview_symbols` | `symbols` |
| `src/tools/symbol/display.rs:335` | `format_overview_symbols` | `symbols` |
| `src/tools/grep.rs:493` | `format_grep` | `grep` |
| `src/tools/semantic/semantic_search.rs:893` | `format_semantic_search` | `semantic_search` |
| `src/tools/tree.rs:332` | `format_list_dir` | `tree` |

**Note the last two carefully.** `semantic_search` is the surface that already carries
the head-placement fix — yet its `format_overflow` call is *still* at the tail (`:893`).
So the existing fix protects the state fields (`hint`, `truncated_hint`,
`drift_note`, …) but **not** the overflow line itself. The generalisation is therefore
broader than "copy what semantic_search did" — even the reference implementation is
only half-protected.

`tree` is the surface `get_guide("progressive-disclosure")` and the reporter both cite
as the example of a tool that *does* announce its cap. If `:332` is tail-placed, that
reputation holds only below the compaction threshold.

Work per site: move the overflow/warning emission above the row rendering, and add a
regression test modelled on
`format_semantic_search_keeps_state_fields_above_the_truncation_cap`.

**Do not "fix" `truncate_compact` itself.** A tail cut is correct for prose, and the
proven remedy is producer-side ordering. Changing the cutter would be the wrong layer.
## Tests added

None yet. Each surface needs its own above-the-cap test; the existing
`semantic_search` one is the template.

## Workarounds

Do not trust a compact summary's completeness. Read the buffer's own overflow field
directly: `read_file("@tool_…", json_path="$.overflow")`, which is unaffected by the
cut.

## Resume

Grep for `fn format_compact` implementations across `src/tools/`, and for each, check
the emission order of overflow/warning lines relative to row rendering. Start with
`src/tools/symbol/display.rs:225-229` — confirmed tail-placed. Mirror the ordering and
the comment from `src/tools/semantic/semantic_search.rs:783-820`.

## References

- `docs/trackers/bistriceanu/index.md` § B-3
- `src/tools/semantic/semantic_search.rs:783-820` — the working remedy and its rationale
- Related: `docs/issues/2026-08-15-read-file-buffered-summary-has-no-incompleteness-signal.md` — its new hint must be head-placed or this bug eats it
