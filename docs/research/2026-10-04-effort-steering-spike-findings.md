# Effort steering: spike findings S1 to S3

**Date:** 2026-10-04
**Spec:** `docs/superpowers/specs/2026-10-04-effort-steering-design.md`
**Plan:** `docs/superpowers/plans/2026-10-04-effort-steering-core-and-spikes.md` (Task 5)
**Code:** the `claude-plugins` repo, branch `feat/effort-steering-core`, in the worktree `claude-plugins-effort-steering`. Not merged, not pushed. Commits: features `claude-plugins:2e7757d` and `claude-plugins:c0e3cd6`; S2 `claude-plugins:13273ea`; S3 `claude-plugins:1b32e2e`, `claude-plugins:e0a355c` and `claude-plugins:e578c9f`; S1 `claude-plugins:c243591`.

## Summary

- S1 and S2 answered their questions. The hook input and the transcripts give the adapter everything it needs for logging and for measuring thinking tokens.
- S3 gives a thin basis for rules. One prompt feature separates thinking depth (`isQuestion`, lower). Prompt length does not. Two features have no data. The effort level matters more than any prompt feature.
- About 25% of the thinking tokens in these sessions follow records that no human prompt caused. A `UserPromptSubmit` hook cannot steer those, unless it also fires for them. That is unverified.
- The plan asks for a gate decision here. The options and my recommendation are at the end.

## S1: what a `UserPromptSubmit` hook receives

The controller ran one scratch call: `claude -p "reply with the word ok" --model sonnet --no-session-persistence --setting-sources local --settings <scratch file>`, from a temp directory. The only hook was `s1-dump-hook.mjs`. It recorded key names and value types only. The hook fired on the first attempt.

Keys received, all of type string: `session_id`, `transcript_path`, `cwd`, `prompt_id`, `permission_mode`, `hook_event_name`, `prompt`.

Consequences for the design:
- `prompt` and `transcript_path` exist. The adapter can read the prompt and the transcript tail.
- `prompt_id` is a per-prompt id. It can serve as the `turn_id` in the decision log.
- `permission_mode` is in the input. It may give the plan-mode signal directly. Its possible values were not checked.
- The input has no `effort` key. The effort level is not available to this hook from its input.
- The existing `codescout-companion` hook `constitution-brief.mjs` also reads `session_id` and `cwd`, so the two sources agree.

Limits: one run, one prompt, and the user-level plugin hooks were switched off by `--setting-sources local`. The run says nothing about how several hooks merge their `additionalContext`.

## S2: transcript usage

Method: `s2-usage-report.mjs` over the top-level `*.jsonl` files of the codescout project directory in the `.claude-sdd` profile.

- **Dedupe key:** `message.id`. It is on all 86,940 usage records in that run, collapses them to 36,970 groups, and `thinking_tokens` is identical inside every group. `uuid` and `parentUuid` are unique per line. `requestId` is on only 6,284 records. `sessionId` is constant per session. A later run counted 86,955 usage records because the transcripts are live.
- **Effort level is recorded.** Every assistant record has a top-level string field `effort` (values seen: `xhigh`, `high`, `medium`, `low`). It is on 86,891 of 86,940 usage records. It changed within a session in 26 of 59 sessions. The controller confirmed `"effort":"xhigh"` in a raw transcript.
- A second top-level field, `perTurnEffort`, exists. It was `null` in the early sample the controller read, and non-null on 21,275 of 86,955 usage records in a later run. Its meaning was not investigated.
- Transcript usage lines also carry `cache_creation_input_tokens` and `cache_read_input_tokens`. Cache misses can therefore be measured from transcripts.

## S3: offline replay

Method: `s3-replay.mjs` over the top-level `*.jsonl` files of the codescout project directory in all three profiles (`.claude-sdd` 62 files, `.claude` 53, `.claude-kat` 46). A turn starts at a real human prompt. Its thinking tokens are the sum over deduped assistant messages (key `message.id`, one set shared across the run) until the next prompt. Replies that follow a peer message, a task notification or a slash-command record are dropped, not attributed to the previous prompt. A command echo that arrives before any reply is the typed prompt's own expansion and stays in its turn. Turns with no assistant message are excluded from the tables.

Data: 3,556 turns, 767 without any assistant message, 2,789 measured, 25,497,592 thinking tokens in the measured turns. The data is a live snapshot of 2026-10-04.

### All measured turns (thinking tokens per turn)

