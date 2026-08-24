---
title: "The prime-radiant-inc eval stack, read against our hidden-information eval"
date: 2026-08-24
topic: evaluation
summary: "Source-level study of quorum, gauntlet, obol and serf against five problems in our hidden-information eval: fixture leakage, vacuous assertions, judge reliability, arm symmetry, statistics."
status: complete
---

# The prime-radiant-inc Eval Stack

## Key Takeaways

1. **They do not face our fixture-leakage problem at all.** All 85 scenarios use
   hand-authored fixtures of 5–35 lines; the largest fixture in the corpus is
   124 KB. Their task is behavioural, not a search. There is no imported
   solution — only three practices around the edges (§ 4) and one principle
   worth more than any of them (§ 4a).
2. **Their defence against vacuous assertions is the best thing in the stack**
   and is directly portable: a statically-extracted, git-committed manifest of
   the assertions each checker is expected to emit, compared as a multiset
   against what actually ran, with a mismatch composing `indeterminate` (§ 5.4).
3. **The judge is one LLM that both drives and grades** — no vote, no quorum,
   no deterministic post-check on the semantic criteria (§ 6.1). Our
   set-arithmetic verdict is stronger on this axis and should not be traded.
4. **Arms are not a harness concept.** An arm is which checkout of the subject
   got staged, and arm attribution is recovered post hoc from a `provenance`
   block in each verdict (§ 7). Confounds are controlled by interleaved run
   ordering and matched cross-arm backfill.
5. **Their statistical discipline exceeds ours**: committed pre-registration,
   a stdlib-only script that generates every quoted p-value and power figure,
   pre-registered cell classes, determinate-n floors, medians over means, and a
   published integrity ledger (§ 8).
6. **A worked example of an instrument faking a model-level finding** — a
   normalizer blindspot voided 30 cells and forced a same-day public retraction
   — is the most useful cautionary tale in either repo (§ 6.5).

Read from source on 2026-08-24 against the design in
`docs/superpowers/specs/2026-08-23-hidden-information-eval-design.md` (artifact
`556cc34167321863`). Clones live at
`/home/marius/work/claude/changelog-reader/sideprojects/`. Every path below is
relative to that directory unless it starts with `docs/` in a codescout context.

**Method note.** Everything stated as a mechanism was read in the file cited.
Where I am inferring from naming, structure, or a design document rather than
from running code, the sentence says so. Line numbers are given only where I
read them directly; elsewhere I cite file plus symbol.

---

## 1. What each repo is

| repo | what it actually is | activity (git log) |
|---|---|---|
| `superpowers` | The public skills repo we already use. `evals/` is gitignored; the eval harness is a separate clone. | last commit 2026-08-12, `Release v6.3.0` |
| `superpowers-evals` | Package name **`quorum`**. TypeScript on Bun. The *wrapper*: fixture setup, per-coding-agent provisioning, deterministic checks, verdict composition. ~7k lines in `src/` plus a ~170-file `test/` suite. | last commit 2026-08-19, credential/appliance scoping work |
| `gauntlet` | A general-purpose LLM QA framework, owned by `mhat`, lifecycle **production**, family `eval-labs` — *not* superpowers-specific. Drives a target through a `web` (Chrome/CDP), `cli` (stdin/stdout) or `tui` (tmux) adapter and returns a `pass`/`fail`/`investigate` verdict. Quorum shells out to it with `--adapter tui`. | last commit 2026-08-06 |
| `obol` | Rust. Cost accounting, nothing to do with evaluation logic. `obol-core` + `obol-cli` + `obol-ffi` (a cdylib with Go/Python/TS bindings) reads an agent transcript in one of two dialects (`obol` usage-sidecar JSONL, or ATIF `trajectory.json`) and prices it against LiteLLM/OpenRouter snapshots so every consumer yields a byte-identical `total_usd`. (`obol/ABOUT.md`, `crates/`) | last commit 2026-08-06, `v0.9.0` bundled price refresh |
| `serf` | **The brief is wrong about this one — the checkout is populated.** `git log -5` fails (`your current branch appears to be broken`) but `git log --all` works: 164 MB of `.git`, a full Go tree, tip `b21db7bd` dated 2026-08-23. Its `ABOUT.md` self-describes as **`evener`** (owner `obra`) — a non-interactive Go coding agent with `--sandbox` confinement, plus `llmcall` and a `hub` orchestrator. So `serf` is one of the coding agents *under test*, and appears to have been renamed to evener. | tip 2026-08-23 (most active of the five) |

**Do `gauntlet` or `obol` matter to us?**

- `gauntlet` matters as a *reference design for an LLM judge*, not as
  infrastructure. It is not a sandbox and not a tool protocol; it is an agent
  loop plus three input/output adapters. Its one genuinely portable part is the
  structural validation it imposes on the judge's own output (§ 6.3).
- `obol` does **not** matter to us. Its Rust-ness is a coincidence; it is a
  pricing library. We already take token counts straight out of Claude Code's
  JSON. The only idea worth a footnote is that both quorum and gauntlet emit a
  common `usage.jsonl` sidecar so cost is computed once, in one place, for
  every backend — a file-format contract, not a package dependency
  (`gauntlet/ABOUT.md` § How it fits).

---

## 2. How the eval framework actually works

### 2.1 The actors

`superpowers-evals/README.md` § Canonical Actors names four, and keeping them
straight matters for everything below:

- **Coding-Agent** — the subject under test (Claude Code, Codex, Gemini,
  Copilot, Kimi, OpenCode, Pi, Antigravity, Hermes, serf).
- **Gauntlet-Agent** — one LLM that *both* drives the Coding-Agent as a
  simulated user *and* grades the transcript against the story's acceptance
  criteria. Default model `claude-sonnet-5` (`src/run-all/options.ts`,
  `--grader-model` description).
- **Gauntlet** — the CLI hosting that agent.
- **Quorum** — the wrapper that owns setup, checks and the final verdict.

Two LLMs per run. The subject is not the judge, but the *driver* is the judge.

### 2.2 A scenario is a directory of exactly four files

All 85 scenarios have exactly `story.md`, `setup.sh`, `checks.sh`,
`checks-manifest.json`; two carry one extra file each (`codex.config.toml`,
`baseline-manifest.json`). Verified by a filename census over
`scenarios/*/`.

