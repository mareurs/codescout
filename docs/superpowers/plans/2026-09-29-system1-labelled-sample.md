---
id: '007a53aac937fdf7'
kind: plan
status: draft
title: System 1 labelled sample — implementation plan
tags:
- system1
- measurement
- labelling
---

# System 1 Labelled Sample Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Draw a stratified uniform sample of assistant messages from the frozen codescout corpus, render
blinded packets, let the operator label them in their own terminal, and estimate the rate of useful
System 1 interventions under a decision rule fixed in advance.

**Architecture:** Four focused modules in `scripts/measure/`:
- `sampler.py`: the frame, the strata and a seeded draw;
- `packet.py`: the packet rule, the token scan and a sha256 per packet;
- `label.py`: the operator's command-line tool;
- `estimate.py`: intervals and the decision.

`run.py` gains the subcommands that wire them together. Private material (packets, the key, notes)
lives only under `~/work/claude/measurement-corpora/`. Only ids, hashes, labels and aggregates are
committed.

**Tech Stack:** Python 3 standard library only, with `unittest` tests run through the prompt-engineering venv's pytest.

**Spec:** `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` (catalog id `0bdaeecdc70db511`, committed `f357adad`).

## Global Constraints

**Environment and data:**

- Run everything from the repo root with `~/work/claude/prompt-engineering/.venv/bin/python`. Tests
  are `unittest` classes loaded by path, like the existing `tests/test_measure_transcripts.py`
  (`_load(name)` via `importlib.util`).
- **No model calls anywhere in this plan:** no `claude -p`, no `codex`, no API.
- **Tests use synthetic transcripts only.** No test reads `~/work/claude/measurement-corpora/`.
- **Frame corpus:** `~/work/claude/measurement-corpora/2026-09-29-codescout`. Sessions are the kept
  copies under `transcripts.exclusions(transcripts.sessions(c), join.SPEC_EXCLUDED_SIDS)`, over the
  whole retained window.
- **Private data:** packets, the key file and operator notes live only in a label-set directory
  under `~/work/claude/measurement-corpora/` (mode 700). Every writer refuses a directory inside the
  repo, reusing `archive._refuse_if_inside_repo`.
- **Committed data** goes under `docs/evals/data/2026-09-27-system1-base-rates/labelled-sample/`, and
  only as: case ids, packet sha256s, strata, unit kinds, labels, deliveries, recall flags, seconds, and
  aggregates. **Never packet text or notes.**

**Blindness:**

- **No agent ever reads packet text.** No agent runs `label.py next`, opens a packet file, or prints a
  packet. Agents may run `label.py summary`, `label.py verify`, and the `run.py` subcommands, whose
  output is counts and ids only.

**Exact values from the spec:**

| value | setting |
|---|---|
| threshold T | 0.10 |
| Wilson z | 1.959964 |
| main sizes | 60 substantive, 20 routine |
| pilot | 5 units, separate seed, disjoint from main |
| re-label | 10 main units, at least 3 days after first labelling |
| unresolved guard | more than 25% of substantive labels unresolved makes the outcome inconclusive |
| bootstrap | 10,000 session resamples, 95% percentile |
| packet: operator message / dispatch prompt | first 1,500 chars |
| packet: context | last 6 API messages |
| packet: tool arguments | cut to 300 chars |
| packet: tool results | last 1,500 chars, plus the exit code when parseable |
| packet: total | at most 20,000 chars |

**Process:**

