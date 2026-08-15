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

**Confirmed at the bytes 2026-08-16** (previously *inferred from the actions' contracts*). The
mechanism is sharper than inferred, and it has a name:

**The payload caps its smallest component and leaves its largest uncapped.**

`tracker_design::call` (`src/librarian/tools/tracker_design.rs:532-578`) assembles:

| Component | Source | Capped? |
|---|---|---|
| `system_prompt` | `SYSTEM_PROMPT`, `tracker_design.rs:428-531` (103 lines) | **no** |
| `archetypes` | `archetypes()`, `tracker_design.rs:29-426` — all **nine** built unconditionally | **no** |
| `existing_trackers` | catalog walk | **yes** — `EXISTING_TRACKERS_CAP`, `tracker_design.rs:27` |

The one component with a cap, and the one with an overflow hint
(`existing_trackers_overflow_hint`), is the small variable list. The ~400 lines of static content
that actually blow the budget have neither.

**Measured, and the near-constancy is the proof.** `overflow_tokens` for the six
`tracker_design` calls in the corpus:

```
2026-07-16  10260      2026-08-06  10310
2026-07-17  10276      2026-08-15  10187
2026-08-03  10328      2026-08-15  10256
```

**10,270 tokens mean, against a `MAX_INLINE_TOKENS` of 2,500 — 4.1× the budget.** A 1.4% spread
across 13 months, while the catalog's tracker count grew substantially, is direct evidence that
the variable component contributes almost nothing: the number is the static payload.

(`overflow_tokens` is in tokens, not bytes — confirmed independently by `link_scan` at 4,614
overflowing 8/8, which a 10,000-**byte** threshold could not explain.)

**This resolves the open question this file previously carried.** The Fix section asked whether the
payload was marginal (trim) or far over (split). At 4.1×, **the split is required** — trimming the
archetype library cannot recover a 4× overshoot.

`measured 2026-08-16: SELECT json_extract(input_json,'$.action'), count(*), avg(overflow_tokens)
FROM tool_calls WHERE tool_name='librarian' GROUP BY 1` — `tracker_design` 6 calls, avg 10,270;
`audit_doc_refs` 37, avg 9,947 (max 18,724); `link_scan` 8, avg 4,614; `doctor` 7 of 47, avg 10,113
(max 45,088).

**The hypothesis this refutes matters for triage.** A 39.7% tool-level rate reads at first like
`context`, which packs to a `max_tokens` budget and would be *expected* to be large. `context` was
called once and did not overflow. None of the three actions that do overflow takes a budget
parameter.

**Why `tracker_design` is worse than the other two.** `link_scan` and `audit_doc_refs` return
*findings* — a buffer is a reasonable home for a long report, and the caller queries it for what
they need. `tracker_design` returns *instructions the caller is meant to follow before acting*.
Buffering those inverts the intent: CLAUDE.md directs callers here before `artifact(create)`, and a
caller who skips the extra fetch proceeds unguided — a step where agents measurably fail
(`docs/issues/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md`).
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

Measurement is **done** — the sizing this section previously called for returned 10,270 tokens,
4.1× the 2,500-token inline budget, so Fix step 1's conditional is resolved in favour of the
**split**; trimming cannot close a 4× gap.

Next action: split `tracker_design::call`
(`src/librarian/tools/tracker_design.rs:532-578`) so `system_prompt` returns inline and
`archetypes()` (`tracker_design.rs:29-426`) moves behind an explicit follow-up — e.g. an
`archetype` argument returning one by name, with the inline response listing the names and when to
use each. Size the trimmed envelope against `MAX_INLINE_TOKENS` before committing: `SYSTEM_PROMPT`
alone (103 lines) may still be close to the 2,500-token line.

Regression test: assert the default `tracker_design` invocation returns **no** `output_id`.
## References

- `docs/trackers/2026-08-15-tool-usage-investigation.md` § History → 2026-08-16, *Overflow*.
- `docs/issues/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md` — why the extra
  buffer-fetch step is not free.