`scenarios/code-review-catches-planted-bugs/story.md` is representative:
YAML frontmatter (`id`, `title`, `status`, `tags`), then prose addressed to the
Gauntlet-Agent as a persona ("You just finished a refactor…"), a verbatim
message to type into the Coding-Agent, explicit *negative* instructions ("Do
NOT mention SQL injection, hashing, credentials"), a run-completeness rule
distinct from the pass criteria, and a `## Acceptance Criteria` list.

`setup.sh` is three lines: `setup-helpers run create_code_review_planted_bugs`.
The verb resolves through `src/checks/prelude.sh`, which is sourced before every
scenario script and defines one bash function per check verb by *reading the
verb vocabulary out of the TypeScript dispatcher*
(`bun run …/src/cli/list-check-verbs.ts`) so the DSL cannot drift from its
implementation.

### 2.3 The run pipeline

`src/runner/index.ts` (2080 lines) runs: setup → pre-checks → gauntlet drive →
capture → post-checks → compose. Per-agent differences live in two parallel
fan-outs keyed by agent name — `src/agents/<name>.ts` seeds config under a
throwaway per-run `$HOME` at `<run>/home`, and `src/normalize/<name>.ts` turns
that agent's session log into a uniform ATIF trace at `<run>/trajectory.json`
(README § Architecture). All live subprocess launches go through
`src/agents/command-runner.ts`, so the unit suite fakes CLI launches.

### 2.4 The verdict is an AND of two independent layers

`src/composer.ts`'s `compose()` is the whole decision, and it is short enough to
state exactly. In precedence order:

1. a staged `RunError` → `indeterminate`;
2. **any failed `pre()` check → `indeterminate`** (fixture integrity is not a
   subject failure);
3. no Gauntlet verdict, or Gauntlet status `investigate` / `errored` →
   `indeterminate`;
4. **capture empty and any `TRACE_PRIMITIVES` check ran → `indeterminate`**
   ("tool-call capture was empty; trace checks meaningless");
5. **emitted check records ≠ the frozen `checks-manifest.json` →
   `indeterminate`** with a `checks`-stage error;
6. Gauntlet `pass` **and** zero failed `post()` checks → `pass`;
7. otherwise → `fail`.

The deterministic layer can only *veto*; it can never rescue a run the judge
failed. The three-valued vocabulary is pinned in `src/contracts/verdict.ts`
(`FINAL_STATUSES = ['pass','fail','indeterminate']`, `RUN_ERROR_STAGES` with
eight named stages).

### 2.5 The deterministic check layer

Two verb namespaces, both dispatched in-process through `src/cli/check-tool.ts`:

- **FS/git/env verbs** — `FS_VERBS` in `src/check/dispatch.ts`: `file-exists`,
  `file-contains`, `command-succeeds`, `git-repo`, `git-branch`, `git-clean`,
  `git-count`, `requires-tool`, `baseline-manifest`, plus per-agent
  plugin-installed probes.
- **Transcript verbs** — `src/check/verbs.ts` / `transcript-dispatch.ts`:
  `tool-called`, `tool-not-called`, `tool-count`, `tool-before`,
  `tool-arg-match`, `tool-match-before-tool-match`, `skill-called`,
  `skill-not-called`, `skill-before-tool`, `skill-before-implementation-tool`,
  `implementation-tool-not-called`, `investigated`, `worktree-created`.

Every verb returns a `CheckOutcome`; `src/check/record.ts` is the sole emitter
of the `{check, args, negated, passed, detail, phase}` record.

### 2.6 The container and isolation model

`container/Dockerfile` builds `FROM ghcr.io/prime-radiant-inc/everyharness-container`,
a digest-pinned shared base image owning the install layer for every
coding-agent CLI plus Node/bun/uv/Rust/mise/Python/Go/Ruby; this repo's image
adds only serf, gauntlet and the `quorum` shim (`container/README.md`). Per-run
isolation is a throwaway `$HOME` under `<run>/home`, never the operator's real
`~/.<agent>`, credentials seeded `0600` and reaped afterwards
(`docs/adding-a-coding-agent.md` § Implementation Rules; `cleanupAgentRuntime`
in `src/runner/index.ts`). Concurrent runs share a filesystem — which is how the
cross-run leak of § 4c happened; per-VM isolation is proposed but unbuilt
(`docs/superpowers/specs/2026-08-12-quorum-overhaul-program-design.md:1121-1125`).

`fixtures/` at repo top level is 16 KB: one shared `template-repo` read by
`create_base_repo`. Eleven scenarios carry their own `fixtures/`, largest 124 KB.

**Worth copying?** Only the throwaway-`$HOME` discipline, and only if we add a
second backend. A digest-pinned base image is the right answer to "ten agent
CLIs, three OSes", which is not our problem.

### 2.7 How the harness drives each coding-agent CLI

A shared spine with four small per-backend seams. Per
`docs/adding-a-coding-agent.md`, a new backend costs:

1. `coding-agents/<name>.yaml` — 14 declarative lines. The whole of
   `coding-agents/claude.yaml`: `binary`, `home_config_subdir`,
   `session_log_dir` (templated on `${QUORUM_AGENT_HOME}`), `session_log_glob`,
   `normalizer`, `required_env`, `max_time`, `project_prompt`, `model`,
   `default_credential`, `os_support`.
2. `coding-agents/<name>-context/HOWTO.md` — **prose the Gauntlet-Agent reads**
   explaining how to launch the CLI, where the session log is, and how to tell
   the run is done. The interesting move: per-CLI operational differences are
   absorbed by an LLM reading English, not by code.
3. An optional `launch-agent` shell launcher pinning `HOME`/XDG/`TMPDIR`.
4. `src/agents/<name>.ts` — provisioning adapter (config seeding, auth-file
   copying, preflight, plugin staging), routed through `command-runner.ts`.
5. `src/normalize/<name>.ts` — session log → ATIF `Trajectory` rows. There are
   21; the transcript verbs read only the normalized form.
6. Registration in `src/agents/index.ts` plus a bootstrap scenario gated by a
   `# coding-agents: <name>` directive in `checks.sh`.

So: **shared interface, bespoke normalizer.** Cost is concentrated in the
normalizer and the provisioning adapter; the rest is config or prose.
`src/agents/command-runner.ts` as an injectable seam is why the unit suite
covers all ten backends without launching anything.

---

## 3. Answers to the tracker's open questions

Answering `docs/trackers/superpowers-evals-study.md` (artifact
`4c46865781a09c9e`) § *Open questions to investigate*, from source:

- **Contradiction TypeScript-vs-Python: resolved, and the tracker already had it
  right.** It is TypeScript on Bun. The Python/`uv` description in superpowers'
  own docs referred to the pre-migration `drill` era; a Python husk (`drill/`)
  was still being tracked as dead code as late as
  `docs/audits/2026-06-13-liveness-and-bitrot-audit.md` § D1, and the TS port
  landed through `docs/superpowers/plans/2026-06-12-quorum-ts-*.md`. No live
  Python remains except analysis scripts under `docs/experiments/`.
- **`quorum_tier` enumerates exactly `sentinel | full | adhoc`**, defaulting to
  `full`, validated in `src/story-meta.ts`'s `readQuorumTier` (a closed set;
  anything else throws `StoryMetaError`). It feeds `--tier` filtering in
  `run-all` and the `skipped_reason: 'tier'` cell in `GridManifestCell`
  (`src/contracts/grid-manifest.ts`).
- **"Quorum" is not a vote — confirmed a second way.** `gauntlet/src/fanout/`
  is the only thing in either repo that resembles multi-sampling, and reading
  `generator.ts` shows it is *scenario* generation (an LLM writes story-card
  variations, or promotes observations/failures into follow-up cards), not
  multi-judge grading. Grading is one agent, one verdict.
- **Verdict shape:** `FinalVerdictSchema` in `src/contracts/verdict.ts` —
  `{schema:1, final, final_reason, gauntlet:{status,summary,reasoning,run_id},
  checks:[CheckRecord], error:{stage,message}, economics, scenario,
  coding_agent, started_at, finished_at, credential, os, labels, provenance}`.
  The `provenance` block is the interesting field: `superpowers_rev`,
  `superpowers_dirty`, `harness_rev`, `agent_cli_version`, `gauntlet_version`,
  `host_platform` — *what was under test*, recorded per run.
- **`.gauntlet/context/` in quorum scenarios:** the mechanism is real
  (`gauntlet/README.md` § Context) but quorum uses the equivalent seam
  differently — `src/runner/context.ts` populates the Gauntlet-Agent's context
  dir with the per-agent `HOWTO.md` and the launcher shim. I found no
  scenario-local `.gauntlet/context/` directory in the corpus. *Inference from
  a directory listing, not from reading the population code end to end.*
- **Why `prime-radiant-inc` rather than `obra/`, and the public/private
  superpowers relationship:** still not determinable from source. `ABOUT.md`
  names owners obra/arittr/mhat for quorum and mhat alone for gauntlet and obol;
  `serf`'s ABOUT names obra. Nothing explains the org boundary.
- **Real `results/` artifacts:** only one stale run directory is present
  (`triggering-test-driven-development-antigravity-20260601T232434Z-d1ec`);
  `results/` is gitignored precisely because it can hold credentials and
  transcripts. No public eval-results repo exists. The schema is fully
  specified in `src/contracts/`, which I read; sample verdict JSON I did not.

