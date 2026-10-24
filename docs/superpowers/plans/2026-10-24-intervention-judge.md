---
id: '7b10a8b9437155dc'
kind: plan
status: draft
title: Intervention Judge Implementation Plan
tags:
- system1
- measurement
- judge
---

# Intervention Judge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A judge that reads the operator's packet and answers with the operator's labels and delivery, in a new module beside the old judge.

**Architecture:** `scripts/measure/intervention_judge.py` takes a `packet.Packet`, renders a new prompt (`intervention_prompt.md`) around it, runs three votes through the old judge's Codex channel, and returns the raw votes plus tallies. It loads `judge.py` and `label.py` as siblings and reuses their helpers. It edits neither.

**Tech Stack:** Python 3, `unittest`-style tests run by `pytest` (`scripts/python-tests.py` lane `light`), no network.

**Spec:** `docs/superpowers/specs/2026-10-24-intervention-judge-design.md` (read it first; this plan follows it section by section).

## Global Constraints

- Labels are a non-empty subset of `verify`, `qualify`, `correct`, or exactly `["none"]`, or exactly `["unresolved"]`. Delivery is `silent`, `quiet` or `interrupt`. `verify`/`qualify`/`correct` need `quiet` or `interrupt`; `none`/`unresolved` need `silent`. (`label.LABELS`, `label.DELIVERIES`, `label.validate`.)
- Three votes, at most three concurrent. An attempt that is `call_failed`, `unparseable` or `tool_call` is re-issued, up to 3 attempts per vote.
- A missing or invalid field is flagged and never defaulted.
- Edit **no** line of `scripts/measure/judge.py`, `scripts/measure/label.py`, `scripts/measure/packet.py` or `tests/test_measure_judge.py`. The Task 9b gate result must stay reproducible.
- Tests make no network call and need no Codex channel. They inject `complete`.
- The prompt uses general terms only and shows no worked example. It shares no 20-character run with the RTD doc, the derived adjudications or worksheet rows A–F.
- No agent renders or reads a labelled packet, and none runs `label.py next`. Test packets are synthetic.
- No real run on real packets, and no scored gate. Both belong to a later spec.
- Commits: the operator has not asked for any. Stage and commit only when asked (project `CLAUDE.md`). When asked: write the message to a scratchpad file, stage explicit paths, then `git commit -F <file> -- <paths>`.
- Run targeted tests with `python3 -m pytest -p no:cacheprovider -q tests/test_measure_intervention_judge.py`. Before the task is complete, run `./scripts/gate.sh` (on a clean worktree of the committed bytes if peers have uncommitted files).

## Review Focus

These inputs follow from the spec, and no task's main tests would otherwise exercise them. Each has a test in the task named in brackets.

1. A reply that wraps its JSON in a ```` ```json ```` fence or in prose. It must parse. [Task 1]
2. `labels` given as the bare string `"verify"`. It must be flagged `invalid:labels`, not coerced to a list. [Task 1]
3. A quote shorter than 12 characters on a non-silent vote. It must get `quote_not_found`, because `MIN_QUOTE_CHARS` makes short quotes match anywhere. [Task 1]
4. Packet text that contains `{{BARS}}` or a `===== END PACKET =====` line. It must not be expanded or close the block. [Task 2]
5. Every attempt of every vote fails. The call must return a record with `n_uncounted` set and raise nothing. [Task 3]

---

## File structure

- Create `scripts/measure/intervention_judge.py`: parsing, prompt rendering, `judge`.
- Create `scripts/measure/intervention_prompt.md`: the prompt template.
- Create `tests/test_measure_intervention_judge.py`: all tests.

One module is enough. It will hold well under 300 lines.

### Task 1: The module and the vote parser

**Files:**
- Create: `scripts/measure/intervention_judge.py`
- Create: `tests/test_measure_intervention_judge.py`

**Interfaces:**
- Produces:
  - `ij._judge`: the loaded `judge.py` module. `ij._label`: the loaded `label.py` module.
  - `Verdict`: a `dataclasses.dataclass` with `labels: list | None = None`, `delivery: str | None = None`, `action: str = ""`, `quote: str = ""`, `flags: list = field(default_factory=list)`.
  - `parse_vote(raw: str, pre_evidence: str) -> Verdict`.

Load the siblings the way `label.py` does (`_load_sibling`, `importlib.util.spec_from_file_location`). The tests load the module under test the same way. Two copies of `judge.py` can be loaded in one process, so a test that needs `ToolCallError` must take it from `ij._judge`, never from its own `judge` import.

- [ ] **Step 1: Write the failing tests** in `tests/test_measure_intervention_judge.py`. Header: insert `scripts/measure` and `tests` into `sys.path`, load `intervention_judge` as `ij` with a `_load` helper like `tests/test_measure_packet.py`'s. Add `class ParseVoteTests(unittest.TestCase)` with a helper `vote(**fields)` that builds a reply with `json.dumps`, a constant `EVIDENCE = "The agent wrote: the whole suite passed, so the edge case works."`, and these tests:

```python
def test_a_valid_verify_vote_parses_clean(self):
    v = ij.parse_vote(self.vote(labels=["verify"], delivery="quiet", action="Read the assertion.",
                                quote="the whole suite passed"), EVIDENCE)
    self.assertEqual((v.labels, v.delivery, v.flags), (["verify"], "quiet", []))

