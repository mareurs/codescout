# Effort steering: verified findings and instrument rules (as of 2026-10-07)

Work lives in the `claude-plugins` repo (`effort-steering/`); the live list is its tracker
`docs/trackers/effort-steering-second-phase.md` (id `af4ccc8b94a95c91`, ES-1 to ES-23) and the passover
`ec8f1f91d436b653`. Everything below was measured with Sonnet 5.5 and Claude Code 2.1.292 unless said otherwise.
Numbers and caveats: ES-17 (mods), ES-18 (baselines), ES-20 (exact-answer tasks), ES-21 (S4 result).
The quality test (S5-lite, ES-22): spec `claude-plugins/docs/superpowers/specs/2026-10-07-effort-steering-quality-test-design.md`,
plan `claude-plugins/docs/superpowers/plans/2026-10-07-effort-steering-quality-test.md`.

## What is established

- **Steering sentence works at xhigh.** The hook appends "Answer directly without deliberating." as
  `additionalContext`. Registered S4 run (second registration, 8 prompts that think, `claude -p --effort xhigh`):
  mechanism_pass, mean thinking -37 percent, output -27 percent, p ~ 1e-4; reproduced from the raw transcripts.
  The first registration (medium, 12 prompts) failed because the model thinks ~0 tokens at medium (floor effect).
  QUALITY IS NOT MEASURED: the verdict (G4) is held until a quality check on real coding tasks with tests (ES-22).
- **The sentence is Anthropic's documented per-message suppressor** (platform.claude.com "Steering thinking", read
  2026-10-07), with a warning that thinking less "may reduce quality on tasks that benefit from reasoning" and that
  lowering effort "is usually the better first lever". No vendor quality number exists for it.
- **A Claude Code mod can set effort per request.** A hook on `turn.step` (fires before every model request, main and
  subagents) can `next({ ...e, effort })`. Headless works with `--plugin-dir`. The rewrite reaches the wire as the
  request's `output_config.effort` plus an appended `role: system` pin message. Mid-turn and turn-boundary changes
  caused no cache miss in the tested setup. Classic hooks cannot set effort. Mods run only in Claude Code.
- **At medium the model rarely thinks and solves nearly all exact-answer tasks** (33 of 35 tasks 3 of 3), so such tasks
  cannot show a quality effect. At xhigh it thinks a lot on design and debugging prompts (3 of 12 prompts hold ~94 percent).
- `claude -p --effort <level>` works for a child run; a saved settings or /model default does not reach a
  `--setting-sources local` child. `--tools ""` disables all tools (a model that wants Bash then writes raw tool XML).

## Instrument rules (each cost real time)

- `claude -p` in a loop needs stdin from `/dev/null`; check every first user message is one line (S5: equals the prompt file).
- Count a message's thinking blocks over ALL transcript records of its `message.id` (the thinking block sits in an
  earlier record than the tool_use or text).
- Take a message's USAGE from the record with the largest `output_tokens`, never the first record. In tool-loop
  transcripts early records can carry partial usage (2026-10-07: 13 of 596 multi-record messages; first record 3
  output tokens and no thinking field, last 4,916 with 2,756 thinking). S4's `tokensFromTranscript` keeps the first
  record; the S4 transcripts had 0 such messages, so S4 stands, but do not reuse it for tool loops.
- Subagent transcripts live at `<config>/projects/<slug>/<sessionId>/subagents/agent-*.jsonl`; count them too.
- Measured runs go DIRECT. A local wire recorder (ANTHROPIC_BASE_URL set) doubled the prefix and changed thinking.
- A "no effect" or "no miss" result needs a positive control (a model switch breaks the cache; use it).
- Grade with scripts, never from a remembered number (a remembered 153 was wrong; the DP says 571). Run model-written
  code only in `python3 -I` in a temp dir with a timeout, or in a container; never run `git` on an agent's tree on
  the host (a `.gitattributes` filter driver would execute).
- A `--max-budget-usd` cap is checked after the fact (a runaway cost $1.29 against a $1 cap); use a call timeout.
  Claude Code 2.1.292 has no max-turns flag.
- `claude --debug-file` shows no request parameters.
- Do not extrapolate a power or cost number without stating its assumptions; measure a pilot first.

## Where things are

