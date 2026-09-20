---
id: '555135b94c321741'
kind: research
status: draft
title: Predicate candidates for the rung-1a observer phase — measured
tags:
- usage
- observational
- deep-agent
- predicates
topic: deep-agent-observation
---

# Predicate candidates for the rung-1a observer phase

**Read-only measurement, 2026-09-20.** Frozen at `max_id = 133283`, taken
2026-09-20T17:26Z, tree `be598fb5` on `experiments`. Aggregate only: no path,
pattern, or code fragment from `input_json` / `output_json` reaches this
document. Instrument: [`scripts/probe-predicate-candidates.py`](../../scripts/probe-predicate-candidates.py).

The question is which predicates the rung-1a observer phase should evaluate —
chosen from evidence rather than from instinct. Three candidates were
pre-registered before querying: reader thrashing, suspicious zeros, and
"this moment is an eligible observation point". **One survives as framed, one
survives weakened and partly redundant, one produced no definition at all** —
and the data suggests a fourth nobody proposed.

## Method

One `mode=ro` connection, one deferred read transaction, one frozen `max_id`,
following the precedent of the
[2026-09-18 baseline](2026-09-18-deep-agent-observation-baseline.md). Every
aggregate is calibrated against direct SQL over the same cutoff; all five
calibration booleans returned true.

Three method choices are load-bearing and each would silently corrupt a figure
if reversed:

**Alias union.** Tool names cut over on 2026-09-02/03 and the retained corpus
spans it, so `read_markdown`→`read_file`, `edit_markdown`→`edit_file`,
`artifact*`→`doc` are unioned. `--no-alias` reproduces the wrong figures on
purpose, which is how the gap below was measured rather than asserted.

**Session key is `session_id`,** per-process and correct — not
`cc_session_id`, which is wrong on ~31% of rows. **The 2026-09-18 baseline
sequenced on `cc_session_id`, so its sequence figures and the Q4/Q5 figures
here are not comparable.** This is a difference in instrument, not a
disagreement about the world.

**Ordering is by completion at one-second resolution.** `called_at` is written
after the call returns and `id` is assigned at INSERT, so both order by
completion, not start. Q4 and Q5 are sequence questions and inherit that; the
tie rate is published below rather than left to the sort.

## Q1 — base rates and the denominator

| | |
|---|---|
| rows | **69,444** |
| covered UTC | 2026-08-24 15:21:13 → 2026-09-20 17:26:26 |
| distinct `session_id` | 636 |
| distinct `project_root` | 12 — **all of them codescout** checkouts or worktrees |
| `input_json` / `output_json` present | 67,887 (**97.76%**) |
| overflowed | 4,502 (**6.48%**) |
| non-success | 3,898 (**5.61%**) |

The corpus is hard-bounded by the 30-day retention sweep in `write_record`, so
every count is a floor on all-time. The 27-day span observed is consistent with
that bound.

**The alias union is not a detail.** Measured both ways at the same cutoff:

| tool | with union | raw name only | hidden under an old name |
|---|---:|---:|---:|
| `read_file` | 10,462 | 8,763 | 1,699 (**16.2%**) |
| `doc` | 8,371 | 4,810 | 3,561 (**42.5%**) |
| `edit_file` | 6,034 | 4,944 | 1,090 (**18.1%**) |

A per-tool figure taken without the union is a floor that reads like a total,
and for `doc` it loses more than two calls in five.

### The schema does not yet carry today's fix

`started_at` and `agent_id` are **absent from the live table**, and
`rows_with_subsecond_called_at` is **0** of 69,444. The migration shipped in
`1dd363eb`, but `open_db` only runs it when a binary carrying that code opens
the database, and the live MCP server predates it. Until the release binary is
rebuilt, every row is second-resolution, completion-stamped, and carries only
half the principal — so the ordering limits stated above are total rather than
historical.

## Q2 — reader thrashing: the candidate does not survive as framed

Of 7,380 `read_file` calls naming a path (`@ref` buffer reads excluded — those
page a result already held and are not visits to a file):

| | n | |
|---|---:|---|
| first visit to that path in that session | 3,195 | 43.3% |
| **repeat visit** | **4,185** | **56.7%** |
| …of which the call **narrows** (line range, heading, json_path, …) | 3,823 | **91.3% of repeats** |
| …of which arguments are **byte-identical** to an earlier call | 326 | 4.4% of all path-naming reads |

**A predicate on "Nth read of the same path" would fire on 56.7% of reads, and
nine in ten of those firings are the designed interaction.** `read_file` on
markdown returns a heading map and invites a follow-up call for a section; a
line-range read of a large source file is paging. Both are the tool working as
intended. A predicate cannot be useful at a 56.7% firing rate against a
91.3%-benign population.

The residual is the real signal: **326 calls (4.4%) repeat a path with
byte-identical arguments** — the same bytes fetched twice, which no interaction
design asks for. Visits-per-pair is long-tailed: 1,890 pairs visited once, 600
twice, and 72 pairs at ten or more.

**Reframe rather than discard: the predicate is "identical arguments repeated",
not "same path repeated".**

## Q3 — suspicious zeros: survives weakened, and is partly already built

| | n | |
|---|---:|---|
| `grep` calls | 9,325 | |
| …with output recorded | 9,291 | |
| **returning zero matches** | **790** | **8.50%** of those with output |
| zero-match whose pattern carries `/` or `*` | 165 | 20.9% of zeros |
| non-zero-match whose pattern carries `/` or `*` | 1,179 | **13.9%** of non-zeros |

