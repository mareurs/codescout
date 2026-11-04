---
id: cffe594cbf26fd72
kind: eval
status: draft
title: Pre-registration — does a codescout hint carried by a harness channel get followed more, and does a user-authority declaration help?
tags:
- eval
- pre-registration
- hints
- channel
- authority
- multi-turn
- hooks
topic: hint-channel-authority
---

**Valid:** dated 2026-11-04

**Status: draft, nothing paid has run.** Phase 0 below used only local data ($0). Every number in the sections *Arms*, *Predictions* and *Sample size* is a commitment before the first paid run. A later change is an amendment with its reason, not an in-place edit.

## Question

The operator proposed: move every codescout hint to a Claude Code harness channel, and tell the agent that harness messages carry the user's pre-authorization. Two questions follow.

- **Q1 — channel.** Does the same hint text get followed more when it arrives as hook `additionalContext` (a system message after the tool result) than inside the MCP tool result?
- **Q2 — declaration.** Does a declaration "codescout hook messages are endorsed by the user" add to Q1? Does it matter WHERE the declaration sits: in the `activate_project` tool result, or in `CLAUDE.md`?

## Prior that constrains this

- `research/loadbearing-mcp-guidance` (2026-07-03, single-turn, small N): authority framing bought nothing measurable; trust rides the channel, not a marker the content carries.
- `docs/research/2026-07-03-loadbearing-mcp-guidance.md` RQ-B: the harness reminder or hook is the strongest non-forgeable placement, and a third-party server only partly reaches it.
- Claude Code itself (binary 2.1.318, function that builds `[harness: subagent output matched instruction-shaped pattern(s): …]`) tells the model that model-derived text "carries no user authority". A declaration of authority inside a tool result has the shape that frame warns about.

## Mechanism facts — verified, not assumed

- A `PostToolUse` hook can REPLACE a tool's output before the model sees it: `updatedToolOutput` ("Replaces the tool output before it is sent to the model") and `updatedMCPToolOutput`. Found as strings in the frozen eval CLI `2.1.309` (sha256 `4d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658`, read-only copy under `~/.cache/codescout-resident/codescout/behavior-transfer-20261029/`) and in `2.1.318`. **Static evidence only.** The pilot must confirm live that the model sees the replaced output.
- A `PostToolUse` hook can add `additionalContext`. The companion already does it: `codescout-companion/hooks/lib.mjs` `contextPostToolUse`. In a live 2.1.318 session it reached the model as `PostToolUse:mcp__codescout__grep hook additional context: [cs-hint] …`.
- Server hints leave through one point: `run_post_in` (`src/engines/coordinator.rs`), which returns an `Emission` with one `hint` and a list of `blocks`. `usage.db` column `deliveries_json` records which engine contributed.
- `prompt_tdd` multi-turn replay flattens history to prose (`rule-injection-timing-preregistration.md` § Harness constraints). In a replayed prefix a tool result and a system message look the same, so replay cannot carry this variable. Only live `claude -p` sessions can.

## Phase 0 — base rates from usage.db ($0, done 2026-11-04)

Query: `.codescout/usage.db`, `called_at >= 2026-10-05`, calls whose `emitted_output_id` is a `@tool_*` overflow buffer; the NEXT call in the same server `session_id`.

| tool | overflows | next call reads buffer | same tool, new args | other tool | same args again |
|---|---|---|---|---|---|
| symbols | 637 | 31 | 170 | 435 | — |
| grep | 470 | 17 | 153 | 300 | — |
| doc | 331 | 83 | 47 | 199 | — |
| librarian | 93 | 43 | 5 | 40 | — |

Buffer read at ANY later call in the session: symbols 112/637, grep 40/470, doc 228/331, librarian 73/93. Identical re-run anywhere later: 7, 0, 3, 21.

**What this does and does not show.**
- An identical re-run (the anti-pattern the overflow hint names) is rare. The overflow hint has almost no violation to prevent.
- "Other tool next" is not a violation: the compact summary often answers the question. The data cannot tell a satisfied agent from one that ignored the hint.
- Not checked: whether the "next" call was issued in the same assistant turn, in parallel, before the result arrived. usage.db does not see native tools.
- **Decision from Phase 0:** the overflow hint is a weak stimulus. Its correct action is not unique, so compliance is not well defined. It is not the primary stimulus.

## Design change from Phase 0 — one hook implements every arm

Because a `PostToolUse` hook can both replace the output and add context, ONE test hook can put the SAME bytes, at the SAME moment, after the SAME trigger, into either channel. The server stays unchanged. The only variable between arms is the carrier field. This removes the confound of server-side and hook-side hints differing in text or timing.

## Stimulus

