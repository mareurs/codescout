# Setup A/B — daily-setup arm with onboarding and operator rules (amendment, DRAFT)

**Status: DRAFT for operator review, 2026-11-04. Nothing here is registered, built or run. No spend is authorised by this document.**

Amends `2026-10-27-codescout-setup-ab-design.md` (§1 arms, §3 gate 3, §9.1) and
`2026-11-01-setup-ab-live-screen-design.md` (runner and arms). The screen itself is unchanged and
stays valid as bare-arm calibration.

## Why

The operator's point (2026-11-04): a comparison of "codescout against native" must use the setup
as it is used daily. A project there carries its onboarding (memories, system prompt), the rules in
the operator's CLAUDE.md, all codescout tools and the reconnaissance skill. The arms built so far
do not: they strip the onboarding memories on purpose (parent §3 gate 3, `PREPARE_CLEANUP` in
`tools/setup-ab-eval/arms.py`), and parent §9.1 already names the gap ("a later study could
rebuild memories ... as of each task's parent commit, scrubbed against the answer"). This is that
study.

## Verified facts (read 2026-11-04, not assumed)

| Fact | Where |
|---|---|
| `cs` and `full` expose all 21 codescout tools, including the three LSP-gated ones (`call_graph`, `references`, `symbol_at`), made available by `[project] languages` from `onboarding()`. | `arms.py` `CODESCOUT_TOOLS`, `LSP_GATED_TOOLS`, `expected_mcp_tools` |
| The companion plugin (which carries the reconnaissance skill) is mounted in `cs` and `full`; the `Skill` and `Agent` tools exist in `full` only (`FULL_ONLY_TOOLS`). So the recon skill is invocable in `full` only. | `arms.py` docstring, `FULL_TOOLS` |
| `full` copies the frozen `CLAUDE.md` and `settings.json` of profile `/home/marius/.claude` (setting sources `user,local`). `cs` and `bare` get no CLAUDE.md. | `setup-manifest.json` (`profile_dir`, `full_config_files`), `arms.py` `FULL_SETTING_SOURCES` |
| That `~/.claude/CLAUDE.md` holds: four-profile note, memory rule (use codescout, not built-in memory), Opus review escalation, monitor-do-not-poll, generated operator rules OP-1 (always verify) and OP-6 (architecture lens). It holds NO mutation-testing rule. The words "mutation testing" occur in the codescout project `CLAUDE.md` (Testing Discipline: `./scripts/mutation-probe.sh`, `docs/PROBES.md`) and in `docs/trackers/operator-rules.md` OP-2 (as the lens of a validating review). | `~/.claude/CLAUDE.md`; `CLAUDE.md:19`; `operator-rules.md` OP-2 |
| `onboarding()` itself writes only the `onboarding` and `language-patterns` memories (`PROGRAMMATIC`). The other memories and `.codescout/system-prompt.md` are written by a subagent that follows the instructions the call returns; the source warns that this step "may never run". A faithful onboarding therefore needs an LLM run per task. | `src/tools/onboarding.rs` (`PROGRAMMATIC` const, doc comment near line 68) |
| `prepare_index` deletes every `memories` directory and the hand-off prompt after `onboarding()`, then indexes. All arms share one run image, `bare` included. | `arms.py` `PREPARE_CLEANUP`, `prepare_index` |

## Proposed arms

| Arm | Configuration | Answers |
|---|---|---|
| `bare` | As today: native tools, empty config, no rules, no memories. | baseline |
| `rules` (candidate, see D1) | `bare` plus the same rules text as the treated arm, native tools only. | the rules' own effect |
| `daily` | The full daily setup: all 21 codescout tools, companion hooks, `Skill` and `Agent`, the other frozen plugins, the frozen user-level `CLAUDE.md` plus the operator rules block (D3), and the task's onboarding artifacts (memories, system prompt). Replaces `full`. | the whole setup |
| `cs` (optional ablation, see D1) | Tools and hooks only, as today. | tools without instructions |

Differences to report: `daily − bare` (bundle), `daily − rules` (what the tools and onboarding add
once the rules are held equal), `cs − bare` (tools alone). No claim about codescout may rest on
`daily − bare` alone (parent §1 stands).

## Onboarding artifacts

1. Generated once per task, at the task's base commit, inside the study network, never from the
   issue text or the test patch. Frozen and hashed; every rep and both models reuse the same bytes,
   like the index.
2. Produced by running `onboarding()` and then an LLM subagent that follows the returned
   instructions. Model, effort and cost cap: D2. Cost is unknown until one dry run measures it.
3. Leak gate (extends parent §3 gate 2): the existing n-gram scan (`leak_scan.py`) runs over every
   generated memory and `system-prompt.md`, with the hidden-test names and gold-patch added lines as
   needles and a planted-needle positive control. Count must be 0. A non-zero count regenerates the
   artifact once; a second failure drops the task.
4. Visibility: the artifacts exist only in the treated arm(s), delivered by a per-arm mount or a
   separate run image. A gate must show, with a positive control, that `bare` cannot read them.
   Today every arm shares one image, which is why this cannot be a simple "stop deleting".

## Rules content

- Keep the frozen `~/.claude/CLAUDE.md` as it is.
- Add an operator-rules block (D3). Candidates found, none yet decided: the mutation-testing and
  testing-discipline text of the codescout `CLAUDE.md`; OP-2 (review escalation); OP-3 (durable
  facts to codescout memory). The mutation text names `./scripts/mutation-probe.sh` and
  `docs/PROBES.md`, which do not exist in a foreign repository, so it needs a generic wording (for
  Rust, `cargo-mutants` or a manual "mutate the fix, confirm the test fails" step). A rule that asks
  for extra test runs changes cost and turns; that is part of the setup being measured, which is why
  `rules` exists.
- The `rules` and `daily` arms receive byte-identical rules text.

## Risks to test, not assume

- **Recon skill outside the codescout repo.** The skill writes F-N and W-N entries into
  `docs/trackers/<topic>-session-log.md` through `doc(append_entry)` and copies a template from the
  codescout repo. A foreign task repo has no tracker catalog. A no-spend dry run must show the skill
  loads, and either works or fails cleanly, before registration.
- **Tracker or catalog writes dirty the patch.** Files the agent writes under `docs/trackers/` or
  `.codescout/` must not enter `agent.patch` or the grade. Check `extract_patch` exclusions.
- **Memory writes.** If OP-3 is included, the agent may call `memory()`; confirm these writes stay in
  the run's own `.codescout/` and are discarded.
- **Cost and turns.** Onboarding artifacts and rules add context; the per-run cap (`--max-budget-usd`)
  must be re-derived from a dry run, not carried over from the screen.
- **Selection.** Tasks come from the validated fresh pool (memory `research/harder-task-benchmarks-20261104`),
  each read against its test for under-specification (the lexical overlap check does not detect it).

## Carried fixes

`numTurns` must sum `num_turns` over all `result` records (bug file
`docs/issues/2026-11-03-setup-ab-run-row-numturns-reads-only-the-last-result-record.md`); the
registration freeze for this study is where `run.py` may change.

## Open decisions (operator)

- **D1** Arms: `bare`, `daily`, plus `rules`? plus `cs`? Each arm multiplies cost.
- **D2** Onboarding generator: model, effort, per-task cost cap. Spend needs an explicit go.
- **D3** Which rules go in the operator-rules block, and the generic wording of the mutation rule.
- **D4** For foreign repositories, a project-level CLAUDE.md: none (default here), or one generated
  per task by the onboarding step?
- **D5** Whether the other frozen plugins (superpowers, buddy, legible, document-skills,
  frontend-design, claude-md-management) stay in `daily`, as they were in `full`.
- **D6** Opus 5.5 training cutoff (parent §9.2, still unverified) against the fresh pool's dates.

## Not changed

Same model, CLI version, prompt bytes, turn limit, resource limits and network in every arm; effort
equality gate; the F2P rules; the budget gate (cap, per-call, reserve) and the operator-only GO.json;
the rule that no spend starts without an explicit go.
