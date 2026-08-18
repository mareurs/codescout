---
id: b4d48dbfecc205c9
kind: bug
status: fixed
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
closed: 2026-08-18
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
### Regression: the clause was deleted again on 2026-08-16, and the cap arithmetic above is retired (added 2026-08-17)

Step 3's wording did not survive one day. `391fdcdc` — *fix(prompts): the
instructions channel is 2048 CHARACTERS — measure it, then fit it* — refitted the
slice and paid for part of it by reverting exactly this clause:

```diff
-   read_file is right for imports/glue — refused only when the
-   range overlaps a symbol; force=true reads it anyway.
+   read_file is right for imports/glue; force=true overrides.
```

So the surface spent 2026-08-16 through 2026-08-17 back in the state this bug
describes. Nothing failed: no test asserted the clause, and the refit's own goal
(fit 1900 chars) was met. A 57-character clause that reads like prose is exactly
what a character-budget pass deletes first — which is the argument for a guard
rather than a re-edit.

**The § Fix cap table is retired.** It reasons from a 2200-**byte** budget with 11
bytes of headroom, and that rule is now known to have been wrong twice over: the
unit was bytes where the client cuts characters, and 2200 sat *above* the measured
2048-char cliff it existed to protect (`CLIENT_INSTRUCTIONS_CHAR_LIMIT`,
`src/prompts/mod.rs:39`; BL-9). Do not re-derive anything from that table.

The real accounting, measured 2026-08-17:

| | Chars |
|---|---|
| Budget (`STATIC_SLICE_CHAR_BUDGET`) | 1900 |
| Slice as `391fdcdc` left it | 1722 |
| Restore the IL1 condition | **+57** |
| Drop the `docs/trackers → artifact(find/get)` quickref row | **−68** |
| After | **1711** |

The clause was affordable all along — there were 178 characters of headroom, not
11. It was cut under an accounting error, and the refit had no test telling it
what the 57 characters were worth.