- **Security, verbatim in every dispatch:** "NEVER print environment variables: no `env`, `printenv` or
  `set`, and no unfiltered /proc/*/environ. A prior subagent's env dump leaked a GitHub token into its
  transcript. Read one named variable only when needed, and never a credential."
- **Committing:** by pathspec only. `git add <paths>`, then `git diff --cached --name-only`, then
  `git commit -m … -- <paths>`, each as a separate call, with the trailer
  `Session-Id: <your session id>`. Never `git add -A`, never a directory, never stash, reset, checkout or
  rebase, no push. The peers' staged `.buddy/memory/INDEX.md` and
  `.buddy/memory/data-leakage-snow-pheasant/rule-tell-phase1-autopsy.md` stay staged and untouched.
- **Before each commit under `scripts/`:** run `scripts/with-slot.sh cargo test --test committed_paths`,
  and the task's own suite plus every existing `tests/test_measure_*.py` suite, each printed beside its count.
- **Mutation evidence (R38):** every guarded site named in a task's tests is mutated once in a scratch
  copy of the module, never the shared file. Assert the pattern occurs exactly once, print the loaded
  path, and record each red as an `AssertionError`, with the baseline printed first.
- **Subagents** use the codescout MCP tools (`run_command`, `symbols`, `edit_code`, `create_file`),
  never call `workspace(action="activate")`, and pass `workspace="/home/marius/work/claude/codescout"`
  on every write.

## Review Focus

1. **A message split across entries.** Claude Code writes one API message as several transcript
   entries sharing `message.id` (text in one, `tool_use` in another). The unit must group them, and
   its pending action must include tool calls from the sibling entries. *Task 1 and Task 2 tests.*
2. **Synthetic and API-error assistant entries** (`message.model == "<synthetic>"`, or
   `isApiErrorMessage` true) are not agent decisions and must never be units. *Task 1 test.*
3. **Tool results come in many shapes:** a list of content blocks, a single long line, and an exit
   code stated either as JSON `"exit_code": N` or as a line `Exit code N`. The exit code is parsed from
   the **whole** result before its tail is cut, so a code outside the kept tail is still shown.
   *Task 2 test.*
4. **A subagent file whose last assistant message has no text** (tool-only, or the file ends
   mid-call). The hand-back is the last assistant message **with text**; a file with none contributes
   no unit. *Task 1 test.*
5. **A session copied across profiles (R22) and compaction summaries.** Only the kept copy's messages
   enter the frame, and "the operator's last message" is never a compaction summary or a
   command wrapper (`transcripts.operator_messages` already excludes them). *Tests in Task 1 and Task 2.*

---

### Task 1: The frame, strata and draw (`sampler.py`)

**Files:**
- Create: `scripts/measure/sampler.py`
- Create: `tests/measure_corpus_fixture.py`, the synthetic corpus builder shared by Tasks 1, 2 and 5
- Test: `tests/test_measure_sampler.py`

**Interfaces:**
- Consumes: `transcripts.sessions`, `transcripts.exclusions`, `transcripts.copy_id`,
  `transcripts.read_jsonl(path) -> (entries, skipped)`, `join.SPEC_EXCLUDED_SIDS`.
- Produces:
  - `build_corpus(root: pathlib.Path, sessions: list[dict]) -> pathlib.Path` in the fixture module.
    Each dict is `{"sid", "profile", "slug", "entries": [...], "subagents": {name: [entries]}}`, and
    the builder writes `transcripts/<NN-profile>/<slug>/<sid>.jsonl` plus
    `<sid>/subagents/<name>.jsonl`. It also provides `assistant(uuid, ts, mid, text=None,
    tool_uses=(), stop=None, model="claude", sidechain=False)`, `user_prompt(uuid, ts, text)` and
    `tool_result(uuid, ts, tool_use_id, content, is_error=False)` entry makers.
  - `@dataclass(frozen=True) Unit` with fields `case_key: str`, `copy_id: str`, `transcript: str`
    (corpus-relative), `message_id: str`, `kind: str` (`"top"` or `"handback"`), `stratum: str`
    (`"substantive"` or `"routine"`), `reasons: tuple[str, ...]`, `first_entry_index: int` and
    `decision_ts: str`. `case_key = f"{copy_id}|{transcript}|{message_id}"`.
  - `frame(corpus_dir, excluded_sids=None) -> list[Unit]`, where `None` means `join.SPEC_EXCLUDED_SIDS`.
  - `classify(message_entries: list[dict]) -> tuple[str, tuple[str, ...]]`, returning the stratum and
    the admitting reasons.
  - `draw(units: list[Unit], sizes: dict[str, int], seed: int, exclude: set[str] = frozenset()) -> list[Unit]`.
  - `frame_counts(units) -> dict`, as `{stratum: {kind: n}}`.
  - Constants `CLAIM_PATTERNS`, `EDIT_TOOLS`, `DISPATCH_TOOLS`, `SHELL_TOOLS`, `CONSEQ_CMD`,
    `CATALOG_TOOLS`, `CATALOG_WRITE_ACTIONS` and `REASONS`.

**Fixed values.** The claim markers are exactly these four, compiled with `re.IGNORECASE`, verbatim
from the corpus-profile run of 2026-09-29:

```python
CLAIM_PATTERNS = {
    "completion": r"\b(?:done|fixed|passes|passing|verified|confirmed|committed|green|completed?|resolved|implemented|landed|merged|pushed|shipped|works now|now works|all set)\b",
    "test_result": r"\b\d[\d,]*\s+(?:passed|failed|passing|failing|tests? pass(?:ed)?|tests? fail(?:ed)?|ignored|skipped)\b|\b0\s+(?:failed|failures|errors|warnings)\b|test result:\s*ok|\ball\s+(?:tests?\s+)?(?:pass|green)\b|\b\d+\s*/\s*\d+\s+(?:pass|tests?)\b|\bexit(?:ed)?(?: code)?\s*[=:]?\s*0\b",
    "count_noun": r"\b\d[\d,]*\s+(?:[A-Za-z][A-Za-z-]*\s+){0,2}?(?:files?|tests?|lines?|messages?|sessions?|entries|rows?|commits?|bugs?|findings?|hits?|matches|occurrences?|functions?|symbols?|tools?|items?|calls?|errors?|instances?|cases?|sites?|callers?|references?|turns?|bytes|chars|characters|tokens|docs?|artifacts?|trackers?|crates?|modules?|failures?|warnings?|assertions?|mutations?|packets?|samples?|strata|percent|%)\b",
    "absence": r"\b(?:no|none|never|nothing|zero|nowhere|nobody|neither)\b|\bnot found\b|\bno such\b|\bnot (?:present|exist|there|used|reached|called|set)\b|\b(?:doesn't|does not|don't|do not|didn't|did not|isn't|is not|aren't|are not|cannot|can't|won't|will not)\s+(?:exist|appear|contain|occur|match|find|show|carry|reach|fire|hold|have)\b|\bwithout any\b",
}
EDIT_TOOLS = {"edit_file", "edit_code", "create_file", "Write", "Edit", "MultiEdit", "NotebookEdit", "edit_markdown"}
DISPATCH_TOOLS = {"Agent", "Task"}
SHELL_TOOLS = {"run_command", "Bash"}
CONSEQ_CMD = r"\bgit\s+(commit|push|reset|rebase|checkout|stash|clean)\b|\brm\s+-|\bcargo\s+rb\b|\brb\.sh\b"
CATALOG_TOOLS = {"doc", "memory"}
CATALOG_WRITE_ACTIONS = {"create", "update", "move", "delete", "graft", "link", "append_entry", "update_entry",
                         "rekey_prefix", "event_create", "augment", "write", "remember", "forget"}
