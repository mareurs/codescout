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
| `rules` | `bare` plus the same rules text as the treated arm, native tools only. | the rules' own effect |
| `daily` | The full daily setup: all 21 codescout tools, companion hooks, `Skill` and `Agent`, the other frozen plugins, the frozen user-level `CLAUDE.md` plus the operator rules block (D3), and the task's onboarding artifacts (memories, system prompt). Replaces `full`. | the whole setup |
| `cs` (not built unless needed, D1) | Tools and hooks only, as today. | tools without instructions |

Differences to report: `daily − bare` (bundle), `daily − rules` (what the tools and onboarding add
once the rules are held equal), `cs − bare` (tools alone). No claim about codescout may rest on
`daily − bare` alone (parent §1 stands).

## Onboarding artifacts

Operator rule (2026-11-04): a project is never onboarded more than once, and no other LLM call is
repeated where its result can be cached. The unit of onboarding is the REPOSITORY, not the task.

1. **One onboarding per repository**, at the OLDEST base commit among that repository's pooled
   tasks. No task's fix exists at that commit, so the artifact cannot contain an answer. Later tasks
   get memories that are somewhat stale, as in daily use; this is disclosed, not corrected. The
   artifact is frozen and reused by every task of the repository, both models and every repetition,
   in the treated arm(s) only. Onboarding count = repositories in the pool, so the pool should hold
   several tasks per repository (cost amortises per task) but at least 4 repositories (see Risks).
2. **Generation.** `onboarding()` then an LLM subagent that follows the returned instructions (the
   subagent writes everything except the `onboarding` and `language-patterns` memories). One
   generator model for every repository, whichever model is under test (D2); the same-model
   advantage this may give one tested model is disclosed. Run inside the study network, from the
   repository tree only, never from issue text or a test patch.
3. **Never regenerated.** Failure handling, in order: (a) an interrupted generation RESUMES, skipping
   the topics already written; (b) a leak hit is fixed by deterministic redaction of the offending
   lines, recorded in the manifest; (c) if redaction leaves the artifact unusable, the repository's
   tasks are dropped. No path runs the generator a second time for the same repository.
4. **Cache.** Key = `(repo, oldest base commit, codescout version and ONBOARDING_VERSION, generator
   model, prompt hash)`. Store: `setup-ab-data/onboarding-cache/<repo>@<sha>/` (not in git) with a
   manifest of file hashes, generation cost and the leak-scan result. A matching key means no LLM
   call. The registration records the manifest hash; the runner refuses an artifact that does not
   match it.
5. **Leak gate** (extends parent §3 gate 2). The n-gram scan (`leak_scan.py`) runs over every
   generated memory and `system-prompt.md` with the needles (hidden-test names, gold-patch added
   lines) of ALL tasks of the repository, with a planted-needle positive control. Needles are first
   baselined against the repository tree at the oldest base commit, so code that already existed
   does not trigger the gate. Count must be 0 after redaction.
6. **Visibility.** The artifacts exist only in the treated arm(s), delivered by a per-arm mount or a
   separate run image. A gate shows, with a positive control, that `bare` cannot read them. Today
   every arm shares one image, which is why this is not a simple "stop deleting".
7. **Cost.** Unknown until one dry run on the smallest pooled repository, under an operator-approved
   cap, measures it. Total onboarding spend = repositories x measured cost. No generation starts
   without an explicit go.

## Reuse of other LLM calls

- **`bare` runs** are executed once per (task, model, repetition) and are the baseline of every
  comparison (`daily − bare`, `rules − bare`, `cs − bare`). They are never repeated per pair.
- **Under-specification verdict** (does the issue state what the test asserts; lexical overlap fails
  this, memory `research/harder-task-benchmarks-20261104`): one cheap LLM read per task, cached on the
  hash of `(issue, test patch, gold patch)`, plus a manual read of a sample.
- **Pre-screen** (bare arm, to find tasks the plain agent sometimes fails): cheap model only, 2
  repetitions, capped. Its runs are NOT measurement data: selecting tasks on bare failure and then
  measuring on the same runs would bias the bare arm downward. Measurement uses fresh repetitions.
- **Index and Qdrant template**: not an LLM cost; already one per task image. Tasks of one
  repository may share a base index and re-index incrementally, if the runner supports it.
- **Prompt cache**: runs of the same task and arm share an identical prefix (tools, CLAUDE.md, memory
  index). Scheduling them back to back may raise cache reads. Not measured; the screen's `rows.jsonl`
  holds the cache fields needed to check it before relying on it.

## Rules content

Two layers (D3):

1. **R-block**: a tool-neutral rules text, byte-identical in `rules` and `daily`. In `daily` it is
   appended to the frozen user-level `CLAUDE.md`; in `rules` it is the whole CLAUDE.md.
2. **Frozen `~/.claude/CLAUDE.md`** of profile `/home/marius/.claude`, in `daily` only. It names
   codescout memory, skills and Opus reviews, which `bare` and `rules` do not have, so it belongs to
   the setup under test, not to the rules under test.

So `rules − bare` is the R-block alone, and `daily − rules` is tools, hooks, skills, plugins,
onboarding and the frozen CLAUDE.md together.