---

## 4. Problem 1 — Fixture leakage

**Do they face our problem? No. Not even slightly.**

Their fixtures are hand-authored string constants of five to thirty-five lines.
`createCodeReviewPlantedBugs` (`src/setup-helpers/behavior-fixtures.ts:221-238`)
does `git init`, writes `PLANTED_PACKAGE_JSON` (`:171-177`) and `DB_INITIAL`
(`:180-193`), commits, overwrites with `DB_PLANTED` (`:197-217`), commits again
— a two-commit repo whose entire content is two files. The other fixture
families in that file (`CLAIM_*`, `PHANTOM_*`, `PUSHBACK_*`, `FINISHING_*`) are
the same shape. The largest per-scenario fixture in the corpus is 124 KB; the
shared `fixtures/` tree is 16 KB. There is no generator, no filler, and no
needle-in-a-haystack task anywhere in 85 scenarios.

That is not an oversight — it follows from what they measure. Their question is
*did the agent invoke the right skill and reach the right judgement*, on a repo
small enough to read entirely. Ours is *can the agent find twelve things among
fifteen thousand lines*. Search-difficulty is our independent variable and is
not a variable for them at all. **There is no imported solution to our
five-round leakage problem here.** Say that plainly.

Four adjacent things *are* worth taking:

**(a) The elicited-fixture rule — the closest thing to a principled answer.**
`docs/scenario-authoring.md` § *The elicited-fixture methodology*: for
scenarios where the agent executes a plan, **generate the fixture plan with the
skill under test; do not hand-write it.** Hand-authored prose plans execute
roughly 2× costlier than real `writing-plans` output and overstate the
baseline; the correction is recorded in
`docs/experiments/2026-06-10-sdd-cost-experiments.md`. The root cause they name
is exactly ours stated from the other side: *hand-authored artifacts differ
systematically from machine-produced ones in dimensions nobody enumerated in
advance, and that difference contaminates the measurement.* Their remedy is to
remove the provenance difference rather than to patch its symptoms one filter
at a time.

Applied to us: our truth sites should be emitted **through the same generator
path as the filler** — same naming policy, same docstring policy, same
annotation policy, same body-shape templates — and be truth sites by virtue of
*what the generated body computes*, not by virtue of having been written by a
different hand. Every one of the twelve leakage channels the brief lists
(filename pattern, file length, digit suffix, docstring presence, type
annotations, name uniqueness, class presence, float literals, token frequency,
parameter vocabulary, call-graph liveness, body AST shape) is a provenance
signal, not a semantic one. Fixing them individually is a losing race; removing
the provenance split ends it. **This is an analogy I am drawing, not a
mechanism they built for our problem** — but it is drawn from a documented,
measured correction of theirs, not invented.

**(b) `baseline-manifest.json` — content-addressed fixture pinning.**
`docs/scenario-authoring.md` § *Content-addressed fixture baselines*: schema
version 1, listing every file under `fixtures/` sorted by path with its git mode
(`100644`/`100755`) and SHA-256, plus named role paths. It is an *exact
inventory*: "unsafe or duplicate paths, symlinks, non-regular files, mode/hash
drift, missing files, and undeclared extra files all fail validation." It is
checked at **two** boundaries — statically by `quorum check` against the
checked-in tree, and at runtime by the bare `baseline-manifest` verb in `pre()`
against the seeded worktree (`verbBaselineManifest` in
`src/check/dispatch.ts`, delegating to `verifyBaselineTree` in
`src/scenario-manifest.ts`). Only `.git/` is exempt at runtime. One scenario
uses it: `scenarios/serf-builder-fractals/baseline-manifest.json`.

We claim byte-reproducibility from a seed. A manifest turns that claim into a
gate, and catches the case a seed does not: a generator change that silently
alters the fixture between the pilot and phase 2.

**(c) They were burned by an answer-key leak, at $650.** The 2026-08-06/07
overnight gate (352 runs) was **discredited** because, among thirteen defects,
"the agent under test could read its own scenario's `story.md` answer key" and
"`find /workspace` exposed other concurrent runs' trees"
(`docs/experiments/2026-08-08-fresh-release-gate.md:13`; detail at
`2026-08-06-dev-vs-main-overnight-gate.md:441`). Our spec's rule that
`ground_truth.json` ships outside the fixture tarball is the right instinct, and
this is a $650 confirmation of it. Their follow-on is stronger than a rule: a
**leak detector** that greps each run's transcript for paths outside its own run
root, with a *pre-registered adjudication rule* fixing in advance which
over-triggers count as self-references
(`2026-08-08-fresh-release-gate.md` § *Leak-flag adjudication rule*), flagged
runs auto-discarded and re-bought, and every kept and dismissed flag listed in
the read-out. I could not find this detector in `src/`; the greps for
`leak` there return only Copilot secret scanning and signal-handler comments, so
**the leak police appears to be an out-of-band driver script, not harness code.**

**(d) `pre()` as a fixture-integrity phase.** `checks.sh` in the planted-bugs
scenario asserts the planted content is actually present before the run:
`file-contains src/db.js '\+ email \+'` and
`file-contains src/db.js 'function hash\(s\) \{[[:space:]]*return s'`. A failed
pre-check composes `indeterminate`, never `fail`. This exists because a scenario
can silently rot: `codex-tool-mapping-comprehension` was **VOID on both arms**
for roughly six weeks, testing a mapping table deleted from superpowers on
2026-06-30, and nothing noticed until a judge disputed the premise
(`2026-08-06-dev-vs-main-overnight-gate.md`, offline pass). The prescribed fix
was "a pre-phase `file-contains` premise guard so future rot becomes a
deterministic preflight failure."

**Verdict on problem 1: nothing to adopt for the leakage mechanics themselves;
three concrete practices to adopt around them (b, c, d), and one principle (a)
that is the best argument I have seen for restructuring the generator rather
than adding a sixth filter round.**

---

## 5. Problem 2 — Vacuous assertions

**This is where they are strongest, and it is not close.** Guarding against a
check that passes for the wrong reason is a first-class, named concern
implemented at four independent layers. If we take one thing from this study,
take it from here.

### 5.1 The broken-check band (127) — a check that cannot be answered is not answered

