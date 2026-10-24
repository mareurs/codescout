---
id: '6c3208076b28d4f8'
kind: spec
status: draft
title: Intervention judge — design
tags:
- system1
- measurement
- judge
---

# Intervention judge — design

**Valid:** dated 2026-10-24

**Status:** design approved by the operator, 2026-10-24, session `a2a15b69-d17c-4c3a-b8ba-2f69fd8c0825`, after the builder check and its four settled questions. Approach chosen earlier in the same session. Nothing below has been built or run.

**Builds on:** `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md`. Its § *Out of scope* defers this work: "Moving `judge.py` onto this packet rule, a new prompt and a new gate belong to a later spec." This spec covers the first two. It does not cover the gate.

## Intent and why

The operator labels each sampled unit from a **packet**: the operator's last message, the last 6 API messages with truncated tool calls and results, and the unit with its calls marked "about to run". `scripts/measure/packet.py` `build_packet` renders it. A label holds only for the packet it was given.

The existing judge does not read that packet, and it asks a different question.

- **Different context.** `judge._session_part` reads rows of the events DB. Option B (commit `767ac29a4`) stopped text-less rows from spending its 12-turn cap. It still shows no tool call and no tool result. Bug `61f699816f5ee2fd` records the gap.
- **Different target.** `judge.Verdict` asks "is this a mistake?" (`is_mistake`, `detectability`, `lessons`, `origin_uuid`). The operator's approved target is a *useful intervention*: a subset of {verify, qualify, correct}, or none, or unresolved, with a delivery of silent, quiet or interrupt (`scripts/measure/label.py` `validate`).
- **No production caller.** In `scripts/measure/judge.py`, `build_input` is called with `events_db=None` only (the gate, `_gate`). The session path, `_session_part`, is reached only when `build_input` receives an events DB (`judge.py:540`), and only tests pass one. Audit mode has never run on the corpus.

The goal is a judge whose input is the operator's packet and whose answer has the operator's label shape. A later spec can then score it against the operator's labels, and those labels are its held-out gold.

## Goals

- A judge input built from a `Packet`, never from the events DB or a transcript.
- A new prompt that asks for the operator's labels and delivery, with the operator's bars.
- A verdict with the operator's label shape, validated by the same rule `label.py` uses.
- A result that records the packet's sha256, so a judgement binds to the packet the operator labelled.
- The old judge, its gate and its recorded result (Task 9b, INCONCLUSIVE) stay unchanged and reproducible.

## Non-goals

- **The gate:** thresholds, a consensus rule, a scored run, any comparison with the labels. That is a later spec.
- **Any run on real packets.** This spec sends no packet to a model. Tests inject `complete`.
- **Removing the old audit and correction modes.** A later spec retires them (see *Open for the operator*).
- **Tuning the packet.** The packet rule is registered and frozen. The judge reads it as it is.

## Anti-goals

The work fails even if its tests pass when any of these holds.

