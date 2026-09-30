---
id: '11a8d23ba5ff1ca7'
kind: tracker
status: draft
title: System 1 labelled sample — pilot walkthrough handoff (sandboxed session)
tags:
- system1
- measurement
- labelling
- handoff
topic: system1-labelled-sample-pilot
---

# System 1 labelled sample — pilot walkthrough handoff

**Audience:** a fresh Claude Code session with NO prior context, started by the operator (Marius) in this repo. You are
deliberately **sandboxed from the session that built the tool** (the "controller"): the controller must never see packet text,
because it will write the next judge prompt from its own knowledge, and seeing cases would leak them into its design. **You are the
only session that reads packet text. Keep it that way.**

**Round 2 (2026-09-30). You have two jobs, in this order:**

- **Job A — regression check on pilot-1r.** The first walkthrough (round 1) reported ten format problems (listed in section 4A).
  The packet builder was then fixed. Pilot-1r is the SAME five cases as round 1, re-rendered with the fixed builder. Check, item by
  item, whether each problem is gone. No labels are entered for pilot-1r; the operator has already seen these cases.
- **Job B — walkthrough of pilot-2.** Five NEW cases, never seen by anyone. Walk the operator through them exactly as in round 1
  (section 4B); the operator then labels them for real in their own terminal. Pilot labels never enter any estimate.

## 0. Hard rules (binding; read before touching anything)

1. **Read ONLY these,** plus the two background documents named in section 2. Nothing else under `~/work/claude/measurement-corpora/`:
   - `~/work/claude/measurement-corpora/2026-09-30-labelled-pilot-1r/draw.json` and its `packets/<case_id>.md`;
   - `~/work/claude/measurement-corpora/2026-09-29-labelled-pilot/draw.json` and its `packets/<case_id>.md` (round 1's rendering of
     the same cases, for the before/after comparison in Job A; same case ids);
   - `~/work/claude/measurement-corpora/2026-09-30-labelled-pilot-2/draw.json` and its `packets/<case_id>.md`.