| Bucket | n | Median | p90 |
|---|---|---|---|
| all | 2,789 | 5,444 | 21,435 |
| chars ≤ 40 | 2,150 | 5,490.5 | 21,457 |
| chars 41–400 | 582 | 5,411.5 | 21,428 |
| chars > 400 | 57 | 4,360 | 17,001 |
| isQuestion | 380 | 3,441 | 16,442 |
| hasStackTrace | 0 | no data | no data |
| startsWithSlash | 10 | 0 | 2,069 |
| explicitDeep | 0 | no data | no data |

The tables print `0` for an empty bucket. Read `n` first.

### By effort level

| Effort | n (all) | Median | p90 | isQuestion n | isQuestion median |
|---|---|---|---|---|---|
| xhigh | 2,190 | 5,942.5 | 21,791 | 321 | 3,764 |
| high | 412 | 3,848.5 | 19,076 | 44 | 1,057.5 |
| medium | 169 | 2,328 | 14,451 | 14 | 710 |
| low | 1 | 83 | 83 | 0 | no data |
| unknown | 17 | 0 | 0 | 1 | 0 |

### What the numbers show

- Prompt length does not separate thinking depth. The medians of the first two length buckets are close (5,490.5 and 5,411.5). The long-prompt bucket is lower, but n is 57.
- `isQuestion` is lower at every effort level: 3,764 against 5,942.5 at `xhigh`, 1,057.5 against 3,848.5 at `high`, and 710 against 2,328 at `medium`. The model already thinks less on questions.
- The effort level separates more than any prompt feature. The all-turn median falls from 5,942.5 (`xhigh`) to 3,848.5 (`high`) to 2,328 (`medium`). This is an association. It is not a causal estimate: the effort level may track task difficulty, and the replay cannot tell.
- The spread inside every bucket is large. The p90 is about four times the median in the pooled bucket. Prompt features explain little of it.
- `hasStackTrace` and `explicitDeep` have no data in three profiles. The sessions never contained a measured turn with either. S3 cannot support any rule on them.
- `startsWithSlash` has n=10 and is not readable. Of 244 typed slash prompts, 202 are local commands (for example `/clear`) with no model reply, and 32 have replies only after a peer or task-notification record. Eight of the 10 measured turns have no `effort` field and zero thinking.

### Replies the replay dropped

| Trigger | Messages | Thinking tokens | Turns affected |
|---|---|---|---|
| peer | 11,757 | 5,135,385 | 556 |
| task-notification | 8,342 | 2,908,348 | 394 |
| command | 1,080 | 595,168 | 57 |

That is 8,638,901 thinking tokens, about 25% of the 34.1 million that a plain "until the next prompt" rule would have attributed to human prompts. The first version of the replay counted them in the preceding human turn. An independent Opus review found this, and the fix is in `claude-plugins:e0a355c`.

### Limits of S3

- The replay tests prompt features only. It does not test the session signals the spec names: a failure in the previous turn, a retry count and plan mode. Those are the likeliest levers for escalation, and nothing here measures them.
- Records that do not terminate a turn (local command output, caveats, meta context without an origin, interrupt markers) were not checked for contamination.
- Work after a mid-turn trigger inside a long human task is dropped. The late part of long tasks is not counted in its turn.
- Buckets overlap, and the features are weak labels. A bucket median is not the effect of a prompt.
- Adaptive thinking is on. The model decides its own depth, so `thinking_tokens` is an outcome of the model.
- The 25% of thinking tokens after peer messages, task notifications and commands lie outside what a prompt hook can steer. Whether `UserPromptSubmit` fires for those triggers was not checked.
- Minor review findings for each task are in the SDD ledger and not repeated here. The two that matter for reading S3: empty buckets print median and p90 as 0, and `<synthetic>` replies count as zero-thinking turns (all 8 turns with `effort` unknown are of this kind).

## What this means for the plan

- The measurement path does not depend on the rules. The log, the holdout arm and the CLI (Tasks 7 to 10) are needed whatever the rules turn out to be. Spike S4 (does steering text move `thinking_tokens`?) is the experiment that can show value, and it needs that path.
- Task 11 (first rules) has a weak basis: one candidate rule for `shallow` (`isQuestion`), where the model already thinks less, and nothing for `deep`.
- The strongest escalation signals (previous-turn failure, retries) are untested.

## Decision requested (the Task 5 gate)

Choose one:

1. **Proceed as planned.** Build Tasks 6 to 11, with a first rules file drawn from this thin data.
2. **Proceed with the engine, defer the rules (recommended).** Build Tasks 6 to 10. Ship `rules.json` empty. Hold Task 11 until spike S4 or a session-signal replay supplies a basis. The second plan then covers the adapter and S4.
3. **Extend S3 first.** Replay the session signals (previous-turn failure, retries, plan mode through `permission_mode`) before building anything. This is read-only and cheap.
4. **Stop.** The signal is too weak to justify the build.