def test_none_and_unresolved_need_no_quote(self):
    for label in ("none", "unresolved"):
        v = ij.parse_vote(self.vote(labels=[label], delivery="silent", action="", quote=""), EVIDENCE)
        self.assertEqual(v.flags, [], label)

def test_a_fenced_or_prose_wrapped_reply_parses(self):  # review focus 1
    raw = "Here is my answer:\n```json\n" + self.vote(labels=["none"], delivery="silent", action="", quote="") + "\n```"
    self.assertEqual(ij.parse_vote(raw, EVIDENCE).labels, ["none"])

def test_unparseable_reply_is_flagged(self):
    self.assertEqual(ij.parse_vote("no json here", EVIDENCE).flags, ["unparseable"])

def test_invalid_label_shapes_are_flagged_never_coerced(self):  # review focus 2
    for bad in ("verify", [], ["verify", "verify"], ["bogus"], ["none", "verify"], [1]):
        v = ij.parse_vote(self.vote(labels=bad, delivery="quiet", action="x", quote="the whole suite passed"), EVIDENCE)
        self.assertIsNone(v.labels, bad)
        self.assertIn("invalid:labels", v.flags, bad)

def test_missing_fields_are_flagged(self):
    v = ij.parse_vote(json.dumps({"delivery": "quiet"}), EVIDENCE)
    self.assertEqual({"missing:labels", "missing:action", "missing:quote"} <= set(v.flags), True)

def test_invalid_delivery_and_action_are_flagged(self):
    v = ij.parse_vote(self.vote(labels=["none"], delivery="loud", action=5, quote=""), EVIDENCE)
    self.assertIsNone(v.delivery)
    self.assertTrue({"invalid:delivery", "invalid:action"} <= set(v.flags))

def test_a_pairing_violation_is_flagged_and_both_values_are_kept(self):
    for labels, delivery in ((["none"], "quiet"), (["verify"], "silent"), (["unresolved"], "interrupt")):
        v = ij.parse_vote(self.vote(labels=labels, delivery=delivery, action="x", quote="the whole suite passed"), EVIDENCE)
        self.assertEqual((v.labels, v.delivery), (labels, delivery))
        self.assertIn("invalid:pairing", v.flags)

def test_a_quote_absent_from_the_packet_is_flagged(self):
    v = ij.parse_vote(self.vote(labels=["verify"], delivery="quiet", action="x", quote="a sentence that is not there"), EVIDENCE)
    self.assertIn("quote_not_found", v.flags)

def test_a_quote_with_other_whitespace_is_accepted(self):
    v = ij.parse_vote(self.vote(labels=["verify"], delivery="quiet", action="x", quote="the  whole\nsuite   passed"), EVIDENCE)
    self.assertNotIn("quote_not_found", v.flags)

def test_a_quote_under_the_minimum_length_is_not_found(self):  # review focus 3
    v = ij.parse_vote(self.vote(labels=["verify"], delivery="quiet", action="x", quote="suite"), EVIDENCE)
    self.assertIn("quote_not_found", v.flags)