- Probe scripts, flat mod sources, wire recorder: `claude-plugins/effort-steering/spikes/probes/` (README); calibration
  runner, task sets, graders: `.../spikes/dose/`. Both are throwaway (delete before any plugin.json). The S5 harness
  will live in `.../spikes/s5/` (throwaway too).
- Design and first-phase records: codescout `docs/superpowers/specs/2026-10-04-effort-steering-design.md`,
  `docs/research/2026-10-04-effort-steering-*.md`.

## S5 smoke, control and pilot (2026-10-11 and 12, CLI 2.1.296, Sonnet 5.5, personal account)

- **Cost is far below the published $2 to $5 per `xhigh` run.** Client-reported `costUsd` means: A $0.53, B $0.45, C $0.18 (pilot, 3 tasks, 1 run each). Whole smoke + control + pilot: about $7.06 notional. Indicative only (3 easy tasks).
- **All 9 pilot runs passed, in every arm, including `low`.** A ceiling signal: calibration must find tasks where the arms differ.
- **Claude Code fires `UserPromptSubmit` on a queued `<task-notification>` prompt.** A Bash command over 120 s is auto-backgrounded, and its completion re-fires the hook, so arm B gets the steer a second time. The old "exactly one context" rule (R147) classified such a run as `steer_mismatch`; R-ST1 now accepts one or more identical contexts with the first before the first assistant record (S5 README rule 20, commits 294192b..29c3ee3). `analyze` reports `counts.<arm>.multiSteer`.
- **`num_turns` of the stdout JSON is unreliable for a run that ends with a trailing task-notification** (cs-p07.C.0: 1 turn reported, about 13 in the transcript) and the wall time then includes the CLI waiting for the background task (about 10 minutes there).
- **Proxy control:** the first request has the same total input (16,545 tokens) open and proxied; thinking and output were lower through the proxy in 2 runs per side (not settled with n=2). Allowlist hosts seen: `api.anthropic.com`, `http-intake.logs.us5.datadoghq.com`, `static.rust-lang.org`; no refusal in allowlist mode.
- **Kept run directories are large:** 3 to 6.5 GB each; `runs/` was 62 GB after the pilot.

## S5 calibration of the first 10 tasks (2026-10-12, CLI 2.1.296, Sonnet 5.5)

