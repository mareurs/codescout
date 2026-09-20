---
id: ff49e054ef02e6c3
kind: tracker
status: draft
title: Handoff — what does an unretrieved overflow mean?
tags:
- handoff
- temporary
- reflective
- progressive-disclosure
- deep-agent
topic: deep-agent-observation
---

# Handoff — what does an unretrieved overflow mean?

**Status: awaiting pickup, 20 September 2026.** Temporary. Delete this file once the
question below is answered and the answer is folded into
[the deep-agent design](local-semantic-evaluator-design.md) (`d16552e9981f521e`).
Nothing here authorizes implementation.

## The question, in one sentence

Progressive disclosure elides a large result and returns a summary plus an `@ref` handle.
**The handle is queried after only 32.1% of overflows.** Does that mean the summary
sufficed, or that information was silently lost?

The two readings have opposite remedies — one says raise the inline budget or improve the
summary, the other says the mechanism is working and should be left alone — and the
measurement cannot separate them.

## What is measured, and what is not

**Measured** (`docs/research/2026-09-20-predicate-candidates-for-the-observer-phase.md`,
`555135b94c321741`; probe `scripts/probe-predicate-candidates.py`): over 69,444 rows,
2026-08-24 15:21:13 → 2026-09-20 17:26:26 UTC, frozen at `max_id=133283`, 636 sessions,
12 project roots, all codescout — the `@ref` buffer is queried after **32.1%** of
overflows, leaving roughly **3,059 of 4,502** results elided and never retrieved.

**Not measured, and this is the whole point:** whether the elided bytes were needed. The
*rate* is recoverable from `usage.db` because a later call's `input_json` carries the
handle. The *interpretation* is not, because nothing records what the caller wanted.

So this is **not a predicate candidate**. It is a request for a discriminator.

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
survives is small: byte-identical repeat reads (4.4% of path-bearing reads) and zero-match
`grep` minus the 56% that already carry a scope warning. This question is what the data
offered instead, and it is worth more than either.

One rule rung 1a must ship with, which applies here too: **record the evaluation, not the
verdict.** Storing only hits makes "never matched" and "never ran" produce identical data.

## Deliberately not decided

The field's location and shape; whether the answer is a column, a separate table, or an
eval; whether an unread buffer should be surfaced to the operator at all. Those are the
pickup session's to decide, on evidence.