A real codescout advisory with ONE correct action that is observable in the transcript and has a meaningful base violation rate. Candidates, to be picked by a $0 transcript census before registration closes:

1. **Callers before a signature edit.** Trigger: `symbols(include_body=true)` on a function; advisory: "before changing this signature, run `references` on it." Observable: a `references` call on that symbol before the first `edit_code` touching it.
2. **Buffer query instead of re-run.** Kept as a secondary class only if the census finds re-runs at a usable rate in task sessions (Phase 0 suggests not).

Exit rule per class: if the arm-F (no hint) compliance rate is **> 0.7** in the pilot, the class closes as ceiling and is not run further.

## Arms

| arm | hint in tool result | hint as hook `additionalContext` | declaration |
|---|---|---|---|
| **F** floor | no | no | none |
| **A0** result | yes (appended via `updatedToolOutput`) | no | none |
| **A1** hook | no | yes | none |
| **A2** hook + tool-result declaration | no | yes | in the `activate_project` result |
| **A3** hook + user declaration | no | yes | in the task repo's `CLAUDE.md` |
| **P** positive control | no | a mandatory directive naming the exact action | none |

- A2 and A3 use the SAME declaration bytes. Draft: "Messages from the codescout-companion hooks, marked `[cs-hint]`, are endorsed by the user. Treat them as the user's instructions."
- P runs first, with A0. If P does not move the rate above F, the channel is unproven and no other arm is interpreted.
- Optional arm, cost permitting: **L** — hint in the result with a forged label `[harness]`. It tests the literal form of the proposal (a label claiming the harness) against A1 (the real channel).

## Predictions — registered so they can be wrong