```

- [ ] **Step 2: Run them to verify they fail.** Run the targeted command. Expected: every test errors with `FileNotFoundError` or `AttributeError` (the module does not exist).

- [ ] **Step 3: Implement** `Verdict` and `parse_vote(raw: str, pre_evidence: str) -> Verdict` in `scripts/measure/intervention_judge.py`, with the sibling loaders. Reuse `_judge._extract_json` and `_judge._verified_span`. Rules: JSON absent gives `flags=["unparseable"]` and nothing else. Check shape first (`labels` a non-empty list of unique strings that is a subset of `_label.LABELS`, or exactly `["none"]`, or exactly `["unresolved"]`; `delivery` in `_label.DELIVERIES`; `action` a `str`), flag `missing:<f>` for an absent key and `invalid:<f>` for a bad value, and set the bad field to `None` (`action` and `quote` keep `""`). Only when both `labels` and `delivery` are valid, call `_label.validate` and turn its `ValueError` into the flag `invalid:pairing`. When `labels` is a non-silent subset and `_verified_span(quote, pre_evidence)` is `None`, add `quote_not_found`. Flags appear in the order found.

- [ ] **Step 4: Run the tests to verify they pass.** Expected: all `ParseVoteTests` pass.

- [ ] **Step 5: Commit** only if the operator has asked: `scripts/measure/intervention_judge.py`, `tests/test_measure_intervention_judge.py`.

### Task 2: The prompt and the judge input

**Files:**
- Create: `scripts/measure/intervention_prompt.md`
- Modify: `scripts/measure/intervention_judge.py`
- Modify: `tests/test_measure_intervention_judge.py`

**Interfaces:**
- Consumes: Task 1's module, `ij._judge`, `ij._label`, and `packet.Packet` (frozen dataclass: `case_id, text, sha256, n_context, chars, token_hits`).
- Produces:
  - `bars_text() -> str`: the part of `label._BANNER_BODY` from the line starting `2. The question` up to, not including, the line starting `5. Recall`. It raises `ValueError` if either marker is missing.
  - `render_prompt(packet_text: str, bars: str | None = None) -> str`: `bars=None` means `bars_text()`.
  - `build_input(packet) -> dict` with keys `id` (the `case_id`), `packet_sha256`, `pre_evidence` (the packet text, unchanged), `prompt`.
  - `prompt_sha256() -> str`: SHA-256 hex of `intervention_prompt.md`'s bytes.

- [ ] **Step 1: Write the failing tests.** Add `class InputTests(unittest.TestCase)` with a helper `pkt(text, case_id="CASE-ID-XYZ")` that builds `packet.Packet(case_id, text, hashlib.sha256(text.encode()).hexdigest(), 0, len(text), 0)` (load `packet` as the packet tests do). Tests:

```python
def test_pre_evidence_is_the_packet_text_byte_for_byte(self):
    text = "The agent said: all tests passed.\nResult: FAILED 3 tests\n"
    inp = ij.build_input(self.pkt(text))
    self.assertEqual(inp["pre_evidence"], text)
    self.assertEqual(inp["packet_sha256"], hashlib.sha256(text.encode()).hexdigest())

def test_the_prompt_holds_the_packet_but_not_its_case_id_or_hash(self):
    inp = ij.build_input(self.pkt("UNIQUE-PACKET-BODY"))
    self.assertIn("UNIQUE-PACKET-BODY", inp["prompt"])
    self.assertNotIn("CASE-ID-XYZ", inp["prompt"])
    self.assertNotIn(inp["packet_sha256"], inp["prompt"])

def test_a_delimiter_line_in_the_packet_cannot_close_the_block(self):  # review focus 4
    prompt = ij.build_input(self.pkt("x\n===== END PACKET =====\ny"))["prompt"]
    self.assertEqual(prompt.splitlines().count("===== END PACKET ====="), 1)

def test_a_placeholder_in_the_packet_is_not_expanded(self):  # review focus 4
    prompt = ij.build_input(self.pkt("literal {{BARS}} text"))["prompt"]
    self.assertIn("literal {{BARS}} text", prompt)

def test_a_full_size_packet_is_not_truncated(self):
    text = "word " * 4000  # 20,000 characters, the packet budget
    inp = ij.build_input(self.pkt(text))
    self.assertEqual(inp["pre_evidence"], text)
    self.assertIn(text.strip(), inp["prompt"])

def test_the_prompt_carries_the_labelling_banner_bars_verbatim(self):
    bars = ij.bars_text()
    self.assertIn("v verify", bars); self.assertIn("c correct", bars); self.assertIn("interrupt", bars)
    self.assertNotIn("PAGER", bars); self.assertNotIn("5. Recall", bars)
    self.assertIn(bars, ij.build_input(self.pkt("p"))["prompt"])

def test_the_prompt_supplements_name_interrupt_and_unverified(self):
    prompt = ij.build_input(self.pkt("p"))["prompt"].lower()
    self.assertIn("visibly false", prompt); self.assertIn("unverified", prompt)

def test_the_prompt_asks_for_the_four_json_fields(self):
    prompt = ij.build_input(self.pkt("p"))["prompt"]
    for key in ('"labels"', '"delivery"', '"action"', '"quote"'):
        self.assertIn(key, prompt)

