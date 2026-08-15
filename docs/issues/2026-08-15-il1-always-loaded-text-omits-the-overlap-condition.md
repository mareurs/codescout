---
id: ab0b30dc9053aa6c
kind: bug
status: open
title: 'BUG: Iron Law 1''s always-loaded text grants line-range reads without the overlap condition — 416 refusals, 4.7 per session, the largest error class in the corpus'
owners:
- marius
tags:
- iron-laws
- prompt-surface
- read_file
- agent-guidance
- usage-db-evidence
topic: prompt-surface-consistency
---

## Summary

`server_instructions` — the surface loaded on every session — says *"Line-range read_file is fine
for imports/glue."* The gate it describes fires whenever the requested range overlaps **any** named
symbol in a source file. In a typical source file almost every line sits inside some symbol, so the
permission as stated is far wider than the gate allows.

The on-demand guide is accurate (`get_guide("iron-laws-detail")` says *"Gate is overlap-based, not
absolute"*). But agents act on the always-loaded text, attempt the read, and are refused. This is
the **single largest error class across the entire recorded corpus**: 416 occurrences across 89
sessions, 4.7 per session.

Not doc-vs-code drift in the usual sense — the detail guide is right. It is a **compression
defect**: the condition that makes the rule actionable was dropped from the surface that is always
in context, and kept only in the one that must be asked for.

## Symptom (Effect)

    source range overlaps named symbol(s): 'open_db' — hint: Use symbols(name='open_db',
    include_body=true) to read the body directly. Pass force=true to read the raw line range anyway.

## Reproduction

Both of these occurred unprompted in one session (2026-08-15) while doing ordinary work:

    read_file("src/server.rs", start_line=964, end_line=990)
      → source range overlaps named symbol(s): 'CodeScoutServer/call_tool'

    read_file("crates/codescout-embed/src/local.rs", start_line=325, end_line=420)
      → source range overlaps named symbol(s): 'tests', 'tests/from_dir_produces_a_stable_384d_vector'

In both cases the intent was a **slice** — 27 lines of a large method, a specific region of a test
module — and the hint's primary suggestion (`symbols(name=…, include_body=true)`) returns the
**whole** symbol, which is the thing Iron Law 1 exists to avoid. `force=true` was the correct
escape both times and is offered second.

## Environment

codescout on `experiments` @ `a7da09c6`. Evidence from 13 `.codescout/usage.db` files, 53,916
recorded tool calls, 460 sessions, 13 projects, read 2026-08-15.

## Root cause

**The gate** (`src/tools/read_file.rs:544-565`) refuses when all of: `!force`, the file classifies
as `FileSummaryType::Source`, and `find_symbols_for_range` returns a non-empty match. There is no
size threshold and no partial-overlap allowance — one overlapping symbol is enough.

**The always-loaded text** (`src/prompts/source.md:8-9`) reads:

    1. NEVER full-read source → symbols(path) overview,
       symbols(name=..., include_body=true) bodies. Line-range
       read_file is fine for imports/glue.

"is fine for imports/glue" states a permission and omits the condition. An agent reading only this
concludes line-range reads of source are available, and only discovers the overlap rule by being
refused.

**The on-demand guide is correct and unread by default.**
`src/prompts/guides/iron-laws-detail.md:15-18` says a line-range read is *"the correct tool, not a
fallback"* for imports, macro output and exact bytes, **and** states *"Gate is overlap-based, not
absolute."* That is the missing condition — but a guide topic is injected on first trigger, and IL1
is stated in full on every session, so the compressed version is what most reads are planned
against.

Measured 2026-08-15: 416 refusals / 89 sessions = 4.7 per session; 14% are immediately followed by
another refusal of the same family.

## Evidence

### Largest error class in the corpus

Across 53,916 calls, grouped by `err_family`:

    il1_read_overlaps_symbol   416
    il3_shell_on_source        332
    il3_pipe_to_trimmer        262
    il2_structural_edit        218
    edit_stale_match           141
    il4_read_markdown_routing  139
    il5_edit_markdown_routing  130
    replace_dropped_sibling     78
    write_scope_denied          42
    symbol_not_found            32
    json_path_unsupported       30

### Volume is not the same as cost — the comparison that isolates this one

Sequencing each family against what the agent did next distinguishes a guard that teaches from one
that merely blocks:

    family                     hits  per_session  same-tool recovery
    il3_pipe_to_trimmer         262      4.0             85%
    il1_read_overlaps_symbol    416      4.7             35%

`il3_pipe_to_trimmer` fires just as often and costs almost nothing: the agent re-runs the command
bare and succeeds 85% of the time. It is a **healthy** guard. `il1` fires more often and resolves
by same-tool retry only 35% of the time.

### The recovery does not converge

What immediately follows an `il1` refusal (416 total):

    153  symbols : success        (37% — took the hint's primary suggestion)
    145  read_file : success      (35% — retried read_file, force=true or adjusted range)
     60  read_file : ERROR        (14% — retried and was refused again)
     32  grep : success
      7  run_command : success

A near-even split between the two escapes means the hint does not tell the agent *which* one fits
the intent. The 60 repeat failures are agents guessing at a new range rather than reaching for
`force=true`.

### Concentrated where source reading is dense

    codescout          230
    code-explorer.old  167
    prompt-engineering  12
    (others)             6

Both leaders are Rust repos worked symbol-heavily — consistent with a rule about source files
rather than a per-project misconfiguration.

### Correction: the headline figure is a lifetime average and hides the trend (added 2026-08-15)

This file originally led with "416 occurrences across 89 sessions, 4.7 per session". That is the
corpus-lifetime average and it is misleading in both directions. Splitting **reach** (share of
sessions that hit the guard at all) from **intensity** (hits per affected session):

| Month | Sessions | Hit it | Reach | Hits | Per affected session |
|---|---:|---:|---:|---:|---:|
| 2026-05 | 360 | 59 | 16% | 169 | 2.9 |
| 2026-06 | 11 | 1 | 9% | 2 | 2.0 |
| 2026-07 | 38 | 14 | 37% | 158 | **11.3** |
| 2026-08 | 51 | 15 | 29% | 87 | 5.8 |

So the current intensity is **5.8 per affected session**, higher than the 4.7 lifetime figure, and
August is **worse than May on both measures**. It did not get fixed — it got much worse in July and
partially recovered.

**Month-over-month is confounded by project, except for one comparison.** Each month's corpus is
dominated by a different repo (May: `code-explorer.old`, 30,556 calls; July and August: `codescout`).
So May→July is not a like-for-like comparison. July→August **is** — same project — and within
`codescout` the rate fell from 23.1 to 6.4 per 1,000 calls, i.e. 6.55 → 2.05 hits per session
counting all sessions. `read_file`'s share of all calls fell only 14.4% → 11.0% over the same span,
so a change in tool mix accounts for a minority of the drop; the rest is unexplained by this data.

**What this does not establish.** No cause. `input_json` is `--debug`-gated (see
`docs/trackers/capability-proposals.md` CAP-1), so the requested ranges are not recorded and it is
impossible to tell from usage.db whether agents changed what they asked for, or the guard's
classification changed underneath them. Do not read the August improvement as a fix having landed —
nothing in the repo claims one, and the rate is still above May's.

**Revised acceptance test.** Use **reach and intensity together**, measured on a single project so
the confound cannot recur: `codescout` August baseline is 29% reach / 5.8 per affected session.
A wording fix should move reach first — fewer sessions attempting the blocked operation at all.
### What the refused reads actually asked for (added 2026-08-15, from `input_json`)

The statistical passes above characterise this bug by *frequency* and *what tool came next*. Neither
shows **intent**. `input_json` turns out to be populated on 95% of rows (debug mode has been on),
so the requested ranges are recoverable after all — and they sharpen the finding considerably.

Shape of the 244 refused reads in the window that carry arguments:

| Shape of requested range | n |
|---|---:|
| small slice (≤40 lines) | 97 |
| **file HEAD (`start_line` ≤ 5)** | **84** |
| medium slice (41–150) | 56 |
| large slice (>150) | 7 |

And of the 84 head reads, **69 extend no further than line 60** — they are not whole-file reads in
disguise. Verbatim samples, all refused:

    read_file("src/librarian/catalog/mod.rs",     start_line=1, end_line=20)
    read_file("src/librarian/workspace.rs",       start_line=1, end_line=20)
    read_file("src/librarian/current_project.rs", start_line=1, end_line=30)
    read_file("src/librarian/adapter.rs",         start_line=1, end_line=60)

**Lines 1–20 of a source file is the canonical "show me the imports" read** — the exact operation
Iron Law 1 names as permitted and `iron-laws-detail.md:15-16` calls *"the correct tool, not a
fallback"*. It is refused because a `mod` declaration or a struct begins inside the first 20 lines.

**And the recommended recovery cannot serve these.** The same guide states that `symbols` is a
*definition projection* that **does not return imports / `use` / `package`**
(`iron-laws-detail.md:12-16`). So for 69 of 244 refusals (28%), the hint's primary suggestion —
`symbols(name=…, include_body=true)` — is **structurally incapable** of answering the question, and
`force=true`, the only thing that works, is offered second.

That is a sharper defect than "the always-loaded text omits the condition", and it is the one worth
fixing first.

### Correction: "35% same-tool recovery" measured the wrong thing

The recovery figure quoted earlier counts a retry of **`read_file`** as recovery. Tracing actual
sequences shows the *correct* recovery for the symbol-body case is a **different tool**:

    ERR  read_file("src/embed/ast_chunker.rs", 2076, 2189)
      N1 symbols(name="tests/split_file_rust_populates_metadata_headers", include_body=true)  ok
      N2 symbols(name="tests/inner_method_signature_skips_doc_comments", include_body=true)   ok

That is the guard working exactly as designed — the agent wanted two test bodies and got them by
name — and the same-tool metric scores it as a failure to recover. **For the symbol-body case the
guard is healthy.** The defect is confined to the cases where no symbol is the answer: imports,
glue, and cross-symbol slices.

Revised characterisation: this is not "a guard that fires too often". It is **one guard serving two
populations**, healthy for the larger one and structurally wrong for the ~28% that want
non-definition text.
## Hypotheses tried

1. **Hypothesis:** the guide is wrong and needs fixing (doc-vs-code drift, like the sibling bug
   `docs/issues/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md`).
   **Test:** read `iron-laws-detail.md:15-18`.
   **Verdict:** rejected — the detail guide is accurate, and explicitly states the overlap
   condition. The defect is in the compressed always-loaded surface, not the guide.

2. **Hypothesis:** high hit count means the guard is too aggressive and should be relaxed.
   **Test:** compare against `il3_pipe_to_trimmer`, a guard of similar frequency, on same-tool
   recovery rate.
   **Verdict:** rejected as stated — frequency alone does not indicate a bad guard (85% vs 35%
   recovery separates them). The problem is that the operation is *attempted* so often, which is a
   guidance property, not a gate property.

3. **Hypothesis:** agents simply do not know `force=true` exists.
   **Test:** check the split of recovery paths.
   **Verdict:** partially confirmed — 35% do retry `read_file` successfully, so it is discoverable
   from the hint. But 14% retry and fail again, which suggests they adjusted the range instead.

## Fix

Not implemented. Revised 2026-08-15 after reading the refused arguments — the wording change alone
is no longer the first move.

**1. Exempt the case the guard cannot serve.** When the requested range is non-definition text — in
practice a file head, `start_line` ≤ 5 with a modest extent — the recommended alternative
(`symbols(include_body=true)`) *cannot* return it, because `symbols` does not surface imports
(`iron-laws-detail.md:12-16`). Either allow the read, or lead the hint with `force=true` for that
shape instead of offering it second. 69 of 244 refusals in the window are this case.

**2. Order the hint by what the caller asked for.** For a small slice of a large symbol, leading
with `symbols(include_body=true)` returns strictly more than was requested — the opposite of Iron
Law 1's intent. The requested extent is known at refusal time; use it to pick which escape leads.

**3. Then the wording.** State the condition on the always-loaded surface
(`src/prompts/source.md:8-9`), respecting the 2200-byte slice cap (`src/prompts/README.md`).

**Do not relax the gate wholesale.** The traced sequences show it is genuinely useful for the
symbol-body population — agents refused a blind line range go on to fetch the exact symbols they
wanted, by name. Fixing (1) and (2) preserves that while removing the population it cannot serve.
## Tests added

None yet — filed on discovery.

## Workarounds

- Pass `force=true` on the first call when an exact slice is wanted. It works and is the correct
  escape for imports, macro output, and byte-exact regions.
- Reach for `symbols(name=…, include_body=true)` when the whole definition is wanted anyway.

## Resume

Start with Fix (1), which is the measurable one. In `src/tools/read_file.rs:544-565`, the refusal
has both the requested range and the overlapping symbols in hand; add the file-head exemption (or
the hint reordering) there.

Acceptance test: re-run the range-shape query in Evidence and confirm the `file HEAD` bucket drops
from 84 (of which 69 are ≤60 lines) to ~0, while the `small slice` and `medium slice` buckets are
unchanged — those are the healthy population and must keep being refused.

Secondary baseline, per the earlier correction: `codescout` August reach/intensity of 29% / 5.8,
measured on a single project.

The range-shape query needs `input_json`, which is `--debug`-gated (`src/tools/../usage/mod.rs:85-89`)
but in practice populated on 95% of recorded rows on this machine. Confirm it is still being
captured before re-measuring, or the query silently returns nothing.
## References

- `src/tools/read_file.rs:544-565` — the gate
- `src/prompts/source.md:8-9` — the always-loaded IL1 text, missing the condition
- `src/prompts/guides/iron-laws-detail.md:15-18` — the on-demand guide, which has it right
- `src/usage/db.rs:216-218` — `normalize_err_family`, where the class is named
- `docs/issues/2026-08-15-read-file-force-ignored-on-full-reads.md` (open) — sibling: the `force=true`
  escape is discarded on whole-file reads; this bug relies on it working for line ranges, where it does
- `docs/issues/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md` (open) — same
  family (IL surface vs gate), different law