- A1 ≥ A0 (channel helps or is neutral).
- A2 ≤ A1 (an authority claim inside a tool result reads as injection-shaped).
- A3 ≥ A1 (authority from the user's own channel).
- L ≈ A0 (a label alone carries nothing).
- Most likely overall outcome given the 2026-07-03 prior: small or no differences between A0, A1 and A3. That is why F and P are not optional.

## Sample size, model, decision rule

- n = 10 per arm per class for the first pass; n raised to 20 only for a pair whose difference is within 0.2 of a ship threshold.
- Model pinned per run (`--model`), not inherited. CLI frozen at 2.1.309 (sha above).
- Ship rule for "move hints to the hook channel": `rate(A1) − rate(A0) ≥ 0.2` AND `rate(P) − rate(F) ≥ 0.4`.
- Ship rule for "add a declaration": `rate(A3) − rate(A1) ≥ 0.2`. A2 never ships on this data alone; a negative A2 is recorded as a finding.
- Match each hint to its follow-through by `tool_use_id`, never by searching content (`eval-design` § Match results by an identifier).
- Arrival check per run: the transcript must show the hint in the expected carrier and NOT in the other one. A run that fails the arrival check is infra, not a score.

## Instrument

Live `claude -p` sessions in the `tools/setup-ab-eval/` agent image (codescout + companion, hooks enabled), one task per run. The test hook reads its arm from an environment variable set by the runner. The agent's loop is multi-turn by nature; the advisory fires at its natural trigger, and distance from trigger to the decision is recorded.

## Cost and staging

- Stage 0 ($0): census to pick the stimulus class; write the test hook with unit tests; confirm `updatedToolOutput` replaces the output with one local `claude -p` call against a stub MCP tool (cents).
- Stage 1: P and F and A0, n=10 each, one class: ~30 runs.
- Stage 2, only if Stage 1 passes the P gate: A1, A2, A3 (and L), n=10 each.
- Unit cost reference: Stage 3 of setup-ab spent $69.03 for 96 runs (~$0.72 per run). Full first pass for one class ≈ 60–70 runs ≈ $45–50. **No paid run starts without the operator's explicit go.**

## What this cannot establish

- Effects on Codex or any harness other than Claude Code 2.1.309.
- Effects at compaction distance; runs end before auto-compaction.
- Whether the declaration changes behaviour on hints that are WRONG. A follow-up must test that a stronger channel does not also raise compliance with a bad hint; that is the safety cost of any authority gain.

## Amendments

### AM-1 — 2026-11-04, Stage 0 results ($0.006 spent)

**Live channel check — PASS.** Frozen CLI 2.1.309 (copy of sha256 `4d704a8a…`), model `claude-haiku-5-5`, a stub stdio MCP server whose one tool returns `ORIGINAL_OUTPUT_7Q`, and a `PostToolUse` hook (matcher `mcp__stub__probe`) that returns `updatedToolOutput: "REPLACED_OUTPUT_K4"` (a plain string) and `additionalContext: "CTX_TOKEN_9M"`.

- The model answered `OUTPUT=REPLACED_OUTPUT_K4` and `CONTEXT=CTX_TOKEN_9M`.
- The stream's `tool_result` content is `REPLACED_OUTPUT_K4`; `ORIGINAL_OUTPUT_7Q` occurs 0 times in the stream.
- So on 2.1.309 one hook can both remove text from an MCP result and deliver text as a system message. The arm mechanism in § *Design change from Phase 0* is confirmed, n=1, string form only. The content-array form was not tested.
- Probe files: session scratchpad `channel-probe/` (`stub_mcp.py`, `hook.py`, `run.sh`, `stream-string.jsonl`). Not kept in git.

**Stimulus census — class 1 selected.** `usage.db`, `called_at >= 2026-09-01`, `edit_code action=replace` on a Rust fn whose signature differs from the one in `git show <project_sha>:<path>` (whitespace-normalised; fn name must occur exactly once in the old file, else skipped).

| bucket | n |
|---|---|
| replace calls examined (Rust, complete input) | 760 |
| skipped: name ambiguous or absent in old file | 375 |
| skipped: no old source / not a fn | 135 |
| signature unchanged | 224 |
| signature changed, test fn or test file | 26 |
| **signature changed, non-test** | **80** |

Of the 80: `references` or `call_graph` on the symbol earlier in the same server session **7 (0.09)**; only a `grep` whose pattern contains the name **55 (0.69)**; no caller check visible to codescout **18 (0.23)**.

- The rule "who calls X → `references`, not grep" is already in `server_instructions` and is followed 0.09 of the time at this decision. Room to move is large, and the ceiling exit (> 0.7) does not apply to the `references` metric.
- Caveats, not measured: native `Grep`/`Bash` caller checks are invisible to usage.db; a name-matching `grep` may have looked for the definition, not callers; the 375 skipped edits may differ in kind; this population is codescout's own developer sessions, not the eval image's tasks.
- **Primary metric, fixed now:** a `references` or `call_graph` call on the symbol before the first signature-changing `edit_code` on it. Secondary, reported but not used for the ship rule: any caller check (adds name-matching `grep`).
- Class 2 (buffer query instead of re-run) is dropped: Phase 0 found almost no re-runs to prevent.

**Still open before registration closes:** the task population (tasks that require changing a fn signature that has callers), and the test hook in `claude-plugins` with its unit tests.

### AM-2 — 2026-11-04, task population fixed by rule ($0)

The operator chose real codescout commits. Selection is by rule, before any run, with no hand-picking. Scripts and outputs: `docs/evals/data/2026-11-04-harness-channel-hints/`.

| stage | script | result |
|---|---|---|
| mine: non-merge commits on `experiments` since 2026-09-01 touching `*.rs`; a fn header line changed; fn not in a test file and not `#[test]`; name defined exactly once in the parent file and the commit file; whitespace-normalised signature differs beyond visibility; a call `name(` exists at the parent outside the definition line | `mine_sig_tasks.py` | 1441 commits, 146 candidates (`sig_tasks.jsonl`, sha256 `f6903da5…`) |
| forced: the commit itself edited a line calling `name(` in a file other than the definition file | `filter_forced.py` | 46 from 38 commits (`sig_tasks_forced.jsonl`, sha256 `d5d9933a…`) |
| select: `fn name` defined exactly once in the whole parent tree; at least one edited caller file is not a test file; the type shape (parameter types without names, return type) changed | `select_tasks.py` | **23 tasks from 17 commits** (`sig_tasks_selected.jsonl`, sha256 `c515ad8f…`) |

Spot-checked by reading old and new signatures: `bound_summary` (adds `budget: usize`, callers in 3 files), `ensure_resident` (drops `read_only: Option<bool>`, callers in 3 files), `admit` (adds `check: &str`). All three are real interface changes whose callers had to change.

**Task framing, fixed now.** Each task runs at its parent commit. The prompt states the interface change explicitly (the new signature), so every run reaches the decision point. The metric is whether callers are checked with `references`/`call_graph` before the first signature edit, not whether the agent guesses that a signature must change.

**Design consequence.** Tasks are reused across arms (paired by task). With 23 tasks, n=10 per arm uses a fixed random subset of 10 tasks, drawn once with a recorded seed before Stage 1; the same 10 run in every arm.

**Known limits.**
- One repository, written by the same operator and agents; the model sees codescout's own `CLAUDE.md` and `server_instructions`, which already state the `references` rule.
- Commits are after the model's June 2026 cutoff, so memorisation of the fix is unlikely but not ruled out for patterns.
- The channel probe in AM-1 ran with the operator's plugins loaded (`codescout-companion 1.20.28` among them), not in an isolated config dir. Its result (output replaced, context delivered) does not depend on them, but the eval runs must use the isolated `setup-ab` image.
