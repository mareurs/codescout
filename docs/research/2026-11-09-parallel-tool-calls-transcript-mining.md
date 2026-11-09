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
- Measured in Finding 4: 34% of `run_command` calls are read-only by their text, and overlapping the in-message pairs would hide about 2 minutes of execution in a month. Claude Code classifies built-in Bash per command; a static MCP annotation cannot.

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

## Finding 4 — `run_command`: how many calls are read-only, and what overlapping them is worth

`run_command` (63,087 calls with `Bash`) is 42% of all calls and never overlaps, so a read-only variant with `readOnlyHint` looked like the obvious lever. Script: `run_command_ro.py`.

A call is read-only when every segment of its `&&`, `;` or `|` chain is a known read-only command (`ls`, `cat`, `grep`, `rg`, `git status|log|diff|show`, `sed -n`, `sqlite3` without writes, …) and it has no redirection, `tee` or `sed -i`. Builds and tests are `VERIFY`. Anything else, `python3` scripts included, is `MUT`, so the read-only share is a floor.

| Class | Calls | Share |
|---|---|---|
| read-only | 21,518 | 34.1% |
| other (writes, scripts) | 31,939 | 50.6% |
| build or test | 9,630 | 15.3% |

- Timed read-only calls (15,732): median 0.06 s, p90 1.15 s. Their mean is 21 s because a few are waits (`sleep`, `tail --pid`).
- 5,468 pairs of `run_command` calls sit inside one message; 2,091 of them are both read-only. Overlapping those 1,659 that have timing would hide the shorter call of each pair: 0.03 h, about 2 minutes, in a month.
- The agent declares `effect` on `run_command`: `read` on 23,282 calls. My text rule disagrees on 11,355 of them (48.8%), but the rule is the conservative side (it counts every `python3` script as not read-only). The server's own observation says the declaration is honest: `usage.db` `observed_changed` records whether the git-visible tree changed while a declared-read command ran (`effect_check.py`). Of 7,269 declared-read calls with an observation and a parsable command, 117 (1.6%) saw a change; 26 of those 117 are read-only by their text (the change is a peer's, on a shared checkout), 91 are scripts or test runs (`python3 -m pytest`, `strace`, `codescout doctor`). 7,152 left the tree unchanged, and my rule calls 48% of those not read-only. Limits: 2,836 declared-read rows with an observation have no parsable `command` and are not in these counts, and "unchanged" does not rule out a write to a gitignored path or outside the repo.

So a separate read-only tool would buy almost no execution overlap, because these commands finish in tens of milliseconds. The saving that is left is the model turn between two calls, and that comes from putting both calls in one message, which a serial harness still gives (Finding 3).

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

- Replay a sample of pairs with the second call forced into the first message, to replace the judges' opinion with an outcome.

## Not pursued

- A prompt A/B that tells the model to group write chains in one message. Decided against on 2026-11-09: it pays for model runs, it risks a refused `edit` followed by a dependent call in the same message, and the gain is at most the 3–4% of Finding 3.
- A separate read-only `run_command` tool with `readOnlyHint` (Finding 4): the read-only commands finish in tens of milliseconds, so there is nothing to overlap.
