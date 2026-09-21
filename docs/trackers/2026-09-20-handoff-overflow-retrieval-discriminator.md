---
id: ff49e054ef02e6c3
kind: tracker
status: draft
title: Handoff — how do we tell whether an elided result was needed?
tags:
- handoff
- temporary
- reflective
- progressive-disclosure
- deep-agent
topic: deep-agent-observation
---

# Handoff — how do we tell whether an elided result was needed?

**Status: awaiting pickup, 20 September 2026.** Temporary. Delete this file once the
question below is answered and the answer is folded into
[the deep-agent design](local-semantic-evaluator-design.md) (`d16552e9981f521e`).
Nothing here authorizes implementation.

## The question, in one sentence

Progressive disclosure elides a large result and returns a summary plus an `@ref` handle.
When the elided content is not picked up, does that mean the summary sufficed, or that
information was silently lost?

The two readings have opposite remedies — one says raise the inline budget or improve the
summary, the other says the mechanism is working and should be left alone.

**A second question now sits in front of that one** (added 2026-09-21): the instrument
cannot currently tell you which overflows went unretrieved at all. Read the next section
before quoting any figure from this file.

## What is measured, and what is not

**Measured** (`docs/research/2026-09-20-predicate-candidates-for-the-observer-phase.md`,
`555135b94c321741`; probe `scripts/probe-predicate-candidates.py`): over 69,444 rows,
2026-08-24 15:21:13 → 2026-09-20 17:26:26 UTC, frozen at `max_id=133283`, 636 sessions,
12 project roots, all codescout — **the call immediately following an overflow contains
buffer-reference syntax in 32.1% of cases (1,443 of 4,502)**.

Read that sentence literally. It is a **next-call classification**, and the research
artifact's Q4 table always labelled it as one. The promotion of it into a retrieval rate
happened in prose downstream — including in the first version of this file.

**Two linkage defects sit between that figure and any claim about retrieval, and they run
in opposite directions.** Filed as
`docs/issues/2026-09-20-predicate-probe-overstates-retrieval-and-redundancy.md`
(`a8f384cc0052d7b9`), root causes verified at the bytes 2026-09-21:

- `classify_next` (`scripts/probe-predicate-candidates.py:301-316`) receives only the next
  call. A buffer read two or more calls later is invisible — retrieval **undercounted**.
- It returns `queried_the_buffer` when any handle-shaped string appears in that call's
  input, never comparing it against the handle the overflow emitted. Reading an
  *unrelated* buffer counts — retrieval **overcounted**.

Neither error's magnitude is known. The counterexamples that established them are
synthetic, not a re-measurement of the corpus. So **the eventual-retrieval rate is
unmeasured**, and 32.1% is neither an upper nor a lower bound on it. It is a correct
measurement of a narrower thing.

**Separately not measured:** whether the elided bytes were needed. That question survives
the linkage repair intact — matching handles would tell you *whether* a buffer was read,
never *whether its contents mattered*. Nothing in `usage.db` records what the caller
wanted.

So this is **not a predicate candidate**. It is a request for a discriminator, and the
linkage repair is now the first half of that work rather than a precondition someone else
already met.

## The actual design problem

Name an observable that distinguishes *summary sufficed* from *information lost*. Some
starting directions, none validated:

- **Re-run with narrower arguments.** If a session re-issues the same tool with a tighter
  scope shortly after an overflow, the summary probably did not suffice. Derivable from
  existing data; needs a definition of "shortly after" that survives the ordering limits
  below.
- **Abandonment.** The line of inquiry stops after the overflow. Hard to define without
  task outcomes, which `usage.db` does not hold.
- **Proximity to a wrong turn.** An overflow followed by an error, a retry, or a
  correction. `err_family` exists; "correction" does not.
- **Ask the caller.** A field the reader sets when it gives up on a buffer rather than
  reading it. Cheap, but it is a self-report and ranks below anything observed.

It is a legitimate outcome to conclude that telemetry alone cannot answer this and that it
needs an eval arm in `prompt-engineering` instead. Say so if that is where it lands.