**Why that row was the right thing to sell.** It is a routing preference, not a
gate: ignoring it costs one redundant read, where ignoring IL1's condition costs a
refusal. And it is stated in full in `get_guide("librarian")` § *docs/trackers/ —
Backing Store, Not a Docs Folder* ("Never read files there directly with
`read_markdown` or `read_file`"), which **auto-injects on the first `artifact`
call of a session** — verified in the session that made this edit, which received
it unprompted. So the rule still reaches any agent doing tracker work.

**A second-order cost caught by two tests, worth recording.** The static slice and
the dynamic `## Project Status` block share one pool:
`2048 − 48 (CHANNEL_SAFETY_MARGIN) − static`. At +57 with no offsetting cut, the
dynamic budget fell 278 → 221 and `fit_dynamic_block` began trimming real content —
`build_with_worktree_emits_worktree_banner` and
`build_with_no_memories_suggests_onboarding` both failed, having lost the worktree
banner and the index hint respectively. Growing IL1 by 57 characters silently
spends 57 characters of *someone else's* surface. Both tests pass at 1711, which
leaves the dynamic block 289 — more room than it had before this change.
## Hypotheses tried

1. **Hypothesis:** the guide is wrong and needs fixing (doc-vs-code drift, like the sibling bug
   `docs/issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md`).
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

> **VERDICT 2026-08-18 — the gate ran and step 3 LOST. Steps 1-2 are the fix; the wording is
> reverted.** `32b34efa`.
>
> The subtract-and-measure protocol was the only thing this bug stayed open on. It ran as
> hamsa **A-25**, 10 runs per arm, sonnet pinned, ship rule pre-registered at `e2fbefe2`
> **before either arm ran**. Result:
>
> | Arm | Planned the refused shape |
> |---|---|
> | A — base, no clause | **10/10** |
> | B — with the clause | **8/10** |
>
> The rule needed arm A ≥ 3/10 (met, overwhelmingly — the deficit is **confirmed**) **and**
> arm B ≤ 1/10 (missed by a mile). 0/10 → 2/10 passing is Fisher p≈0.47, noise.
>
> **Why it failed, and this is the part worth carrying forward:** the clause is
> *informational*, not *directive*. It states the gate's condition but supplies no
> procedure — and an agent asked for lines 40-55 cannot know whether they overlap a symbol
> without checking, so the clause adds a fact it cannot act on. The single passing arm-B run
> is the tell: it called `symbol_at` to resolve exactly that unknown. Arm A corroborates
> from the other side — its ten answers disagreed on the *signature* (`lines=`, `start/end`,
> `start_line/end_line`, `line_range=[..]`) while agreeing completely on the *permission*.
> The always-loaded text leaves an agent unsure only about parameter spelling.
>
> Both pre-registered validity caveats predicted a **false ceiling** and neither
> materialised, which strengthens the deficit finding rather than weakening it: the
> deliberative elicitation did not rescue arm A, and not one of the twenty runs inspected
> `report.rs` to check for overlap.
>
> **So this bug closes `fixed` on steps 1-2, not on step 3.** The head-read exemption
> (`start == 1 && end <= 60`) and the extent-ordered hint are code, carry their own tests,
> were never subject to the prompt-eval gate, and already exempt 102 of 103 `start == 1`
> refusals. Reverting the wording leaves that population handled.
>
> The guard test is **inverted, not deleted** — `il1_does_not_carry_the_refuted_overlap_clause`
> fails if the clause returns, because step 3 below otherwise reads as a standing invitation
> to re-add 57 characters now known not to work. Mutation-verified.
>
> **Everything below this banner is the pre-eval plan, kept as history.** Step 3's wording,
> its budget arithmetic, and its "authored but not eval-validated" caveat are all superseded
> by the result above. Do not implement step 3.

Not implemented. Revised 2026-08-15 after reading the refused arguments — the wording change alone
is no longer the first move.

> **Steps 1 and 2 IMPLEMENTED 2026-08-16** with regression tests (see § Tests added). Step 3 is
> authored but not eval-validated — details below. The bug stays `open` on that last gate alone.

**1. Exempt the case the guard cannot serve. — DONE.** When the requested range is non-definition text — in
practice a file head, `start_line` ≤ 5 with a modest extent — the recommended alternative
(`symbols(include_body=true)`) *cannot* return it, because `symbols` does not surface imports
(`iron-laws-detail.md:12-16`). Either allow the read, or lead the hint with `force=true` for that
shape instead of offering it second. 69 of 244 refusals in the window are this case.

**Implemented as `start == 1 && end <= 60`, and the `start` bound is measured, not the `<= 5`
written above.** Re-querying the corpus at implementation time: of **373** refused reads carrying a
range, **131 start at line 1** and **exactly one** starts between lines 2 and 5. So `start <= 5` buys
one extra call out of 103 — and it costs correctness: in a short file a read of lines 3-5 is a whole
function body, not a head read, which is precisely what the pre-existing gate test
(`read_file_source_range_blocked_when_symbol_overlaps`, `src/tools/edit_file/tests.rs`) asserts. A
`start <= 5` window broke that test; `start == 1` keeps it green and still covers 102 of 103.

**2. Order the hint by what the caller asked for. — DONE.** For a small slice of a large symbol, leading
with `symbols(include_body=true)` returns strictly more than was requested — the opposite of Iron
Law 1's intent. The requested extent is known at refusal time; use it to pick which escape leads.

**3. Then the wording. — AUTHORED 2026-08-16, not yet eval-validated.** The always-loaded IL1 text
now states the condition:

    read_file is right for imports/glue — refused only when the
    range overlaps a symbol; force=true reads it anyway.

**How the 2200-byte cap was paid.** The slice had **11 bytes of headroom** (2189/2200), so this
required a near-equal cut. Measured trade:

| | Bytes |
|---|---|
| Before | 2189 |
| IL1 wording | **+82** |
| Workspace-gate ¶2 (parallel-subagent pinning) removed | **−144** |
| After | **2127** (73 spare) |

The cut is near-free: `src/prompts/guides/workspace-state.md:119-130` § *Per-call workspace pinning*
carries the same rule in more detail, and the `"workspace-state"` row in § Deeper guidance still
points there — verified before deleting, so nothing became unreachable.

Two alternatives were costed and rejected: paying with the quickref row that restates Iron Law 2
(−70, leaves only 9 bytes spare) and a fuller wording paying by compressing IL6 (−145, but loses
*"a dispatch defect — yours, not theirs"*, the clause that assigns blame).

**No `ONBOARDING_VERSION` bump** — `server_instructions` is re-read at every MCP session start
(`src/prompts/README.md` § Versioning).

**⚠ The subtract-and-measure protocol has NOT been run.** `src/prompts/README.md` § *Measure before
shipping* states that whether a prompt-surface change ships is governed by P-1..P-8
(`docs/trackers/prompt-hamsa-audit-log.md`) — base arm first, numeric pre-registered ship/no-ship
rule, via the `prompt-tdd` harness in `../prompt-engineering/`. This change is authored and on
`experiments` with the gate green; it is **not** cleared for promotion to `master` until that run
exists. Treat the wording as a candidate, not a validated fix.

**Implemented with two conditions, because a ratio alone misleads.** The refusal now leads with
`force=true` when the overlapping symbol is both **≥ 2×** the requested extent **and** more than **40
lines** larger than it. The ratio alone was wrong and a test caught it: returning a 4-line body for a
2-line request is 2× but costs nothing, while returning 102 lines for 5 is the case worth
reordering. The 40-line figure is the corpus's own boundary — its "small slice" bucket (97 of 244
refusals, the largest) is defined as ≤ 40 lines, so an excess past that exceeds a whole typical
request's worth of unasked-for content.