- 30 runs (A xhigh x2, C low x1 on cs-p01, p02, p04, p05, p06, p07, p08, p10, p11, p12): **30 of 30 passed**, no infra row, no timeout, no p2p regression. `s5 select`: **all 10 tasks dropped `effort_idle`** (A 2/2 and C 1/1), 0 kept of the spec's target 32.
- Means: A $1.13, 616 s, 23,326 thinking tokens, 39,881 output tokens; C $0.22, 466 s, 1,274 thinking tokens, 5,370 output tokens. So `low` thinks about 18 times less and passes the same.
- Meaning: on these well-specified tasks there is no headroom for the steer to hurt (the spec's "xhigh equals high" risk, here low equals xhigh). A wider margin does not fix it; the tasks must be harder or the question must change. Decision for the operator.
- Task authoring yield: 3 usable of 6 attempts in batch 1 (cs-p09, p13, p14 dropped: the fix's own tests call helpers the fix adds). Window moved to 2026-07-15 (spec amended 2026-10-12); `tasks/candidates-0715.json` has 196 candidates.

## S5 calibration of batch 2 (cs-p15..p20, 2026-10-13) and a grader defect

- 18 runs, $11.76 (client estimate): 17 of 18 passed. `s5 select`: cs-p15, p16, p17, p18, p20 dropped `effort_idle`; cs-p19 "kept" only because of an invalid failure (below). With the 10 earlier tasks, 0 of 16 survive the drop rule: arm C (`low`) passed 16 of 16, arm A 31 of 32. Means batch 2: A $0.91, 15,069 thinking tokens; C $0.15, 764.
- **Grader defect (cs-p19.A.0):** `grade.mjs` graftScript appends a graftLines line only when the agent's file does not already declare that module. The agent's CLAUDE.md is built from `p2p.command`, which named `server::pin_scope_checks::...`; the agent wrote its own inline `mod pin_scope_checks { ... }` with the same test names, so the hidden module was never mounted: f2p `notRun`, p2p "passed" (its own tests). Fixed in the task definitions: graft module names now carry the prefix `s5h_`, and no `p2p.command` names a hidden module (cs-p19 uses `server::`, cs-p10 `integration_ unaltered_`). Not fixed in code: no loader check yet; the author brief has the rule. A passing row proves the hidden tests ran (a shadowed module gives `notRun` = fail), so the 33 other passing rows are not affected.
- Tasks whose `p2p.command` named hidden tests also leak them to the agent's CLAUDE.md: always give a prefix filter.

## S5 thin-spec probe (2026-10-13, CLI 2.1.296, Sonnet 5.5)

Question: do thin-spec variants (shorter TASK.md) of cs-p15, p16, p17, p19, p20 give effort headroom (arm A passes, arm C fails)? Config `s5-work/configs/probe-thin.json`, rows `s5-work/rows-probe-thin.jsonl`, 10 runs (A and C once per variant), cost $5.18 client-reported (A $4.24, C $0.94), no infra failures, no plan-limit stop, ~1 h.

Result: A passed 5/5 and C passed 5/5. Every pass has f2p all passed, p2p all passed, no timeout. No headroom: thin specs did not make arm C fail.

Process still differs by effort: A took 28-58 turns, C took 10-18 turns. A cost about 4.5 times what C cost. So effort changes the process and the bill, not the outcome, on these tasks.

Caveat: n=1 per arm per variant. A 5/5 vs 5/5 tie does not exclude a small effect. It only says that the effect is not large on these 5 tasks.

## S5 verdict to date (2026-10-13): uninformative, G4 stays held

Written into the spec section "Result to date (2026-10-13)" and into ES-22 (claude-plugins commit ab35073, not pushed). By the spec's decision rule the verdict is "uninformative": arm C passed 21 of 21 valid runs, arm A 36 of 37, the drop rule leaves 0 of 16 tasks, so the validity gate cannot hold. The registered measured run was not made and no prereg exists. Do not word this as "no harm found": the spec says the verdict is never "pass" in this case.

Pilot mechanism lead (3 tasks, 1 run each, rows-pilot.jsonl, `usage.main`): the steer cut main-session thinking 21 percent (25,422 to 20,032) and output 23 percent (58,485 to 44,837); per task thinking minus 27, 28, 3 percent; cost minus 17 percent. Arm C: thinking minus 94 percent, cost minus 67 percent. Earlier in the session I said the steer "saves little" next to `low`; that compared B with C. Against A it saves about a fifth.

Operator options still open: close ES-22 with this result; redesign with a new task source (a spec amendment and budget); or run arm B on the calibrated tasks for a cost and mechanism number only. Cleanup pending: 62 GB of run dirs in `s5-work/runs/`, the `s5-proxy` container, the exposed OAuth token.

## R1 (effort routing, first rule): state at 2026-10-13

Design: claude-plugins spec `docs/superpowers/specs/2026-10-13-effort-routing-first-rule-design.md` (id `fc50071781001cd0`). R1 = `hasStackTrace` -> shallow -> `low`; stage 0 (no model call), stage 1 (arm C, ~32 runs, ~$6), stage 2 (arm A, ~32 runs, ~$34, only if C failed). Margin 0.15, 12 usable tasks.

Where the code is: branch `effort-routing-r1` in claude-plugins (the main checkout is on it; `main` stays at 7ba960f). Code in `effort-steering/spikes/r1/`. **Operator ruling 2026-10-13: the branch stays separate from `main` until after stage 1.** Nothing is pushed.

Verified 2026-10-13: 30 of 30 test files pass from the repo root (the S5 `hook-copy` test fails when run from `effort-steering/`, a cwd quirk that predates R1); no model call made (no rows files); the registration equals its HEAD blob.

Open before stage 1 (decision 4 in the spec): the agent's tree must hold no `TASK.md` and no `CLAUDE.md` pointer. The first implementation kept both, which made the lab easier than S5. Needs: strip the files in `r1-workroot.mjs`, recompute treeHash, re-gate the 12 tasks (one at a time, 4 CPUs / 8 GB), re-register, update the README, Opus review. Stage 1 needs the operator's separate approval (run count, model, cost).

## R1 V2 stage 1 (2026-10-14): arm C passes 19 of 24 (verdict needs_stage_2)

State: branch `effort-routing-r1` in claude-plugins (HEAD eb90bd3 + later), never pushed, separate from main until the operator rules. V2 = hidden tests in the agent's tree (spec decision 5). The V1 stage 1 (10 of 24 runs asked for the test file, C 4 of 24) is a pilot only.
- V2 stage 1: 24 runs, arm C (`low`), Sonnet 5.5, $3.7766 real, no infra, no stop. C passed 19 of 24; every run edited; the shadow guard never fired. Per task: 9 tasks 2 of 2, cs-p12 1 of 2, cs-p18 0 of 2, cs-p20 0 of 2.
- cs-p18 (both runs, verified in the transcripts): the agent ran `cargo test` only with `--no-default-features`, so the `librarian` module (default feature) never compiled and the 2 `update_entry` f2p tests never existed for it. The warm target is built with `--no-default-features`, so the flag is also the cheap path: a lab artifact of hint plus cache, not shown to be an effort effect.
- cs-p20 (both runs): grade `build=failed`, 5 files edited, agent said it changed `progress` in the context builders and the `derive_ctx` call sites; cause of the build error not established (the grade dir is not kept; set S5_KEEP_GRADES=1 to keep it).
- cs-p12.C.1: fixed one part, ended by asking whether to fix the other 5 f2p tests.
- Lab changes made for V2: grader shadow guard (diff-based, in the graft step), `r1-v2-adjust.json` (cs-p01/p06/p08 hidden file from P, cs-p16 doc comment dropped). Guard NOT closed against: git attribute/config tampering, skip-worktree, git replace, `#[cfg(any())]`; a sentinel test appended to each hidden file would close it (not built).
- Next gate: stage 2 = 24 runs of arm A (`xhigh`), about $25 by the S5 per-run figure, cap $60; operator approval needed. G4 stays held.

### R1 V2 stage 1: why cs-p18 and cs-p20 failed (checked 2026-10-14)
- The build-hint sentence anchors the agent on `--no-default-features`: 22 of 24 stage 1 agents used it at least once (20 in every cargo command); with the sentence removed (diagnostic, 4 runs, $0.53, rows in `s5-work-v2d/rows-diag.jsonl`, not in the verdict) 0 of 4 did.
- cs-p20: replaying the agent's edits onto the V2 tree (fidelity checked against the row's files/insertions/deletions) and grading with S5_KEEP_GRADES=1 gives `E0063 missing field progress in ToolContext` in `tests/link_scan.rs:29` and `tests/audit_shards_cross_machine.rs:50`: the agent added a field and did not compile the other test targets. Under the flag it could not see them at all (the `librarian` module is gated); without the sentence one run still failed the same way and one passed after running the integration tests.
- cs-p18: NOT the flag. Without the sentence both agents ran default features and still fixed only the `edit_file` site; the prompt trace shows one failing test, the two `update_entry` tests are a second site they never ran. 0 of 2 before and after.
- So the sentence is a real harness artifact, but the flag is not the main cause of the failures; both look like incomplete verification or an incomplete fix at `low`. Whether `xhigh` does better is what stage 2 measures. n is 4, so p20 0 of 2 to 1 of 2 is not evidence.
- Infra note: Docker's address pool was full (a new unrelated network took the last slot), so `proxy up` failed; creating `s5-internal` by hand (`--internal --label s5=1 --subnet 10.253.77.0/24`) lets `proxyUp` use it. `proxy down` removes it again.

## R1 V2 verdict (2026-10-14): `not_shown_safe` (R1 no-ship), G4 still held
- Stage 2 (arm A, `xhigh`, 24 runs, $14.38, no infra, no stop): A passed 22 of 24 against C 19 of 24. Per run: C $0.157, 2.5 min median, 12 turns; A $0.599, 8.3 min, 28 turns (74 percent cheaper at `low`).
- Per task (C then A, of 2): 9 tasks 2/2 and 2/2; cs-p12 1/2 and 2/2; cs-p18 0/2 and 0/2 (tie, the second bug site is not in the trace and both arms used `--no-default-features`); cs-p20 0/2 and 2/2.
- `analyze`: mean C minus A = -0.125, one-sided 95 percent lower bound -0.286 (margin 0.15), verdict `not_shown_safe`. Not "unsafe": two tasks carry the difference at N = 12.
- What separates the arms is verification depth: on cs-p20 A ran a plain `cargo test` (whole suite, integration targets, `git stash` to separate old failures) and caught `E0063 missing field progress` in two other test targets; C ran narrow filters. cs-p12.C.1 stopped to ask. A candidate lever for a later experiment: a prompt line that asks for the whole suite before finishing (arm B was never run). Not tested.
- Full write-up with the artifacts and caveats: R1 spec section "Result (2026-10-14)" in claude-plugins, branch `effort-routing-r1` (not merged, never pushed). Rows and analyses: `~/work/claude/s5-work-v2/` (sha256 prefixes in the spec).
- Caveats that stay: V2 gives the exact test interface (easier than a real prompt); the build sentence is a harness artifact (symmetric between arms); V2, the grader guard and the repairs were chosen after the V1 pilot outcome; the guard does not stop deliberate git manipulation.

## When to switch effort: research (2026-10-14)

- Index with verified claims and the four-stage design: `docs/research/2026-10-14-effort-steering-when-to-switch.md` (three raw subagent reports beside it; only the index items were checked at the source).
- Anthropic's Effort doc: a per-message effort change after a `tool_result`-only user turn "waits for the next `user` turn with new input". So "a `turn.step` rewrite reaches the wire" (our 2.1.292 probe) does NOT show that a mid-loop change takes effect. Stage 0 measures it (thinking per request after a switch, positive control, cache-read share).
- Discordance between effort levels is unstable on rerun (arXiv 2608.01347: 3 of 6 Sonnet 5 rescues reproduced). Untrained prompted selectors lose to fixed effort (ARES 2603.07915; Nadir). Always compare a router with random at the same dose.
- Transcripts record `effort` and `perTurnEffort` on every `assistant` record (mining source; correlational only).
- CLI on this host is 2.1.299 (2026-10-14); R1 ran on 2.1.296; `VERIFIED_CLI_VERSIONS` in the plan-limit detector lists only 2.1.292 and 2.1.296.
- Stage 0 (2026-10-14, CLI 2.1.296, 14 runs incl. 12 calibration): thinking tokens at the tool-result step did not separate `xhigh` from `low` on any of 6 questions (q1: low 2094 and 3198 vs xhigh 3319 and 3295; easy questions near 0 at both). So that readout cannot serve as the positive control for a mid-loop switch; whether a switch takes effect is still not shown. Details and instrument paths: the index doc, section "Stage 0 result".
- Rerun of the three losses (2026-10-14, root `/home/marius/work/claude/s5-work-v2r`, 12 runs, $6.62): cs-p20 reproduces (C 0/3, A 3/3; pooled with V2 0/5 vs 5/5), cs-p12 does not (C 2/3, A 3/3; the new C failure is a wrong fix, not a stop-to-ask). cs-p20 cause in the command text: all three `low` runs compiled only `--lib` or `--no-default-features` targets, all three `xhigh` runs ran plain `cargo test`, `--tests` or `clippy --all-targets`; all three `low` rows grade `build=failed`. Breadth of verification, not whether a test ran. Over all 48 V2 runs "any test after the last edit" was 21/24 at `low` vs 16/24 at `xhigh`. Details: index doc, sections "Verification behaviour" and "Rerun".
- Verification paragraph at `low` on cs-p20 (2026-10-14, root `s5-work-v2p`, 5 runs, $1.11): 0/5 pass, all `build=failed`; no run compiled all targets; two final messages state what was not run. Rule fixed before (effect if >= 4/5, none if <= 1/5): no effect. So an instruction asking for a real check did not widen the verification at `low` on this task. Not tested: a more specific instruction; the paragraph in the system prompt. Observation, not shown causal: `low` followed the prompt's `--no-default-features` hint from its first command in 8 of 8 cs-p20 runs, `xhigh` in 0 of 3.
- Termination gate at `low` on cs-p20 (2026-10-15, root `s5-work-v2g`, 5 runs, $1.26): 5/5 pass (rule fixed before: gap closed if >= 4/5). A `Stop` hook in a COPY of the harness (`es-stage0/es-gate`) runs `cargo check --offline --all-targets` and blocks the stop once with the compiler output; it blocked in 4 of 5 runs, always `E0063 missing field progress` in other test targets, and each of those 4 then passed. Claude Code 2.1.296 runs `Stop` hooks in `-p` mode and honours `decision: block` (seen in the transcripts as "Stop hook feedback"). One task, compile-class failure, references from other batches (low without the gate 0/10, xhigh 5/5). Cost per run: gated low $0.25, low $0.18, xhigh $0.48 (cs-p20 rerun means).
- Stage A, a critic as a gate for plans and design docs (2026-10-16, `/home/marius/work/claude/es-stage0/critic-a/`, pre-registered in `prereg.md`, 80 calls, $14.55, Sonnet 5.5, CLI 2.1.296, tools Read/Grep/Glob on the code snapshot the author wrote against): 20 codescout docs, each clean and with one planted defect. Found as blocking: `low` 18/20, `high` 17/20; no clean original blocked without a real defect (0 at both); verdict usable at both. The critic's effort made no difference (paired −1) and `low` costs half ($0.124 vs $0.240 per call). 58 of 72 verified blocking findings on the UNMODIFIED docs were real defects, so a gate would send nearly every doc back once. Traps hit: the parent of a doc's first commit can already hold the implementation (pick the snapshot from a marker the implementation introduced, and check the marker is the name the code used); removing CLAUDE.md from the tree makes critics report it missing. Not shown: that a producer fixes what the gate reports, the cost of that round, defects not planted by us. Index doc section "Generalizing the gate beyond code".
- Stage C, the critic as a `Stop` hook while an agent writes a plan (2026-10-16, `/home/marius/work/claude/es-stage0/critic-c/`, pre-registered, $35.40): verified blocking defects in the final plans, 6 plans per arm — `low` 5, `low` + gate 4, `xhigh` 0. The gate blocked once in 6 runs and returned "no findings" on plans an Opus `high` audit found real defects in; so the `low` critic's Stage A recall on PLANTED defects does not carry over to natural ones. `xhigh` cost $1.92 per run vs $0.35 (`low`) and $0.54 (gated). For plan writing, producer effort mattered; contrast the code gate, where `cargo check` is a complete oracle. Trap: Claude Code does not pass `CLAUDE_CODE_OAUTH_TOKEN` to hook processes, so a nested `claude -p` in a hook is "Not logged in" unless the hook reads the token itself; and never launch a long run with `&` inside a time-limited tool call.
- Stage D, the knee between `low` and `xhigh` for plan writing (2026-10-16, `/home/marius/work/claude/es-stage0/critic-d/`, pre-registered, phase 1 only, $18.93): same 6 specs and prompt as Stage C, no gate, new arms `medium` and `high`; `low` and `xhigh` reused from Stage C. Verified blocking defects in the final plans: `low` 5, `medium` 5, `high` 3, `xhigh` 0; producer cost $0.35, $0.47, $0.81, $1.92. Only p02 and p09 discriminate (`low` had 0 on the other four). Verdict by the fixed rules: no tested level below `xhigh` matched. Open confound: plan length rises with effort (206 to 984 lines), so the audit may look less closely at long plans; "xhigh = 0" is not proven real. Phase 2 (conditional) was not approved. Index doc section "Stage D".
- Stage E, is the audit less sensitive on long plans? (2026-10-17, `/home/marius/work/claude/es-stage0/critic-e/`, pre-registered, 8 audits, $6.06): 8 clean Stage C plans (4 `low` of 139 to 241 lines, 4 `xhigh` of 612 to 1,240 lines), 2 planted defects each (classes drawn by seed before reading), audited by Opus 5.5 `high`, arm hidden, judge Opus. All 16 planted defects were named by some finding; named as blocking 5 of 8 in each arm (the misses of "blocking" are `minor` ratings of a nonexistent-symbol or value defect, the same in both arms). Rule fixed before the run: no length effect shown. Ceiling result on planted defects of the rubric's classes; natural defects not tested, so the Stage D confound stays open for them. Index doc section "Stage E".
- Stage F, does a repeat audit pass the same long plan again? (2026-10-17, `/home/marius/work/claude/es-stage0/critic-f/`, pre-registered, 6 audits, $9.37): the 6 Stage C `xhigh` plans (first audit `ok=true`, minor only) audited once more, same Opus 5.5 `high` script and rubric, arm hidden, no plant. All 6 repeats `ok=true`, minor only, so N = 0 under the fixed rule: no hidden defect shown by one repeat. Approved 12 audits / $12; ran 6 because the real cost of an audit on a long plan ($0.64 to $3.10) was above my $0.76 Stage E average (12 would have cost about $17). Same auditor, no positive control on natural defects, minor findings not verified (some look real: e.g. a test filter that runs zero tests), so \"xhigh = 0\" is a count of blocking findings, not proof. Index doc section \"Stage F\".
