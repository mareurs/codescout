---
status: open
opened: 2026-08-15
closed:
severity: high
owner: marius
related: []
tags: [read_file, progressive-disclosure, external-report]
kind: bug
---

# BUG: the buffered full-read summary carries no incompleteness signal

## Summary

When `read_file` buffers a large file, the returned summary contains no
`complete: false`, no shown-vs-total count, and no hint. The renderer prints
`line_count` as a bare header — `"1505 lines"` — which reads as a property of the
answer rather than a warning that 1,505 lines were withheld. An agent can conclude
from a preview without knowing it was a preview.

## Symptom (Effect)

The response to a full read of a large source file is:

```json
{ "type": "source", "line_count": 1505, "symbols": [...], "file_id": "@file_…" }
```

Rendered, the only signal that content was withheld is a trailing `Buffer: @file_…`,
which reads as a bonus affordance rather than a warning.

## Reproduction

```
read_file(path="<any source file over ~10 KB>")
```

At `821f9d0d`. Compare the response to `tree`'s, which announces its cap explicitly, or
to `run_command`'s, which carries shown/total.

## Environment

Reported on macOS against `experiments @ d7988aca`; verified on Linux at `821f9d0d`.

## Root cause

`read_full_file` (`src/tools/read_file.rs:643-752`) — the `exceeds_inline_limit` branch
builds its result from `summarize_source` / `summarize_markdown` / etc., adds only
`result["file_id"]` (plus markdown `coverage`), and returns.

Thirteen lines below in the **same function**, the under-budget branch constructs a
full `OverflowInfo { shown, total, hint }` with a tailored, tool-specific hint. So this
is a local inconsistency, not a design philosophy: the function does it right for the
milder case and wrong for the worse one.

*Verified 2026-08-15 by reading `symbols(name="read_full_file", include_body=true)`.
Inferred from source — not measured at runtime on this host.*

## Evidence

### The two branches, same function

`src/tools/read_file.rs:643-752`. Overflow branch: `result["file_id"] = json!(file_id)`
and nothing else. Under-budget branch: `OverflowInfo { shown: max_lines, total:
total_lines, hint: <tailored>, … }` written to `result["overflow"]`.

### The reporter's enabling discovery (not yet verified here)

He reports that `format_read_file_summary` already renders a `hint` field when one is
present:

```rust
if let Some(hint) = val["hint"].as_str() {
    out.push_str(&format!("\n  {hint}"));
}
```

If that holds, the fix needs population at the source only — no renderer change.
**Unverified on this tree; check before scoping the fix.**

### Why this ranked as the one real defect

The reporter spent a session hunting codescout defects and concluded: *"One codescout
defect produced a wrong deliverable."* This is that one.

## Hypotheses tried

1. **Hypothesis:** the omission is deliberate, to keep the summary small.
   **Test:** compare against sibling surfaces at the same budget.
   **Verdict:** rejected — `tree` emits an explicit `[depth capped at 3 …]` note and
   `run_command` carries shown/total at the same budget. `read_file`'s full-read path
   is the outlier, not the policy.

## Fix

Not yet implemented. Populate the overflow branch with an incompleteness signal:
`complete: false`, a shown-vs-total pair, and a `hint` naming the buffer handle and how
to page it. Verify the `format_read_file_summary` claim first — if the renderer already
handles `hint`, this is a source-population change only.

**Sequence with `docs/issues/2026-08-15-truncate-compact-tail-cut-destroys-overflow-signal.md`:**
that bug destroys tail-placed signals during compaction. Emit this hint at the **head**
of the compact summary, or it will be the first thing cut.

## Tests added

None yet. Needs a test asserting the buffered full-read response carries an
incompleteness marker, and one asserting it survives `truncate_compact`.

## Workarounds

Treat any `read_file` response containing `file_id` as incomplete by construction, and
page the buffer with `read_file("@file_…", start_line=N, end_line=M)`. The buffer is
byte-exact — the reporter verified two separate ranges against source.

## Resume

Read `format_read_file_summary` in `src/tools/read_file.rs` and confirm whether it
renders a `hint` field. Then add `complete: false` + counts + hint to the
`exceeds_inline_limit` branch of `read_full_file` (`src/tools/read_file.rs:643`),
placing them ahead of any row content in the compact summary.

## References

- `docs/trackers/bistriceanu/index.md` § B-2
- `docs/trackers/bistriceanu/full-read-fidelity-design.md` § D2 — the reporter's writeup
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — the sizing/overflow-hint contract this violates