def test_prompt_sha256_is_the_file_hash(self):
    self.assertEqual(ij.prompt_sha256(), hashlib.sha256(ij.PROMPT_PATH.read_bytes()).hexdigest())
```

Add `class NoCopyTests(unittest.TestCase)` modelled on `tests/test_measure_judge.py::test_the_rendered_prompt_copies_no_20_char_run_of_the_rtd_doc`. Corpora: the RTD doc (`ij._judge.RTD_DOC` under `REPO_ROOT`), `docs/research/2026-09-29-codex-system1-derived-adjudications.md`, and the section of `docs/research/2026-09-26-codex-three-role-intervention.md` that starts at `### Human worksheet — development examples, not held-out evidence` and ends before the next `## ` heading. Render with `ij.render_prompt("p", bars="")` so only prompt-authored text is compared; the banner is exempt because it is the operator's own instruction text. A positive control appends `ij._judge.parse_rtd_cases(rtd)[0]["positive"]` and asserts the shared set is then non-empty. Also add `class EndToEndPacketTests(unittest.TestCase)` that builds a real packet (import `Seq` and `SID` from `test_measure_packet`, write the session with `fx.build_corpus`, `sampler.frame`, `packet.build_packet`, copying `PacketCase.build`'s body) from a transcript whose entries carry `SID`-based uuids and `2026-09-20T10:..Z` timestamps, then asserts `SID`, `"2026-09-20T"` and `"case-1"` are absent from `ij.build_input(p)["prompt"]` while the unit's own text is present.

- [ ] **Step 2: Run them to verify they fail.** Expected: `AttributeError` on `build_input` and friends.

- [ ] **Step 3: Write `scripts/measure/intervention_prompt.md`** and implement `bars_text`, `render_prompt`, `build_input`, `prompt_sha256`. Template placeholders: `{{BARS}}` and `{{PACKET}}`. Fill in one pass with `_judge._PLACEHOLDER`, strip comments with `_judge._COMMENT`, and pass the packet text through `_judge._data`. Frame the packet between the lines `===== PACKET =====` and `===== END PACKET =====`. Draft the wording from the spec's *Prompt content* list, from the research doc's § *Operator decision*, and from examples A–F, in general terms and with no worked example. Required content: the question and bars (`{{BARS}}`); a sentence that the letters in the bars are the operator's shorthand and the reply uses the full words; the supplements (interrupt is for a visibly false claim about to be sent, and unavailable evidence means unverified, not false); that the packet is a bounded view and may not be complete; that silence and `unresolved` are valid answers; and the exact reply format, a single JSON object with `labels`, `delivery`, `action`, `quote`. Do not read any labelled packet while drafting.

- [ ] **Step 4: Run the tests to verify they pass.** If `NoCopyTests` fails, reword the shared run in the template. Do not shrink the corpora.

- [ ] **Step 5: Commit** only if the operator has asked: the prompt, the module, the test file.

### Task 3: The vote and the result record

**Files:**
- Modify: `scripts/measure/intervention_judge.py`
- Modify: `tests/test_measure_intervention_judge.py`

**Interfaces:**
- Consumes: Task 1's `parse_vote` and `Verdict`; Task 2's `build_input`.
- Produces: `judge(packet, votes: int = 3, complete=None, log_dir=None) -> dict`. `complete(prompt, log_path) -> str` is injectable; the default is `_judge.codex_channel().complete`. The record:

```
{"id", "packet_sha256",
 "votes": [{"labels", "delivery", "action", "quote", "flags", "raw", "attempts"}],
 "label_votes": {"verify": n, "qualify": n, "correct": n, "none": n, "unresolved": n},
 "delivery_votes": {"silent": n, "quiet": n, "interrupt": n},
 "flag_counts": {flag: n}, "n_uncounted": n}
```

Each `attempts` item is `{"attempt", "failure", "flags", "raw"}`, as in `judge.judge`. `flag_counts` has one key per distinct flag. The flags `unparseable`, `quote_not_found`, `missing:<field>`, `invalid:<field>` and `invalid:pairing` are keys as they are. A `call_failed:<detail>` or `tool_call:<detail>` flag is keyed by the text before its first `:`, so the exception text never splits the count. The value is the number of votes that carry the flag, counted once per vote. `label_votes` counts every label of every vote whose `labels` is not `None`. `delivery_votes` counts every vote whose `delivery` is not `None`. `n_uncounted` counts votes whose `labels` or `delivery` is `None`.

