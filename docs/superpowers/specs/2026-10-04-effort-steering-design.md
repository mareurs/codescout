# Effort Steering: dynamic thinking-depth control

**Date:** 2026-10-04
**Branch:** experiments
**Status:** Design approved by the operator (sections 1 to 3, in conversation). Implemented in two phases in the `claude-plugins` repo (first phase and the hook adapter are on `origin/main`). Status as of 2026-10-07: the live work list is the tracker `effort-steering-second-phase` in that repo (`docs/trackers/effort-steering-second-phase.md`, id `af4ccc8b94a95c91`) and the passover `ec8f1f91d436b653`. S4 ran twice on 2026-10-07 (second registration `mechanism_pass`); the verdict is held for a quality check. Two parts of this design have been overtaken by evidence: a Claude Code mod can set effort per request (so the opt-in settings write that the stop rule names as the fallback is not needed), and the steering sentence only has room to work where effort buys thinking (at `xhigh`, not at `medium`).

## Purpose

The operator wants thinking depth to change per turn, automatically, in Claude Code. The design must stay generic enough for other harnesses.

The goal has two parts:
- Cut cost and latency on routine turns.
- Raise quality on hard turns.

Constraints, as the operator stated them:
- Do not depend on `llm-proxy`. Other people do not have that infrastructure.
- The first version must work without any classifier model.

Assumptions, not stated by the operator:
- The deliverable is a shareable component, not a personal tweak.
- It must work on stock Claude Code, with no `ANTHROPIC_BASE_URL` rewriting.
- "Dynamic" means the depth changes per user turn, not only at launch.

## Research summary

Four research subagents covered four angles: Claude Code, the Anthropic API, other harnesses and programmatic control. The `researcher` MCP failed or returned thin results for three of them. Those three used WebFetch on vendor documents. Most claims therefore rest on one source. Subagent reports are model output. Version numbers come from summaries of the pages.

Findings the design relies on:
- On Claude 4.7 and later models, a token budget (`budget_tokens`, `MAX_THINKING_TOKENS`) is not the control. Depth is a soft `effort` hint, and the model decides how much to think.
- The API does not allow a thinking change inside a turn. A tool loop is one turn. The first request after new user input carries most of the reasoning.
- A change of effort or thinking mode between requests invalidates the prompt cache. Anthropic's docs show `cache_read` dropping to 0 after such a change.
- Anthropic's docs suggest appending steering text to the newest user message. This keeps earlier cache breakpoints. The docs name two phrases: "Please think hard before responding." and "Answer directly without deliberating."
- Claude Code hooks have no output field for effort, model or thinking. They can read the effort level. `UserPromptSubmit` can add `additionalContext` or block.
- The one hook-based implementation found, `claude-code-auto-effort`, writes `effortLevel` to `.claude/settings.local.json`. Its evidence has n=1 and measures token volume only.
- No source describes a true per-turn automatic effort in the other harnesses that were checked.

I verified one fact in this repo's own data. Claude Code transcripts carry `usage.output_tokens_details.thinking_tokens` and the cache token fields on assistant records.

## Decisions

### D1: A harness-neutral core and one adapter per harness

- **Decision:** The core holds `policy.decide`, the rules and `steer.render`. The core knows nothing about Claude Code. One adapter per harness builds the context and applies the result.
- **Context:** The operator wants a design that is generic across harnesses.
- **Alternatives considered:**
  - A Claude Code hook only. Rejected: it gives no path to another harness.
  - A full multi-harness abstraction layer now. Rejected: no second adapter exists yet, so the abstraction would have one user.
- **Consequences:**
  - Now easier: a new harness needs only one adapter.
  - Now harder: the adapter must translate its transcript format into a neutral context.
- **Change scenarios absorbed:** a new harness, a retuned heuristic, new steering phrases, a v2 classifier.
- **Revisit when:** a second adapter exposes a missing field in the context.
- **Confidence:** medium.

### D2: Steering text is the first actuation channel

- **Decision:** The Claude Code adapter appends a steering sentence through `additionalContext`. It does not write the effort level to a settings file in v1.
- **Context:** A settings write is shared state. `settings.local.json` is per project, and the operator runs several sessions on one checkout. A write also changes the effort value, which probably costs a cache miss on each change.
- **Alternatives considered:**
  - A settings write first. Rejected: it clobbers across sessions, drops `max`, and its timing is unconfirmed.
  - Both channels from the start. Rejected: it carries both sets of risk.
- **Consequences:**
  - Now easier: no shared state, and a cache-friendly lever.
  - Now harder: the lever is soft. Spike S4 must show that it moves thinking depth.
- **Change scenarios absorbed:** a stronger channel is added later behind the same decision output.
- **Revisit when:** spike S4 shows no movement.
- **Confidence:** medium.

