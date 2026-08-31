---
kind: bug
status: fixed
tags:
- progressive-disclosure
- symbols
- output-buffer
- external-report
- cluster/capped-result-presented-as-complete
closed: null
opened: 2026-08-15
owner: marius
related: []
severity: high
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

Fixed on `experiments` in `bb2a9625`. All nine sites, producer-side, as prescribed.

**`truncate_compact` is untouched**, per this section's own instruction — a tail cut is
correct for prose, and the cutter was the wrong layer.

Two helpers in `src/tools/format.rs` carry it:

- `overflow_head(val)` renders the note, or `""` when there is no overflow object, so a
  caller pushes it unconditionally with no surrounding `if let`.
- `insert_below_header(body, extra)` slots it under the **first** line rather than above it.

That second helper exists because of a collision worth recording. Placing the note first
broke `grep_capped_collection_never_renders_as_a_complete_result` (a concurrent session's
BL-2 fix, `4b77dff5`), which requires the first line to carry the `capped` marker — it is
the line a reader anchors on. Both requirements are real and both hold with the note
**second**: the header keeps first place, and the note is metres from the top of a budget
measured in kilobytes. Weakening the other guard would have been the easy, wrong move.

**Three more tail-placed signals came along**, each hidden by the same cut on exactly the
results that needed them, and none named in the original title:

| Signal | Surface |
|---|---|
| `completeness_warning` | `grep` |
| `depth_capped` note | `tree` |
| `[lsp warming]` marker | `symbols` (file overview) |

`tree` is the surface `get_guide("progressive-disclosure")` cites as the tool that *does*
announce its cap — tail placement made that reputation hold only below the compaction
threshold, exactly as this section predicted.

The audit above was **re-derived before editing** rather than trusted: still nine sites
across the same five surfaces at `64082e8e`, ~200 commits after the `821f9d0d` measurement.
Only the line numbers had moved.
## Tests added

`every_surface_keeps_its_overflow_note_above_the_truncation_cap` (`src/tools/format.rs`)
is the regression test, and it is deliberately end-to-end: it renders through each tool's
real `format_compact`, applies the real `COMPACT_SUMMARY_*` caps `call_content` uses, and
asserts the hint and the withheld-count are still present. It also asserts each fixture
exceeds the hard cap, so it cannot pass vacuously.

A test of `overflow_head` alone would have passed while all nine sites went on appending
at the tail — which is precisely the state it replaced.

Plus three unit tests: the no-overflow case contributes no stray newline; the header stays
first; single-line bodies and empty extras behave.

Mutation-verified on `read_file`, the sharpest surface. Restoring the tail append produces
a cut summary reading `4321 lines`, 150 identical content lines, `… (truncated)` — and
nothing whatever about the 4,271 lines withheld or how to reach them. That output is the
bug, printed.

**Verified live** on the rebuilt server, on a real 28,390-byte `symbols` result: the note
renders second, immediately under the `src/tools/symbol/tests.rs (149)` header, and
survives a cut that lands around row 44 of 149.

Gate: `cargo fmt` + `cargo clippy --all-targets -D warnings` clean, `cargo test --lib`
3765 passed / 0 failed / 7 ignored.
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
- Related: `docs/issues/archive/2026-08-15-read-file-buffered-summary-has-no-incompleteness-signal.md` — its hint had to be head-placed or this bug would eat it, and it was (`16a6b561`, using the helpers this fix added)
