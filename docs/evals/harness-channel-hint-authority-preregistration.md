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

### AM-3 — 2026-11-04, the arm hook and the task set ($0.011 spent in total)

**Hook input, measured.** On CLI 2.1.309 a `PostToolUse` hook for an MCP tool receives `tool_response` as a LIST of content blocks (`[{"type":"text","text":"ORIGINAL_OUTPUT_7Q"}]`), with keys `cwd, duration_ms, effort, hook_event_name, mcp_server, permission_mode, prompt_id, session_id, tool_input, tool_name, tool_response, tool_use_id, transcript_path`. `updatedToolOutput` given as a content-block list also replaced the result (model answered `REPLACED_OUTPUT_K4`; original 0 times in the stream; $0.0045). Both the string and the list form now pass, n=1 each.

**The arm hook.** `tools/hint-channel-eval/hook.py`, tests `tests/test_hint_channel_hook.py` (21 tests, 30 subtests).

- Acts only on `mcp__codescout__*`. Trigger: the first codescout call whose input or result names the target fn as a whole identifier. Fires once per run (atomic `O_EXCL` marker, because parallel calls run hooks concurrently).
- On the trigger, every arm re-emits the result through `updatedToolOutput`; the arms differ only in what is added and where: F nothing; A0 the hint as an extra result block; L `[harness] ` + hint as an extra result block; A1/A2/A3 the hint as `additionalContext`; P a mandatory directive as `additionalContext`.
- **Deviation from § Arms, recorded here:** A2's declaration goes into the FIRST codescout result of the run, not specifically the `activate_project` result. Reason: an agent may not call activate, and then A2 would never deliver. In the companion setup the first codescout call is usually the activation. The event log names the tool that carried it.
- `events.jsonl` in the run's state dir records `trigger` (with `carrier` = none/result/context and `tool_use_id`), `declaration` and `error`. It is the arrival check's first source; the transcript is the second.
- Never fails a session: garbage input, an unknown arm or a missing state dir print `{}` and exit 0.
- Mutation probe (mutants written outside the checkout, loaded through `HCE_HOOK_UNDER_TEST`): control green; 10 of 10 killed (A0 hint also in context, fire-once removed, declaration in every arm, word boundary dropped, result not re-emitted, non-codescout tools hooked, P uses the plain hint, L drops its label, main lets errors escape, floor gets context). The suite run against an identical copy through the same variable is green, so a kill is the mutation's.

**The task set.** `make_tasks.py`, rule fixed before the draw: one task per commit (the fn whose commit edited callers in the most files; ties → alphabetically first), pool sorted by sha, `random.Random(20261104).sample(pool, 10)`. Pool 17 commits. Drawn: `unexported_count`, `prefix_conflicts`, `cross_root_cites_edges`, `append_preferences_section`, `ensure_resident`, `read_capture`, `build_chunks`, `store_background`, `note_activation`, `in_one_commit`. Output `tasks.jsonl`, sha256 `b4c6d3df…`.

The prompt gives the old and the required signature, the commit subject as the reason, and "keep the workspace compiling". It never mentions callers or tools, because that text would be a hint in every arm, the floor included.

**Open for the instrument (not built):**
- The image needs the codescout source at each task's parent, a fixed codescout MCP binary, rust-analyzer, and the dependency registry offline, because `references` needs a loaded workspace.
- A run can end right after the first signature-changing edit of the target (a hook returning `continue: false`), since the metric is decided by then. That bounds cost and removes the need to compile. `stopReason` exists in the 2.1.309 hook schema; the stop itself is NOT yet verified live.

### AM-4 — 2026-11-04, the instrument ($0.015 spent in total)

**Metric anchor, changed before any run.** A run now ENDS at the first edit whose input names the target fn (`mcp__codescout__edit_code`, `edit_file`, `create_file`, native `Edit`, `Write`, `MultiEdit`, `NotebookEdit`): the hook logs `stop` and returns `continue: false`. The primary metric becomes: a `references` or `call_graph` call on the target **before that stop event**. This point comes at or before the first signature-changing edit, so the metric is at least as strict as registered. An agent that edits a caller first is stopped there and scored on what it did before. Live check on 2.1.309 (stub MCP, prompt asking for two calls): the session ended after 1 call, $0.004.

**A server hint that does not contaminate the metric.** codescout's `edit_code` response already carries `verify callers: references(symbol=..., path=...)`. It arrives AFTER the edit, and the run stops at that edit, so it cannot reach the pre-edit decision.

**Runner.** `tools/hint-channel-eval/run.py` (stdlib only), data under `/home/marius/work/claude/hce-data/` (outside git):