### D3: The policy is hybrid. v1 uses rules. v2 adds an optional classifier.

- **Decision:** v1 ships deterministic rules and a decision log. A classifier provider is an optional v2 tier. It is consulted only when the rules return low confidence. The component must work with no classifier.
- **Context:** The operator chose a hybrid with a Jev-class model. The repo already runs a local Jev effort. That effort is a different task, and both its phases stopped at their gates (see Jev section). No labels for "the right effort" exist.
- **Alternatives considered:**
  - Rules only, with no classifier path. Rejected: the operator wants the hybrid.
  - A classifier in v1. Rejected: it blocks on labels that do not exist.
  - A Haiku classifier per prompt. Rejected for v1: the reference implementation adds about 4 to 7 seconds per prompt and needs credentials.
- **Consequences:**
  - Now easier: v1 ships to people without a GPU or model.
  - Now harder: v1 rules have no ground truth until the log produces data.
- **Change scenarios absorbed:** swapping the classifier backend, or running without one.
- **Revisit when:** the log and an adjudicated set exist.
- **Confidence:** medium.

### D4: The decision vocabulary is `shallow`, `default`, `deep`

- **Decision:** The core returns one of three values. Each adapter maps them to its own levels.
- **Context:** Harnesses use different effort levels. Steering text is relative, not absolute.
- **Alternatives considered:** the API levels `low` to `max`. Rejected: they tie the core to one vendor.
- **Consequences:**
  - Now easier: fewer classes mean easier labels for v2.
  - Now harder: a harness with finer levels loses resolution.
- **Revisit when:** a harness needs more than three values.
- **Confidence:** medium.

### D5: A random holdout measures the effect

- **Decision:** For a fixed share of turns, the adapter decides but does not steer. It logs `arm: holdout`. Other turns log `arm: treated`.
- **Context:** No published evidence shows that effort heuristics help. The only implementation found has n=1.
- **Alternatives considered:** a before and after comparison. Rejected: adaptive thinking and task mix confound it.
- **Consequences:**
  - Now easier: a causal estimate of the steering effect on `thinking_tokens`.
  - Now harder: some turns are deliberately unsteered.
- **Revisit when:** the effect size is known.
- **Confidence:** high for the method. The effect size is unknown.

### D6: A standalone plugin

- **Decision:** The component is its own plugin in the `claude-plugins` repository. It is not part of `codescout-companion`.
- **Context:** Effort steering is not specific to codescout. The operator approved this proposal.
- **Alternatives considered:** a module inside `codescout-companion`. Rejected: it would tie a generic tool to a codescout plugin.
- **Revisit when:** the plugin repository structure makes a separate plugin costly.
- **Confidence:** medium. I have not checked the plugin repository layout beyond two plugins.

## Architecture

| Component | Responsibility | Depends on |
|---|---|---|
| `policy.decide(ctx)` | Pure function. Context in. `{level, confidence, reasons[]}` out. Low confidence means "ambiguous". | Nothing |
| `rules` (data file) | The tunable heuristics. A change needs no code change. | Nothing |
| `steer.render(level)` | Returns the sentence to append, or nothing at `default`. | Nothing |
| `adapter/claude-code` | A `UserPromptSubmit` hook. It builds the context, calls the core, emits `additionalContext` and appends the decision log. | Core only |
| decision log | One JSON line per prompt. | Nothing |
| `effort-policy` CLI | `decide < ctx.json > decision.json`, so any harness hook can call the core. | Core only |

Dependencies point from the adapter to the core. Only the adapter parses the transcript format.

Language: Node ESM, as in the existing companion hooks and their tests.

## Rules

Inputs, built by the adapter:
- Prompt features: length, question or command, planning or debugging words, a pasted stack trace, a `/skill` call, and short confirmations.
- Session signals: tool errors or a test failure in the previous turn, a retry count, plan mode and the turn index.
- User override: an explicit "ultrathink" always wins. The core adds no second sentence.

Behavior:
1. An explicit user signal wins over a rule. A rule wins over `default`.
2. Any error, timeout or low confidence returns `default` with no steering.
3. `shallow` needs high confidence and no failure signal in the previous turn.
4. `deep` may fire at medium confidence.
5. Each rule declares its own confidence class (`high`, `medium` or `low`) in the rules file. Confidence is a class that a rule declares. It is not a probability.
6. When two rules disagree, the rule with the higher confidence class wins. A tie between `shallow` and `deep` returns `default`.

Rules 5 and 6 are a proposal from the spec self-review. The operator should confirm them or replace them.

Rules 3 and 4 encode a cost asymmetry. A wrong `shallow` costs answer quality. A wrong `deep` costs only tokens. This is a judgment with medium confidence.