`find_symbols_for_range` now returns `(name, start_line, end_line)` rather than bare names so the
extent is available at refusal time. It is private with a single caller, so the change is contained
to this file.

**Do not relax the gate wholesale.** The traced sequences show it is genuinely useful for the
symbol-body population — agents refused a blind line range go on to fetch the exact symbols they
wanted, by name. Fixing (1) and (2) preserves that while removing the population it cannot serve.
## Tests added

In `src/tools/read_file.rs` (the module had **no** gate tests of its own before this — the
pre-existing coverage lives in `src/tools/edit_file/tests.rs`, which is where
`read_file_source_range_blocked_when_symbol_overlaps` and
`read_file_source_range_force_bypasses_gate` sit; both still pass):

| Test | Mutation it catches |
|---|---|
| `head_read_of_imports_is_allowed_though_symbols_overlap` | deleting the head-read exemption — restores the refusal on the largest recoverable population |
| `non_head_read_overlapping_a_symbol_is_still_refused` | widening the exemption into a general hole |
| `head_read_past_the_window_is_still_refused` | dropping the `end <= 60` bound, letting a whole-file read pass as a head read |
| `hint_leads_with_force_for_a_small_slice_of_a_large_symbol` | dropping the extent comparison — restores a hint pushing a 5-line request toward a 102-line response |
| `hint_leads_with_symbols_when_the_symbol_is_not_much_larger` | making the reorder unconditional — this one **failed first** on a ratio-only threshold and is what forced the absolute-excess condition |

The last row is the load-bearing one: without it, a ratio-only rule would have shipped and would
have told callers to `force` past the gate on trivially-larger symbols.

All green with the full suite: **3834 passed, 0 failed**, `cargo clippy --all-targets -- -D warnings`
clean.

**Verified live on the running server** after `cargo rb` + `/mcp` — a passing unit test and a working
tool are different claims, and this bug exists because the deployed behaviour diverged from what the
docs said:

    read_file("src/tools/read_file.rs", start_line=1, end_line=20)
    → returns the imports. Previously refused: the `ReadFile` struct (line 10) and
      `impl Tool for ReadFile` (line 13) both sit inside the range.

    read_file("src/tools/read_file.rs", start_line=600, end_line=604)
    → source range overlaps named symbol(s): 'read_with_line_range'
      hint: Pass force=true to read exactly the 5 line(s) you asked for —
      'read_with_line_range' spans 171 lines, so symbols(name=...,
      include_body=true) would return the whole body.

