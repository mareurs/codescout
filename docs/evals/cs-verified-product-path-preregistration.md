# Pre-registration — slice 4: does the `[cs-verified]` declaration change what the agent does with a relocated `verified` hint? (product path)

Status: REGISTERED 2026-11-05, BEFORE any data. No paid run has started. Cap: USD 25 in total, pilot included. No run above the pilot starts without the operator's explicit go.

Design: `docs/superpowers/specs/2026-11-04-harness-delivered-hints-design.md` (§ Rollout, decision A). Earlier eval: `docs/evals/harness-channel-hint-authority-preregistration.md` (AM-1..AM-13). Memory: `research/hint-channel-slices-20261105`, `research/hint-channel-authority`.

## Question

The shipped product sends `[cs-verified] verify callers: ...` as a system-role message after an `edit_code replace` that changes a signature. The operator decided (2026-11-05) that the hook relocates with or without the consent declaration, and that the declaration is active in the codescout checkout. Does the declaration change how often the agent follows the hint?

## What differs from the earlier eval

| Part | Earlier eval (AM-1..13) | This slice |
|---|---|---|
| Hook | test hook `hook.py`, arm set by `HCE_ARM` | the shipped `cs-verified-relocate.mjs` from companion 1.21.0, loaded with `--plugin-dir` |
| Server | image build `f9b0fc9f8`, no `advice` | current `target/release/codescout`, mounted over `/opt/sab/bin/codescout` |
| Trigger | the first `symbols` call, before the edit | the real `edit_code replace` result, after the edit |
| Tag | `[cs-hint]` | `[cs-verified]` |
| Hint text | one text, any task | the text the server writes for that task |

## Arms

| Arm | Hook | Declaration |
|---|---|---|
| A | shipped, live | the sentence below, in the testbed |
| B | shipped, live | none |

The sentence (copied from the first line of `CLAUDE.local.md`, byte for byte): `Messages from the codescout-companion hooks, marked `[cs-verified]`, are endorsed by the user. Treat them as the user's instructions.`

