# Could consecutive tool calls run in parallel? Mining 151,565 calls (2026-11-09)

Question: when the agent issues tool calls one after another, how many could have been issued in one message, and how much time would that save?

Short answer: about 37% of consecutive round pairs could be one round, which is about 3–4% of working time. That is an upper bound with a wide margin. The first automatic estimate (80%, later 67.6%) was about twice too high.

## Scope

- Transcripts: 1,243 files of the codescout project in the four profiles (`~/.claude`, `-sdd`, `-kat`, `-stef`), modified since 2026-10-01. 96,850 calls are in subagent files, 54,701 in main sessions.
- Exact call timing: `.codescout/usage.db` (`started_at`, `latency_ms`, joined on `tool_use_id`). 98,106 of 151,565 calls joined.
- A **round** is the set of calls of one assistant message. 28,267 of 115,861 rounds have two or more calls.
- Scripts, judge inputs and verdicts: `docs/research/data/2026-11-09-parallel-tool-calls/`. Run `extract.py out` first. The other scripts read `out/all.jsonl` from the current directory.

## Finding 1 — inside one message, only `readOnlyHint` tools overlap

Pairs of consecutive calls in one message whose first call lasted at least 300 ms (shorter calls cannot show overlap):

| Pair | Overlapped / serial |
|---|---|
| `symbols` → `symbols` | 137 / 12 |
| `grep` → `grep` | 92 / 19 |
| `symbols` → `grep` | 64 / 9 |
| `references` → `references` | 47 / 1 |
| `run_command` → `run_command` | 3 / 906 |
| `edit_code` → `edit_code` | 0 / 93 |
| `edit_file` → `edit_file` | 0 / 49 |
| `doc` → `doc` | 0 / 25 |

- `grep`, `read_file`, `symbols`, `references`, `tree` and `get_guide` advertise `readOnlyHint` (`src/tools/annot`, `read_only_closed`). `run_command`, `edit_*`, `doc`, `memory` and `workspace` do not.
- A live test confirmed it: three `grep` calls in one message started 0.3 s apart and all ended at the same moment.
- The first version of the metric counted calls shorter than 300 ms and reported `read_file` pairs as serial. That was wrong: a 1 ms call cannot overlap anything.
- The harness is the likely cause of the serial runs, because the server takes no lock for reads. This was not tested by changing the annotation.
- Not measured: how many `run_command` calls are read-only (`git status`, `ls`, `sed -n`). Claude Code classifies built-in Bash per command, but a static MCP annotation cannot.

## Finding 2 — how many rounds could have been one round

Method: for each pair of adjacent rounds with no user prompt between them (104,553 pairs; 6,231 pairs with agents, `SendMessage` or questions to the user were left out), test whether the second round depends on the first:

- **data**: a token of the first result that appeared nowhere earlier in the file is in the second input;
- **control**: the first result is an error or refusal, a verification command (`cargo`, `gate.sh`, …), or a `Skill` or `ToolSearch` load.

Effect order needs no test: inside one message the harness runs calls in order, and only adjacent read-only calls overlap. So "edit, then build" is safe in one message.

| Stage | Pairs independent |
|---|---|
| Heuristic v1 (data + error + verify) | 79.8% |
| Heuristic v2 (adds `"ok": false`, refusals, empty results, outline reads, 3-digit numbers) | 67.6% |
| After blind judges, on 90 sampled pairs marked independent | 49% (95% CI 39–59%) |
| Judges on 30 sampled pairs marked dependent | 13% were collapsible anyway (CI 5–30%) |
| Calibrated estimate over all pairs | **37%** |

The hand review of 24 pairs before the judges gave about 60% precision for v1 and showed three misses: a line range read from an outline, a retry after an error, a refinement after an empty result. Rules for them are in v2.

Four Opus judges, blind to the heuristic's label, answered: could the second round have been written without the first result? 16 of 120 verdicts were UNSURE and counted as "needs result".

Share confirmed by the judges, among pairs the heuristic marked independent:

| First → second | Confirmed |
|---|---|
| no-hint tool → no-hint tool (`edit` → `run_command`, `edit` → `edit`) | 28 / 40 (70%) |
| no-hint tool → read-only | 6 / 10 (60%) |
| read-only → read-only | 7 / 25 (28%) |
| read-only → no-hint tool | 3 / 15 (20%) |

Read-only chains rarely collapse because the agent searches and then reads what the search found. Write chains collapse more often because the agent had already planned them.

## Finding 3 — time

- Model latency between a result and the next call (capped at 120 s): median 4.4 s over all pairs, 3.9 s over the judged-collapsible pairs.
- 37% of 104,553 pairs is about 39,000 rounds. At 3.9 s each: about 42 h, about 3–4% of 1,144 h of segment wall time.
- This is an upper bound: the extra generation time of the merged call's arguments is not subtracted.
- The 1,144 h adds the main session and its subagents, which run in parallel. The share is approximate.

## What others do (retrieved 2026-11-09)

- Anthropic: parallel tool use is on by default. The API prescribes no execution order. Prompting raises the rate, and all `tool_result` blocks go in one user message. <https://platform.claude.com/docs/en/agents-and-tools/tool-use/parallel-tool-use>
- Claude Code: `partitionToolCalls` builds concurrent-safe batches and single serial calls; limit 10 (`CLAUDE_CODE_MAX_TOOL_USE_CONCURRENCY`). <https://claude-code-from-source.com/ch07-concurrency/>
- Codex CLI ≥ 0.134 runs MCP tools concurrently only when they advertise `readOnlyHint`. <https://codex.danielvaughan.com/2026/05/27/codex-cli-mcp-readonlyhint-concurrent-tool-execution-annotations-parallelism/>
- Programmatic tool calling runs many calls without sampling the model between them. <https://platform.claude.com/docs/en/agents-and-tools/tool-use/programmatic-tool-calling>
- LLMCompiler plans calls as a dependency graph (<https://arxiv.org/abs/2312.04511>). PASTE speculates on recurring call patterns (<https://arxiv.org/html/2603.18897v1>).

The reports were read from search excerpts. The Claude Code chapter and the Codex page are third-party write-ups, not vendor documentation.

## Limits

- One project, one month, one operator.
- 120 judged pairs. The dependent sample covers only the `failsig` and `data` reasons; `verify`, `error` and `loader` pairs are not calibrated.
- The judges are models. The collapse verdict is an informed opinion, not a replay: nobody ran the agent without the earlier result.
- Pairs only. A chain of three independent rounds saves two round trips; the pair sum counts each pair once, which is close but not equal.

## Open

- Measure how many `run_command` calls are read-only. If many are, a separate read-only tool with `readOnlyHint` would let them overlap.
- Test a prompt that groups write chains in one message. A refused `edit` does not stop the next call in the same message, so the risk is larger than for reads.
- Replay a sample of pairs with the second call forced into the first message, to replace the judges' opinion with an outcome.
