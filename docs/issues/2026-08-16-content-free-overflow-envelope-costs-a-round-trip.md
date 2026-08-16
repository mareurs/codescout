---
kind: bug
status: mitigated
tags:
- progressive-disclosure
- buffer-handles
- librarian
- round-trip-cost
closed: null
opened: 2026-08-16
owner: marius
related:
- docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md
- docs/issues/archive/2026-08-15-read-file-buffered-summary-has-no-incompleteness-signal.md
- docs/trackers/open-issue-work-queue.md
severity: medium
---

# BUG: an overflow envelope with no compact summary spends a whole call returning nothing — `artifact(get)` answers with a handle and a byte count

## Summary

When a result overflows, the agent gets a compact envelope instead. For tools with a real
`format_compact` that envelope often **answers the question outright** — `grep` returns matching
lines, `symbols` returns the symbol list. For tools without one, the envelope is pure metadata:
`output_id`, a byte count, and a hint. The call cost a full round-trip and returned **no payload at
all**, guaranteeing a second call before any work can happen.

`artifact` is the high-traffic instance: **104 overflows in the live corpus** (7.9% of 1,309 calls),
each one a guaranteed wasted turn.

## Symptom (Effect)

Observed live 2026-08-16:

```
artifact(action="get", id="9a892c2a5976e296", full=true)
→ {
    "output_id": "@tool_09956521",
    "summary": "Result stored in @tool_09956521 (16819 bytes)",
    "hint": "read_file(\"@tool_09956521\", json_path=\"$.tags[*]\") to extract a specific
             field, or read_file(\"@tool_09956521\", start_line=N, end_line=M) to browse",
    "buffered_bytes": 16819
  }
```

Nothing here is about the artifact. Not its title, status, section headings, entry count — nothing
the caller asked `get` for. `summary` restates `output_id` and `buffered_bytes` in prose.

Contrast `grep`, whose envelope for a comparably-sized result carries actual matching lines with
file and line numbers, and is frequently sufficient on its own.

## Reproduction

```
artifact(action="get", id=<any artifact whose full response exceeds ~10 KB>, full=true)
```

Returns the metadata-only envelope above. The artifact's body must be **under** the `get` soft cap —
see § Root cause for why that matters.

## Environment

Linux, codescout `v0.15.0`, branch `experiments`, MCP stdio. Live-DB corpus, 21,638 calls.

## Root cause

`Tool::format_compact` returns `Option<String>`; `None` falls back to the generic
`"Result stored in <id> (N bytes)"`. Every librarian tool routes through one adapter, and that
adapter's implementation has exactly **one** case:

`src/librarian/adapter.rs` — `librarian_compact_summary(inner_name, result)` returns `Some` only
when *all* of: the tool is `artifact`, the result carries an `overflow` object, and that object has
`shown_lines` + `total_lines`. That object is emitted only when `get` truncates a body at
`SOFT_CAP_LINES`.

So the one summary the librarian can produce is a **truncation warning** — added to fix a real
silent-truncation bug (`docs/issues/archive/2026-07-09-artifact-get-full-true-body-silent-truncation.md`),
and correct for that case. Every other librarian response — `get` on a body under the cap, `find`,
`graph`, `state_at` — returns `None` and falls through to the metadata envelope.

`measured 2026-08-16: artifact(get, full=true)` on a tracker whose body is under the soft cap →
no `overflow` object → `None` → generic envelope, reproduced above.

**Why this is a design gap rather than a missing implementation.** The generic fallback is the
*right* default for a tool whose payload has no summarisable shape. But it is the fallback for
"nobody wrote a summary" too, and those two cases are indistinguishable to the caller. The envelope
does not say "there is nothing useful to show" — it just shows nothing.

## Evidence

### Overflow volume by tool (live DBs, 2026-08-16)

```
tool             calls   overflowed   has a real compact summary?
run_command       7238        714     yes
symbols           2933        228     yes
artifact          1309        104     only for the truncation case
grep              2285         68     yes
librarian          146         58     no
read_file         2620         40     yes
semantic_search    172         36     yes
```

`artifact` + `librarian` = **162 content-free envelopes**, each costing a turn that returned nothing
actionable.

