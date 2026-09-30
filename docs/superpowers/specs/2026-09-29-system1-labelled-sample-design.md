---
id: '0bdaeecdc70db511'
kind: spec
status: draft
title: System 1 labelled sample — design
tags:
- system1
- measurement
- labelling
---

# System 1 labelled sample — design

**Valid:** dated 2026-09-29

**Status:** design approved section by section with the operator in session
`3c5b02df-b6ce-45f5-9d03-1194e38465c0`, 2026-09-29. Nothing below has been run. The registration
(seed, code hashes, frame counts) is recorded as Amendment 1 **after the pilot and before the main draw**.

**Supersedes:** Task 10 of `docs/superpowers/plans/2026-09-26-system1-base-rate-measurement.md`.
**Builds on:** `docs/superpowers/specs/2026-09-26-system1-base-rate-measurement-design.md` (its frozen
corpus, its `transcripts.py` exclusions, its private-data rules). That plan stopped at Task 9: the
judge gate returned `passed=False`, so the measurement was INCONCLUSIVE.

## Why this replaces the judge-first route

The gate taught three things, each settled by an operator decision rather than by code:

- **The target was wrong.** The gate asked whether a statement is a *mistake*. The operator's approved
  target is a *useful intervention*, labelled verify / qualify / correct / none / unresolved with the
  bars in `docs/research/2026-09-26-codex-three-role-intervention.md` and approved examples A–F.
- **The context was too thin.** Those bars turn on what the current decision relies on and on nearby
  evidence, and the judge's context showed neither. The existing rule, R117, even spends its cap on
  tool rows with no text (bug `61f699816f5ee2fd`).
- **The cases were picked with hindsight.** RTD positives were chosen by someone who had seen the
  corrections, and three of their gold labels were wrong. The controls were assumed correct because
  nobody corrected them, and CTL3-7 was not.

The answer is a **uniform draw from a frozen corpus, labelled by the operator.** Those labels measure
the base rate directly, with no judge in the loop, and they become the held-out gold for any later
judge gate.

## The question and the decision rule

**Question:** among substantive assistant messages in real codescout work, how often would the operator
want a System 1 to speak up, either quietly suggesting a check to System 2 or interrupting?

**Hit:** a labelled case whose delivery is `quiet` or `interrupt`.

**Decision rule, fixed before any case is drawn.** Threshold T = 10%, over the 60 substantive-stratum
labels, using the Wilson 95% interval:

| outcome | condition | at n = 60 |
|---|---|---|
| **go** | lower bound ≥ 10% | 11 or more hits (lower bound 10.6% at 11) |
| **no-go** | upper bound < 10% | at most 1 hit (upper bound 8.9% at 1; 6.0% at 0) |
| **inconclusive** | otherwise | then label more, under a new registration |

**Guards on the rule:**

- **Thin packets:** if more than 25% of substantive labels are `unresolved`, the outcome is
  inconclusive. `unresolved` always counts as a non-hit.
- **Session concentration:** the session-resampled interval (below) is reported beside Wilson. If the
  two intervals give different outcomes, the result is inconclusive.
- **Why 10%:** the corpus profile measured 33,405 substantive messages in about five weeks under the
  first strata definition. At 10%, that is about 3,340 useful interventions, roughly 95 per calendar
  day. The operator judged that enough to justify a detector even at modest precision.

This measures a **ceiling**: what a perfect System 1 with a perfect intervention would add, in the
operator's judgement. It does not measure a detector.

## Frame and units

- **Corpus:** the frozen snapshot `2026-09-29-codescout` (manifest at
  `docs/evals/data/2026-09-27-system1-base-rates/corpora/2026-09-29-codescout.manifest.json`).
  - It is a preservation snapshot that does not meet the analysis freeze procedure (R78). R78 protects
    `usage.db` joins, and this design reads transcripts only, so the snapshot is valid as a frame
    "as of the freeze".
  - The one session that was live at the freeze is included, truncated at the freeze instant.
