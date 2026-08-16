---
status: open
opened: 2026-08-16
closed:
severity: medium
owner: marius
related:
  - docs/issues/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md
  - docs/issues/2026-08-15-read-file-buffered-summary-has-no-incompleteness-signal.md
  - docs/trackers/open-issue-work-queue.md
tags:
  - progressive-disclosure
  - buffer-handles
  - librarian
  - round-trip-cost
kind: bug
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

`docs/issues/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md` fixed the *hint* so
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

**1. Make the generic fallback shape-aware.** Instead of restating the byte count, describe the
payload: top-level keys, the largest array's name and length, and — where cheap — a head of it. That
turns the envelope from metadata into a map, and benefits every tool lacking a bespoke summary, not
just the librarian's.

**2. Give the librarian per-action summaries.** `get` should name the artifact's title, status and
section headings; `find` should list the matched titles. Both are small and are the question the
caller asked. Keep the existing truncation warning as an additional line, not a precondition.

**3. Consider always emitting a small preview alongside the handle.** The current model is
all-or-nothing per result. A fixed-size head (the first N entries of the dominant array) would make
the common "just show me what is in there" case free.

Fix 1 is the widest win for the least code and should come first.

## Tests added

`N/A — not yet fixed.` A regression test should assert on the *observable* envelope: buffer an
`artifact(get)` response and assert its `summary` names the artifact's title, not only its byte
count. Asserting that `format_compact` returns `Some` would pass while the string stayed useless.

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

- `docs/issues/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md` — the hint half of
  the same envelope.
- `docs/issues/archive/2026-07-09-artifact-get-full-true-body-silent-truncation.md` — why the one
  existing librarian summary is shaped the way it is.