`src/check/dispatch.ts`, header comment: "a broken/under-specified check returns
`{broken:true}`, which the CLI turns into exit 127 (non-invertible) so it can't
vacuously pass or be inverted by `not`." A 127 crashes the phase and composes
`indeterminate`, explicitly *not* `fail`
(`docs/scenario-authoring.md` § *Common authoring traps*: "Don't mistake it for
a real negative").

### 5.2 Arity and matcher-shape gates, with the vacuity spelled out

`src/check/transcript-dispatch.ts`, above `REQUIRED_ARGS`: "A missing arg must
NOT silently pass: e.g. `skill-before-tool <skill>` with no `<tool>` would set
`tool=""`, match nothing, and vacuously pass." And above the `tool-arg-match`
gate: "a missing or keyless spec parses to `{keys:[], expected:''}`, which
matches every call → silent pass. Reject it as a broken (non-invertible)
check."

That second one is our "a predicate matching 133 of 133 files", caught by
construction.

### 5.3 A per-verb vacuity audit, written down

Every negative transcript verb takes an `empty` flag and fails on an empty
capture: `verbToolNotCalled` (`src/check/verbs.ts:41-49`), `verbSkillNotCalled`
(`:157-168`), `verbImplementationToolNotCalled`, `verbSkillBeforeTool`,
`verbSkillBeforeImplementationTool`, `verbToolMatchBeforeToolMatch`. Positive
verbs take `_empty` and carry a comment saying why they don't need it — e.g.
`tool-arg-match` at `:433-438`: "This is a positive existence assertion, so it
naturally fails on an empty transcript (no call can satisfy it) — no empty-guard
is needed."

Above that, `src/composer.ts` forces `indeterminate` whenever capture was empty
and *any* member of `TRACE_PRIMITIVES` ran, so the guard survives an author
forgetting it. And the authoring guide names the three verbs that *legitimately*
pass vacuously (`skill-before-tool`, `skill-before-implementation-tool`,
`tool-match-before-tool-match` — "X before Y" is vacuously true when there is no
Y) and instructs pairing them with a positive verb.

This was learned, not designed: `docs/audits/2026-06-13-liveness-and-bitrot-audit.md:144`
— "C1 — negative-assertion check tools false-pass on empty capture" — records
`tool-not-called` and `skill-not-called` passing on an empty capture, "proven by
direct run", meaning every cost scenario asserting "the skill did not
over-trigger" had been falsely passing. The fix entry names the discipline
explicitly: "watched the two empty-capture tests fail RED first, then green."

### 5.4 The frozen expected-check manifest — the best single idea in the stack

Every scenario commits `checks-manifest.json` beside `checks.sh`: the frozen
multiset of check records `checks.sh` is expected to emit, as
`{phase, check, args, negated, count}`. It is produced by *static analysis of
the bash*, not by recording a run — `extractManifest`
(`src/check/manifest.ts:257-287`) tokenizes function bodies
(`tokenize` `:50-144`, `functionBodies` `:152-218`) and keys each line through
`toEntryKeyParts` (`:220-255`). At run time `compareRecords` (`:319-366`) does a
multiset match of expected against emitted; any `missing` or `unexpected` entry
composes **`indeterminate`** with `expected-check manifest mismatch (missing: … |
unexpected: …)` (`src/composer.ts`).

Two details that make it sound:

- **The extractor refuses to guess.** `UNMODELABLE_METACHARS`
  (`src/check/manifest.ts:41`) rejects `;`, `&`, `|`, `(`, `)`, `<`, `>` and
  backquote; unknown verbs, inline comments, line continuations and one-line
  function bodies also throw `ManifestExtractionError`. The header comment
  (`:1-17`) states why: "a silently mis-extracted check is exactly the false-pass
  this module exists to prevent."
- **Runtime-expanded args become wildcards, and wildcards cannot steal exact
  matches.** A line whose raw text contains `$` freezes with `args: null`;
  `compareRecords` consumes exact-args entries first.

The authoring loop is `edit checks.sh → quorum check --update-manifests → commit
both`, and `quorum check` fails on a missing or stale manifest. Stated in
`docs/scenario-authoring.md`: **"A check that silently stops emitting — the
classic false-pass — is now unrepresentable in a verdict."**

That sentence is the answer to our exec-bit incident, where a checker missing
its exec bit reported a clean `0/N` character-identical to a genuine floor.

### 5.5 Mutation-style discipline, required by the authoring guide

`docs/scenario-authoring.md` § *Designing a check that discriminates*: a check
must separate a correct fix from a plausible-but-wrong one, not merely "does it
work". The worked example is `systematic-debugging-fixes-root-cause`, where the
end-to-end price is correct under both a real root-cause fix and a consumer-side
guard, so the e2e check cannot discriminate; the discriminating check calls the
upstream producer directly. The instruction is explicit: **"hand-verify the
discriminator against all three states: broken, symptom-only, and root-cause. A
check that cannot fail the wrong-but-plausible fix is Pattern 4 waiting to
happen."**

And there is a *positive control on a detector* in the pre-registration:
`2026-08-08-fresh-release-gate.md:183` requires "TWO cells run concurrently
asserting each transcript shows no path outside its own root (**proves the leak
detector fires, not vacuous**)."

### 5.6 Self-test surface

`test/` holds ~170 files, including `check-manifest.test.ts` (pinned
corpus-acceptance and rejection censuses, referenced from `manifest.ts`'s header
comment), `check-tool.test.ts`, `check-transcript.test.ts`, `composer.test.ts`,
`composer-manifest.test.ts`, `prelude-drift.test.ts`,
`prelude-record-equality.test.ts`, and a `mock-gauntlet` for end-to-end runner
tests without a live judge.

**Verdict on problem 2: adopt. The frozen expected-check manifest and the
broken-check band are directly portable to a Python checker, and the per-verb
vacuity audit is a template for a one-page document we should write about our
own predicates.**

---

## 6. Problem 3 — Judge reliability

### 6.1 One judge, and it is also the driver

There is no vote. `--grader-model <id>` selects "the Gauntlet-Agent (grader)
model for every cell (default: `claude-sonnet-5`)"
(`src/run-all/options.ts`). The design document is blunt that fusing driver and
grader is a defect: **"A driver that knows the acceptance criteria leads the
witness toward graded behaviors"**
(`docs/superpowers/specs/2026-08-12-quorum-overhaul-program-design.md:248-252`).
The planned fix — W4, a runner/grader split with a rubric-blind driver, gated on
frozen-evidence grader parity and a rubric-aware-vs-rubric-blind canary
(`:943-963`) — is **design only, not implemented.** Fused mode is the current
reality.

Our verdict is deterministic set arithmetic. On this axis we are ahead of them,
and they know it.

### 6.2 What actually constrains the judge

Three things, all worth knowing:

**Structural output constraints, enforced deterministically.** `report_result`
must carry a `criteria` array with exactly one entry per acceptance criterion,
in order, each with a verdict from a closed enum and **non-empty evidence**
(`parseReportCriteria`, `gauntlet/src/agent/validators.ts:219-286`). A
cross-field consistency check refuses an overall `pass` that carries any `fail`
or `unclear` criterion — and deliberately does *not* enforce the converse, since
something outside the listed criteria can legitimately sink a run
(`checkCriteriaConsistency`, `:296-313`). A violation is fed back to the model
for correction up to `MAX_REPORT_VALIDATION_RETRIES = 2`
(`gauntlet/src/agent/agent.ts:25-29`, applied at `:439-455`), then falls back to
`salvageReportResult`, which will salvage a malformed *observations* sidecar but
never a malformed verdict.

**Prompt-level evidence rules.** `gauntlet/src/agent/prompts/evaluation.md`:
"Cite what you saw, not what you remember… A claim that something never happened
must cite the search you performed — the command you ran and what it returned —
not your impression. A criteria entry without real evidence will be rejected and
you will be asked to report again."

**A reflection checkpoint that grants permission to give up.**
`gauntlet/src/agent/reflection.ts` periodically injects a `<SYSTEM-REMINDER>`
containing a literal trace of the agent's own recent mutating tool calls, with
constant text (only the trace varies, deliberately, "so the agent's own action
history is what does the persuading rather than tonal escalation") saying
stories, fixtures and systems can be wrong and `investigate` is a valid answer.

### 6.3 How pass/fail criteria are actually written

Quoting `scenarios/code-review-catches-planted-bugs/story.md:36-54` in full,
because the specificity is the finding:

> - The agent loaded `superpowers:requesting-code-review` and dispatched a
>   reviewer subagent — a `Skill` invocation naming
>   `superpowers:requesting-code-review` and an `Agent` tool call appear in the
>   session log.
> - The reviewer flagged the SQL injection in findUserByEmail: the query
>   switched from a parameterized statement to string concatenation with the
>   user-supplied email. It must be called out at Critical or Important severity
>   — not Minor / Nice-to-have.
> - The reviewer flagged the credential-handling problem. Either counts:
>   (a) `hash()` is an identity function, so passwords are stored and compared in
>   plaintext; (b) `console.log` writes the password hash to stdout on every
>   successful login. At least one must be called out at Critical or Important
>   severity.
> - The reviewer did NOT approve the diff for merge. A correct reviewer says "do
>   not merge", "with fixes", "blocking", or explicitly refuses approval. A
>   reviewer that says "looks good", "ready to merge", or "safe to ship" without
>   qualification fails this criterion.

**How brittle does this look?** More robust than I expected. It names the exact
evidence and its location; it sets a severity floor so a "Minor: consider
parameterizing" does not count; it accepts either of two alternative
credential findings so a correct-but-differently-focused review is not
false-failed; and it closes the rationalization escape hatch by naming the exact
phrases that fail. `docs/scenario-authoring.md` § *Acceptance Criteria are
graded semantically by an LLM* codifies all four moves, and adds a fifth:
"Allows legitimate harness variants" — Claude loads a skill via a native `Skill`
call, Codex greps `SKILL.md` via the shell, and an AC demanding only the native
form false-fails Codex.

The residual brittleness is real and they name it. The listed anti-patterns are
"grading the agent's narration instead of its observable actions", "un-observable
ACs", "stop conditions tied to the verdict", and "over-fitting to one
implementation of a correct answer".

### 6.4 The deterministic post-checks beside it

For this scenario, `post()` is two lines:
`check-transcript skill-called superpowers:requesting-code-review` and
`check-transcript tool-called Agent`. The *semantic* judgement — did the reviewer
flag the SQL injection at the right severity — is entirely judge-owned. That is
the honest shape of it: the deterministic layer covers *mechanism* (a skill
fired, a subagent was dispatched, the fixture was right), and the judge covers
*quality*.

The strongest scenarios assert the same fact in **both** layers —
`docs/scenario-authoring.md` § *Belt-and-braces*: "The Gauntlet-Agent and the
post-checks are independent witnesses; agreement is a strong signal, disagreement
is a triage flag." The exemplar is `sdd-go-fractals-opus48`, whose ACs say the
build passes and the work is on the main checkout, and whose `checks.sh`
independently runs `command-succeeds 'go test ./...'` and
`git-count commits gte 4`.

### 6.5 The judge's failure mode, caught in the wild

`docs/experiments/2026-08-09-fresh-release-gate-readout.md` § CORRECTION is the
best real-world evidence in either repo about judge reliability, and it is a
*shared-instrument* failure rather than a judge failure per se. The `luna` codex
column read 0/9 on fractals, and the published "model findings" said luna never
completes. Seven hours later: every luna workspace built, passed tests and
rendered. Root cause — luna routes multi-agent calls through scripted exec cells,
`src/normalize/codex.ts` does not unwrap them, so every luna trajectory showed
zero `Agent`/`wait_agent` events. Sixteen wait-mapping, ten fractals and four
compaction cells were declared **VOID** because "the sole failing check is the
blind transcript verb; gauntlet passed on every one." And critically: "13 also
carry judge fails — **but judges read the same blinded transcript**, so all 20
need re-judging after the normalizer fix."

The lesson is not "LLM judges are unreliable". It is that **the judge and the
deterministic checks are only independent witnesses if they read independent
evidence** — and a shared normalizer makes them correlated. Our deterministic
checker reads the agent's `## FINDINGS` block; our tool-call metrics read the
trace. Those are independent. Our `native-tool-used` veto reads the trace, which
is the one place a normalizer blindspot could bite us the same way.

**Verdict on problem 3: our deterministic verdict is stronger than theirs and we
should not trade it. Adopt three things from the judge layer anyway — (i)
structural output constraints with deterministic post-validation and a bounded
re-ask, applicable directly to our `## FINDINGS` block; (ii) belt-and-braces
double-assertion of the same fact through independent evidence paths; (iii) the
evidence-independence caveat from § 6.5.**

---

## 7. Problem 4 — Arm symmetry

**They run comparative arms constantly, and arms are not a harness concept at
all.** Grepping `src/` for `arm`/`arms` returns two incidental matches in
`export-runs/index.ts` and `seats/types.ts`. There is no arm flag, no arm field
in `FinalVerdict`, and no paired-run machinery.

Instead, an arm is *which checkout of the system under test is staged*.
`stageSuperpowersPlugin` (`src/setup-helpers/plugin-stage.ts:71-78`) is "THE
single way to stage the Superpowers plugin into an agent's sandbox", copying
from `SUPERPOWERS_ROOT` into the throwaway home, resolving file symlinks to
their contents so the staged plugin is self-contained rather than "a web of
links back into `SUPERPOWERS_ROOT`". Arms are then two pinned SHAs
(`2026-08-06-dev-vs-main-overnight-gate.md` § Arms: CONTROL `origin/main`
`44c9b2d6…`, TREATMENT `origin/dev` `c367f804…`), and **arm attribution is
recovered post hoc from `verdict.json → provenance.superpowers_rev`**, never
from the invocation. Same doc: "Never pool with historical runs (pre-provenance
artifacts cannot be arm-attributed)."

That is a discipline worth copying verbatim: *the run records what it actually
ran against, and analysis reads that, not the operator's intent.*

Their confound controls, all from `docs/experiments/2026-08-08-fresh-release-gate.md`:

1. **Interleaved arms within the job schedule.** "Ordering: per rep index k —
   A-k(main), A-k(dev), C-k(main), C-k(dev), B-k(main), B-k(dev) — so truncation
   at any point leaves matched pairs with decision cells maximally complete."
2. **Matched-pair backfill.** Discarded runs are "re-bought in matched cross-arm
   pairs (keeps time-of-day symmetric)", capped at +20% of cell n, tagged
   first-attempt vs backfill in the read-out (`:113-120`).
3. **Arm-neutral invariant scenarios as an instrument control.** The cell grid
   lists `sdd-breaker-structural-blocks (arm-neutral invariant)` as a tripwire
   class — a scenario where the arms should not differ; a split there indicts the
   instrument. This is the closest thing they have to our noise floor, and it is
   structurally the same idea.
4. **Explicit exclusion of non-discriminating cells, with the reason recorded.**
   `codex-tool-mapping-comprehension` excluded as VOID; `cost-checkbox-over-trigger`
   excluded as "floored both arms 08-06, uninformative"; brainstorming-resists
   excluded on claude columns as "10/10 ceiling both arms — it cannot see the
   router change". Their read-out marks live cells the same way: "ceiling both
   arms", "model floor BOTH arms". This is our pilot gate conditions 1 and 4,
   applied per-cell rather than per-study.
5. **Same-limiter-pool serialization declared as a wall-clock cost, not a
   validity cost** — sol and luna share an OpenAI limiter key, "which costs wall
   clock, not validity."

And the single most transferable *cross-arm* lesson, from
`2026-08-06-dev-vs-main-overnight-gate.md` § Scoring discipline:

> Attribution endpoints are scored from tool-observation output only, never from
> agent prose — treatment arms echo skill text (e.g. `Ruling:`) into context, so
> a transcript full-text grep auto-passes treatment. This burned PR-2024.
> Exact-case `Ruling:` (3.5% base rate) and `Task <N>: BLOCKED` (2%) are usable
> anchors; `/ruling/i` (31.5%) and bare `BLOCKED` (95%) are confounded by main's
> own skill text.

**That is an arm-asymmetric vacuous assertion** — a checker that passes more
easily in one arm because that arm's own instructions put the matched token into
the transcript. It is the exact class our validity criterion is about, with
measured base rates for the confounded and unconfounded variants. Our analogue:
any checker predicate whose match probability differs between `hidden-cs` and
`hidden-native` for reasons unrelated to the agent finding the site — e.g.
scoring on a `path:symbol` string shape that codescout tool output emits
verbatim and `grep` output does not.

**Verdict on problem 4: adopt four things — post-hoc arm attribution from
recorded provenance; interleaved-and-matched run ordering; arm-neutral invariant
cells; and a written check that no scoring predicate is easier to satisfy in one
arm because of what that arm's tools print.**

---

## 8. Problem 5 — Statistical discipline

**Better than ours in analysis, weaker than ours in tooling.** This is the
section where I most expected to find nothing and found the most.

### 8.1 Replication

Yes, heavily — but **not in the harness.** `run-all` has no `--reps`/`-n` flag
(`src/run-all/options.ts` enumerates all twelve options; repetition is not among
them). Reps come from an external driver: 66 jobs generating 388 runs from four
job templates, "generated and selftest-asserted by `gate-driver.py` (session
scratchpad; dry-run prints every job with its full flag string; selftest proves
the generated cell grid equals this doc's table, 33 cells/arm)". Cell sizes
n=2/4/5/6/8/10 per arm.

Our `runs: 10` in a YAML arm is *better tooling*. Their compensating discipline
— a generator with a selftest asserting the grid equals the published table — is
a good answer to our own "`Summary: 1/1 passed` is the scenario count" trap.

`src/seats/aggregate.ts:152-154` carries a note we should internalize:
"Reps = runs. The rep is the randomization unit, so a rate over seats and a rate
over reps answer different questions and both are reported."

### 8.2 Pre-registration and power

`docs/experiments/2026-08-08-fresh-release-gate.md` § Pre-registration is
committed at freeze time before run 1, and states: "Every p-value and power
figure below is generated by `2026-08-08-fresh-release-gate-power.py`
(committed alongside; stdlib-only). **No hand-computed statistic may appear in
the read-out.**"

That script is 89 lines of stdlib Python: exact two-sided Fisher by summing all
tables with point probability ≤ the observed, exact power by summing binomial
products over the whole 2D outcome space, and exact Mann-Whitney minimum
two-sided p for complete rank separation. Its output tables are pasted into the
doc.

The pre-registration also fixes, in advance:

- **Cell classes with pre-committed interpretation:** C confirmatory (can move
  the verdict), P probe ("pre-registered underpowered; null reads 'unresolved',
  never 'no effect'"), T tripwire (colorless; fired → transcript investigation),
  D descriptive ("numbers only, no verdict language").
- **Decision rule**, including "A single surprising significant cell in either
  direction triggers replication before it colors any verdict; it never changes
  the same battery's verdict", and "No manual rescoring of fresh runs by
  instrument authors. Any rescore ships as a separately labeled secondary
  number, never in the headline."
- **Determinate-n floors**: n=10 cells report only at ≥8/arm determinate; n=8 →
  ≥6; n=6 → ≥5. "Below floor, the cell reads **UNDERPOWERED** regardless of
  split."
- **Honest limits stated before results**: "n=10 cells cannot resolve 20–30-point
  drifts (that needs n≥25/arm); 2/6 vs 6/6 is not significant; n≤4 cells reach
  significance only on a perfect split and are therefore tripwire-class by
  construction; the n=2 rider is statistically blind and exists only to catch
  collapse."
- **A "what this gate cannot answer" list attached verbatim to any GREEN.**

### 8.3 Medians, not means

`2026-08-06-dev-vs-main-overnight-gate.md` § Statistical correction retracts its
own pooled figure: "the +12%/run pooled figure is mix-confounded; the honest
central tendency is the matched-cell median **+3%** (IQR −12% to +32%, 40
cells). All fractals claude cost cells are n=1/arm — 'opus5 +28%' is a
single-run anecdote and a hypothesis, not a measurement. 'Biggest skill effect
ever measured on this corpus' is retracted as unverifiable."

Also: "Raw pass rates include unlatched 429s — post-filter indeterminates before
any rate is quoted." Their `indeterminate` state is what makes that possible —
an infrastructure failure never enters a denominator as a `fail`.

### 8.4 Controls and nulls

- **No null control in our sense** (byte-identical arms differing only in a
  nominal label). The nearest equivalents are the arm-neutral invariant tripwire
  cells (§ 7.3) and the floor/ceiling exclusions.
- **Positive controls exist as preflight smoke**: "one bootstrap cell per column
  asserting resolved refs == exactly `44c9b2d6`/`2d4b675b`, model ids from live
  runs (all four), est_cost_usd non-null and >0 on codex rows"
  (`2026-08-08-fresh-release-gate.md:183`), plus the leak-detector positive
  control in the same row.
- **An integrity ledger published with the results**
  (`2026-08-09-fresh-release-gate-readout.md` § Integrity ledger): 100%
  verification that the model actually run matched the credential pin, the full
  leak-flag adjudication with a sensitivity column showing "every delta is ≤1 run
  and same-verdict", and ops incidents with their lessons ("battery-critical
  state never lives in /tmp").

### 8.5 Published results

Yes — `docs/experiments/` holds 36 dated files, and `docs/baselines/` holds four
plus three per-agent sweep directories. They are internal, sanitized to the
repo's confidentiality rules ("Campaign inputs are not fixture data… After
trusted acceptance, follow the existing convention of a dated
`docs/experiments/YYYY-MM-DD-<topic>.md` note containing only release-reviewed,
sanitized conclusions. **Record failures at equal billing to wins so future work
does not repurchase disproven candidates.**"). There is no public results repo.

The 2026-08-09 read-out is a real, quotable comparative result: e.g.
`sdd-escalates-broken-plan` codex-sol 0/10 (main) vs 10/10 (dev), p<.0001;
`sdd-breaker-rules-and-continues` opus-4.8 0/10 vs 8/10, p=.0007; alongside
honest nulls (`codex-subagent-wait-mapping` sol 8/8 vs 8/8, "ceiling both arms";
luna 0/8 vs 0/8, "floor both arms") — and a seven-hour-later correction
retracting the model-level findings as instrument artifact.

**Verdict on problem 5: adopt. Our pre-registration practice is already the
right shape; theirs adds four things we do not have — a committed
statistic-generating script with a no-hand-computed-numbers rule, pre-registered
cell classes with fixed interpretation, determinate-n floors that print
UNDERPOWERED instead of a split, and an integrity ledger published beside the
numbers.**

---

## 9. What is not transferable, and why

- **The grading architecture.** One LLM that both drives and grades, versus our
  set arithmetic over a constrained answer block. Adopting theirs would be a
  regression, and their own design documents are moving the other way (§ 6.1).
- **The fixture philosophy.** Tiny hand-authored repos suit behavioural
  scenarios and are useless for a search task. Nothing in `setup-helpers/`
  generalizes to a 15k-line fixture.
- **The multi-backend spine.** Ten CLIs, 21 normalizers, a ~15 GB base image,
  credential axes, an appliance scheduler. We support one backend and a second
  is not on this eval's critical path — though the *shape* (declarative YAML +
  prose HOWTO + normalizer) is worth remembering if we ever do.
- **tmux driving.** Gauntlet's `tui` adapter exists because its subjects are
  interactive terminal UIs. We drive `claude -p` headless.
- **`obol`.** A pricing library; we already have token counts.
- **The bash check DSL.** `prelude.sh` + `check-tool.ts` is a lot of machinery
  so authors can write bare verbs in bash. The *manifest* transfers; the DSL
  does not.
- **Fanout.** LLM-generated scenario variations would be actively harmful: our
  ground truth is hand-verified and a generated variation has no answer key.

Things that look **worse than what we already have**, stated plainly:

1. Judge-based verdicts, fused with the driver. Ours is deterministic.
2. No in-harness replication. Ours has `runs: N` per arm.
3. No null control in the byte-identical sense. We just built one.
4. The leak detector appears to live outside the codebase, in an uncommitted
   driver script — which is why its over-triggering had to be adjudicated by a
   rule written mid-battery.
5. Their fixtures cannot answer a search question at all.

---

## 10. Recommendations, ranked by value to our eval

1. **Ship a frozen expected-check manifest for our checker.** Emit, alongside
   each scenario's checker, a committed manifest of the assertions it is
   expected to make (name, args, count), and have the run compare emitted
   assertion records against it; a mismatch is `indeterminate`, never a pass or a
   fail. This makes "the checker silently stopped asserting" — our exec-bit `0/N`
   floor, and several of the seven vacuous assertions — unrepresentable in a
   result. Model: `src/check/manifest.ts` + `src/composer.ts`.
   *(Problem 2. Highest value, lowest cost, portable this week.)*
2. **Add a broken-check band distinct from fail.** Any predicate that cannot be
   answered — missing argument, empty input where the assertion needs non-empty,
   unparseable answer block — returns `broken`, which composes as its own class
   and is never invertible. Pair it with a per-predicate vacuity audit: for each
   checker predicate, write down whether it can pass on empty input and why not,
   the way `src/check/verbs.ts` annotates `_empty` versus `empty`.
   *(Problem 2.)*
3. **Regenerate the truth sites through the same generator path as the filler.**
   Make a site a truth site by what its generated body computes, not by who wrote
   it. Every leakage channel we have patched is a provenance signal; removing the
   provenance split retires the whole class instead of the next filter.
   Cross-check afterwards with the same battery of mechanical filters, expecting
   them to find nothing rather than to find one less thing.
   *(Problem 1. Highest value if it works, highest cost, and it is an analogy
   from their elicited-fixture rule rather than a mechanism I can hand you.)*
4. **Add a `baseline-manifest.json` for the fixture and verify it at two
   boundaries** — once statically against the checked-in generator output, once
   at run start against the materialized tree, with exact-inventory semantics
   (extra, missing, mode drift, symlink all fail). Seeded reproducibility is a
   claim; this is the gate.
   *(Problem 1.)*
5. **Record provenance in every run result and attribute arms from it post
   hoc** — fixture hash, generator seed, codescout binary rev, model id,
   harness rev, tool-deny list actually passed. Then read the arm out of the
   result, never out of the invocation, and refuse to pool runs that predate the
   provenance field. Model: `FinalVerdict.provenance` in
   `src/contracts/verdict.ts` and the arm-attribution rule in
   `2026-08-06-dev-vs-main-overnight-gate.md` § Arms.
   *(Problems 4 and 5.)*
6. **Interleave arms within the run schedule and backfill in matched pairs.**
   Order runs `k(cs), k(native), k+1(cs), k+1(native)…` so a truncated or
   interrupted batch still yields matched pairs, and re-buy discarded runs
   cross-arm so time-of-day and API-load effects stay symmetric.
   *(Problem 4.)*
7. **Write the arm-asymmetry check into the scoring design, explicitly.** Before
   phase 1, state for each scoring predicate why its match probability does not
   differ between arms for reasons other than the agent finding the site — with
   the `Ruling:`-token incident as the worked cautionary example. Include the
   `native-tool-used` veto in that audit: it reads the trace, and a trace-format
   blindspot is exactly what voided thirty of their cells.
   *(Problem 4, and § 6.5.)*
8. **Commit a stdlib-only statistics script and forbid hand-computed numbers in
   the report.** Ours needs medians, IQRs, a paired rank test on tokens, and an
   exact test on per-band recall counts. Copy the constraint, not the code:
   `2026-08-08-fresh-release-gate-power.py` is 89 lines and every figure in the
   read-out comes out of it.
   *(Problem 5.)*
9. **Pre-register cell classes and determinate-n floors.** Label each cell
   confirmatory / probe / tripwire / descriptive before the first run, fix what a
   null in each means, and set an n floor below which the cell prints
   UNDERPOWERED rather than a split. This kills the "N=2 looked promising"
   failure mode structurally.
   *(Problem 5.)*
10. **Constrain the answer block structurally and post-validate it
    deterministically, with one bounded re-ask.** We already require
    `## FINDINGS`, one `path:symbol` per line. Add: a parse pass that classifies
    each line as canonical / normalizable / unparseable, a refusal on a block
    that is empty when the response text claims findings, and — if the harness
    permits — one re-ask on a malformed block before scoring it as
    `no-findings-block`. Model: `parseReportCriteria` and
    `checkCriteriaConsistency` in `gauntlet/src/agent/validators.ts:219-313`.
    *(Problem 3.)*
11. **Add a fixture-premise pre-check.** Before each run, assert the twelve truth
    sites and eight decoys are actually present in the materialized fixture, and
    make a failure `indeterminate` rather than zero recall. Their
    `codex-tool-mapping-comprehension` scenario was void for six weeks for want
    of exactly this.
    *(Problem 1.)*
12. **Publish failures at equal billing.** Their experiments directory records
    disproven candidates so nobody repurchases them, and the 2026-08-09 read-out
    carries a same-day correction retracting its own headline. Our
    `prompt-hamsa-audit-log` already does the pre-registration half; the
    retraction half is the part worth borrowing.
    *(Problem 5.)*

---

## 11. Gaps — what I could not determine

- **I did not read a real `verdict.json` or `trajectory.json`.** The one run
  directory present is from 2026-06-01 and I read the zod contracts instead.
  Everything I say about verdict shape comes from `src/contracts/`.
- **I could not find the leak detector in source.** `grep -i leak` over
  `superpowers-evals/src` returns only Copilot secret-scanning
  (`copilotCascadeVerdict` in `src/runner/index.ts:777-804`) and signal-handler
  comments. The transcript leak police described in the gate documents appears to
  be an external driver script that is not committed.
- **I did not verify the ATIF normalizers behave as described.** I read
  `docs/adding-a-coding-agent.md` and the module list, not the twenty-one
  `src/normalize/*.ts` bodies. The luna incident (§ 6.5) is reported from their
  read-out, not reproduced.
- **I did not run anything.** Read-only, per instruction. No `bun run check`, no
  `quorum check`.
- **The `prime-radiant-inc` org boundary and the public/private superpowers
  relationship remain unresolved**, as they were in the tracker.
- **`serf`/`evener`'s role is inferred** from `ABOUT.md` and its presence in the
  agent list; I did not read its Go source.

Ledger checked: none — no codescout bug files were consulted; this task audits an
external codebase, not codescout's own.


---


## 12. Addendum — answers from a peer session reading the same source (2026-08-24)

A second Claude session working in `changelog-reader` read `superpowers-evals` and
`gauntlet` directly and answered four questions this report left open. Everything below
carries its citation; where its read and this report's disagreed, the disagreement was
settled by reading the file, not by weighing the two agents.

### 12.1 The expected-check manifest — extraction is textual, and conditionals are forbidden

`extractManifest()` (`src/check/manifest.ts`) is **not** an AST walk or a decorator
registry. It is a line-oriented state machine over `checks.sh` as **raw text**: it tracks
brace depth to find the `pre(){…}` / `post(){…}` bodies and treats every non-blank,
non-comment line inside as a check-verb invocation, tokenised and validated against a
fixed verb registry. It throws `ManifestExtractionError` on anything structurally
unexpected — stray brace, unknown verb, content outside a function. Fail-loud, not
lenient. Each unique `(phase, check, negated, args)` key carries a `count`; a line whose
args contain `$` becomes a **wildcard** matched on phase+check+negated only. Output is a
git-committed `checks-manifest.json` — a static build artifact, not recomputed per run.

`compareRecords` consumes exact-arg entries first so wildcards cannot steal their slots,
then wildcards; anything unconsumed on either side becomes `missing` / `unexpected`, and
`compose()` turns **any** non-empty list into `indeterminate` — never partial credit.

**The answer to "how does it handle a legitimate conditional skip" is that it cannot
arise.** The extractor never evaluates bash; an `if` line would fail tokenisation as an
unknown check verb. **The DSL structurally forbids conditionals in phase bodies**, so a
check that only sometimes runs cannot be expressed, and every verb is expected exactly as
many times as it appears textually. That is the design: remove the branching rather than
model it.

`indeterminate` is first-class end to end — `FINAL_STATUSES = ['pass','fail','indeterminate']`
in the verdict schema, its own glyph and count bucket in the batch runner, its own
`SlotKind` and CSS class in the dashboard. One exception: `packages/dashboard/src/view.ts`
buckets it with `unknown` under a display label `incomplete` in the header tally only.

**What this means for us.** A straight port fails — our checkers are Python with real
branches, so there is no `checks.sh` analogue to scan statically. But the failure mode we
need to kill is narrower than theirs: **a checker that silently stops emitting.** Ours
already write a fixed six-line facts block per run (`VERDICT`, `TOKENS`, `PROMPT_PER_TURN`,
`COST_USD`, `TOOLS`, `GUIDES`), unconditionally. So the portable core is: commit the
expected key multiset, compare it against what each run's log actually contains, and
compose `indeterminate` on mismatch. No static extraction needed, because the emission
contract is already unconditional. It must survive `score_arm.py`'s re-scoring path, which
re-runs checkers over logged text with the token env stripped.

### 12.2 The $650 answer-key leak — verified verbatim, and its control is not in the repo

The peer could not find this incident and asked for a pointer. Read directly:
`superpowers-evals/docs/experiments/2026-08-08-fresh-release-gate.md:10-14`.

> "The 2026-08-06/07 overnight gate (352 runs, $650, verdict GREEN) is discredited as a
> release gate: its codex column silently ran gpt-5.5 …, **the agent under test could read
> its own scenario's `story.md` answer key**, and `find /workspace` exposed other
> concurrent runs' trees. Full defect list: `2026-08-06-dev-vs-main-overnight-gate.md`
> (13 defects, 3 result-invalidating)."

Followed by: *"nothing from the old run is used as gate evidence. There is no lean
option."* The old corpus survives only for instrument calibration and one other
non-evidence purpose.

It sits in `docs/experiments/`, not `src/` or `docs/superpowers/specs/`, which is why a
source-tree search misses it. **The asymmetry is the finding**: the incident is
documented, but greps for `leak` in `src/` return only Copilot secret scanning and
signal-handler comments. Their leak detector is an out-of-band driver script, so their
strongest contamination control is the least reproducible thing they have.

### 12.3 EXECUTED vs TESTED — a name for the failure this task hit eight times

From evener's `make coverage-floor` (`docs/developing-evener/coverage.md`): a per-module
ratchet against a committed `coverage-floors.txt`, unioning the imperative suite's
`-cover` output with a **deterministic replay of committed fuzz seed corpora** (`go test`
without `-fuzz` — no live fuzzing, no provider calls). `CHECK=1` fails when a row drops
past a tolerance band **or when a floored row cannot be measured at all**; `BLESS=1`
re-baselines.

The distinction is concrete, not vocabulary. `cmd/evener-hub/cov_*_test.go` are replay
matrices that call production code and discard the result (`_ = f(x)`); their only oracle
is a crash, panic or `-race` failure. They earn coverage-floor credit **without proving
correctness** — EXECUTED. Upgrading one call site to assert against an independently
written literal converts that call site alone to TESTED, with no obligation on the rest of
the file.

That is the precise name for what bit this task eight times. Every one of our vacuous
assertions was EXECUTED-not-TESTED wearing an assertion's clothes: a test that called
`build()` which writes no JSON; `assert a != b` between sets that always differ; a
predicate matching 133/133 files; length-overlap counts that passed under the mutation
they were restored to catch; a false-positive count excluding decoys while the precision
beside it included them; and the guard itself passing green on an emptied predicate space.
Naming the two states separately is what makes the gap visible before it costs a round.

### 12.4 Per-claim cited evidence, and an asymmetry to copy exactly

`report_result`'s schema (`gauntlet/src/agent/agent.ts:197-267`) requires
`criteria: [{criterion, verdict ∈ {pass,fail,unclear}, evidence}]`, with `criteria` itself
required only when the scenario lists acceptance criteria. **Evidence non-emptiness is
enforced in code, not merely described** (`src/agent/validators.ts:270-277`):
`entry.evidence.trim() === ""` is rejected with *"must be a non-empty string quoting what
you observed (screen text, file content + path, log line, or command output)."*

`checkCriteriaConsistency` (`validators.ts:296-313`) is **deliberately asymmetric**: an
overall `pass` requires **every** criterion to be `pass`, but an overall `fail` or
`investigate` with all-passing criteria is legitimate — something outside the listed
criteria can still sink a run. If our fixture guard's worst-surviving-shortcut read-out
ever grows per-claim verdicts alongside its overall one, encode that asymmetry rather than
demanding agreement in both directions.

A UX lesson from a comment above the schema, worth stealing: property order is
deliberately `observations` before `reasoning`, because **models emit object properties in
schema order** and a long escape-heavy `reasoning` blob had previously pushed
`observations` into being string-wrapped instead of emitted as a real array (PRI-1528).

### 12.5 The timing ratchet, with the caveat that matters more than the mechanism

`make test-timing-budget` checks per-package wall time against a committed
`testing-budget.json`: **fails at 1.5×** a package's budget, **warns at 1.1×**, plus a flat
**3-second per-test ceiling** regardless of package. `make test-rebaseline` re-measures.

**It is not wired into CI.** Enforcement activates only under `CHECK=1` in a CI-shaped
environment, and it is deliberately excluded from their merge gate because measuring
durations requires a second full non-fuzz `go test -json` run — doubling the gate runtime
the ratchet exists to protect. They have an open follow-up to source durations from the
gate's own run instead. Anyone porting this should solve that first rather than discover
it later.

### 12.6 ATIF's disjoint-bucket rule — the highest-value line to steal

`ATIF-v1.7` (`src/atif/types.ts`) is quorum's **in-house** trajectory format, not an
external spec, though genuinely shared: evener emits it natively (`--export-atif`), obol
consumes it for pricing, and Harbor (Terminal-Bench) is a third-party contender pinned to
v1.6 whose summed/inclusive token model quorum explicitly refuses.

The rule worth taking verbatim, independent of the envelope: **`prompt_tokens` = UNCACHED
input only; `cached_tokens` = cache-read; `extra.cache_write` = cache-creation;
`completion_tokens` = output. A converter emits per-step `metrics` OR `final_metrics`,
never both** — a hybrid silently drops buckets, which they call *"the copilot at 1k bug"*.
And **never fabricate `cost_usd`**: leave it unset when the source log does not record one.

We report cache-inclusive prompt tokens as the primary surface metric, so this is a
concrete audit item for `collect_facts` before any number is published.
## Sources

All read from local clones under
`/home/marius/work/claude/changelog-reader/sideprojects/` on 2026-08-24,
read-only. Tips as observed:

- **prime-radiant-inc/superpowers-evals** (package `quorum`) —
  <https://github.com/prime-radiant-inc/superpowers-evals> — tip `339779f`,
  2026-08-19. Primary source for §§ 2, 4, 5, 7, 8.
- **prime-radiant-inc/gauntlet** —
  <https://github.com/prime-radiant-inc/gauntlet> — tip `91b6f7e`, 2026-08-06.
  Primary source for § 6.
- **prime-radiant-inc/obol** — <https://github.com/prime-radiant-inc/obol> —
  tip `28e3dba`, 2026-08-06 (`v0.9.0`).
- **prime-radiant-inc/serf** (self-described as `evener` in `ABOUT.md`) —
  <https://github.com/prime-radiant-inc/serf> — tip `b21db7bd`, 2026-08-23.
- **obra/superpowers** — <https://github.com/obra/superpowers> — tip `b36e082`,
  2026-08-12 (`v6.3.0`).

Internal prior art this report answers or corrects:

- `changelog-reader:docs/trackers/superpowers-evals-study.md` (artifact
  `4c46865781a09c9e`) — the second-hand tracker whose open questions § 3
  addresses.
- `codescout:docs/superpowers/specs/2026-08-23-hidden-information-eval-design.md`
  (artifact `556cc34167321863`) — the design this report is read against.
