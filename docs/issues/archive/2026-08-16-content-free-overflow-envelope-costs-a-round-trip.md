---
kind: bug
status: fixed
tags:
- progressive-disclosure
- buffer-handles
- librarian
- round-trip-cost
closed: 2026-08-16
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

**All three done.** Fix 1 shipped earlier (`2099d4ce`); fixes 2 and 3 landed 2026-08-16, and
fix 3's shape was decided by measurement rather than by the plan written here.

**1. Shape-aware generic fallback — done (`2099d4ce`).** Unchanged. It lives in
`src/tools/core/types.rs`, so every tool lacking a bespoke `format_compact` benefits, not
just the librarian.

**2. Per-action librarian summaries — done.** `matched_items_summary` lists what `find`
matched: id, status and an ellipsized title per row, capped at 8. `section_headings_summary`
renders `get`'s `preview.headings` with their level markers, so a heading can be passed
straight back as `artifact(get, heading="…")`.

The sequencing constraint this file recorded was honoured: the truncation warning is now an
**additional line, not a precondition**, and it leads — `truncate_compact` cuts from the tail
(BL-8), so a correctness signal placed after a long summary can be cut off.

**The trap in fix 2, worth recording:** `format_compact` *replaces* the generic fallback
(`unwrap_or_else` in `ToolContext::call_content`). Returning `Some` here would have silently
dropped fix 1's top-level key list — the thing a `json_path` is aimed with — trading one gap
for another. The summary therefore **composes**: librarian-specific lines first, generic
shape appended.

**3. A preview alongside the handle — done, and re-scoped by a measurement.** Rather than
assume fixes 1 and 2 had subsumed it, the question was put to a live call:

```
librarian(action="context", topic="frontmatter serialization and the librarian catalog")
→ summary: "Result stored in @tool_… (15357 bytes)\n  4 keys: markdown, included_ids,
            overflow, scope\n  arrays: included_ids[10]"
```

That answered it twice over.

- **`markdown` *was* the entire answer** — named, never shown. `dominant_text_preview` now
  names the largest string field and shows its first 240 chars. Bounded on both ends: below
  200 bytes the generic describer already prints the value verbatim, and above the cap we
  would be inlining the payload and undoing the buffering that produced the envelope.
- **A correctness bug the plan had not anticipated.** That same response carried
  `$.overflow.hint` saying *40 of 50 candidates omitted, discovery capped* — an
  incompleteness signal — and the summary dropped it, because the whole function was gated
  on `inner_name == "artifact"`. A partial answer that does not announce itself reads as a
  complete one. `overflow_hint` now promotes it for **every** librarian tool, declining only
  on the `artifact(get)` body cap, which is stated more loudly just above it.

The artifact-shaped messages stay gated on the tool name, so another action carrying a
similar-looking field is never described as an artifact body — the property
`compact_summary_none_for_non_artifact_tools` was written to protect.
## Tests added

Two from fix 1 in `src/tools/format.rs` (unchanged). Five added in
`src/librarian/adapter.rs`, all written before the implementation and watched fail:

- `compact_summary_lists_what_find_matched` — ids (so the follow-up call needs no second
  lookup), titles (the answer), and status (what a triage query filters on).
- `compact_summary_lists_section_headings_for_get` — headings *with* their level markers.
- `compact_summary_keeps_the_truncation_warning_first_and_adds_the_answer` — asserts on
  **relative position**, not just presence: the correctness signal must precede the answer
  because `truncate_compact` cuts from the tail.
- `compact_summary_promotes_an_overflow_hint_from_any_librarian_tool` — built from the real
  `librarian(context)` payload. Also asserts the body-cap case is announced **once**, since
  the generic path could otherwise duplicate the specific one.
- `compact_summary_previews_the_dominant_text_field_without_inlining_it` — the negative
  assertions carry the weight: a tail marker beyond the preview window must be absent, and
  the whole summary must stay under 1200 bytes. A preview that inlines the payload has
  undone the buffering it exists to summarise.

The two pre-existing `None` tests still pass unchanged, which is the check that the new
paths did not swallow the generic fallback.

Gate: **3973 tests**, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`.
## Workarounds

Both obsolete, kept for the record.

- ~~Follow the envelope's `hint`.~~ Still sound, and still improved by the jsonpath fix — but
  the summary now usually answers without a second call.
- ~~Prefer a narrower first call (`heading=`, `entry_filter=`).~~ Still the better habit when
  you know which slice you want. The difference is that `get` now *tells you the headings*,
  so the narrower call no longer requires already knowing them.
## Resume

None — all three fixes are in.

One thing worth carrying: this file's own Resume said *"decide whether fix 2 is still needed
once fix 1 lands"*, and later *"fix 3 … still the right idea"*. Both questions were settled
by **running the tool and reading the envelope**, not by reasoning about the code. The
measurement changed the answer in both directions — it confirmed fix 2's gap (`find` reporting
`items[50]` and nothing in it) and it re-scoped fix 3 from "emit a preview" into a preview
*plus* a dropped incompleteness signal that no part of the plan had named.
## References

- `docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md` — the hint half of
  the same envelope.
- `docs/issues/archive/2026-07-09-artifact-get-full-true-body-silent-truncation.md` — why the one
  existing librarian summary is shaped the way it is.