### It compounds the hint bug

`docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md` fixed the *hint* so
the follow-up call is likely to succeed. That reduces the cost of the second call; it does not
remove it. The two together define the shape of a good envelope: **carry enough that the second call
is often unnecessary, and make it work when it is.**

## Hypotheses tried

1. **Hypothesis:** the librarian adapter simply has no `format_compact`.
   **Test:** read `src/librarian/adapter.rs`.
   **Verdict:** **rejected** — it has one, wired correctly. It is *narrow*, not absent: gated on an
   `overflow` object only `get`'s body-truncation path emits.
   **Evidence:** § Root cause.

## Fix

**Status: fix 1 done (`2099d4ce`), fixes 2 and 3 still open.** Marked `mitigated` rather
than `fixed`, and deliberately NOT archived — the widest case is handled, the bug is not.

**1. Make the generic fallback shape-aware. — DONE (`2099d4ce`).** It now returns the
top-level keys (so a `json_path` can be aimed without a second call), each array's length,
and short scalars verbatim, since those are frequently the answer outright. Bounded: a wide
object must not spend the whole summary budget listing keys, and a large value is *named,
not inlined* — inlining it would undo the buffering that produced the envelope.

One thing worth pinning, because it decided where the fix went: this is not librarian code.
The fallback lives in `src/tools/core/types.rs`, so **every** tool lacking a bespoke
`format_compact` was taking it; the librarian is just where it was loudest. Fixing
`librarian/adapter.rs` instead would have left every other such tool untouched.

**2. Give the librarian per-action summaries. — not done.** `get` naming the artifact's
title, status and section headings; `find` listing the matched titles. Fix 1 now gets some
of this incidentally — `title` and `status` are short scalars and so appear — but headings
and matched-title lists are still absent, and those are the question the caller asked.

Note the sequencing constraint this section already recorded: *"Keep the existing
truncation warning as an additional line, not a precondition."* That rework belongs to fix
2 and was left alone here. The existing precondition guards a real silent-truncation bug
that once cost duplicate sections
(`docs/issues/archive/2026-07-09-artifact-get-full-true-body-silent-truncation.md`), so it
is not to be relaxed casually.

**3. Always emit a small preview alongside the handle. — not done.** Still the right idea
and still the largest change of the three.
## Tests added

Two in `src/tools/format.rs`, both against the real emitter.

- `the_generic_fallback_describes_the_payload_instead_of_the_envelope` — built on the shape
  of a real librarian `artifact(get)` response, the measured case. Asserts the key count,
  that the big field is *named*, array sizes, short scalars verbatim — and, negatively,
  that a 20 KB body is **not** inlined and the whole description stays under 600 bytes. The
  negative assertions are the load-bearing ones: a describer that solves this bug by
  pasting the payload back has reintroduced the problem buffering exists to solve.
- `describe_payload_shape_handles_arrays_and_declines_scalars` — root arrays report element
  keys (what a `[*]` projection needs to name a field), and bare scalars / empty objects
  return `None` so the caller keeps its own wording rather than being handed a description
  of a scalar.

Gate: `cargo fmt` + `cargo clippy --all-targets -D warnings` clean, `cargo test --lib`
3770 passed / 0 failed / 7 ignored.
## Workarounds

- Follow the envelope's `hint` — since 2026-08-16 it names a real path derived from the payload's
  largest array, so the second call usually lands.
- For artifacts specifically, prefer a narrower first call: `artifact(get, id=…, heading="…")` or
  `entry_filter=…` returns the slice you want without ever overflowing.

## Resume

Start with fix 1, in the generic fallback that builds `"Result stored in <id> (N bytes)"` — find its
emission site and replace the byte-count prose with a shape description (top-level keys, largest
array name + length). Measure before and after on the same `artifact(get, full=true)` call used in
§ Symptom.

Then decide whether fix 2 is still needed once fix 1 lands: a good generic summary may cover the
librarian's cases well enough that bespoke per-action summaries are not worth the surface.

## References

- `docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md` — the hint half of
  the same envelope.
- `docs/issues/archive/2026-07-09-artifact-get-full-true-body-silent-truncation.md` — why the one
  existing librarian summary is shaped the way it is.