Steering text in Claude Code (v1 seeds):
- `shallow`: "Answer directly without deliberating."
- `default`: nothing.
- `deep`: "Please think hard before responding."

The first rules come from the offline replay (spike S3). The operator reviews them before they are frozen.

## Decision log

The adapter appends one JSON line per prompt to a per-session file in the user's state directory. The existing `lib.mjs` has `guideLedgerPath`, which uses `xdgStateHome`. The log follows that convention.

Fields: timestamp, session id, turn id, harness, versions of the rules and the steering table, decision, confidence, reasons, features and `arm`.

The log holds a prompt hash and features. It never holds prompt text. One file per session avoids a clash between concurrent sessions.

## Measurement and gates

Cost outcomes come from the transcript: `thinking_tokens`, output tokens and the cache fields. Quality outcomes in v1 are weak proxies only: retries, tool failures and user corrections in the next turns. True quality needs an adjudicated set. That belongs to v2.

Gates, in the preregistration style of this repo. The numbers stay open until the operator and the author register them, before any run:
- **Mechanism:** treated turns must move `thinking_tokens` in the intended direction against holdout, by a registered effect size.
- **Cache:** the share of `cache_creation_input_tokens` must not rise.
- **Latency:** the hook overhead must stay under a registered budget.
- **Stop rule:** if steering text moves nothing, actuation A fails. The project then stops, or it reopens the opt-in settings write.

## Testing

Core:
- Each rule has a positive case and a near-miss negative case. The negative text resembles another rule's cue. This applies the phase-1 lesson, where heads trained without cross-rule negatives fired on 39% of other rules' texts.
- Tests cover precedence, fail-open behavior and the `shallow` guard.
- Holdout tests check that the same session and turn always give the same arm, and that the holdout share is close to the registered rate.
- Mutation lens: the hash key and the `shallow` guard are the discriminating logic. The suite must fail when a mutation drops the session or the turn from the key, or drops one guard condition.

Adapter and CLI:
- Contract tests with recorded hook-input fixtures. The companion plugin already uses this pattern in `explore-inject.fixtures.jsonl`.
- A schema test for the CLI standard input and output.
- Log tests: no raw prompt substring in the log, one file per session, one line per append.

## Spike plan

Each step needs the operator's approval before it touches any configuration. Sample sizes, prompts and metrics are registered before any costly run.

| Step | Question | Cost |
|---|---|---|
| S1 | Which fields does the `UserPromptSubmit` input carry? | Low |
| S2 | Which message-id field dedupes transcript usage, and does a transcript record the effort level? | Low, read-only |
| S3 | What does the offline replay over the existing transcripts show, and which first rules does it suggest? | Low, read-only |
| S4 | Does injected steering text move `thinking_tokens`, against no steering? | Medium: model calls, with repeats |
| S5 | Does injected text change `cache_creation_input_tokens`? | From the S4 runs |

Order: S1, S2, S3, then S4 and S5.

## The Jev track

Jev is TypeSafe.ai's fast classifier. It returns calibrated probabilities over a fixed set of answers (`bool`, `score` or `choice`). The 100 to 300 ms latency figure comes from a promotional video and is not verified.

The repo runs it locally: `alibiserikbay/JevK5`, code `allebee/jevk5` 0.2.2, in a separate `uv` venv on an RTX A5000. The repo's effort fine-tunes a local model for a different task: choosing which of 22 rules applies to a text.
- Phase 1 failed its gate at Stage 4 (2026-09-25).
- Phase 1b: no checkpoint passed its gate on 2026-09-27. The registered reading is that the local route stops for that data and backbone.
- Later status is not checked.

What transfers: the runtime, the LoRA path, the preregistered gates, the determinism checks and the lesson on cross-rule negatives. What does not transfer: a trained model and labels for effort.

v2 plan: the decision log and the holdout arms become labeled data. The classifier track gets its own preregistration and its own gate. The provider interface is documented only. It gets code when the first backend exists.

## Unverified claims register

Verified by the spikes (results in `docs/research/2026-10-04-effort-steering-spike-findings.md`):
- The `UserPromptSubmit` input carries `session_id`, `transcript_path`, `cwd`, `prompt_id`, `permission_mode`, `hook_event_name` and `prompt`, all strings (S1, one scratch run). It carries no effort level.
- The transcript dedupe key is `message.id`, and a transcript records the effort level as a top-level `effort` field on assistant records (S2).