- The judge sees anything the operator did not see: a case key, a transcript path, a stratum, a session id, the unit's own timestamp, or the correction text. Bare calendar dates inside the packet's own prose stay, exactly as the operator saw them: `packet.py` blinds only timestamps with HH:MM:SS, and the judge adds no second scrub. (The labelling spec's "no session id, date or absolute timestamp" is wider than the code. See *Disclosures*.)
- The controller who writes the prompt reads a held-out packet. The labelling spec keeps the controller blind so that the prompt cannot leak the held-out cases.
- A judge answer is stored or reported as a label. It is a model output, scored against labels.
- A missing or invalid field defaults to a quiet or favourable value. The old judge's R119 rule applies: flag it, never default it.

## Constraints

- `build_packet` already refuses a packet that holds a GitHub-token-shaped string, and includes only entries strictly before the unit. The judge adds no second guard and loosens neither.
- The prompt is developed on development material only: approved examples A–F in `docs/research/2026-09-26-codex-three-role-intervention.md`, and the derived adjudications in `docs/research/2026-09-29-codex-system1-derived-adjudications.md`. Both are marked "not held-out evidence". The prompt copies no 20-character run of the RTD doc (`judge.RTD_DOC`), of the derived adjudications, or of the worksheet rows A–F, and it shows no worked example. This extends the old R134 rule, and an example copied from development material would contaminate any later scoring on it. This is my call.
- The packet is embedded as data. Text inside it must not be able to close its own fence or inject an instruction. The old `judge._data` solves this for the old prompt.
- Python tests run in `./scripts/gate.sh`. Tests make no network call and need no Codex channel.

## Approaches

**1. A new module beside the old judge (chosen).** A new module in `scripts/measure/` takes a `Packet`, has its own prompt and verdict, and borrows the old judge's channel, JSON extraction, quote matching and retry policy. The old modes stay as they are. A prompt revision touches only the new module. It costs two verdict schemas in the repo until the old one is retired.

**2. Replace the old judge in place.** `build_input`, `Verdict`, `parse_verdict`, the majority vote and the leak scan all change. One schema remains. The blast radius is large, and the Task 9b result stops being reproducible. Rejected.

**3. Swap the packet only and keep the old schema.** It is the smallest change. It keeps the "mistake" target that the operator already rejected. Rejected.

Confidence in approach 1: medium to high. The main risk is the duplicated schema, which a named later step retires.

## Design

All names below are proposals. The builder may rename them.

**Module and prompt.** `scripts/measure/intervention_judge.py` and `scripts/measure/intervention_prompt.md`. The module loads `judge.py` and `label.py` as siblings, as `label.py` already loads its own siblings (`_load_sibling`), and reuses `codex_channel`, `_extract_json`, `_verified_span`, `_data`, `ToolCallError` and the retry constants. It does not copy them.

**Input.** `build_input(packet)` returns a dict with `id` (the packet's `case_id`), `packet_sha256`, `pre_evidence` (the packet text) and `prompt`. The prompt holds the instructions, the operator's bars and the packet text as data. It holds nothing else about the case.

**Prompt content** (the builder drafts it; these are requirements):
- The five annotation distinctions: error, needs verification, needs qualification, no intervention needed, cannot tell. They map to the labels correct, verify, qualify, none and unresolved.
- The bars exactly as the operator labelled under them: the text of `label.py` `_BANNER_BODY` (the question in step 2, the bars in step 3, the delivery rule in step 4), verbatim, so that the judge and the gold read the same instructions. The banner is shorter than the research doc's § *Operator decision*. The prompt adds the routing lines the banner omits, each marked as a supplement taken from that approved section: interrupt is for a visibly false claim about to be sent, and unavailable evidence means unverified, not false. This is my call.
- Silence is the answer when a check is already the next action, and when an accurate scoped historical snapshot is not being treated as current.
- Delivery: quiet routes a check to System 2. Interrupt is for a visibly false claim about to be sent.
- An unresolved packet is a valid answer. Missing trace evidence is not an error, and unavailable evidence means unverified, not false.
- The judge must not assume the packet is complete. It is a bounded view.

**Verdict.** One JSON object:
- `labels`: a list. A non-empty subset of `verify`, `qualify`, `correct`, or exactly `["none"]`, or exactly `["unresolved"]`.
- `delivery`: `silent`, `quiet` or `interrupt`.
- `action`: one sentence naming the specific check or qualification. An empty string when `labels` is `none` or `unresolved`.
- `quote`: a verbatim span of the packet that the judgement rests on. It is required when `labels` is a subset of verify, qualify, correct.

Validation reuses `label.validate(labels, delivery)`. A pairing violation is flagged `invalid:pairing`. The vote is kept, not defaulted, and **not retried**, because the rate at which the model breaks its own pairing rule is itself a result. This is my call.

**Quote check.** A non-silent vote whose `quote` is not found in `pre_evidence` by `_verified_span` gets the flag `quote_not_found`. The vote is kept, not retried. This is my call; it mirrors how the old `verify_quote` flags and does not drop.

**Votes.** Three votes, at most three concurrent, as `judge.judge` does today. An attempt that is `call_failed`, `unparseable` or `tool_call` is re-issued, up to 3 attempts per vote (R137). Raw replies and attempts are kept.

**Result record.**

```
{"id", "packet_sha256",
 "votes": [{"labels", "delivery", "action", "quote", "flags", "raw", "attempts"}, ...],
 "label_votes": {"verify": n, "qualify": n, "correct": n, "none": n, "unresolved": n},
 "delivery_votes": {"silent": n, "quiet": n, "interrupt": n}}
```

The record has **no consensus field**. The consensus rule for a set-valued answer changes what a gate measures, so the gate spec chooses it. This is my call.

**Counting rule.** `label_votes` counts every vote whose `labels` value is valid, and `delivery_votes` counts every vote whose `delivery` value is valid. A vote flagged `invalid:pairing` or `quote_not_found` is counted. A vote with no parseable reply, or with an invalid `labels` or `delivery` value, is not counted in that tally. The record adds `flag_counts` (votes per flag) and `n_uncounted`, so a reader sees how many votes broke a rule and how many failed outright. This is my call.

**Prompt development protocol.**
- The controller drafts the prompt from examples A–F and the derived adjudications.
- Tests use synthetic packets built with `tests/measure_corpus_fixture.py` and `packet.build_packet`.
- No agent session reads or renders a labelled packet. The existing `label.py summary` subcommand gives counts only.
- A committed prompt is hashed. The gate spec registers that hash, as the labelling spec registers its code hashes.

## What the builder decides

- Names, and the split between the module and any helper.
- The prompt's wording and layout, within the requirements above.
- Whether the prompt asks for a short reasoning field. The field would not be scored and would not enter `pre_evidence`.
- How to load `judge.py` and `label.py` without a circular import.

## Tests (required)

Each test must fail against a plausible wrong implementation.

- A packet containing the operator's example D (tests reported as passing, visible failures) produces an input whose `pre_evidence` is the packet text byte for byte.
- The input and the prompt hold no case key, transcript path, stratum, session id or unit timestamp. The test builds a packet from a fixture with known values and searches for each. The fixture avoids calendar dates in its prose, so a date found in the input can only have come from metadata.
- The rendered prompt shares no 20-character run with the RTD doc, the derived adjudications or the worksheet rows A–F, and a positive control shows the check can fire (the old R134 test is the model).
- A packet text that contains the fence and end-marker strings cannot close the data block (adapt the old `_data` test).
- `parse` reads each label shape. A missing or invalid `labels`, `delivery` or `quote` is flagged, never defaulted.
- Each pairing violation (`none` with `quiet`; `verify` with `silent`; `none` with `verify`) gets `invalid:pairing` and is not retried.
- A quote absent from the packet gets `quote_not_found`. A quote present with different whitespace is accepted.
- Retry: a `call_failed` first attempt then a good second attempt gives one good vote with two recorded attempts. Three failures stay a failed vote.
- The result carries `packet_sha256` equal to the packet's, and `label_votes` counts match the votes.
- A mutation probe (`./scripts/mutation-probe.sh`) is out of scope unless the builder judges a check load-bearing.

## Open for the operator

**What closes bug `61f699816f5ee2fd`?** With approach 1 the old `_session_part` still hides tool evidence, but nothing calls it. Two options:
- Keep the bug open until a later spec retires the old audit and correction modes.
- Close it now as superseded, and mark `_session_part` deprecated in its docstring with a pointer to the new module.

My recommendation is the first, because the defect stays in the code until the code is removed. The operator decides.

**Resolved 2026-10-24:** the operator approved the design with this recommendation written in it and did not pick the second option, so the bug stays open. The operator can still reverse this. The plan therefore has no deprecation step and no bug-close step. The old modes' retirement spec is the step that closes the bug.

## Disclosures

- Nothing here was run. The design rests on code read in the session: `judge.build_input`, `judge.judge`, `judge.Verdict`, `judge.parse_verdict`, `packet.build_packet`, `label.validate`, and `sampler.Unit`.
- Found while checking this design: `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` § *Packet rule* says "no session id, date or absolute timestamp", but `scripts/measure/packet.py` keeps bare calendar dates by design (its module docstring). This spec follows the code.
- Not established: that the packet's size (20,000 characters, blinded) suits the judge model's input limit.
- Not established: that a judge prompt which reads the operator's bars reproduces the operator's labels. The gate spec measures that.
- A future run sends operator-session excerpts to the Codex channel. The existing document-item gate already does this. This spec makes no such call.