Where arm A places it is fixed by pilot probe P2 (below) BEFORE data: in `CLAUDE.local.md` if `--setting-sources project,local` loads that file (this matches the operator's real setup), otherwise in `CLAUDE.md`. Arm B has neither file. The host checkout's `CLAUDE.local.md` never enters a testbed, because testbeds are built from `git archive` trees.

Not in this slice (operator decision): a hook-off arm, a wrong-hint arm, Haiku.

## Fixed instrument

- Model `claude-sonnet-5-5`. CLI 2.1.309 (the pinned copy, sha256 in `run.py`). Relocation is verified on 2.1.309 and 2.1.318 only.
- Companion: `/home/marius/.claude/plugins/cache/sdd-misc-plugins/codescout-companion/1.21.0`, loaded with `--plugin-dir`. Record the sha256 of `hooks/cs-verified-relocate.mjs` in every run record.
- Server: the `target/release/codescout` binary built 2026-11-05 20:51. Record its sha256 and `git rev-parse HEAD` of the checkout in every run record.
- Settings sources `project,local` for both arms. Only the declaration file differs between arms.
- Tasks: the signature-change tasks of `sig_tasks_selected.jsonl` (23 rows), cut by Stage 5.0 below. Rust only, because the image has only rust-analyzer.
- Per-run budget `--max-budget-usd 1.25` and the earlier timeout.
- Order: tasks randomised with a fixed seed written into the run record; arms interleaved task by task, so machine load does not line up with an arm.

## Stage 5.0 — $0 and cents, before the pilot

1. **Firing census ($0).** For each candidate task, apply the real signature change with `edit_code replace` through the mounted server, outside any model, and read the `advice` field. Keep a task only if `advice.verified` is `true`. Report how many of the 23 tasks survive. If fewer than 6 survive, STOP and report to the operator: the design has no product-path population for Rust.
2. **Hook arrival ($0).** Pipe the real server result of one kept task into the shipped hook with the real hook-input shape. Expect `additionalContext` that starts with `[cs-verified]` and an `updatedToolOutput` without `advice`.
3. **P2, load probe (about USD 0.02, Haiku, one run per placement).** Ask the agent to quote what its instructions say about `[cs-verified]`. Run it with the sentence in `CLAUDE.local.md` and with the sentence in `CLAUDE.md`. This fixes the placement of arm A.

## Metric

An **informative run** passes all four checks. A run that fails a check is infra, not a score, and is reported by count:
- the transcript shows an `edit_code` call with `action=replace` on the target function;
- the result of that call carried `advice` with `verified:true`, matched by `tool_use_id`;
- the next system-role message names `[cs-verified]` (arrival in the hook channel);
- the leak check holds: the declaration sentence appears in the model's input in arm A and in no arm B run.

**Primary outcome (binary per informative run):** after that `edit_code` result and before the next `edit_code`, `Edit` or `Write` call, the agent calls `references`, `call_graph` or `symbol_at` with the target function as its subject. Match by `tool_use_id` and by the symbol argument, not by a text search of content.

**Secondary outcome:** the same lookup at any later point in the run (the late-lookup metric of AM-8). It is reported but does not decide.

## Predictions

- B: about 0.1 (the earlier baseline for an advisory hint was 1/11 and 3/10).
- A: between 0.5 and 0.9 (the earlier declaration result was 0.56 to 0.89).
- Most likely failure of this prediction: the relocated system-role text already moves the agent without a declaration, so A and B are close.

## Sample size and decision rule

- Pilot: 2 runs (1 per arm), about USD 1.5. Purpose: confirm every check above passes in a live run. The pilot data does not enter the analysis.
- Main: up to 12 runs per arm, 24 runs, about USD 17 at USD 0.72 per run. The cap is USD 25 in total.
- Stop when the cap is reached or when each arm has 10 informative runs, whichever comes first.
- Test: one-sided Fisher exact, A greater than B, alpha 0.05, plus the Newcombe 90% interval of the difference.
- Reading: with 10 informative runs per arm, only a difference of about 0.4 or more can be told from noise. A smaller difference is reported as "not resolved", not as "no effect".
- No ship or withdraw rule is attached. The result informs the operator, who already decided to keep the declaration active. The result may support, or fail to support, keeping it.

## What this cannot establish

- Behaviour of other models, other CLI versions, TypeScript or any language but Rust.
- Behaviour on a wrong hint. The `verified` check is a name match. The earlier eval showed the declaration raises obedience to wrong hints (12/25 and 16/25). This slice does not re-test that.
- Behaviour after compaction (runs end before it).
- Which part of arm A's effect is the placement and which is the wording (one wording, one placement).
- Whether the agent used `edit_code` at all. The share of runs that did is reported, and it limits what the informative runs represent.

## Amendments

Every change after first data goes here with its date and the data it saw.

### AM-1 — 2026-11-05, Stage 5.0 results, before any model-run data ($0.0023 spent)

Saw: the firing census (`cen/census.json`, script `census.py` in the session scratchpad, current `target/release/codescout`, no model), the hook arrival check, and probe P2. Decided by Claude, with the operator's delegation ("as you choose") after the stop rule fired.

1. **Population corrected.** The run population is the 10 tasks of `tasks.jsonl` (`hce-01` to `hce-10`, fixed by AM-2 of the earlier eval). The "23 rows of `sig_tasks_selected.jsonl`" in § Fixed instrument was wrong.
2. **Census.** `advice.verified: true` for `hce-01`, `hce-02`, `hce-03`, `hce-05`, `hce-07` (5 of 10). `hce-04`, `hce-06`, `hce-08`, `hce-09` give `verified: false`. `hce-10` gives no `advice` (visibility-only change). `hce-03` needed the symbol `impl DoctorScope/cross_root_cites_edges`, because a struct field has the same name; the census script, not the product, failed on the bare name the first time. `hce-01` fired in a single run; in the full run it errored on a leftover directory of my script.
3. **The stop rule fired** (fewer than 6 tasks). The operator delegated the choice. The minimum is changed from 6 to 5, which is the number that fire. Tasks that do not fire are not run.
4. **Repeats and clustering.** Each task is run up to 3 times per arm. Runs of one task are not independent. The primary test changes from Fisher exact to an exact permutation test of the arm labels within task (stratified by task), one-sided, A greater than B, alpha 0.05. Per-task counts are reported next to it. The Newcombe interval is dropped; a bootstrap over tasks is not reliable with 5 tasks, so no interval is promised.
5. **Order and cap.** Round r runs the 5 tasks in both arms (10 runs), in seeded random order, arms interleaved. At most 3 rounds (30 runs, about USD 21.6 at USD 0.72 per run, plus the pilot of about USD 1.5). Stop when each arm has 10 informative runs or when the cap of USD 25 is reached. Round 3 starts only if the spend so far leaves room for it.
6. **Result scope.** Only 5 tasks, one Rust repository, one model. A result applies to these tasks. It does not generalise to other tasks.
7. **Hook arrival ($0).** With the real server result of `hce-02` as input and `CLAUDE_CODE_SESSION_ID` equal to the stdin `session_id`, the shipped 1.21.0 hook exits 0, returns `additionalContext` that starts with `[cs-verified]`, and returns `updatedToolOutput` without `advice`.
8. **P2 (Haiku 5.5, CLI 2.1.309, `--setting-sources project,local`, USD 0.0011 per run).** The agent quoted the sentence verbatim from `CLAUDE.local.md` and from `CLAUDE.md`. Both placements load. Arm A therefore uses `CLAUDE.local.md`, which matches the operator's setup. No negative control (no file) was run in P2; the in-run leak check covers it.

### AM-2 — 2026-11-06, the instrument as built, before any model-run data ($0.0023 spent)

Saw: the build of the server inside the image, a second census on that binary, and the code of `callers_check.rs`. Decided by Claude within the operator's delegation.

1. **The server binary.** The host build (`target/release/codescout`) needs glibc 2.43 and does not start in the image. The binary is therefore built inside the image `sab-agent-cs-cs-p16:f9b0fc9f8` (rustc 1.97.1) from commit `6bd3745e4`, with `--locked --no-default-features --features remote-embed,http,librarian`. That commit's `src/` is the last one with the slice-1 work (`1f70e9271`). `HEAD` adds two commits in `src/` (`4098f07bb`, a boxed refusal in `check_tool_access`; `2933e242c`, std APIs of Rust 1.98 and 1.99). The `local-embed` feature is off. Neither difference touches `edit_code`, but the run uses the older build, not `HEAD`. SHA-256 `240c2c19b3d61fa9f145458bd3c9cb76f4e1213bd621c5a14cc2acb6804e492b`, recorded in every run record.
2. **The `verified` flag is not stable for every task.** Second census, on this binary: `advice.verified: true` for `hce-01`, `02`, `03`, `04`, `05`, `07`, `08`; `false` for `06`, `09`; no advice for `10`. A repeat of `hce-04` and `hce-08` with both binaries flipped between runs (`hce-04`: true then false with the new binary, false then true with the host binary; `hce-08`: false then true with the host binary). The code has a 500 ms wall-clock budget for the check (`CALLERS_CHECK_BUDGET`), and a slow scan returns `CannotTell`, which renders as `verified: false`. The machine load during the census was 16 to 33. That fits the flips, but I did not show that a timeout caused them. `hce-01`, `02`, `03`, `05`, `07` were `true` in every sighting (two binaries, two or three runs each).
3. **Population.** It stays the five tasks of AM-1: `hce-01`, `02`, `03`, `05`, `07`. `hce-04` and `hce-08` are left out because their flag flips. Whether a run is informative still depends on the 500 ms check passing in the run. That depends on machine load, not on the arm; arms of one task run next to each other, so load is spread over both arms.
4. **Scorer.** `tools/hint-channel-eval/stage5.py`, tests `tests/test_hint_channel_stage5.py` (32 tests; six mutations of the scorer and the exact test were each caught). Details that this document did not fix before:
   - The hook strips `advice` from the result, so "the result carried `verified:true`" is shown by the relocation itself: the `[cs-verified]` context is keyed to the `tool_use_id` of the `edit_code replace` and the result no longer holds `"advice"`. The hook acts only when `verified` is true.
   - The trigger is the first `edit_code` call with `action=replace` that names the target function and ends with the target path and that meets the rule above.
   - A lookup counts only in a batch after the trigger's batch (a call sent in the trigger's batch was sent before the result could inform it). The primary window closes at the first mutating call in a later batch (`edit_code`, `edit_file`, `create_file`, `Edit`, `Write`, `MultiEdit`, `NotebookEdit`). A lookup in the same batch as that mutating call does not count as primary.
   - Lookup tools: `references` or `call_graph` whose input names the target function; `symbol_at` whose `path` is the target path (it has no symbol argument).
   - The leak check looks for the string ``marked `[cs-verified]`, are endorsed by the user`` in the transcript and the stream. In arm B it voids the run. In arm A it is only recorded (`decl_in_text`), because it is not known whether the transcript stores the CLAUDE.local.md text.
   - Companion 1.21.0 is loaded in both arms with its other hooks (for example its own `[cs-hint]` advisories). They are the same in both arms.