- **Sessions:** the kept sessions under `transcripts.exclusions(sessions, join.SPEC_EXCLUDED_SIDS)`,
  over the whole retained window. The last 7 days hold only 22 sessions, and practice drifted within
  the window.
- **Unit:** an assistant **API message**, meaning all transcript entries sharing one `message.id`. Two kinds:
  - **top-level messages**, every one;
  - **subagent hand-backs**, the final assistant message of each subagent file, where it reports to
    its controller. The profile found 591 of 597 files ending on one.
  - Mid-subagent messages are out of the frame.

## Strata

A unit is **substantive** if any of the following holds; otherwise it is **routine**.

1. its text carries a claim marker: completion or verification, test results, a count next to a noun,
   or an absence or universal negative;
2. it is a final message to the operator (`stop_reason == "end_turn"` with text);
3. it makes a consequential tool call: an edit or file creation, a git
   commit/push/reset/rebase/checkout/stash/clean, `rm -`, a release build, a subagent dispatch, or a
   catalog or memory write;
4. it is a subagent hand-back.

**The markers are regex heuristics, and that is acceptable here.** Both strata are sampled with known
probabilities, so the rule changes the interval's width and never the estimate's bias. The exact
patterns live in code, and Amendment 1 records the code's sha256. Each unit records every rule that
admitted it.

## Sizes

- **Main draw:** 60 substantive units and 20 routine units, uniform without replacement within each
  stratum, in an order fixed by the registered seed. The draw runs once, after the registration commit.
- **Pilot:** 5 units from a separate seed, labelled first to check the packet format. They are excluded
  from every estimate. The packet rule may change after the pilot and before registration, never after.
- **Optional re-label:** 10 main units, presented again at least 3 days after first labelling, to
  measure the operator's agreement with themselves.

## Packet rule

The packet is identical for the operator and for any later judge, because a label holds only for the
packet it was given.

- **The operator's last message** before the unit, truncated to its first 1,500 characters. "Operator
  message" means what `transcripts.operator_messages` classifies as one, the parent spec's own
  classifier (R25, R26, R103, R104).
- **The last 6 API messages** before the unit, oldest first. Each shows:
  - its text;
  - its tool calls, as the tool name plus its arguments cut to 300 characters, or to 1,500 for a call that
    writes a durable record (an edit tool, or a catalog tool with a write action: the body is what the
    judgement depends on);
  - its tool results, whole up to 1,500 characters, and above that the **first 500 and last 1,000**
    with a marker between (counts and headers come first, exit and error text last), plus the exit code
    whenever one can be parsed from the whole result. Every cut, here and above, ends or begins with a
    marker `[… N characters not shown]`, so a partial value never reads as the whole.
    The transcript's `is_error` flag under-reports failures: the profile found 35% of `cargo test`
    results exiting nonzero and 0.4% flagged.
- **The unit itself:** its text, then its tool calls marked **about to run**. Their results are never shown.
- **For a subagent hand-back:** the dispatch prompt (first 1,500 characters) replaces the operator's
  message, and the 6 messages are the subagent's own.
- **Limits:** 20,000 characters in total, trimming the oldest material first. Only entries strictly
  before the unit are included; a test plants a later marker to prove it.
- **Blinding:** no session id, date or absolute timestamp. Messages are shown as `−6 … −1`. A private
  key file maps case ids to units.
- **Privacy:** every packet is scanned for GitHub-token-shaped strings before rendering, and rendering
  refuses on any match.

## Labelling

- **The tool is `scripts/measure/label.py`,** run by the operator in their own terminal. No agent
  session renders or reads packet text.
  - **The controller stays blind to the cases.** The controller will write the next judge prompt, so
    seeing the held-out cases would leak them into its design.
  - **Agents see counts only,** through the `summary` subcommand.