An 8.50% firing rate is workable. But the discriminator is weak: a path-ish
pattern lifts the probability of a zero from 13.9% to 20.9%, a ratio of
**1.5×**. That is a real signal and a poor one; a predicate built on it alone
would be wrong about four times in five when it fires.

**And 444 of the 790 zero-match calls (56%) already carry codescout's own
scope warning** — the `this zero describes what was searched, not the pattern`
text. The ADR this predicate would mechanise is therefore *already mechanised
for the majority of its population*. The marginal value is the remaining 44%,
not the whole 8.50%.

### The positive control, and why it was necessary

`LIKE '%0 matches%'` — the obvious test — returns **1,280**, a **62%
over-count**. The 490 spurious rows are `"10 matches"`, `"20 matches"`,
`"30 matches"`, `"120 matches"` and similar, plus 19 rows where the phrase
appears later in a hint rather than as the result. Checked in the other
direction too: **zero** zero-match results begin a later content block, and
none of the 23 dict-shaped outputs carries one, so the exact-prefix test has no
false negatives in this corpus. Both directions were run because a test that
only over-counts and a test that only under-counts fail differently and neither
control reaches the other.

## Q4 — overflow economics: the finding nobody pre-registered

What a session does on its **next** call after an overflow (n = 4,502):

| next call | n | |
|---|---:|---|
| switched to a different tool | 1,536 | **34.1%** |
| same tool, different arguments | 1,448 | 32.2% |
| **queried the `@ref` buffer** | **1,443** | **32.1%** |
| same tool, explicitly narrowed | 43 | 1.0% |
| session ended | 26 | 0.6% |
| re-ran identical arguments | 6 | 0.1% |

**The buffer is queried after roughly one overflow in three.** Progressive
disclosure exists so that elided content stays retrievable; on ~3,059 of 4,502
occasions the caller moved on without retrieving it.

**This measurement cannot say whether that is good or bad, and that is the
result.** Two readings fit the same number exactly: the compact summary was
sufficient and disclosure did its job, or the caller silently lost information
and proceeded anyway. Nothing in `usage.db` records whether the summary
sufficed, so the instrument cannot discriminate — a gap in the recorder, not an
ambiguity in the data.

Ordering ambiguity: **4,801 of 68,808 adjacent pairs (6.98%) share a
`called_at` second**, so about one classification in fourteen rests on
`id` order — completion order — rather than on an observed gap.

## Q5 — sequence shape: no predicate emerges, and that is reported as-is

**46.36%** of all adjacent tool pairs are the same tool twice. The top bigrams
are overwhelmingly self-transitions: `run_command`→`run_command` 20.86%,
`read_file`→`read_file` 6.22%, `doc`→`doc` 6.03%, `grep`→`grep` 5.18%. The
highest cross-tool transitions are `edit_file`→`run_command` (2.65%) and
`grep`→`read_file` (2.53%) — plausibly "edit then test" and "search then
read", but at under 3% each they are not a structure a predicate can stand on.

**No crisp definition of "an eligible observation point" emerges from this
data.** Forcing one would be inventing it. The honest report is that the third
pre-registered candidate has no evidential support and should not be built on
this basis.

## Bounds

- **`usage.db` records MCP calls only.** Native `Bash` / `Read` / `Edit`, host
  prompts, and task outcomes are invisible. A zero here is evidence about the
  instrument, not about the world.
- **All 12 project roots are codescout** checkouts or worktrees, so this is a
  codescout-only measurement; it does not generalise to other repos.
- **Outcome classes are tool-result categories, not task success.**
  `recoverable_error` is not failure and `overflowed` is not failure.
- **`input_json` / `output_json` are debug-gated.** Presence is 97.76% here;
  a window in which debug was off would answer Q2–Q4 with silence rather than
  an error.
- **Ordering is by completion at one-second resolution** (see Q1). Q4 and Q5
  inherit this; the 6.98% tie rate sizes it.
- **Retention is 30 days**, so every count is a floor.

## What this implies for the predicate set

**Ship one, reframed.** *Identical-argument repeat* — 326 occurrences, 4.4% of
path-naming reads, unambiguously redundant by construction. Cheap to evaluate,
no interpretation needed, and its firing is never the designed interaction.

**Ship one, scoped down and honest about overlap.** *Zero-match grep* at 8.50%,
with the explicit note that 56% of its population already receives a scope
warning, so the predicate's real subject is the 44% that does not. Do not build
on pattern-shape as the discriminator: a 1.5× lift is not enough to act on.

**Do not ship the third.** "Eligible observation point" has no support in the
sequence data. Leave it undefined rather than inventing a definition the
evidence does not carry.

**Consider a fourth, which the data proposed rather than confirmed.**
*An overflow whose buffer is never queried before the session moves on* —
~68% of 4,502 overflows. It is the largest measurable population here and it
sits directly on the token lever. But it cannot be acted on until the recorder
can distinguish "the summary sufficed" from "the caller gave up", and today it
cannot. **That is a request for instrumentation, not a predicate ready to
build** — and naming it as a predicate without that distinction would be
exactly the mistake Q2 nearly made.

**Finally, rung 1a's own denominator rule is vindicated by Q3.** The naive
zero-match test over-counted by 62% and returned no error. Record the
*evaluation* alongside the *verdict* for every predicate shipped, or a
predicate that silently stops matching will be indistinguishable from one that
never runs.