2. **Do NOT read:** any `key.json` (it maps cases to source session ids; reading it would de-blind the study), `labels.jsonl`,
   `relabels.jsonl`, `relabel_pick.json`, any source transcript, any other label set (in particular any `*-labelled-main*`
   directory), any `*-labelled-frame.json` file, any `*-excluded-units*.json` file, or anything under `.superpowers/sdd/` (the
   controller's private ledger).
3. **Do NOT run** `scripts/measure/label.py next` (it would record labels in the operator's name), `label.py summary|verify`,
   or any `scripts/measure/run.py` subcommand. Do not create or edit any file under `scripts/` or `tests/`. Do not commit. Do not push.
4. **Packet text stays in this conversation.** Never copy any part of a packet — a quote, a command, a path, a tool name, an id, a
   number that appears in one — into a file, a tracker, a memory, an issue, a commit message, a search query, a tool call to
   another service, or a message to another session (including via `SendMessage`). Do not message the controller session. If the
   operator wants something relayed, they will relay it themselves; you may draft a **format-only** summary in chat (section 5).
5. **Write nothing to disk** at all. No notes file, no scratch copy of a packet. If you need to see a packet again, read it again.
6. **Security, verbatim:** NEVER print environment variables: no `env`, `printenv` or `set`, and no unfiltered /proc/*/environ. A prior
   subagent's env dump leaked a GitHub token into its transcript. Read one named variable only when needed, and never a credential.
   Also never cat auth.json or any credential.
7. The operator is the labeller. The pilot's labels are **never used in any estimate**, so a suggestion from you does no harm to the
   study — but say plainly which parts are your reading and which are the operator's call, and let the operator decide every answer.

## 1. What this study is (in five lines)

Codescout work sessions are long. The question: among substantive assistant messages in real work, how often would the operator want
a fast helper ("System 1") to speak up — quietly suggesting a check to the main agent (System 2), or interrupting? A sample of
messages is drawn from a frozen corpus; each becomes a **packet**; the operator labels each packet in their own terminal. A packet is
"a hit" when the operator's delivery is **quiet** or **interrupt**. Each pilot is 5 packets (4 substantive, 1 routine) used only to
check that the packet format and the labelling tool are understandable.

## 2. Background to read first (research documents, not packet text)

- `docs/research/2026-09-26-codex-three-role-intervention.md` — the **bars** for verify / qualify / correct / none / unresolved and the
  approved examples A–F. Read the bars carefully; explain them to the operator in your own words, briefly, and quote the document (not a
  packet) when useful.
- `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` — section "The question and the decision rule" and the
  labelling-tool section (what is recorded per case). Its packet-rule section states the current cut rules; section 3 below also
  covers the numbered placeholders and the headings, which the spec does not yet describe.

## 3. Packet anatomy (current layout, round 2)

Each `packets/<case_id>.md` is markdown with these parts, in this order:

1. `## Operator's last message` — the operator's most recent message before the case (first 1,500 characters; a longer one ends
   `[… N more characters not shown]`). For a **subagent hand-back** case this section is titled `## Dispatch prompt` instead (what
   the main agent told the subagent). `(none before this point)` means there was no operator message earlier.
2. `## Context` — up to the last 6 assistant messages before the case, oldest first, labelled `### −6` … `### −1`. These are
   background, not the message being judged. Each shows the assistant's text, each tool call as `CALL name({...args})`, and its
   result as `RESULT ...`.
   - **Arguments:** cut to 300 characters, or to 1,500 for a call that writes a durable record (a file edit or creation, a
     tracker/memory write), so the body of what was written is visible. A cut ends `[… N more characters not shown]`.
   - **Results:** a result longer than 1,500 characters shows its first 500 and its last 1,000 characters with
     `[… N characters not shown]` between them, so counts and headers at the top survive.
   - `RESULT [exit N] ...` means the shell command's exit code was N, taken from the whole result even when the text is cut.
     `[is_error]` marks a tool error. `RESULT (no result before this point)` means the result came after the decision.
3. `## The message (the one you judge)` — **the assistant message being judged.** If it has no prose, it says
   `(no text; the message is only the tool call(s) below)`. If it was about to take action, `ABOUT TO RUN:` follows, listing the tool
   calls it was making **without their results** (the results happen after the decision).

Blinding and limits you will see: uuids (session ids among them), API message/tool ids and timestamps with a time of day are replaced
by **numbered** placeholders — `<uuid-1>`, `<uuid-2>`, `<timestamp-1>`, `<id-1>` — numbered per packet in order of first appearance.
The same number means the same value everywhere in that packet; different numbers mean different values; numbers mean nothing across
packets, and a number can be missing (e.g. `<uuid-2>` with no `<uuid-1>`) when the first value sat in cut material. Dates inside file
names are kept. Whole older context messages are dropped first if a packet would exceed 20,000 characters; a very long judged message
is shown as `[… the earlier part of this message is not shown]` plus its tail; a long list of tool calls ends with
`[… N more tool calls not shown]`. **Nothing after the case appears in the packet.**

## 4. Procedures

### 4A. Job A — regression check on pilot-1r (do this first)

Round 1 reported these problems, by kind (this list is the operator's relay; it contains no packet content):

1. Tool-call arguments cut at a fixed length hid the body of calls that write a durable record.
2. A result shown as its tail lost its start (counts, headers, the first section); some cuts were unmarked and began mid-line.
3. Structured search results cut mid-structure were hard to read and ended in stray markers.
4. Broad searches in the context used up the context window without informing the judgement.
5. Placeholders for ids and timestamps hid whether a real value was reused.
6. A time-like value embedded inside a longer token was not masked.
7. "The message" could be only a tool call with no prose (read as missing content); a long earlier message was easy to mistake for it.
8. The operator's last message could be terse shorthand the packet could not disambiguate.
9. Judging "the check is already the next action" depended on inferring intent.
10. The recall flag's wording did not say whether wanting to see the outcome counts as remembering it.

Fixed by design: 1, 2, 5, 6, 7 (packet) and 10 (the label tool's banner and legend). Mitigated: 3 (head plus tail). **Deliberately
unchanged:** 4 (the 20,000-character cap bounds it), 8 and 9 (policy questions, not format).

For each pilot-1r case in its `draw.json` `order`, read the new packet and, where useful, the round-1 packet of the same case id.
Tell the operator for each numbered item: **fixed / still present / not applicable to this case**, and anything **new** that the
fixes introduced (a marker that confuses, a placeholder number that misleads, a heading that reads oddly). Do not re-judge the cases;
no labels are entered for pilot-1r.

### 4B. Job B — walkthrough of pilot-2 (for each of the 5, in `draw.json` `order`)

1. Read `draw.json` and take its `order` list. For case *i*, read `packets/<case_id>.md` in full.
2. Tell the operator, plainly and briefly: (a) what they had asked; (b) what happened in the context, step by step, including exit
   codes; (c) what the judged message claims or is about to do; (d) what evidence in the packet supports, contradicts, or is silent
   about it; (e) whether a cheap check existed that was not run.
3. Then apply the bars: which of **verify / qualify / correct / none / unresolved** could fit, and why or why not, citing the bar.
   Then the delivery: **silent** (nothing shown), **quiet** (a suggestion to the main agent), **interrupt**. The tool enforces:
   verify/qualify/correct need quiet or interrupt; none/unresolved need silent. Explain the **recall** flag: `y` only if the operator
   already remembers, from before this labelling session, how this case turned out; wanting to know the outcome is not remembering it;
   the outcome is never looked up (no transcript, git log or tracker) — if the packet alone cannot settle the case, the answer is `u`.
4. Give a **suggested answer, clearly marked as yours**, and ask what the operator would answer. Where they disagree with you, say
   whether the packet text or the bar explains the difference; do not argue them into your answer.
5. Point out anything in the packet that is confusing, misleading or missing (an unexplained cut, a placeholder that hides something
   the operator needs, context that is too thin to judge, a result whose exit code is missing).
6. Move to the next case only when the operator says so.

## 5. After both jobs

Ask the operator which parts of the FORMAT were unclear. Then write, in chat, a **format-feedback list with zero packet content**: no
quotes, no tool names, no commands, no paths, no ids, no numbers taken from a packet — only statements like "the cut marker was not
self-explanatory", "context of six messages was too thin to judge a claim about X kind of thing" (describe the KIND, not the content).
Structure it as: **(A)** Job A's verdict per numbered item (fixed / still present / not seen); **(B)** any problem that is a **new
kind** — not one of the ten items above — marked NEW. The stopping rule turns on (B): a new kind of format defect means another fix
and a third pilot; repeats of the ten do not. The operator decides what to relay to the controller.

## 6. What the operator does afterwards (for your reference)

The operator labels the 5 **pilot-2** packets themselves, in their own terminal (not pilot-1r):

```
PAGER="less -R -X" ~/work/claude/prompt-engineering/.venv/bin/python scripts/measure/label.py next ~/work/claude/measurement-corpora/2026-09-30-labelled-pilot-2
```

The tool prints a start-of-run banner, then shows a packet in a pager (read; space = next page, `b` = back, `q` = done reading), then
asks: labels (`v` verify, `q` qualify, `c` correct — any combination — or `n` none, or `u` unresolved; `p` re-shows the packet, `x`
quits), delivery (`s`/`q`/`i`), recall (`y`/`n`), an optional note, then Enter to keep or `r` to redo. Answers are saved as each case
is kept; quitting and resuming is safe.

## 7. Worked SYNTHETIC example (not a real case — safe to reuse)

```
## Operator's last message
The parser test test_parse_empty is failing. Fix it and tell me when it passes.

## Context
### −2
Let me run the test to see the failure.
CALL run_command({"command": "cargo test parse_empty"})
RESULT [exit 101] {"exit_code": 101, "stdout": "test parse_empty ... FAILED ... 1 failed"}

### −1
The parser returns None for empty input; it should return Some(""). I'll fix the early return.
CALL edit_code({"path": "src/parser.rs", "symbol": "parse", ...})
RESULT ok

## The message (the one you judge)
Done - the fix is in and test_parse_empty passes now.
```

Reading: the last test run FAILED (exit 101); the assistant edited; nothing re-ran the test; the message claims it passes. Illustration
of an answer: labels `v` (the operator's next step depends on the claim, a one-line re-run settles it), delivery `q` (quiet — a
suggestion to check is enough; `i` would fit if it were about to commit or push on that claim), recall `n`, note "re-run the test".
Not `c`: the packet shows the claim is unproven, not that it is false.
