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

Not implemented. The cheapest candidate is a wording change on the always-loaded surface: state the
condition, not just the permission — e.g. *"Line-range read_file is fine for imports/glue; on
source files the gate refuses a range that overlaps a symbol — pass force=true for an exact
slice."* That is a `src/prompts/source.md` edit and must respect the 2200-byte slice cap
(`src/prompts/README.md`), so something else in the slice likely has to give.

Worth deciding alongside: whether the hint should lead with `force=true` when the requested range
is **small relative to the overlapping symbol**. The current hint always leads with
`symbols(include_body=true)`, which for a 27-line slice of a 400-line method returns strictly more
than was asked for — the opposite of the Iron Law's intent.

**Do not treat "relax the gate" as the default fix.** The `il3_pipe_to_trimmer` comparison shows a
frequently-fired guard can be healthy; this one's problem is attempt rate and recovery ambiguity.

## Tests added

None yet — filed on discovery.

## Workarounds

- Pass `force=true` on the first call when an exact slice is wanted. It works and is the correct
  escape for imports, macro output, and byte-exact regions.
- Reach for `symbols(name=…, include_body=true)` when the whole definition is wanted anyway.

## Resume

Edit the IL1 line in `src/prompts/source.md:8-9` to carry the overlap condition, checking the
2200-byte cap on the `server_instructions` slice first
(`prompt_surfaces_reference_only_real_tools` and the cap test gate this file). Then re-measure:
`il1_read_overlaps_symbol` per-session rate should fall from 4.7; that number is the acceptance
test. The query is in the Evidence section.

## References

- `src/tools/read_file.rs:544-565` — the gate
- `src/prompts/source.md:8-9` — the always-loaded IL1 text, missing the condition
- `src/prompts/guides/iron-laws-detail.md:15-18` — the on-demand guide, which has it right
- `src/usage/db.rs:216-218` — `normalize_err_family`, where the class is named
- `docs/issues/2026-08-15-read-file-force-ignored-on-full-reads.md` (open) — sibling: the `force=true`
  escape is discarded on whole-file reads; this bug relies on it working for line ranges, where it does
- `docs/issues/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md` (open) — same
  family (IL surface vs gate), different law