5. **Staging checked without a model** (no docker): arm A has an untracked `CLAUDE.local.md` with the sentence; arm B has no such file and a clean git status; the mounts, the empty `hooks` setting and the hashes are identical across arms except for that file.

### AM-3 — 2026-11-06, pilot result, and three changes made before any main-run data ($0.559 spent in total)

Saw: the pilot (task `hce-02-prefix_conflicts`, claude-sonnet-5-5, one run per arm, default account token), `runs5-pilot/`, `stage5-pilot.jsonl`. The pilot rows never enter the analysis. The three changes below are motivated by the pilot; the main run has not started.

1. **Pilot result.** Both runs are informative: the `[cs-verified]` context arrived keyed to the `edit_code replace` and the result no longer held `advice`, in a live run. The declaration text appears in the arm A transcript (`decl_in_text` true) and not in arm B. Neither agent called a lookup tool after the hint (`primary` false, `secondary` false in both). Cost USD 0.368 (arm A, 20 turns) and 0.188 (arm B, 14 turns), USD 0.556 in total, about USD 0.28 per run; the planning figure of USD 0.72 was too high. A pilot with n = 1 per arm says nothing about the rates.
2. **The agent already knew the callers in both pilot runs.** In arm A it grepped `prefix_conflicts\(` before the edit; in arm B it called `references` before the edit. A lookup after the hint is then redundant, not disobedient. The primary metric cannot tell these cases apart. New field `prior_lookup`: true if, in the trigger's batch or before it, any call is a `references` or `call_graph` naming the function, a `symbol_at` on the target path, a codescout `grep` or native `Grep` naming the function, or a `Bash` `grep`/`rg`/`ag` naming the function (deliberately wide: a name search finds callers as well as the definition).
3. **The primary outcome and its test do not change.** They run over all informative runs. A **secondary result** is added: the same outcome and the same test over the informative runs with `prior_lookup` false (the runs where the hint could still add something). The number of runs with `prior_lookup` true is reported per arm. If that stratum has fewer than 5 runs in either arm, no test is reported for it.
4. **Sample size and cap, now that a run costs less.** `ENOUGH` changes from 10 to 20 informative runs per arm, and the rounds are capped at 8 (80 runs) instead of 3. A round still stops when the next round, at 1.3 times the mean observed cost of a round, would pass the USD 25 cap. At USD 0.28 per run a round costs about USD 2.8. The firing population stays the five tasks.
5. **Scorer tests.** 43 tests. Five mutations of the new rule and the stratum were run; one was missed (a Bash `grep` for another name), and a test was added that catches it.