**R-block draft** (controller wording, distilled from `docs/conventions/testing-discipline.md`; the
operator validates or rewrites it before registration):

```
Testing discipline. A green test run proves only what the tests can detect.
1. Before you trust a new test, run it on the code without your fix and confirm it fails for the
   reason you expect.
2. For each assertion ask which change it cannot detect. An absence check (is_empty, !exists) passes
   when the mechanism is dead; an existence check passes when the result is too wide. Break the code
   in the other direction too.
3. When you finish, break your fix on purpose (revert or invert the changed line) and confirm a test
   fails. If none fails, the tests do not guard the fix; add or change one.
4. Where several guards or bounds can refuse the same input, write each case with an input that every
   other guard admits, and repeat step 3 for every guard you add or change.
Always verify claims by reading the code or running the command; do not assume.
```

Left out, with reasons: OP-2 (routes subagent reviews; a task has none), OP-3 and OP-4 (memory and
config rules; the frozen CLAUDE.md already carries the memory rule), and the codescout-specific
`./scripts/mutation-probe.sh` and `docs/PROBES.md`, which do not exist in a foreign repository. Steps
1 to 4 ask for extra test runs, so the R-block raises turns and cost. That is part of what `rules`
measures, and the reason `rules` exists.

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
- **Shared artifact effect.** One onboarding per repository makes artifact quality a repository-level
  effect that repetitions cannot average out. Analyse and report per repository (clustered), pool at
  least 4 repositories, and disclose that the number of independent artifacts equals the number of
  repositories.
- **Staleness.** Tasks newer than the oldest base commit see memories that predate their code. Record
  each task's commit distance from the artifact's commit and report it; do not refresh the memories.

## Carried fixes

`numTurns` must sum `num_turns` over all `result` records (bug file
`docs/issues/archive/2026-11-03-setup-ab-run-row-numturns-reads-only-the-last-result-record.md`;
done 2026-11-09 in `6f769dc55`, before this study's registration freeze); the
registration freeze for this study is where `run.py` may change.

## Decisions (controller's choices, pending operator validation)

Chosen by the controller on 2026-11-04 on the operator's delegation ("alege tu pt d1-d6 si raporteaza
pt validare"). Each stays open until the operator validates it. None authorises spend.

- **D1 Arms: `bare`, `rules`, `daily`.** No `cs` ablation for now; add it only if `daily − rules`
  shows an effect worth splitting into tools and instructions. Reason: without `rules` an effect of
  `daily` cannot be told apart from the effect of the rules; `cs` adds a fourth arm of cost for a
  question not yet asked. Cost of this choice: a claim about tools alone (`cs − bare`) is not
  available.
- **D2 Onboarding generator: Sonnet 5.5, CLI default effort, one model for every repository.**
  Per-repository cap proposed at USD 5 (`--max-budget-usd`), so at most about USD 20 for 4
  repositories. The cap is a placeholder, not a measurement: a dry run on the smallest pooled
  repository measures the real cost first, and spending it needs the operator's explicit go. List
  prices read 2026-11-04 (https://platform.claude.com/docs/en/models/overview): Sonnet 5.5 USD 2 in /
  10 out per MTok, Opus 5.5 USD 4 / 20, cache reads at 5% of the input price for both. Reason for
  Sonnet: half the price of Opus; disclosed cost: a possible same-model advantage for the Sonnet runs.
- **D3 Rules: the two-layer design in "Rules content" (R-block in `rules` and `daily`, frozen
  CLAUDE.md in `daily` only).** The R-block wording is the controller's draft; the operator rewrites
  or approves it. The mutation-testing rule is in it as steps 2 to 4, in generic form.
- **D4 No project-level CLAUDE.md for foreign repositories.** The repository's own CLAUDE.md, if it
  has one, is identical in every arm (parent §3 gate 4). Reason: inventing one per task adds an LLM
  call and an uncontrolled artifact; the onboarding artifacts already carry the project knowledge.
- **D5 The other frozen plugins stay in `daily`** (superpowers, buddy, legible, document-skills,
  frontend-design, claude-md-management), as in `full` (operator decision 2026-10-29: the setup of
  daily use). `chrome-devtools-mcp` stays excluded because it declares an MCP server. Cost: some
  plugins (legible changes the output style; document-skills and frontend-design add skill text) do
  not help a Rust task, so `daily` pays for context it does not use; that is what the daily setup
  does.
- **D6 Training cutoffs: RESOLVED.** Opus 5.5 and Sonnet 5.5 both list "Reliable knowledge cutoff:
  Jun 2026" and "Training data cutoff: Jun 2026" (https://platform.claude.com/docs/en/models/overview,
  read 2026-11-04). The pool therefore takes only tasks whose issue AND pull request date from
  2026-07-01 or later, and records both dates per task. Parent §9.2 is closed by this.

**Still needs the operator:** validation of D1 to D5, the R-block wording, the dry-run cost approval
(D2), and the pool itself (which repositories; the validated fresh candidates in memory
`research/harder-task-benchmarks-20261104` need a build-and-control stage first).

## Not changed

Same model, CLI version, prompt bytes, turn limit, resource limits and network in every arm; effort
equality gate; the F2P rules; the budget gate (cap, per-call, reserve) and the operator-only GO.json;
the rule that no spend starts without an explicit go.
