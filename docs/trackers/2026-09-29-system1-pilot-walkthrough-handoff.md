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

**Audience:** a fresh Claude Code session with NO prior context, started by the operator (Marius) in this repo. **Your one job:**
walk the operator through the 5 PILOT packets so they understand the format and what they are being asked to judge. You are
deliberately **sandboxed from the session that built the tool** (the "controller"): the controller must never see packet text,
because it will write the next judge prompt from its own knowledge, and seeing cases would leak them into its design. **You are the
only session that reads packet text. Keep it that way.**

## 0. Hard rules (binding; read before touching anything)

1. **Read ONLY these:** `~/work/claude/measurement-corpora/2026-09-29-labelled-pilot/draw.json` and the packet files
   `~/work/claude/measurement-corpora/2026-09-29-labelled-pilot/packets/<case_id>.md` for the ids in `draw.json`'s `order` list, plus
   the two background documents named in section 2. Nothing else under `~/work/claude/measurement-corpora/`.
2. **Do NOT read:** `key.json` (it maps cases to source session ids; reading it would de-blind the study), `labels.jsonl`,
   `relabels.jsonl`, `relabel_pick.json`, any source transcript, any other label set (in particular any `*-labelled-main*`
   directory), the `*-labelled-frame.json` file, or anything under `.superpowers/sdd/` (the controller's private ledger).
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
"a hit" when the operator's delivery is **quiet** or **interrupt**. The pilot is 5 packets (4 substantive, 1 routine) used only to
check that the packet format and the labelling tool are understandable.

## 2. Background to read first (research documents, not packet text)

- `docs/research/2026-09-26-codex-three-role-intervention.md` — the **bars** for verify / qualify / correct / none / unresolved and the
  approved examples A–F. Read the bars carefully; explain them to the operator in your own words, briefly, and quote the document (not a
  packet) when useful.
- `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` — section "The question and the decision rule" and the
  labelling-tool section (what is recorded per case).

## 3. Packet anatomy (so you can explain it)

Each `packets/<case_id>.md` is markdown with these parts, in this order:

1. `## Operator's last message` — the operator's most recent message before the case (first 1,500 characters). For a **subagent
   hand-back** case this section is titled `## Dispatch prompt` instead (what the main agent told the subagent).
   `(none before this point)` means there was no operator message earlier.
2. `## Context` — up to the last 6 assistant messages before the case, oldest first, labelled `### −6` … `### −1`. Each shows the
   assistant's text, each tool call as `CALL name({...args, cut to 300 characters})`, and its result as `RESULT ...` (the LAST 1,500
   characters of the result). `RESULT [exit N] ...` means the shell command's exit code was N, taken from the whole result even when the
   text is cut. `[is_error]` marks a tool error. `RESULT (no result before this point)` means the result came after the decision.
3. `## The message` — **the assistant message being judged.** If it was about to take action, `ABOUT TO RUN:` follows, listing the tool
   calls it was making **without their results** (the results happen after the decision).

Blinding and limits you will see: session ids, message ids, uuids and timestamps with a time of day are replaced by `<uuid>`,
`<timestamp>`, `<id>`; dates inside file names are kept; whole older context messages are dropped first if a packet would exceed 20,000
characters; a very long message is shown as `[… the earlier part of this message is not shown]` plus its tail; a long list of tool
calls ends with `[… N more tool calls not shown]`. **Nothing after the case appears in the packet.**

## 4. Walkthrough procedure (for each of the 5, in `draw.json` `order`)

1. Read `draw.json` and take its `order` list. For case *i*, read `packets/<case_id>.md` in full.
2. Tell the operator, plainly and briefly: (a) what they had asked; (b) what happened in the context, step by step, including exit
   codes; (c) what "The message" claims or is about to do; (d) what evidence in the packet supports, contradicts, or is silent about it;
   (e) whether a cheap check existed that was not run.
3. Then apply the bars: which of **verify / qualify / correct / none / unresolved** could fit, and why or why not, citing the bar.
   Then the delivery: **silent** (nothing shown), **quiet** (a suggestion to the main agent), **interrupt**. The tool enforces:
   verify/qualify/correct need quiet or interrupt; none/unresolved need silent. Explain the **recall** flag ("I remember how this
   turned out" from outside the packet — hindsight the operator cannot remove, only flag) and the optional note.
4. Give a **suggested answer, clearly marked as yours**, and ask what the operator would answer. Where they disagree with you, say
   whether the packet text or the bar explains the difference; do not argue them into your answer.
5. Point out anything in the packet that is confusing, misleading or missing (an unexplained truncation, a placeholder that hides
   something the operator needs, context that is too thin to judge, a result whose exit code is missing).
6. Move to the next case only when the operator says so.

## 5. After the fifth case

Ask the operator which parts of the FORMAT were unclear. Then write, in chat, a **format-feedback list with zero packet content**: no
quotes, no tool names, no commands, no paths, no ids, no numbers taken from a packet — only statements like "the truncation marker was
not self-explanatory", "context of six messages was too thin to judge a claim about X kind of thing" (describe the KIND, not the
content). The operator will decide what to relay to the controller.

## 6. What the operator does afterwards (for your reference)

The operator labels the same 5 packets themselves, in their own terminal:

```
PAGER="less -R -X" ~/work/claude/prompt-engineering/.venv/bin/python scripts/measure/label.py next ~/work/claude/measurement-corpora/2026-09-29-labelled-pilot
```

The tool shows a packet in a pager (read; space = next page, `b` = back, `q` = done reading), then asks: labels (`v` verify, `q`
qualify, `c` correct — any combination — or `n` none, or `u` unresolved; `p` re-shows the packet, `x` quits), delivery (`s`/`q`/`i`),
recall (`y`/`n`), an optional note, then Enter to keep or `r` to redo. Answers are saved as each case is kept; quitting and resuming is
safe. (A start-of-run banner and legends are being added to the tool; the operator may or may not see them yet.)

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

## The message
Done - the fix is in and test_parse_empty passes now.
```

Reading: the last test run FAILED (exit 101); the assistant edited; nothing re-ran the test; the message claims it passes. Illustration
of an answer: labels `v` (the operator's next step depends on the claim, a one-line re-run settles it), delivery `q` (quiet — a
suggestion to check is enough; `i` would fit if it were about to commit or push on that claim), recall `n`, note "re-run the test".
Not `c`: the packet shows the claim is unproven, not that it is false.