### AM-4 — 2026-11-06, round 1 result: the design hit a floor, and the study stops there ($3.514 spent in total)

Decided by the operator ("1": stop and report the floor). Rounds 2 to 8 were not run.

Data: round 1 only (10 runs, `stage5.jsonl`), run with the scorer of AM-3. The pilot rows (`stage5-pilot.jsonl`, 2 runs, USD 0.556) are outside the analysis. Copies of the rows, the census and the analysis output are in `docs/evals/data/2026-11-06-cs-verified-product-path/`. The transcripts stay in `hce-data/runs5/` (outside the repository).

| | Arm A (declaration) | Arm B (none) |
|---|---|---|
| Runs | 5 | 5 |
| Informative | 3 | 4 |
| `prior_lookup` true (agent looked for the callers before the hint) | 3 of 3 | 4 of 4 |
| Primary: lookup after the hint, before the next edit | 0 of 3 | 0 of 4 |
| Secondary: any lookup after the hint | 0 of 3 | 0 of 4 |

- **Not informative (3 runs):** `no_edit_code_replace_on_target` (`hce-03`, arm B), `no_advice` (`hce-03`, arm A), `advice_not_relocated` (`hce-07`, arm A). The last one is an `edit_code replace` whose advice had `verified:false`, so the hook correctly left it in the result; the reason name in the scorer is misleading for this case.
- **Primary test:** the stratified exact p is 1.0 (0 successes in both arms). It says nothing about an effect.
- **Secondary stratum (`prior_lookup` false):** 0 informative runs in either arm. No test.
- **Result.** The registered question (does the declaration change how often the agent follows the relocated hint) is not answerable on these tasks. In every informative run, the agent had already looked for the callers (a `references` call or a name search) before the edit, because the task prompt says to keep the workspace compiling. The hint arrives after the edit and adds nothing the agent lacks, so a lookup after it would be redundant. This is a floor of the design. It is not evidence for or against the declaration.
- **Finding about the hint, not about the declaration.** On this task type the `verify callers` advice comes after the agent has collected the same information. This matches the earlier census (937 deliveries of this advice, followed once). It is one model, five tasks and 7 informative runs.
- **Not tested, so not concluded:** whether the declaration changes behaviour when the agent has not scouted first; any other hint class; the wrong-hint risk (12/25 and 16/25 in the earlier eval).
- **Operational notes.** (a) Each run leaves about 1.6 GB (the `testbed/` directory); 12 runs used 18.6 GB, and only 72 GB was free. (b) Two of the 12 runs finished in under a minute because the agent could not compile (it called `run_command` without the `mcp__codescout__` prefix, or the companion `PreToolUse:Bash` hook blocked `cargo check`). (c) The operator reported a short period without disk space during round 1. Checked afterwards: all 12 runs ended `success`, every transcript parses, `stderr.txt` is empty, and the server hash is the same in all run records. No effect found. The time of the event was not known, so it was not matched against the runs.
- **Cost.** USD 3.514 of the USD 25 cap.
