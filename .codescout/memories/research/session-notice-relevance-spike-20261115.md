# Spike: can Haiku decide which project notice matters to which session? (2026-11-15)

Context: brainstorm of a cross-session channel. A session that changes something project-wide writes a
notice on the hot path. A hook nudges other sessions: at the next user prompt (`UserPromptSubmit`) and
after tool calls (`PostToolUse`, rate-limited). Operator chose these two triggers. Haiku filters each
notice per session, from the session's transcript tail (`transcript_path` in the hook input).

## Set-up (throwaway, job tmp `nudge-spike/`, not kept)

- 9 real sessions: 8 in codescout / claude-plugins, 1 control in another repo. Tail = last 6000 chars of
  user + assistant text, no tool output, sidechains skipped.
- 6 real notices from 2026-11-15 (integration move, /mcp schema cache, commit guard, sab image cleanup,
  hot-view release, no-clobber publish). The author session never gets its own notice: 50 pairs.
- Labels yes / no / maybe, written by the lifecycle session BEFORE any Haiku output. 7 yes, 29 no, 14 maybe.
- Call: `claude -p --model claude-haiku-5-5 --tools "" --strict-mcp-config --setting-sources ""
  --no-session-persistence --output-format json --system-prompt <filter>`; one call per session with all
  its notices (the hook shape). `--bare` is not usable: it needs ANTHROPIC_API_KEY.

## Results

| Prompt | yes hit / miss | no: noise / quiet | maybe yes/no | cost, 9 calls | secs per call |
|---|---|---|---|---|---|
| v1 "when unsure, mark relevant" | 6 / 1 | 21 / 8 | 13 / 1 | $0.0097 | 3.1–7.5 |
| v2 quote rule, run 1 | 7 / 0 | 3 / 26 | 2 / 12 | $0.0143 | 4.6–13.9 |
| v2 quote rule, run 2 | 7 / 0 | 4 / 25 | 2 / 12 | $0.0083 | 5.7–11.7 |

- v1 failed on generic overlap ("the session uses codescout tools", "commits on the shared checkout") and
  on invented facts (S1 "runs setup-ab": false).
- v2 rule: relevant only if the transcript names the specific file, branch, commit, plan, command or
  pending decision; Haiku must copy at most 15 words verbatim; no quote = not relevant. The runner
  checks each quote against the tail by machine: 0 of the relevant verdicts carried a quote not in the
  tail, in both runs.
- Run 1 vs run 2: 3 of 50 verdicts flipped.
- v2 noise, read by hand: one is arguably a label error (S3 held the stale fact "integration at
  308f9f5df", and the notice corrects it); two match a commit hash or `experiments` only; one is the
  other-repo control, which a per-project notice store would never reach.

## Limits (do not over-read)

- One rater, who also wrote the notices. 7 positive labels only: 7/7 recall is weak evidence.
- Notices were specific. Vague real notices may behave worse. Tails were text only.
- Two runs, one model. No comparison with Sonnet. No test of a long notice backlog.
- Latency 3–14 s per call. A synchronous `UserPromptSubmit` hook would delay every prompt: the filter
  must run in the background and inject at the next trigger, or cache verdicts per (session, notice).

## Design readings

- The quote is a free anti-hallucination check: drop a verdict whose quote is not in the transcript.
- The quote is also the nudge text: "notice N matters because you said: <quote>".
- Precision comes from the quote rule, not from "when unsure, say yes".