The second confirms the reorder fires on a real 34× mismatch and that the hint now *quantifies* the
cost rather than asserting a preference.
## Workarounds

- Pass `force=true` on the first call when an exact slice is wanted. It works and is the correct
  escape for imports, macro output, and byte-exact regions.
- Reach for `symbols(name=…, include_body=true)` when the whole definition is wanted anyway.

## Resume

N/A — closed `fixed` 2026-08-18 at `32b34efa` on **`experiments`**, and archived from there
per the archive trigger (gate green + regression test; reaching `master` is not required).

**No pending-master-SHA line, and the reason is checked rather than assumed.** `master` is
987 commits behind with **zero** commits of its own (`git rev-list --left-right --count
master...experiments` → `0\t987`, measured 2026-08-18), so the promotion path is a
*fast-forward*: it will move `master` onto this exact commit without minting a second SHA.
`32b34efa` will therefore be the master-side SHA too, and there is nothing for a later
session to go looking for. What is **not** yet true is the promotion itself —
`git merge-base --is-ancestor 32b34efa master` returns NO today. An earlier draft of this
line claimed the fast-forward had already happened; it had not.

What shipped: steps 1-2 (code, with tests). What did not: step 3's wording, measured and
refuted as hamsa A-25 and reverted in the same commit that closes this.

**The live successor is a different intervention, not a re-reading of A-25.** A *directive*
wording — something of the shape "on a mid-file range, pass `force=true` or fetch the symbol
by name" — addresses what A-25 found lacking, because it supplies a procedure rather than a
fact. It needs its own base arm and its own pre-registered A-N row. The scenario to fork is
`prompt-engineering/scenarios/il1-overlap-condition/` (`prompt-engineering:f2f7958`); the
base arm is already measured at 10/10, so a successor only needs its own treatment arm
against that same base.

**Two things a later session should not redo.** The `docs/trackers` quickref row cut to pay
for the clause stays cut — restoring it is a fresh addition needing its own base arm, and
the P-4 argument for the cut was independently confirmed on 2026-08-18: `get_guide(
"librarian")` auto-injects on the first `artifact` call of a session, unprompted, every
time. And the § Fix cap table remains retired; the character accounting in § Evidence
*Regression* is the live one.

**Apparatus note, because it nearly produced a fabricated no-ship.** The first base-arm
attempt ran all 10 generations and then failed every assertion with `Permission denied` —
the checker lacked the exec bit — and summarised as `0/1 passed`. That is
character-identical to a genuine ceiling, which was precisely the outcome that would have
triggered this revert on no evidence at all. P-6 mutation-tests the checker's *logic* and
cannot catch it, because the fault sits one layer below: whether the checker can execute.
Worth a harness-backlog entry.
## References

- `src/tools/read_file.rs:544-565` — the gate
- `src/prompts/source.md:8-9` — the always-loaded IL1 text, missing the condition
- `src/prompts/guides/iron-laws-detail.md` § *Gate is overlap-based, not absolute* — the
  on-demand guide, which has it right. Cited by section rather than by line: `90c5aea1`
  rewrote that file's read-mode paragraph and shifted every line number below it
- `src/usage/db.rs:216-218` — `normalize_err_family`, where the class is named
- `docs/issues/archive/2026-08-15-read-file-force-ignored-on-full-reads.md` (fixed `2703410e`,
  archived) — sibling: the `force=true` escape was discarded *in silence* on whole-file reads.
  This bug relies on it working for line ranges, where it does and always did; the fix made the
  discard legible rather than making `force` defeat the size budget
- `docs/issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md` (fixed `43fac6c8`, archived) — same
  family (IL surface vs gate), different law