- **Per case,** the tool records:
  - **labels:** a non-empty subset of {verify, qualify, correct}, or exactly {none}, or exactly {unresolved};
  - **delivery:** `silent`, `quiet` or `interrupt`. The tool enforces the pairing: verify, qualify or
    correct requires `quiet` or `interrupt`, and none or unresolved requires `silent`;
  - an **optional note**, such as the check the operator would want run;
  - a **recall flag**, "I remember how this turned out";
  - **seconds spent** on the case.
- **Storage:** labels are append-only and each is bound to its packet's sha256. `verify` fails if a
  packet no longer renders to the hash it was labelled against.
- **Order:** cases are shuffled across strata, and a packet does not say which stratum it came from.

## Estimation

`scripts/measure/estimate.py` reports:

- **The decision estimate:** the substantive-stratum hit rate with its Wilson interval, and the outcome
  under the rule above.
- **The sensitivity check:** a session-resampled 95% percentile interval, 10,000 resamples under the
  registered seed.
- **The routine stratum:** its hit rate and interval, which tests the assumption that routine messages
  rarely need an intervention.
- **The overall per-message rate,** re-weighted by the registered stratum counts, as a secondary result
  with its interval.
- **Breakdowns:** by label (V/Q/C), by delivery, by unit kind (top-level versus hand-back), the
  `unresolved` share, and every figure with and without recall-flagged cases.
- **Cost:** seconds per case, which is the measured price of each gold label.
- **Self-agreement,** if the re-label ran: raw agreement and Cohen's κ on delivery.

Every number names its corpus id and its population.

## Registration (Amendment 1, after the pilot)

Committed before the main draw exists:

- the seed;
- the sha256 of `sampler.py`, `packet.py` and the marker patterns;
- the frame counts per stratum and unit kind;
- the sizes;
- this decision rule, unchanged.

The draw then writes the list of drawn case ids and packet sha256s, and is committed next. No packet
text is committed.

## Components and tests

| module | responsibility | interface |
|---|---|---|
| `sampler.py` | frame, strata with admitting reasons, seeded draw | `frame(corpus_dir) -> list[Unit]`, `draw(frame, sizes, seed) -> list[Unit]` |
| `packet.py` | the packet rule, token scan, rendering, sha256 | `build_packet(corpus_dir, unit) -> Packet` |
| `label.py` | the operator tool: `next`, `summary`, `verify` | command line |
| `estimate.py` | the rates, intervals, decision outcome, self-agreement | `estimate(labels, frame_counts, seed) -> dict` |

`run.py` gains `frame`, `draw`, `render` and `estimate`.

**Tests use synthetic transcripts only.** Each is mutation-checked at its own site:

- **The draw is reproducible** from its seed.
- **Each stratum rule has a fixture that only it admits.**
- **The frame holds only kept sessions** and the right unit kinds.
- **No packet contains anything from after the decision** (planted-marker test).
- **A tool result before the decision appears in the packet with its exit code.** This is bug
  `61f699816f5ee2fd`'s regression test for the new rule.
- **Truncation keeps each result's tail.**
- **Rendering refuses** a token-shaped string.
- **`label.py` refuses** an invalid label/delivery pairing and a packet-hash mismatch.
- **The estimator** gives hand-computed values for a small fixture, and the decision outcome is right
  at the boundary counts 1, 2, 10 and 11 of 60.

## Out of scope

- **The judge.** Moving `judge.py` onto this packet rule, a new prompt and a new gate belong to a later
  spec, which uses these labels as its held-out gold. Bug `61f699816f5ee2fd` stays open until then.
- **The contrast project,** MRV-poc. It was descriptive only in the parent spec.
- **Analysis over `usage.db`.**

## Disclosures

- **One labeller, labelling their own sessions.** Hindsight cannot be removed, only counted through the
  recall flag. Self-agreement, if measured, bounds how consistent the gold is.
- **"Useful" is the operator's judgement at the packet,** not an outcome measured later. Accepting a
  suggestion would not prove it helped.
- **The rate is per message, not per task.** A session with many messages can hold several hits for one
  underlying issue.
- **n = 60 decides only clear cases.** Between 2 and 10 hits the outcome is inconclusive by design.