- `prepare`: the 10 parent trees by `git archive` (tracked symlinks to absolute or outside paths are skipped and recorded: `.claude/skills/claude-traces`), and a cargo home seeded from S5's (copied) then `cargo fetch --locked` for every parent. Result: 10 trees, 10 fetches exit 0.
- `smoke` (no model call) and `run`: each run copies its tree, makes it a one-commit git repo (`task <id>`), and for A3 appends `DECLARATION` to `CLAUDE.md` before that commit. Image `sab-agent-cs-cs-p16:f9b0fc9f8` (setup-ab's agent image: codescout `f9b0fc9f8`, rust-analyzer 1.97.1, node, python3) with the tree mounted over `/testbed` and the cargo home over `/cargo`. CLI = the frozen 2.1.309 (sha checked before every run); companion = frozen 1.20.21; flags mirror setup-ab's `cs` arm (`--setting-sources local`, `--strict-mcp-config`, `--dangerously-skip-permissions`, `--tools Bash,Read,Write,Edit,Glob,Grep`, `--max-budget-usd 1.25`) except `--output-format stream-json --verbose`. The hook is registered for every PostToolUse (`matcher: "*"`). 4 CPUs, 24 GB, no swap; 20-minute timeout. The token comes from `HCE_TOKEN_FILE` (mode 0600) through an env file deleted after the run.
- Smoke on `hce-05-ensure_resident`, arm A3: CLI 2.1.309, codescout 0.15.0, rust-analyzer 1.97.1, Python 3.13.5; the hook in the container returned the hint as `additionalContext` and logged `trigger` with `carrier: context`; `cargo metadata --offline` OK; the declaration is the last line of the committed `CLAUDE.md`.

**Known limits added here.**
- Network is docker's default bridge, NOT the allowlisting proxy setup-ab used. The agent could reach the internet. Equal across arms.
- No semantic index is built and no embedder is configured, so `semantic_search` fails the same way in every arm.
- The tree includes the parent's tracked `CLAUDE.md`, `.codescout/` memories and `docs/issues/`, which may describe the change. Equal across arms; it can raise the floor.
- The companion's own `[cs-hint]` advisories run in every arm; under A2 and A3 the declaration endorses them too.
- The agent image's codescout (`f9b0fc9f8`) is older than some task parents. The server is constant across arms; the tree it serves is the task's.

### AM-5 — 2026-11-04, pilot, scorer and the Stage 1 run order ($0.244 spent in total)

**Operator rulings.** Model pinned: `claude-sonnet-5-5`. Token: `~/.config/s5/token-claude-stef`, approved by the operator for this eval. Approved spend so far: the pilot only; Stage 1 needs a separate go.

**Pilot (1 run, arm P, `hce-05-ensure_resident`, $0.229, 24 s, 12 turns).** Read by hand, then by the scorer; both agree.

- Calls: `workspace activate`, `grep ensure_resident` (the trigger), a second `grep`, two native `Read`, three `read_file`, `symbols include_body`, then `edit_code` on the target together with a `Bash sed` in the same message (one batch). The run stopped after that batch.
- The directive arrived: the transcript holds an `attachment` of type `hook_additional_context` with `toolUseID` = the trigger's id and the text "You MUST call references(...)". It is in no tool result.
- Score: `refs_before_stop` false; `caller_check_before_stop` true (the name grep); exposure before; valid.
- So at n=1 the positive control did not produce a `references` call. The trigger is the first call naming the fn, which is usually a grep whose result already lists the callers. **No design change is made on n=1**; the registered P gate decides.

**Scorer.** `tools/hint-channel-eval/score.py`, tests `tests/test_hint_channel_score.py` (21). Reads the CLI transcript as the primary record and joins `events.jsonl` by `tool_use_id`.

- Calls are grouped by `message.id` into batches; only batches EARLIER than the stop's batch count as before the stop. A lookup sent alongside the edit could not inform it.
- Arrival per arm: context arms need the expected text in a `hook_additional_context` keyed to the trigger and the hint in no tool result; result arms need it in the trigger's result and in no hook context; F needs the hint absent everywhere; A2 also needs the declaration in its event's result; P needs the directive, L the labelled hint.
- Valid = stopped, exposure before, arrival ok, no hook `error` event. Invalid runs are reported, not scored.
- Mutation probe: control and identical copy green; 16 of 16 mutants killed.

**Stage 1, fixed now.** Arms P, F, A0; one run per task per arm (n = 10 per arm, paired by task). Order: tasks in `tasks.jsonl` order; within task i (0-based) the arms rotate as `[P, F, A0]` shifted left by `i mod 3`, so no arm always runs first. Runs are sequential. Stop rule: halt if cumulative Stage 1 spend exceeds $20 (about 3× the pilot's $0.229 per run times 30 runs). An invalid run is not replaced in Stage 1; it is reported. Decision after Stage 1, per the registered rule: the P gate is `rate(P) − rate(F) ≥ 0.4` on `refs_before_stop` over valid runs; if it fails, the other arms are not run and the result is recorded as "the channel did not move compliance even for a mandatory directive".

### AM-6 — 2026-11-04, Stage 1's first pass is VOID: `references` was not offered ($2.641 spent in total)

**What happened.** Stage 1 ran all 30 runs (sequential, $2.397, 15 min 41 s). The scorer of AM-5 marked 30/30 valid with `refs_before_stop` P 1/10, F 0/10, A0 0/10 and `caller_check_before_stop` 30/30. Reading the one P run that did call `references` (`hce-03-cross_root_cites_edges`) showed its result: `<tool_use_error>Error: No such tool available: mcp__codescout__references</tool_use_error>`.

**Cause, verified.** Every run's init message lists 18 codescout tools; `references`, `call_graph` and `symbol_at` are missing. They are gated on `has_lsp`, which comes from `[project] languages` in `.codescout/project.toml` (setup-ab `arms.py` § `LSP_GATED_TOOLS`). That file is untracked in this repository, so `git archive` did not carry it into the task trees. The pilot (AM-5) had the same 18-tool list. The primary metric could not be met in any arm, so **Stage 1's first pass and the pilot measure nothing about the hypothesis.** The scorer called them valid because it never checked that the measured tool existed: an instrument defect, the class `eval-design` § *A null control inherits the blind spot of its predicate space* describes.

**Fix.**
- `run.py stage_run` writes `.codescout/project.toml` = `[project] name = "codescout" languages = ["rust"]` and commits it into the task base (`git add -f`), as setup-ab committed its project config. Rust only: rust-analyzer is the one language server in the image.
- `smoke` now speaks MCP to `codescout start` in the container: activate, `tools/list`, then a real `references` call on the target. On `hce-05-ensure_resident`: 21 tools, `references`/`call_graph`/`symbol_at` PRESENT, `references` answered in 25.2 s with "9 references in 4 files" (rust-analyzer loads the workspace offline).
- `score.py` takes the init tool list; `lookup_available` = `mcp__codescout__references` is in it; a run without it, or without an init message, is invalid. Tests 23 (2 new); mutation probe 17 of 17 killed, control and identical copy green.
- Re-scored with the fixed scorer, the first pass is 0/30 valid, 0/30 `lookup_available`. Its rows are kept as `hce-data/stage1-void-18tools.jsonl` and `stage1-void-rescored.jsonl`, never pooled with the rerun.

**Descriptive only, from the void pass (not a measurement):** under P, one of ten agents tried to call a tool its list did not offer, because the directive named it. The other nine used the name grep they had already run.

**Rerun.** Stage 1 reruns unchanged (same tasks, order, arms, model, stop rule of $20 counted from the rerun's start), on the fixed instrument, after a new operator go.

### AM-7 — 2026-11-04, Stage 1 rerun: the registered result, and the confound the audit found ($5.143 spent in total)

**Run.** 30 runs on the fixed instrument, sequential, $2.502, 26 min 35 s. Rows: `docs/evals/data/2026-11-04-harness-channel-hints/stage1-rerun.jsonl` (sha256 `2158805c…`). Audit script: `audit_stage1.py`. Valid 30/30; `lookup_available` 30/30; `arrival_ok` 30/30.

**Registered metric (`refs_before_stop`, valid runs, n = 10 per arm).**

| arm | references/call_graph before the stop | any caller check |
|---|---|---|
| P | 10/10 | 10/10 |
| A0 | 7/10 | 10/10 |
| F | 5/10 | 10/10 |

The P gate as registered: `rate(P) − rate(F)` = 1.0 − 0.5 = 0.5 ≥ 0.4, **passes**.

**The confound, found by the audit before any interpretation.** In every run the trigger is in the agent's FIRST batch (batch 0). In 18 of 30 runs that first batch already held a `references` call on the target, sent in parallel with the triggering grep, before any hint could arrive. Those 18 calls are spontaneous in every arm and cannot be the hint's effect. The registered metric counts them, so in every arm it mixes spontaneous lookups with any effect of the hint, and the gate's 0.5 gap cannot be read as the channel's effect alone.

**Exploratory split, defined AFTER seeing the data, not a registered result.** Runs whose first batch had no lookup on the target, and whether a lookup came in a later batch (after the hint could arrive):

| arm | spontaneous lookup in batch 0 | runs without one | of those, lookup after the trigger |
|---|---|---|---|
| P | 7 | 3 | 3/3 |
| A0 | 6 | 4 | 1/4 |
| F | 5 | 5 | 0/5 |

The direction is the one the channel hypothesis predicts (P > A0 > F), but the informative n is 3, 4 and 5. It supports no claim on its own.

**Other facts from the audit.**
- The spontaneous lookup rate here (18/30 = 0.6) is far above the usage.db census (0.09, AM-1). The population differs: these prompts name the signature change explicitly, and every run starts from codescout's own `CLAUDE.md` and server instructions.
- Four lookups returned `warning: LSP returned no locations at all` (rust-analyzer still loading) and three returned `ambiguous name_path` errors (`cross_root_cites_edges`). The metric counts the CALL, as registered, not its success.
- Cost per run $0.038–$0.200, mean $0.083.

**Consequence for Stage 2.** The registered rule would now run A1, A2, A3 (and L). With a 0.6 spontaneous rate, about 4 of every 10 runs per arm carry information, too few to separate arms. Stage 2 waits for an operator decision on the trigger point.