## Why it is worth someone's time

It is the **token lever with a real denominator**. The other token candidate — predicting
whether an injected guide section gets used — has a delivery ledger but no outcome signal
at all. This one has 4,502 events, a measured split, and a mechanism that already exists.

If the answer is "information is being lost", the remedy touches
`MAX_INLINE_TOKENS` / `INLINE_BYTE_BUDGET` / `COMPACT_SUMMARY_MAX_BYTES`
(`get_guide("progressive-disclosure")` carries the current values) and the summary
shape — not a model. If the answer is "the summary sufficed", that is a strong result
for progressive disclosure and closes a question that would otherwise keep being asked.

## Constraints whoever picks this up must know

- **The retrieval figure's linkage is broken in both directions, and repairing it is part
  of this work.** `classify_next` looks exactly one call ahead, and matches any
  handle-shaped string rather than the handle the overflow emitted. Any eventual-retrieval
  claim needs handle matching over a **declared observation horizon** — choose that horizon
  deliberately and state it, because "the next call" is itself a horizon and it is the one
  that produced the overclaim. Filed as `a8f384cc0052d7b9`.
- **`usage.db` is a rolling 30 days that prunes on write.** `write_record` runs
  `DELETE FROM tool_calls WHERE called_at < datetime('now','-30 days')` on every insert.
  Re-running the probe later describes a different population with no warning. Freeze any
  evaluation set before it ages out.
- **The corpus is mixed-format right now and will be for an unknown period.** The
  sub-second `called_at`, `started_at` and `agent_id` columns shipped in `1dd363eb` and
  only take effect for a session that has reconnected to the rebuilt binary. Every other
  session keeps writing the old shape from its own long-lived process. So *"how many rows
  carry a start instant"* currently measures **how many sessions have reconnected**, not
  anything about behaviour.
- **Ordering is by completion, at one-second resolution, for historical rows**, and 6.98%
  of adjacent pairs tie. Any "what happened next" analysis must state how it broke ties
  rather than letting the sort order decide silently.
- **Sequence on `session_id`, not `cc_session_id`** — `scripts/friction-probe.py` supplies
  this rule. The 2026-09-18 baseline (`6a8d6b8eaea61de9`) sequenced on `cc_session_id`, so
  **its sequence figures are not comparable** to the probe's.
- **Union the pre-2026-09-03 tool aliases or undercount badly**: `doc` hides 42.5% of its
  calls under old names, `edit_file` 18.1%, `read_file` 16.2%.
- **Never project `input_json` / `output_json` contents** into any artifact. They hold real
  source. Structural fields may be read to compute aggregates; only counts land in prose.
- **Check `docs/PROBES.md` before writing a new instrument.** Both `friction-probe.py` and
  `probe-predicate-candidates.py` have rows there naming their blind spots.

## Where this sits in the larger design

[The deep-agent design](local-semantic-evaluator-design.md) was amended on 2026-09-20
(`a1055e47`) with the frame this belongs to: the useful boundary is **noticing versus
deciding**, not cheap versus expensive, and the first rungs need no model. Rung 1a is
predicate evaluation inside the existing `UsageRecorder`, which is already a per-call,
durable, failure-isolated observer.

Measurement killed two of the three predicates originally proposed for rung 1a. What
survives is small, and narrower than it first read: repeat reads with byte-identical
arguments (4.4% of path-bearing reads) — a **candidate**, not a redundancy verdict, since
the probe never checks for an intervening edit and argument equality does not imply
content equality — and zero-match `grep` minus the 56% that already carry a scope warning. This question is what the data
offered instead, and it is worth more than either.

One rule rung 1a must ship with, which applies here too: **record the evaluation, not the
verdict.** Storing only hits makes "never matched" and "never ran" produce identical data.

## Deliberately not decided

The field's location and shape; whether the answer is a column, a separate table, or an
eval; whether an unread buffer should be surfaced to the operator at all. Those are the
pickup session's to decide, on evidence.
