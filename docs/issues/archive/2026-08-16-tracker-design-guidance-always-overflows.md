---
kind: bug
status: fixed
tags:
- librarian
- progressive-disclosure
- overflow
- agent-guidance
closed: 2026-08-16
opened: 2026-08-16
owner: marius
related:
- docs/trackers/2026-08-15-tool-usage-investigation.md
- docs/issues/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md
severity: low
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

**Implemented 2026-08-16 — `0ca6891b` (`experiments`), with a follow-up correction in `7c31f87c`.**
No pending-master-SHA line: the promotion path is fast-forward
(`git rev-list --left-right --count master...experiments` → `0 750`), so these SHAs already *are*
the master SHAs once promoted.

The split landed as planned, but the sizing work found a second
contributor the original Fix had not counted.

**1. `tracker_design` is now two calls.** The default response carries the teaching prompt plus an
**archetype menu** (`name` + `when_to_use`); `librarian(action="tracker_design", archetype="<name>")`
returns one archetype in full. An unknown name errors and lists the valid ones rather than returning
an empty envelope.

Step 1 of the system prompt is *"pick an archetype"*, so every archetype stays choosable inline —
only the bulky per-archetype fields (`params_shape_example`, `params_schema_example`,
`render_template_example`, `body_skeleton`, `prompt_template`) move behind the second call. The menu
also trims each `when_to_use` at its `" Examples:"` clause: examples illustrate, they do not
discriminate, so they belong with the full spec.

**2. `existing_trackers` was the uncounted half.** At `EXISTING_TRACKERS_CAP = 30` with six fields
per row it was **~7 KB of a 10 KB budget** — more than the entire archetype menu. Now capped at 5
rows carrying `{id, title, kind}`. `last_refreshed_at` and `refresh_count` answered no question the
collision check asks, and dropping them also removes an `augmentation::get` per tracker.

Step 7 was rewritten to match: the sample is now described as a **sample**, with the real collision
check being `artifact(find, kind="tracker", semantic="<concern>")` — scanning titles cannot catch a
duplicate worded differently, which is the collision that actually happens.

**3. The prompt was tightened** (Steps 2, 5, 5b, Anti-patterns) without dropping a single rule, and
its Final step was corrected — that half belongs to
`docs/issues/archive/2026-08-16-artifact-create-augment-drops-template-and-schema.md`, fixed in the
same pass as predicted.

**A correction the same-pass fix caused.** The Final step was rewritten to describe the *old*
`create` contract — "prompt and params are the ONLY fields, anything else is silently discarded" —
and the sibling fix then made that false. Both shipped in `0ca6891b` contradicting each other, and it
was caught by reading the live tool output after the rebuild rather than by re-reading the diff.
Fixed in `7c31f87c`. When one fix edits documentation describing a surface another fix changes, the
doc is written against a moving target; the check that catches it is running the shipped thing.

**Result: ~41,000 → 9,358 bytes** with a full catalog, against a 10,000-byte buffering threshold.
From overflowing on 6 of 6 calls to arriving inline.
## Tests added

In `src/librarian/tools/tracker_design.rs`:

| Test | Mutation it catches |
|---|---|
| `default_response_fits_inline` | folding the full specs back in, or growing the prompt past the margin |
| `default_response_still_lists_every_archetype_to_choose_from` | splitting so hard that Step 1 can no longer choose |
| `named_archetype_returns_the_full_spec` | a menu with no way to reach the detail |
| `unknown_archetype_names_the_valid_ones` | an empty envelope the caller must diagnose |

**The first test's fixture is the load-bearing part, and its first version was wrong.** Written
against an empty catalog it measured 10,396 bytes; seeded with a full catalog the same code measured
**17,456**. `existing_trackers` is populated in production and absent from a bare fixture, so the
empty-catalog version would have passed CI while the tool still overflowed on every real call — the
same *measuring the wrong population* error this investigation corrected in TU-5. The test now seeds
`EXISTING_TRACKERS_CAP` trackers with titles sized from this repo's actual ones.

Gate: **3842 passed, 0 failed**, `cargo clippy --all-targets -- -D warnings` clean.
## Workarounds

- After `librarian(action="tracker_design")`, read the returned buffer immediately —
  `read_file("@tool_...")` — and do not proceed to `artifact(create)` from the summary alone.
- For `audit_doc_refs`, `fail_on` gives a pass/fail signal without reading the report.

## Resume

N/A — fixed, verified live, archived.

**Live verification, 2026-08-16** (after `cargo rb` + `/mcp`): `librarian(action="tracker_design")`
returned **inline with no `output_id`**, where the same call buffered 40,756 bytes before. The menu,
the `archetype_detail` pointer and the 5-of-66 tracker sample all rendered as designed.

**Margin is ~640 bytes** (9,358 of 10,000). Growing `SYSTEM_PROMPT` or raising
`EXISTING_TRACKERS_CAP` will re-break this; `default_response_fits_inline` is what will say so — do
not "fix" a failure there by relaxing the assertion.

**Not addressed, and not filed separately:** `link_scan` (8/8 overflow) and `audit_doc_refs`
(37/50). Both return *findings*, where a buffer is defensible — their open half is Fix step 2, making
the compact summary carry counts by severity so the common question needs no buffer read.
## References

- `docs/trackers/2026-08-15-tool-usage-investigation.md` § History → 2026-08-16, *Overflow*.
- `docs/issues/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md` — why the extra
  buffer-fetch step is not free.

## Fix provenance

- **SHA:** `7c31f87c` (experiments-only) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `3fb22773fe1f9ff1eff949214a042637ffd3d7f4` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
codescout's Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep 3fb22773fe1f /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several
branches (cherry-pick) and any of them is the fix. Recorded 2026-08-19.