Still unverified:
- Whether steering text injected through `additionalContext` moves thinking depth. The `ultrathink` variant is also unconfirmed.
- Whether `additionalContext` is cache-neutral.
- How Claude Code orders or merges the `additionalContext` of several hooks. `codescout-companion` already registers a `UserPromptSubmit` hook. S1 ran with user-level plugin hooks switched off.
- Whether `UserPromptSubmit` fires for turns that peer messages, task notifications or slash-command records trigger. About 25% of the thinking tokens in the S3 data follow such records.
- What `perTurnEffort` means. It is non-null on 21,275 of 86,955 usage records.
- The possible values of `permission_mode`, and whether it gives the plan-mode signal.
- Whether the session signals (previous-turn failure, retries, plan mode) separate thinking depth. S3 tested prompt features only.
- Whether the other harnesses expose a prompt hook that can inject context.
- Jev latency, and Jev performance on effort selection. Neither was measured.
- Phase 1b status after 2026-09-27.
- If the settings write is reopened: the timing of the `settings.local.json` hot-reload.

## Constraints carried into the second plan

These come from the per-task and final reviews of the branch `feat/effort-steering-core` in the `claude-plugins` repo (2026-10-04). The scratch ledger that first held them is a gitignored file in a worktree that will be deleted, so they are recorded here. The review notes are in `docs/research/2026-10-04-effort-steering-branch-review-notes.md`.

Adapter:
- The adapter must put a wall-clock timeout around the `effort-policy` CLI and treat a timeout as `default` with no steering. The CLI has no timeout of its own. A rules file with a catastrophic regex, or a FIFO passed as `--rules` or `--table`, can block it. Reading stdin blocks until the caller closes it.
- The adapter must always exit 0 from the hook. In Claude Code an exit code of 2 from a `UserPromptSubmit` hook blocks and erases the user's prompt. The CLI exits 64 for a usage error, so an adapter that passes the exit code through cannot block a prompt by accident.
- The adapter must not derive `turn_id`, `harness` or `session_id` from prompt text. The log writes them as given and does not cap their length. S1 found the hook input fields `prompt_id` (usable as `turn_id`), `session_id`, `transcript_path`, `cwd` and `permission_mode`.
- A CLI usage mistake is now visible: an unknown flag gives `unknown_arg`, and a value flag without a value gives `bad_holdout_rate`, `rules_unreadable` or `table_unreadable`. The adapter should log and alert on any of these reasons during rollout, because each can silently change the experiment.

Rules (Task 11 is held):
- Task 11 (the first rules) stays held until spike S4 or a session-signal replay gives a basis. `core/rules.json` ships empty.
- Review every regex in a future `rules.json` for nested quantifiers. The compile probe does not catch all of them: `(b+)+c` passes it and took 11.5 s on a 30-character prompt.
- The runtime slow-regex probe uses wall-clock time. Under CPU starvation (192 busy loops on 64 cores, node at nice 19), 27 of 1,500 benign regexes were skipped as `slow_regex`. Once rules ship, decisions could become nondeterministic, and a partly skipped file logs the whole file's version. Surface skipped rule ids in the log (for example a `rules_skipped` reason). Check the shipped file at test time (the Task 11 test asserts that `skipped` is empty). Keep the runtime probe for `--rules` overrides only.
- Each regex that is slow costs about 50 ms of compile probe on every call.

Build and repository:
- `tests/run-all.sh` finds `effort-steering/*/*.test.mjs`, one level deep only. A test under `effort-steering/adapter/claude-code/` would never run, and the runner would still print "All suites passed". Use a recursive glob before the first adapter test, and assert the population.
- Delete or exclude `effort-steering/spikes/` before `.claude-plugin/plugin.json` lands. The spikes hard-code machine-specific paths into three Claude Code profiles, and they would ship into every install cache. The findings document cites their commits.
- Rulings still open for the operator: a bad `--holdout-rate` becomes rate 0 (every turn steered; ruling R3). The final reviewer preferred rate 1 (no steering on a configuration error). Either choice loses those rows, and the reason `bad_holdout_rate` is logged.

## Non-goals for v1

- The classifier provider.
- A settings-file writer.
- Any adapter other than Claude Code.

## Open items

- The gate numbers: effect size, latency budget and holdout rate.
- The first rules, from spike S3.
- The location of this spec. The default is this repository.

## References

- `docs/evals/phase1-local-classifier-preregistration.md`, Stage 4 and the footprint table.
- `docs/evals/phase1b-local-classifier-preregistration.md`, Step 5 and the Codex review.
- `docs/media/input/2026-09-18-jev-claude-code-notes.md`.
- `claude-plugins/codescout-companion/hooks/hooks.json`, `lib.mjs` and `host-normalization.test.mjs`.
- Research sources, as the subagents reported them: `platform.claude.com` (extended thinking, thinking, steering and cost, effort), `code.claude.com/docs` (model config, sub-agents, skills, SDK, hooks), `github.com/blackreo123/claude-code-auto-effort`, `opencode.ai/docs/models`, `aider.chat/docs/usage/commands`, `geminicli.com/docs`, `github.com/badlogic/pi-mono`.
