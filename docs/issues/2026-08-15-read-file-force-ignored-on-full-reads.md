---
status: open
opened: 2026-08-15
closed:
severity: medium
owner: marius
related: []
tags: [read_file, progressive-disclosure, external-report]
kind: bug
---

# BUG: `force=true` is silently discarded on whole-file `read_file` reads

## Summary

`read_file(path, force=true)` on a source file larger than the inline limit returns a
symbol outline and zero lines of content. `force` never reaches the whole-file code
path, so there is no parameter of any kind that returns a >10 KB source file inline.
The parameter is accepted and ignored.

## Symptom (Effect)

Reported by an external user: `read_file("src/tools/read_file.rs", force=true)` on a
59,718-byte file returned a 25-symbol outline and **0 lines of content**. No error, no
note that `force` had no effect.

## Reproduction

```
read_file(path="<any source file over ~10 KB>", force=true)
```

Observed at `821f9d0d` on branch `experiments`. Expected: raw content, or an explicit
statement that `force` does not apply here. Actual: the buffered symbol summary,
identical to the call without `force`.

## Environment

Reported on macOS against `experiments @ d7988aca`; verified on Linux at `821f9d0d`.
Transport-independent — this is a parameter-routing defect, not an output-channel one.

## Root cause

`read_file`'s dispatch passes `force` to `read_with_line_range` only:

```rust
let force = input["force"].as_bool().unwrap_or(false);
if let (Some(start), Some(end)) = (start_line, end_line) {
    return read_with_line_range(path, …, start, end, &source_tag, ctx, force);
}
read_full_file(path, &text, &resolved, &input, &source_tag, ctx)   // ← no `force`
```

`read_full_file` (`src/tools/read_file.rs:643-752`) has no `force` parameter to
discard. Its `if crate::tools::exceeds_inline_limit(text)` branch stores the buffer and
returns **early** — before `OutputGuard::from_input(input)` is reached near the bottom
of the same function. So neither `force` nor `detail_level` can influence the
overflow decision.

*Verified 2026-08-15 by reading `symbols(name="read_full_file", include_body=true)` —
the signature is `(path, text, resolved, input, source_tag, ctx)` and the early return
precedes every `OutputGuard` construction. Read from source, not measured at runtime on
this host; the external reporter measured it on his.*

## Evidence

### Current signature and early return

`src/tools/read_file.rs:643-752`, `read_full_file`. The overflow branch sets only
`result["file_id"]` (plus markdown `coverage`) and returns. `OutputGuard::from_input`
appears afterwards, governing only the under-budget path.

### The schema nuance, raised by the reporter himself

The `force` schema documents it as *"Skip source-symbol hint and read the raw line
range"* — so whole-file arguably was never in scope. That makes this a design gap
rather than a coding error. The user-visible effect is unchanged: an agent that passes
`force=true` believes it overrode the default and did not.

`get_guide("iron-laws-detail")` § Iron Law 1 reinforces the wrong expectation:
*"`force=true` returns raw bytes for any range"* — true for ranges, and the guide does
not say the whole-file path is excluded.

## Hypotheses tried

1. **Hypothesis:** `detail_level="full"` is the intended escape hatch for this path.
   **Test:** trace `OutputGuard::from_input` call site relative to the overflow return.
   **Verdict:** rejected — the guard is constructed after the early return, so
   `detail_level` cannot reach the overflow branch either.

## Fix

Not yet implemented. Two coherent options; this is a decision, not a coding question:

- **A — make `force` mean force.** Thread `force` into `read_full_file` and skip
  `exceeds_inline_limit` buffering when set. Honest with the parameter name, and the
  escape hatch the guide implies actually exists.
- **B — say it is range-only.** Amend the schema and Iron Law 1 to state that `force`
  applies to line ranges and that the whole-file path always buffers. Cheaper, and
  arguably the original intent.

Either resolves the report. Doing neither leaves a documented escape hatch inert.

## Tests added

None yet. Whichever option is chosen needs a test asserting the whole-file + `force`
contract explicitly, since the current behaviour is untested in both directions.

## Workarounds

Read the buffer: the full content is stored server-side and is byte-exact. Use
`read_file("@file_…", start_line=N, end_line=M)`, or pass an explicit
`start_line`/`end_line` with `force=true`, which does honour it.

## Resume

Decide A or B. If A: add a `force: bool` parameter to `read_full_file` in
`src/tools/read_file.rs:643` and gate the `exceeds_inline_limit` branch on `!force`;
add a test asserting a >10 KB source file returns `content` when forced. If B: edit the
`force` description in the input schema and the Iron Law 1 section of
`get_guide("iron-laws-detail")`, and close this as `wontfix` with the rationale.

## References

- `docs/trackers/bistriceanu/index.md` § B-1 — the external report and its provenance
- `docs/trackers/bistriceanu/full-read-fidelity-design.md` § D1 — the reporter's own writeup
- Related: `docs/issues/2026-08-15-read-file-buffered-summary-has-no-incompleteness-signal.md` (B-2, same function)