REASONS = ("claim_marker", "end_turn", "edit", "git_or_rm_or_release", "dispatch", "catalog_write", "handback")
```

**Rules:**
- A tool's name is compared after its last `__` (so `mcp__codescout__edit_file` → `edit_file`).
- A shell command is the tool input's `command`.
- `end_turn` requires `stop_reason == "end_turn"` on some entry of the message **and** non-empty text.
- **Top-level units:** every assistant API message in a kept session's top-level file with
  `isSidechain` falsy.
- **Hand-back units:** the last assistant API message **with text** in each subagent file of a kept
  session.
- **Never units:** messages whose model is `<synthetic>` or that carry `isApiErrorMessage`.
- A hand-back is always substantive with reason `handback`, plus any other reasons that also apply.
- `draw` sorts each stratum's candidates by `case_key`, removes `exclude`, then takes
  `random.Random(seed).sample(...)` of the requested size per stratum, **iterating strata in the
  order `("substantive", "routine")`**.

- [ ] **Step 1: Write the failing tests** in `tests/test_measure_sampler.py`:
  - `test_each_reason_has_a_fixture_only_it_admits`: for each of the 7 reasons, a message that
    carries that trigger and nothing else classifies as substantive with `reasons == (that_reason,)`,
    and removing the trigger makes it routine. Every fixture is **annotated on its line** with the
    trigger it carries.
  - `test_short_claim_is_substantive_and_long_narration_alone_is_not`: `"12 tests passed"` gives
    `("claim_marker",)`, and a 400-char text with no marker and no action is routine. **This test
    pins the spec's change from the length rule.**
  - `test_entries_sharing_a_message_id_form_one_unit` (Review Focus 1): a text entry and a
    `tool_use` entry with the same `message.id` give one Unit, whose reasons include the tool's reason.
  - `test_synthetic_and_api_error_messages_are_not_units` (Review Focus 2).
  - `test_handback_is_last_assistant_message_with_text` (Review Focus 4): a subagent file ending in a
    tool-only message yields the earlier text message, and a file with no text yields no unit.
  - `test_frame_holds_only_kept_copies` (Review Focus 5): a sid present in two profiles, where one
    copy is a prefix of the other, yields units from the keeper only, and a spec-excluded sid yields
    none. Pass an explicit `excluded_sids` through `frame`'s keyword argument
    `excluded_sids=join.SPEC_EXCLUDED_SIDS`.
  - `test_draw_is_reproducible_and_disjoint_from_exclude`: the same seed gives the same list; a
    different seed gives a different list on a 50-unit fixture; no drawn `case_key` is in `exclude`;
    per-stratum sizes are honoured; and asking for more than a stratum holds raises `ValueError`.
  - `test_frame_counts_by_stratum_and_kind`.
- [ ] **Step 2: Run them to verify they fail.**
  `~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_sampler.py -v`.
  Expected: they fail on the missing `sampler` module.
- [ ] **Step 3: Implement `sampler.py`** with the interfaces above.
- [ ] **Step 4: Run the tests to verify they pass,** then run the mutation evidence: one mutation per
  reason rule, one for the synthetic exclusion, and one for the hand-back selection.
- [ ] **Step 5: Commit** `feat(measure): labelled-sample frame, claim-and-action strata, seeded draw`,
  covering `scripts/measure/sampler.py`, `tests/measure_corpus_fixture.py` and
  `tests/test_measure_sampler.py`.

### Task 2: The packet rule (`packet.py`)

**Files:**
- Create: `scripts/measure/packet.py`
- Test: `tests/test_measure_packet.py`

**Interfaces:**
- Consumes: `sampler.Unit`, `transcripts.read_jsonl`, `transcripts.operator_messages`, and the fixture module.
- Produces:
  - `@dataclass Packet` with fields `case_id: str`, `text: str`, `sha256: str` (of `text` as UTF-8),
    `n_context: int`, `chars: int` and `token_hits: int`.
  - `build_packet(corpus_dir, unit, case_id: str) -> Packet`.
  - `case_id_for(unit, seed: int) -> str`, the first 10 hex chars of `sha256(f"{seed}|{unit.case_key}")`.
  - `token_hits(text) -> int`, using `re.compile(r"gh[pousr]_[A-Za-z0-9]{36}|github_pat_[A-Za-z0-9_]{50,}")`.
  - `exit_code(result_text) -> int | None`.
  - `class TokenFound(Exception)`, raised by `build_packet` when `token_hits > 0`.

**The rule** (spec § *Packet rule*). The packet is markdown with these sections, in this order:

1. **"Operator's last message":** the last entry before the unit that `operator_messages` returns,
   using its first 1,500 chars. If there is none, the text is `(none before this point)`. For a
   hand-back, the section is titled **"Dispatch prompt"** and holds the first user entry's text in
   the subagent file, first 1,500 chars.
2. **"Context":** the last 6 assistant API messages before the unit's first entry, oldest first,
   labelled `−6 … −1`. Each shows:
   - its text;
   - each tool call as `name(args)`, with the JSON arguments cut to 300 chars;
   - each matching `tool_result` (by `tool_use_id`, found in later user entries before the unit) as
     its **last** 1,500 chars. Prefix the result with `[exit N]` when `exit_code` parses from the
     **whole** result text, and with `[is_error]` when it is flagged.
3. **"The message":** the unit's text, then `ABOUT TO RUN:` and its tool calls in the same form.
   Their results are never included.

**Limits and blinding:**
- Over 20,000 chars, drop whole context messages oldest first. The unit itself is never dropped; if
  it alone exceeds 20,000, keep its last 20,000 chars behind the marker
  `[… the earlier part of this message is not shown]`.
- **No session id, message id, uuid or timestamp** appears in the text.
- `exit_code` recognises the JSON `"exit_code": N` and a line matching `^Exit code (-?\d+)$`
  (multiline), and returns `None` otherwise.

- [ ] **Step 1: Write the failing tests:**
  - `test_nothing_after_the_decision_appears`: plant the string `AFTER-DECISION-MARKER` in the unit's
    own tool result and in the next message; neither may appear in the packet text.
  - `test_a_prior_tool_result_appears_with_its_exit_code`: a preceding `run_command` result whose
    JSON has `"exit_code": 101` and a failing test line appears, prefixed `[exit 101]`. **This is the
    new rule's regression test for bug `61f699816f5ee2fd`,** where the old context carried no tool output.
  - `test_exit_code_outside_the_kept_tail_is_still_shown` (Review Focus 3): a 5,000-char result whose
    only exit code is on its first line.
  - `test_result_tail_kept_and_args_cut`: a result's last 1,500 chars are kept and its first char is
    not; the arguments are cut at 300.
  - `test_operator_message_skips_compaction_and_wrappers` (Review Focus 5).
  - `test_pending_action_includes_sibling_entry_tool_calls` (Review Focus 1).
  - `test_handback_uses_dispatch_prompt_and_subagent_context`.
  - `test_twenty_thousand_cap_drops_oldest_context_first`.
  - `test_no_ids_or_timestamps_in_text`: the fixture's sid, message ids, uuids and ISO timestamps are
    all absent.
  - `test_token_shaped_string_refuses`: a context result containing `ghp_` followed by 36
    alphanumerics raises `TokenFound`.
  - `test_same_inputs_same_sha256`.
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement `packet.py`.**
- [ ] **Step 4: Run the tests to verify they pass;** mutation evidence for the leakage cut-off,
  the exit-code parse, the tail cut, the cap order, and the token refusal.
- [ ] **Step 5: Commit** `feat(measure): labelled-sample packet rule with tool output and exit codes (bug 61f699816f5ee2fd, new rule)`.
  Leave the bug file's status open, because `judge.py` still uses R117.

### Task 3: The operator's labelling tool (`label.py`)

**Files:**
- Create: `scripts/measure/label.py`
- Test: `tests/test_measure_label.py`

**Interfaces:**
- **The label-set directory layout,** produced by Task 5 and consumed here:
  - `draw.json`: `{"set_id", "seed", "cases": [{"case_id", "sha256"}], "order": [case_id, ...]}`;
  - `packets/<case_id>.md`;
  - `labels.jsonl`, written append-only here;
  - `relabels.jsonl`, the same schema, for the re-label pass.
- Produces:
  - `validate(labels: list[str], delivery: str) -> None`, raising `ValueError`. Labels are a
    non-empty subset of `{"verify", "qualify", "correct"}`, or exactly `["none"]`, or exactly
    `["unresolved"]`. Delivery is `"silent"`, `"quiet"` or `"interrupt"`. Verify, qualify or correct
    requires quiet or interrupt; none or unresolved requires silent.
  - `label_one(case_id, sha256, packet_text, ask: Callable[[str], str], show: Callable[[str], None], clock: Callable[[], float]) -> dict`,
    returning the record `{"case_id", "packet_sha256", "labels", "delivery", "note", "recall",
    "seconds", "labelled_at"}`. Input letters map as: labels `v q c` (any combination) or `n` or
    `u`; delivery `s q i`; recall `y n`. Invalid input is asked again.
  - `summary(set_dir) -> dict`: counts only, namely cases, labelled, the label and delivery
    distributions, the unresolved count, the recall count and the median seconds. **No case ids with
    their labels, no notes.**
  - `verify(set_dir) -> dict`: `{"ok": n, "mismatch": [case_id, ...]}`, re-hashing each packet file
    against `draw.json` and each label's `packet_sha256`.
  - `relabel_ids(set_dir, seed: int, n: int = 10, min_days: int = 3, now=None) -> list[str]`.
  - A CLI: `label.py next|summary|verify <set_dir> [--relabel]`. `next` shows packets through
    `$PAGER` (default `less -R`), falling back to printing, in `draw.json`'s `order`, skipping those
    already labelled, and resumes after a quit.

- [ ] **Step 1: Write the failing tests:**
  - `test_validate_enforces_label_delivery_pairing`: every allowed and forbidden pair, including
    `["none", "verify"]` and an empty list.
  - `test_label_one_reasks_on_invalid_input_and_records_seconds`: scripted `ask` with an invalid
    answer first, and a fake clock.
  - `test_next_resumes_after_labelled_cases`: drive the CLI's loop function with a scripted `ask`.
  - `test_summary_contains_no_note_text_and_no_per_case_labels`: a note containing a sentinel string
    is absent from `json.dumps(summary(...))`, and no case id appears in it.
  - `test_verify_flags_an_edited_packet`.
  - `test_relabel_ids_only_picks_cases_at_least_three_days_old`.
- [ ] **Step 2: Run them to verify they fail.** **Step 3: Implement.**
  **Step 4: Run them to verify they pass,** with mutation evidence on the pairing rule, the summary's
  omission of notes, and the verify hash.
- [ ] **Step 5: Commit** `feat(measure): operator labelling tool -- blinded, resumable, hash-bound`.

### Task 4: Intervals and the decision (`estimate.py`)

**Files:**
- Create: `scripts/measure/estimate.py`
- Test: `tests/test_measure_estimate.py`

**Interfaces:**
- Produces:
  - `wilson(k: int, n: int, z: float = 1.959964) -> tuple[float, float]`.
  - `outcome(lo: float, hi: float, t: float = 0.10) -> str`, returning `"go"` when `lo >= t`,
    `"no-go"` when `hi < t`, else `"inconclusive"`.
  - `session_bootstrap(hits_by_session: dict[str, list[int]], seed: int, b: int = 10000) -> tuple[float, float]`:
    resample sessions with replacement and take the 2.5 and 97.5 percentiles of pooled hits/units.
  - `decide(k, n, unresolved, boot: tuple[float, float], t=0.10) -> dict`, returning
    `{"wilson", "boot", "wilson_outcome", "boot_outcome", "outcome", "guards"}`:
    - more than 25% of `n` unresolved gives inconclusive with guard `"unresolved_share"`;
    - the two outcomes disagreeing gives inconclusive with guard `"interval_disagreement"`.
  - `weighted_overall(rates: dict[str, float], counts: dict[str, int]) -> float`.
  - `cohen_kappa(a: list[str], b: list[str]) -> float`.
  - `estimate(labels: list[dict], key: dict[str, dict], frame_counts: dict, seed: int, relabels=None) -> dict`.
    Here `key` maps `case_id` to `{"stratum", "kind", "copy_id"}`. It returns the decision on the
    substantive stratum; the routine stratum's rate and Wilson interval; the overall weighted rate,
    with a stratified-bootstrap interval using the same `b` and seed; breakdowns by label, delivery,
    kind and recall (every figure with and without recall-flagged cases); median seconds; and kappa
    when `relabels` is given. A hit is `delivery in {"quiet", "interrupt"}`.

- [ ] **Step 1: Write the failing tests:**
  - `test_wilson_matches_hand_computed_values`: (0, 60) → upper 0.0602 ± 1e-4; (1, 60) → upper 0.0886; (11, 60) → lower 0.1056; (12, 60) → lower 0.1183. These were computed with the formula at z = 1.959964 when the plan was written.
  - `test_outcome_at_the_registered_boundaries`: 1/60 no-go, 2/60 inconclusive, 10/60 inconclusive,
    11/60 go, using Wilson and a bootstrap interval set to agree.
  - `test_unresolved_guard_trips_above_a_quarter`: 16 unresolved of 60 trips it; 15 does not.
  - `test_interval_disagreement_makes_it_inconclusive`.
  - `test_session_bootstrap_is_reproducible_and_widens_under_clustering`: all hits in one session of
    ten gives a wider interval than the same hits spread across sessions.
  - `test_weighted_overall_uses_frame_counts`: hand-computed on 2 strata.
  - `test_kappa_hand_computed`.
- [ ] **Step 2: Run them to verify they fail.** **Step 3: Implement.**
  **Step 4: Run them to verify they pass,** with mutation evidence on each comparison in `outcome`,
  on the guard's `>`, on the disagreement rule and on the hit definition.
- [ ] **Step 5: Commit** `feat(measure): labelled-sample estimator -- Wilson decision, session bootstrap, guards`.

### Task 5: `run.py` subcommands and an end-to-end fixture test

**Files:**
- Modify: `scripts/measure/run.py`: extend `_parser` and `main`, and update the module docstring
- Test: `tests/test_measure_run_sample.py`

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces these subcommands:
  - `frame --corpus C --out FRAME.json`: writes `{"corpus_id", "counts": frame_counts,
    "n_units"}` plus the sha256 of `sampler.py` and `packet.py`. Prints the counts. May write
    anywhere, since it holds no text.
  - `draw --corpus C --set DIR --seed S --substantive N --routine M [--exclude-set DIR2]`:
    - refuses a `DIR` inside the repo, or one that already exists (creates it with mode 700);
    - draws via `sampler.draw` (`exclude` holds the case keys in DIR2's `key.json`);
    - writes `DIR/key.json` (`{case_id: {"case_key", "stratum", "kind", "copy_id", "reasons"}}`)
      and `DIR/draw.json` with `order` shuffled by `random.Random(S + 1)`.
  - `render --corpus C --set DIR`: builds every packet into `DIR/packets/<case_id>.md` and fills
    `draw.json`'s sha256s. Any `TokenFound` aborts the command and removes nothing written so far.
    Prints counts only.
  - `estimate --set DIR --frame FRAME.json --seed S --out RESULT.json`.
  - `export --set DIR --out-dir D`: writes `D/<set_id>-draw.json` (case ids, sha256s, strata, kinds)
    and `D/<set_id>-labels.jsonl` (labels **without** `note`). It refuses when `D` is outside the repo.

- [ ] **Step 1: Write the failing tests** on a fixture corpus with roughly 30 units:
  - `test_frame_draw_render_estimate_export_end_to_end`: labels come from scripted records written
    straight to `labels.jsonl`.
  - `test_draw_refuses_existing_or_in_repo_dir`.
  - `test_render_aborts_on_token`.
  - `test_export_omits_notes`: a sentinel note string is absent from every exported file.
  - `test_main_draw_excludes_pilot_units`.
- [ ] **Step 2: Run them to verify they fail.** **Step 3: Implement.**
  **Step 4: Run them to verify they pass,** plus all `tests/test_measure_*.py` suites, each printed
  with its count.
- [ ] **Step 5: Commit** `feat(measure): run.py frame/draw/render/estimate/export for the labelled sample`.

### Task 6: Pilot (operator-gated)

- [ ] **Step 1:** `run.py frame --corpus ~/work/claude/measurement-corpora/2026-09-29-codescout --out ~/work/claude/measurement-corpora/2026-09-29-labelled-frame.json`.
  Record the counts.
- [ ] **Step 2: Choose the pilot seed** with `python -c "import secrets; print(secrets.randbelow(2**31))"`,
  then run `run.py draw … --set ~/work/claude/measurement-corpora/2026-09-29-labelled-pilot --seed <pilot seed> --substantive 4 --routine 1`,
  then `run.py render …`.
- [ ] **Step 3: Hand off to the operator.** They run
  `~/work/claude/prompt-engineering/.venv/bin/python scripts/measure/label.py next ~/work/claude/measurement-corpora/2026-09-29-labelled-pilot`
  **in their own terminal.** The controller reads only `label.py summary`. The operator reports any
  format problem in their own words.
- [ ] **Step 4: If the format changes,** change `packet.py` through Task 2's test-first loop in a new
  commit, and re-render the pilot. Pilot units are never used in an estimate.
- [ ] **Step 5: Commit** the pilot's counts only (seed, the summary JSON, and the packet-rule changes
  in one line each) to `docs/evals/data/2026-09-27-system1-base-rates/labelled-sample/pilot.md`.

### Task 7: Registration, Amendment 1 (before the main draw exists)

- [ ] **Step 1: Choose the main seed** the same way as the pilot's.
- [ ] **Step 2: Append "Amendment 1 — registration"** to the spec. It records the main seed; the
  sha256 of `scripts/measure/sampler.py` and `scripts/measure/packet.py` at HEAD; the frame counts
  from Task 6 Step 1 (re-run if either file changed since); the sizes 60/20; the pilot's set id as
  excluded; and the sentence "The decision rule in § *The question and the decision rule* is
  unchanged."
- [ ] **Step 3: Commit** `docs(spec): labelled sample Amendment 1 -- registration before the main draw`.
  **No main draw may exist before this commit.**

### Task 8: Main draw and render

- [ ] **Step 1: Draw.** `run.py draw --corpus … --set ~/work/claude/measurement-corpora/2026-09-29-labelled-main --seed <registered seed> --substantive 60 --routine 20 --exclude-set ~/work/claude/measurement-corpora/2026-09-29-labelled-pilot`,
  then `run.py render …`. Confirm that the sha256 of `sampler.py` and `packet.py` still equal the
  registered values **before** drawing, and stop for a ruling if either differs.
- [ ] **Step 2: Export and commit.** Run `run.py export … --out-dir docs/evals/data/2026-09-27-system1-base-rates/labelled-sample/`,
  so the draw manifest holds ids and hashes and the labels file is still empty. Commit it as
  `data(measure): labelled sample main draw -- 80 case ids and packet hashes`.
- [ ] **Step 3: Hand the set to the operator** for labelling in their own terminal. The controller
  checks progress with `label.py summary` only.

### Task 9: Estimate and readout (after the operator finishes labelling)

- [ ] **Step 1: Verify the labels.** `label.py verify` must report 0 mismatches.
- [ ] **Step 2: Estimate.** `run.py estimate … --seed <registered seed>` and `run.py export …`.
- [ ] **Step 3: Write the readout** at `docs/evals/2026-09-29-system1-labelled-sample-readout.md`. It
  gives the outcome under the registered rule, with both intervals and every guard; the routine
  stratum; the overall weighted rate; the breakdowns; the cost per label; and the disclosures copied
  from the spec. **Every number names corpus `2026-09-29-codescout` and its population.** Run the
  R146 private-text scan over every file before committing.
- [ ] **Step 4: The optional re-label**, when the operator wants it: `label.py next --relabel` at
  least 3 days later, then re-run `estimate` with the relabels and add the kappa to the readout.
- [ ] **Step 5: Commit** the readout and the exported labels.
