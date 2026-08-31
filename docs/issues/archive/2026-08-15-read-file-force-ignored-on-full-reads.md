---
kind: bug
status: fixed
tags:
- read_file
- progressive-disclosure
- external-report
- cluster/accepted-parameter-silently-dropped
closed: 2026-08-17
opened: 2026-08-15
owner: marius
related: []
severity: medium
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

**Measured on this host 2026-08-17 at `021c130d`, before any edit.**
`read_file("src/librarian/classify.rs", force=true)` — 10,559 bytes, just over the
10,000-byte inline limit — returned:

```
… showing 0 of 378 — Outline only — no file content included.
```

Zero content lines, and no mention of `force` anywhere in the response. The
read-from-source root cause was correct; it is now measured rather than inferred. The
file chosen was the smallest source file over the threshold, so a *working* `force`
would have cost 10 KB of context rather than 60 — the probe was sized to be safe under
either outcome.

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

**Landed `2703410e` (experiments). Neither A nor B as filed — B was already true, and
the live defect was a third thing.**

A is wrong on design grounds. Progressive disclosure is the project's stated principle
(`docs/PROGRESSIVE_DISCOVERABILITY.md`), and `force` was never a budget override: it
bypasses the *symbol-overlap refusal* on a line range. Letting it return an arbitrarily
large source file inline would defeat the one guarantee the output budget exists to make.

B was already shipped, and that is the part this file got wrong. The input schema said
"read the raw line **range**", and Iron Law 1 — as restored by `d2cf4449` — says
`force=true` answers the range-overlap refusal. Both surfaces already scoped it.

So the live defect was neither the budget nor the wording. It was the **silence**:
`read_full_file` accepted `force` and dropped it with no signal, which is exactly
`docs/issues/archive/2026-08-07-grep-zero-match-silent-about-hidden-skip.md` one tool
over — fixed there by making the result self-describing rather than by changing what the
tool searched.

Two surfaces, because they reach the caller at different moments:

- **Runtime.** `outline_hint()` extracted as a pure fn and given the flag. When `force`
  was passed and dropped, the overflow hint now says so and names what does work
  (`start_line`/`end_line` *together with* `force`).
- **Schema.** `force` now reads "Line ranges only — an oversized whole-file read is
  summarised either way." The runtime note only reaches a caller who has already spent
  the call; the schema is what they read before spending it.
## Tests added

All three in `src/tools/read_file.rs`'s `tests` module, red observed before the fix,
each failing on its own assertion:

- `outline_hint_says_force_did_not_apply_when_forced` — a discarded `force=true` is named
  in the hint, *and* the hint says what does work.
- `outline_hint_stays_silent_about_force_when_not_forced` — the complement, over both
  `is_source` arms. This is the half that keeps the note from becoming boilerplate, and
  its passing while the first failed is what established the RED was not vacuous.
- `force_schema_says_what_a_whole_file_read_does` — pins the schema half.

`outline_hint` is pure on purpose, so the wording is testable without a `ToolContext` or
a >10 KB fixture on disk — the same shape as `enforce_file_cap` in `audit_doc_refs`.
## Workarounds

Read the buffer: the full content is stored server-side and is byte-exact. Use
`read_file("@file_…", start_line=N, end_line=M)`, or pass an explicit
`start_line`/`end_line` with `force=true`, which does honour it.

## Resume

N/A — fixed and archived.
## References

- `docs/trackers/bistriceanu/index.md` § B-1 — the external report and its provenance
- `docs/trackers/bistriceanu/full-read-fidelity-design.md` § D1 — the reporter's own writeup
- Related: `docs/issues/archive/2026-08-15-read-file-buffered-summary-has-no-incompleteness-signal.md` (B-2, same function)
