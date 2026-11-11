# Miner brief — Opus review reports

You are a miner. Read each review file in your shard and extract what the Opus
reviewer found wrong. Do not fix anything. Do not edit any file in the repo.
Read-only: use the Read tool on the files named below, nothing else.

## Context

A superpowers `subagent-driven-development` run has an implementer subagent
write code per task. An Opus reviewer then checks it (spec compliance + code
quality) and finds many defects. We want to learn which defects repeat, so we
can build a correction mechanism: a prompt rule, a checklist, a hook, a test
pattern, or a script that catches the defect BEFORE the review.

## Per review file, extract

- `agent_id`, `description` (from the file header).
- `verdict`: approved | approved_with_issues | rejected | unclear.
- `findings`: one object per distinct defect the reviewer reports:
  - `title`: one line, your words.
  - `severity`: as the reviewer states it (critical/important/minor); `unstated` if none.
  - `category`: exactly ONE of
    `spec-gap` (spec requirement missing/wrong), `test-weak` (test does not prove the claim,
    vacuous, mock-only, wrong oracle), `edge-case` (unhandled input/state), `wrong-behavior`
    (logic bug), `scope-creep`, `dead-or-unwired` (code never called, not wired in),
    `doc-drift` (comment/doc contradicts code), `error-handling`, `concurrency-or-ordering`,
    `naming-or-structure`, `report-inaccuracy` (the implementer's own report or pasted
    evidence is false, reconstructed, or missing, e.g. fabricated test output),
    `other` (use only when no other fits).
  - `evidence`: a quote of at most 25 words from the report, copied exactly.
  - `mechanical`: `yes` if a script, lint, grep, or test pattern could catch it
    without an LLM; `no` otherwise. Add `mechanical_how` (one line) when `yes`.
  - `implementer_preventable`: `yes` if a line in the implementer's prompt could
    have prevented it; `no` otherwise.
- `reviewer_style_notes`: 0-2 lines on HOW the reviewer found defects
  (e.g. ran the code, mutated a test, grepped callers). Only if the report says it.

## Rules

- Count only defects the reviewer actually reports. Do not invent findings.
- If a report is a re-review, list only the findings that are still open or new,
  and mark each with `"rereview": true`.
- If a field is unknowable from the text, write `"unknown"`. Do not guess.
- Quote exactly. Never paraphrase inside `evidence`.

## Output

Your task prompt names an OUT file path. Write the JSON object there with the Write
tool (valid JSON, nothing else in the file). Your final message is only the word DONE
and the number of findings. Never write any other file. The JSON shape:

```
{"shard": "<shard name>", "reviews": [ {agent_id, description, verdict, findings:[...], reviewer_style_notes:[...]} ],
 "unreadable": ["<agent_id>", ...]}
```
