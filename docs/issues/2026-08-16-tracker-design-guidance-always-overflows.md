---
status: open
opened: 2026-08-16
closed:
severity: low
owner: marius
related:
  - docs/trackers/2026-08-15-tool-usage-investigation.md
  - docs/issues/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md
tags:
  - librarian
  - progressive-disclosure
  - overflow
  - agent-guidance
kind: bug
---

# BUG: librarian's instructional actions always overflow — `tracker_design` has never been delivered inline

## Summary

Three `librarian` actions overflow their inline budget as their *normal* behaviour:
`link_scan` **8/8 (100%)**, `tracker_design` **6/6 (100%)**, `audit_doc_refs` **37/50 (74%)**.
`tracker_design` is the pointed case — it exists purely to *teach the caller* before they create a
tracker, and in the entire live corpus it has never once arrived inline. Guidance that always
lands in a buffer is guidance the caller must take an extra, independently error-prone step to
read.

## Symptom (Effect)

No error. The caller receives an overflow envelope (`output_id` / `summary` / `hint`) instead of
the content. Measured 2026-08-16 over live DBs (`user_version >= 2`):

```
action            calls  overflowed   pct
audit_doc_refs       50          37   74.0
link_scan             8           8  100.0
tracker_design        6           6  100.0
doctor               47           7   14.9
reindex              33           0    0.0
context               1           0    0.0
legibility_scan       1           0    0.0
```

`librarian` overall: 58 of 146 calls (**39.7%**) — the highest overflow rate of any tool in the
corpus.

## Reproduction

```
librarian(action="tracker_design")
```

Returns an overflow envelope rather than the teaching prompt. Same for `link_scan` on any
non-trivial project.

## Environment

Linux, codescout `v0.15.0`, branch `experiments`, MCP stdio. Live-DB corpus, 21,638 calls.

## Root cause

`inferred from the actions' contracts and the measured rates — emitting sites not yet read.`
These three actions return *inherently large* payloads: `tracker_design` returns a teaching prompt
plus an archetype library; `link_scan` returns a per-citation report; `audit_doc_refs` returns a
per-finding report. Each is assembled in full and handed to the same
`TOOL_OUTPUT_BUFFER_THRESHOLD` gate as any other result, so it buffers essentially every time.

`measured 2026-08-16: SELECT json_extract(input_json,'$.action'), count(*), sum(overflowed) FROM
tool_calls WHERE tool_name='librarian' GROUP BY 1`.

**The hypothesis this refutes matters for triage.** A 39.7% tool-level rate reads at first like
`context` — which packs a semantic bundle to a `max_tokens` budget and would be *expected* to be
large. `context` was called once and did not overflow. The rate is entirely the three actions
above, none of which take a budget parameter.

**Why `tracker_design` is worse than the other two.** `link_scan` and `audit_doc_refs` return
*findings* — a buffer is a reasonable place for a long report, and the caller queries it for what
they need. `tracker_design` returns *instructions the caller is supposed to follow before acting*.
Buffering those inverts the intent: the guide's purpose is to be read before `artifact(create)`,
and CLAUDE.md directs callers to `librarian(tracker_design)` for exactly that. A caller who skips
the extra fetch proceeds unguided — and per the filed overflow-recovery bug, that extra fetch is
itself a step where agents measurably fail.

## Evidence

### Comparison with the tools that solved this

`get_guide` returns comparably-sized instructional content and does **not** buffer it — the
`project-activation-bootstrap` and `progressive-disclosure` guides are auto-injected inline as
separate content blocks on first trigger. So the codebase already contains the pattern that fits
`tracker_design`: instructional payloads are delivered, not buffered.

## Hypotheses tried

1. **Hypothesis:** librarian's 39.7% overflow rate is `context` packing large bundles by design.
   **Test:** group librarian's overflow by `json_extract(input_json,'$.action')`.
   **Verdict:** **rejected.** `context`: 1 call, 0 overflows. The rate is `audit_doc_refs` (74%),
   `link_scan` (100%), `tracker_design` (100%).
   **Evidence:** § Symptom table.

## Fix

**Plan, in priority order:**

1. **`tracker_design`** — deliver inline like `get_guide` does. If the archetype library is what
   pushes it over, split it: return the teaching prompt inline and the archetype library behind an
   explicit follow-up, rather than buffering the whole thing.
2. **`link_scan` / `audit_doc_refs`** — buffering a long report is defensible; the defect is that
   the *summary* must then carry enough to act on. Ensure the compact summary names the finding
   counts by severity so the common case ("did anything break?") needs no buffer read at all.
3. Consider a `max_tokens`-style budget on the report actions, as `context` already has.

**Not a fix:** raising the global threshold. The budget exists for the model's context health; the
problem is these payloads' shape, not the cap.

## Tests added

`N/A — not yet fixed.` A regression test should assert `tracker_design`'s response is delivered
inline (no `output_id`) for the default invocation.

## Workarounds

- After `librarian(action="tracker_design")`, read the returned buffer immediately —
  `read_file("@tool_...")` — and do not proceed to `artifact(create)` from the summary alone.
- For `audit_doc_refs`, `fail_on` gives a pass/fail signal without reading the report.

## Resume

Read `tracker_design`'s emitting site and measure the payload against
`TOOL_OUTPUT_BUFFER_THRESHOLD` (9KB inline / 10KB threshold per
`get_guide("progressive-disclosure")`) to see how far over it lands. If it is marginal, trimming
the archetype library may be enough; if it is far over, the split in Fix step 1 is required. Root
cause above is inferred from contracts — read the bytes first.

## References

- `docs/trackers/2026-08-15-tool-usage-investigation.md` § History → 2026-08-16, *Overflow*.
- `docs/issues/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md` — why the extra
  buffer-fetch step is not free.