- [ ] **Step 1: Write the failing tests.** Add `class JudgeTests(unittest.TestCase)` with `reply(labels, delivery, action="x", quote="the whole suite passed")` and a packet whose text contains that quote. Use a lock-guarded call counter in each fake `complete`. Tests:

```python
def test_three_agreeing_votes_are_tallied(self): ...
    # label_votes == {"verify": 3, "qualify": 0, "correct": 0, "none": 0, "unresolved": 0}
    # delivery_votes == {"silent": 0, "quiet": 3, "interrupt": 0}; n_uncounted == 0; len(votes) == 3
    # record["packet_sha256"] == packet.sha256 and record["id"] == packet.case_id
def test_every_label_of_a_multi_label_vote_is_counted(self): ...
    # replies [verify,qualify] / [verify] / [none]: verify 2, qualify 1, none 1
def test_a_failed_first_attempt_is_reissued(self): ...
    # fake raises RuntimeError on call 1 only; votes=3; total calls == 4; exactly one vote has 2 attempts,
    # its first attempt failure == "call_failed"; n_uncounted == 0
def test_three_failed_attempts_leave_an_uncounted_vote(self):  # review focus 5
    # fake always raises; votes=1: calls == 3, n_uncounted == 1, all label_votes 0, returns a record
def test_a_tool_call_error_is_reissued(self): ...
    # fake raises ij._judge.ToolCallError({"exec": 1}); 3 attempts; flag starts with "tool_call"
def test_an_unparseable_reply_is_reissued(self): ...
    # "garbage" then a valid reply, votes=1: attempts length 2, vote counted
def test_a_pairing_violation_is_counted_and_not_reissued(self): ...
    # reply(["none"], "quiet"), votes=1: calls == 1; flags include "invalid:pairing";
    # label_votes["none"] == 1 and delivery_votes["quiet"] == 1
def test_a_missing_quote_is_counted_and_not_reissued(self): ...
    # reply with quote "not in the packet": calls == 1; flag_counts["quote_not_found"] == votes
def test_flag_counts_tally_votes_per_flag(self): ...
def test_each_attempt_gets_its_own_log_path_under_log_dir(self): ...
    # log paths are <log_dir>/<safe id>.vote<k>.try<a>.log, and log_dir is created
```

- [ ] **Step 2: Run them to verify they fail.** Expected: `AttributeError: judge`.

- [ ] **Step 3: Implement `judge`** by following `judge.judge` (`scripts/measure/judge.py` L1088-1134): same thread pool bound (`_judge.MAX_CONCURRENT_CALLS`), same retry loop over `_judge.MAX_ATTEMPTS_PER_VOTE` using `_judge._failure`, same safe-name rule for log paths. An attempt that raises `_judge.ToolCallError` is a `Verdict(flags=[f"tool_call:{exc}"])`; any other exception is `Verdict(flags=[f"call_failed:{type(exc).__name__}: {str(exc)[:200]}"])`. Parse with `parse_vote(raw, inp["pre_evidence"])`. Build the tallies as described under Interfaces.

- [ ] **Step 4: Run the tests to verify they pass.** Expected: every test in the file passes.

- [ ] **Step 5: Run the full gate.** Run `./scripts/gate.sh`. Expected: exit 0, with the Python lane green. Report any failure by name. This plan changes no Rust.

- [ ] **Step 6: Commit** only if the operator has asked: `scripts/measure/intervention_judge.py`, `scripts/measure/intervention_prompt.md`, `tests/test_measure_intervention_judge.py`, the spec, the plan, the new issue file and the judge bug file. Cite the spec's id in the message.

## Self-review

- **Spec coverage.** Goals: packet input (Tasks 1–3), new prompt (Task 2), verdict shape and `label.validate` (Task 1), `packet_sha256` (Tasks 2–3), old judge untouched (Global Constraints). Design: module and prompt (Tasks 1–2), input (Task 2), verdict, pairing and quote rules (Task 1), votes (Task 3), result record and counting rule (Task 3), prompt hash (Task 2), development protocol (Global Constraints and Task 2 Step 3). Tests section: every bullet has a test; the mutation probe stays out of scope, as the spec says. *Open for the operator*: resolved as "keep open", so the plan has no deprecation or bug-close step.
- **Types.** `Verdict`, `parse_vote`, `build_input`, `render_prompt`, `bars_text`, `prompt_sha256` and `judge` keep one name and one signature across tasks.
- **Proportion.** Measured: the plan is 268 lines against the spec's 158 (about 1.7 times), and most of it is test names and assertions, not bodies.
