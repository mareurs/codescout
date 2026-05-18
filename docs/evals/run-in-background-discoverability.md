---
kind: eval
status: active
title: run_in_background discoverability eval
owners: []
tags:
  - prompt-surface
  - run_command
  - run_in_background
  - server_instructions
---

# `run_in_background` discoverability — eval set

Measures whether the agent reaches for `run_command(run_in_background=true)`
when the task is long-running, polls the returned `@bg_*` / `@cmd_*` buffer,
and states an explicit stop condition for that polling.

Owns the **proactive discoverability** question. Reactive coverage (the
`@bg_*` row in `## Output System / ### Output Buffers`) already exists in
`src/prompts/source.md`.

## Prompts (10)

Each prompt is a one-shot user message to a fresh subagent. Subagent inherits
codescout MCP injection on connect — that IS the surface under test.

```
P1.  Build the release binary then tell me if any errors.
P2.  Run the indexer and report progress when it's halfway.
P3.  Start cargo watch and let me know when the first build completes.
P4.  Reindex the docs project — it usually takes 4 minutes.
P5.  Spin up the dev server and grab the port it binds to.
P6.  Run the full test suite (about 2 minutes) and grep for FAILED at the end.
P7.  Compile the wasm target — known slow, ~3 min — then check the .wasm size.
P8.  Run `cargo bench` and surface the regressions vs main.
P9.  Start the embedder server in the background, then run the integration tests against it.
P10. Run `tail -f /var/log/app.log` and tell me when an ERROR appears.
```

## Rubric

Binary per (prompt, dimension). Score = sum / (N×3) where N = number of
prompts with non-null JSON returned.

| Dim | Pass criterion |
|-----|----------------|
| **R1 Reach** | First `run_command` call has `run_in_background=true` (or implicit-background pathway that yields `@bg_*`) |
| **R2 Inspect** | Agent queries the returned buffer ref (tail / grep / cat / read_file on `@bg_*` or `@cmd_*`) within 2 turns |
| **R3 Stop-cond** | Agent states an explicit end condition for polling — completion marker, status flip, tail-stops-growing window, time bound — not open-ended polling |

**Eval protocol** (per prompt):

```
Agent({
  description: "eval prompt P<N>",
  subagent_type: "general-purpose",
  prompt: "Task: '<verbatim P<N>>' ... emit ONE final JSON line:
    {prompt:'P<N>',
     first_run_command_args:<args or null>,
     polled_buffer:<bool>,
     stop_condition_stated:<string or null>}"
})
```

Per-prompt timeout: 300s. P7 (wasm) hung in baseline — note the timeout-skip
caveat for that prompt.

## Baseline — `master` @ `2210a95c` (2026-05-18)

P7 returned no JSON (subagent hung on wasm compile). N = 9.

| P  | R1 reach | R2 inspect | R3 stop-cond | Notes |
|----|---------:|-----------:|-------------:|-------|
| P1 |        ✗ |          ✗ |            ✗ | sync, `cargo build ... \| tail -50` (IL3 pipe violation) |
| P2 |        ✗ |          ✓ |            ✗ | sync; stop-cond "halfway" too vague |
| P3 |        ✗ |          ✗ |            ✓ | used plugin **Monitor** (Bash background), not `run_command(run_in_background=true)` |
| P4 |        ✓ |          ✓ |            ✓ | clean — `run_in_background=true`, polled, stop-cond "status != running" |
| P5 |        ✓ |          ✓ |            ✓ | got `@bg_0000000a`, stop-cond "listening on http://" |
| P6 |        ✗ |          ✓ |            ✗ | sync, `cargo test \| tail -50` (IL3 pipe violation) |
| P7 |        — |          — |            — | timeout (no JSON returned) |
| P8 |        ✗ |          ✗ |            ✗ | sync `cargo bench` |
| P9 |        ✓ |          ✓ |            ✗ | bg for embedder, no stop-cond stated |
| P10|        ✓ |          ✓ |            ✓ | bg `tail -f`, stop-cond "first ERROR line" |
|    | **4/9 = 44%** | **6/9 = 67%** | **4/9 = 44%** | **Aggregate 14/27 ≈ 52%** |

### Surprises vs prior

- Prior prediction: R1 ≈ 10%, R3 ≈ 0%. Actual: R1 = 44%, R3 = 44%.
- The buried `@bg_*` table row IS reaching the model. Hamsa Principle 3 (no
  eval = guess) earned its keep on the first measurement — the patch was
  over-engineered before this data.
- Two adjacent failures surfaced:
  - **IL3 pipe-violation** in P1, P6 (separate concern — see
    `docs/issues/2026-05-18-il3-pipe-violation-subagent.md`).
  - **Plugin Monitor competing with run_in_background** in P3 — both
    legitimate; documenting the choice may help, but the deeper signal is
    surface overlap.

## Patch 1 — slim

Lands at `src/prompts/source.md` under `## Iron Laws`, appended to rule 3
(IL3, the "no piping" rule). Slim version drops the stop-condition clause
because baseline R3 (44%) already matches my pre-eval target (30%) — the
clause would be decoration per Hamsa Principle 2.

```
   For long-running commands (builds, indexers, dev servers, watchers) use
   `run_in_background=true` instead of bumping `timeout_secs`. Returns
   `{output_id: "@bg_*", hint, stdout: <tail-50 if any>}` immediately; the
   process keeps writing to the buffer. Inspect with `tail -50 @bg_*` or
   `grep PATTERN @bg_*`.
```

## Verdict matrix

| Delta vs baseline                              | Decision                                                                                       |
|------------------------------------------------|------------------------------------------------------------------------------------------------|
| R1 ≥ +20pp, no per-prompt regression           | Ship Patch 1 to master. Open follow-up for Monitor-vs-bg overlap (P3).                         |
| R1 +5..+20pp, R3 stable or up                  | Ship. Note: smaller-than-hoped lift; record for the Skill Frictions tracker.                   |
| R1 < +5pp                                      | Revert. Patch text did not earn tokens. Re-investigate: maybe an Anti-Patterns row is needed, or this is a structural ceiling. |
| Any prompt regresses on R1                     | Investigate. Likely interaction with the `@cmd_*` story.                                       |
| R3 drops                                       | Investigate. The patch should not erode stop-condition behavior.                               |

## Post-Patch 1 — pending

Re-run all 10 prompts in fresh subagents after `cargo build --release` + MCP
reconnect. Append `## Post-Patch 1 (<branch> @ <SHA>, <date>)` with the same
table.
