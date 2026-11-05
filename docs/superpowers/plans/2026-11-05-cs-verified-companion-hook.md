---
id: '1741311c9fa4d5af'
kind: plan
status: draft
title: Companion hook that relocates verified advice (slice 2) Implementation Plan
---

# Companion hook that relocates verified advice (slice 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A PostToolUse hook in `codescout-companion` that, when a codescout result carries `advice` with `verified: true` and the host is Claude Code, removes `advice` from the result (every content block kept) and delivers its text as `[cs-verified] <text>` hook context; in every other case it prints nothing and exits 0.

**Architecture:** One node (ESM) hook script `codescout-companion/hooks/cs-verified-relocate.mjs` with a pure exported function `relocateVerifiedAdvice(input)` that decides and returns the output object or `null`, and a thin `main()` that reads stdin, calls it, prints JSON only when it returns an object, and exits 0 on every path. A first task measures the three unknowns the spec left open (empty stdout, host signals in a clean environment, failure modes) on the installed CLI and records real hook input as fixtures; a last task proves the shipped hook end to end on that CLI.

**Tech Stack:** node ESM, `node --experimental-strip-types` test files (`*.test.mjs`, discovered by `tests/run-all.sh`), the companion's `hooks/lib.mjs` helpers, Claude Code 2.1.318 (installed; 2.1.311 to 2.1.317 also present under `~/.local/share/claude/versions/`).

**Spec:** `docs/superpowers/specs/2026-11-04-harness-delivered-hints-design.md` (id `95abaa31c77daa39`), part 3 "Companion hook", part 4 "A new tag", the Constraints, "What is left to the builder", and Rollout item 2. Server side is done (slice 1, `experiments` at `4214c5f77`): `edit_code replace` emits `advice {text, verified}`.

**Repo and workspace:** code lands in `/home/marius/work/claude/claude-plugins` (branch `main`, HEAD `54c2f25` when scouted; another session has uncommitted work there: `codescout-companion/background-wake/codex-hook*.mjs`, `docs/background-wake.md`, two issue files, `docs/plans/`). Work in a worktree `/home/marius/work/claude/claude-plugins.worktrees/cs-verified-hook` on branch `feat/cs-verified-hook` created from `main`; never touch those uncommitted files. Land by fast-forward of `main` only when it applies cleanly. The plan and the design doc live in the codescout repo.

## Global Constraints

- The hook acts only when ALL hold: the host gate (Task 1 decides it) passes; `tool_response` is the list-of-content-blocks form; exactly one content block's text parses as a JSON object whose top-level `advice` is an object with `verified === true` (the boolean) and a string `text`; the text is a single line of printable characters, at most 1000 characters, and does not contain the marker `[cs-verified]`. Anything else: print nothing.
- When it acts it prints `{"hookSpecificOutput":{"hookEventName":"PostToolUse","updatedToolOutput":[...all blocks, in order, only the one block's text edited...],"additionalContext":"[cs-verified] <text>"}}`. Only the top-level `advice` key is removed from that block's JSON; every other field is deep-equal before and after; every other block is byte-identical.
- Fail-open: every path exits 0; an uncaught exception prints nothing. Install `process.on('uncaughtException', () => process.exit(0))` and `main().catch(() => process.exit(0))` as `wake-on-job-end.mjs` does.
- The hook never re-derives "verified" (standing rule `docs/architecture/companion-plugin.md`: no advisory mirror of a server predicate). It reads the server's flag only.
- Registration: a PostToolUse entry, matcher `mcp__codescout__.*`, command `node ${CLAUDE_PLUGIN_ROOT}/hooks/cs-verified-relocate.mjs`, no `args` field (`tests/test-hooks-json-registration.sh` enforces this). Node only; no bash, jq or python at run time.
- No version bump in any task. Release goes through `./scripts/release.sh` and needs a clean `main` plus a cold restart of every Claude Code instance; the controller reports it as owed and does not run it.
- Budget for the real-CLI runs in Tasks 1 and 4: use `--model claude-haiku-5-5` and `--max-budget-usd 0.25` per run, at most 40 runs in total, and record each run's cost.
- Tests: run one file with `node --experimental-strip-types codescout-companion/hooks/<file>.test.mjs`; the whole suite with `./tests/run-all.sh` before the last commit of each task.

## Review Focus

- A result with several content blocks (two text blocks; a text block plus a non-text block): all kept, in order; only the block holding the advice changes.
- `advice` present but `verified` false, missing, the string `"true"`, or `advice` not an object; `advice` nested below the top level; two blocks each carrying a verified advice: print nothing.
- A text block that is not JSON, is a JSON array or a string, or is invalid JSON: print nothing. A `tool_response` that is a string, a dict, null or missing: print nothing.
- Advice text with a newline, a control character, more than 1000 characters, or the marker `[cs-verified]` inside (the text can carry a file path or a symbol name taken from the user's code): print nothing, so the advice stays in the result as today.
- A protocol hint (`hint` only: job started, buffer pointer, approve-write) and a safety warning in `hint` next to a verified `advice`: `hint` is untouched in the output.
- Hook failures: stdin empty, malformed JSON, `tool_response` huge; the process still exits 0 with nothing on stdout.

---

### Task 1: measure the open points and record real hook input

**Files:**
- Create: `codescout-companion/contracts/fixtures/cs-verified/` (recorded hook inputs, one JSON file per case, plus `README.md` stating the CLI version, date and command for each), `codescout-companion/scripts/probe-cs-verified/` (the stub MCP server and the run script, adapted from `docs/evals/data/2026-11-04-harness-channel-hints/relocation-probe/` in the codescout repo: `stub_mcp.py`, `run.sh`, `hook.py`)
- Test: none (a measurement task); its output is the fixtures and a results note `codescout-companion/contracts/fixtures/cs-verified/results.md`

**Interfaces:**
- Consumes: the relocation probe's stub MCP server and run script (read them first).
- Produces, for Task 2: (a) real PostToolUse stdin for a tool named `mcp__codescout__edit_code` (the stub server must be registered under the name `codescout` so the tool name matches) whose result is a list of blocks: block 1 the JSON envelope with `advice {text, verified:true}` and `hint`, block 2 a second text block, optionally a non-text block if the stub can emit one; (b) a written decision, binding on Tasks 2 to 4, for each question below.

- [ ] **Step 1: Read the probe and build the stub.** Copy the relocation probe into `scripts/probe-cs-verified/`, register the stub as MCP server `codescout`, give it a tool `edit_code` returning the envelope shape the server emits (copy field names from `src/tools/symbol/edit_code.rs` `do_replace` and the `advice` tests in `src/tools/symbol/tests.rs` of the codescout repo: `status`, `replaced_lines`, `hint`, `advice {text, verified}`).
- [ ] **Step 2: Measure on CLI 2.1.318 (and 2.1.311 as an older point), each question one or more runs, record stdin/stdout/exit and the model-visible result:**
  1. A hook that prints nothing and exits 0: is the result unchanged and no error shown? (the spec wants "prints nothing"; prior art printed `{}`.) Decision: print nothing if accepted on both versions, else `{}`.
  2. The `tool_response` form for an MCP tool result with 2 blocks and with 1 block: list of blocks (expected), or a string? Record both.
  3. `updatedToolOutput` as a list of 2 blocks with one edited: both blocks reach the model, the edited text without `advice`; `additionalContext` arrives as a system-reminder carrying `[cs-verified] ...`.
  4. Host signals in a CLEAN environment (`env -i` plus only `HOME`, `PATH` and what the CLI needs): which of `CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT`, `CLAUDE_PROJECT_DIR`, `CLAUDE_PLUGIN_ROOT` are present in the hook process, and which stdin keys (`permission_mode`, `prompt_id`, `effort`, `hook_event_name`, `session_id`, `transcript_path`). Decision: the host gate, using only signals the CLI itself provides in a clean run (never one that can be inherited from an outer session), stated as an exact predicate.
  5. Failure modes: timeout (hook sleeps past a `timeout` of 2 s), `exit 2` with stderr text, malformed JSON on stdout, a 2 MB stdout. For each: is the result unchanged, is anything shown to the model, is the session affected?
  6. A subagent's tool call: does the hook fire for a tool call made inside a subagent (a Task subagent calling the stub tool), and does stdin carry `agent_id`? (record only; the answer goes into the design's "revisit when").
- [ ] **Step 3: Record fixtures and results.** One fixture per case in 2 and 4 (real stdin captured by the probe hook, with the session id and paths left as they were). `results.md` has one section per question with the CLI version, the exact settings JSON and stub tool call used, observed stdin keys, observed result, and the decision. Cost per run listed.
- [ ] **Step 4: Commit** — `git add` the two new directories by name; message `test(companion): record real PostToolUse input for a multi-block codescout result and measure the cs-verified relocation preconditions`.

### Task 2: the hook and its tests

**Files:**
- Create: `codescout-companion/hooks/cs-verified-relocate.mjs`
- Test: `codescout-companion/hooks/cs-verified-relocate.test.mjs`

**Interfaces:**
- Consumes: the Task 1 fixtures and decisions (host gate predicate, empty-stdout vs `{}`); `lib.mjs` (`contextPostToolUse` at L54 shows the output shape; `isCodescoutTool` at L653 and `normalizedToolName` at L636 normalise tool names; read the file's stdin helpers before writing your own).
- Produces: `export function relocateVerifiedAdvice(input: unknown): { hookSpecificOutput: { hookEventName: 'PostToolUse', updatedToolOutput: Block[], additionalContext: string } } | null` where `Block` is `{type: string, text?: string, ...}`; and `main()` run only when the file is the entry point.

- [ ] **Step 1: Write the failing tests** (names and assertions):
  - `relocates_a_verified_advice_and_keeps_every_other_block`: from the Task 1 fixture (and a hand-built two-text-block one): output `updatedToolOutput` has the same number of blocks, block order preserved, the edited block's JSON deep-equals the original minus `advice`, other blocks byte-identical, `additionalContext === '[cs-verified] ' + advice.text`.
  - `leaves_hint_and_every_other_field_untouched`: `hint` (a safety warning string), `status`, `replaced_lines`, nested objects, unicode, `_`-prefixed keys, numbers, arrays: deep-equal before and after.
  - `acts_only_on_verified_true`: `verified:false`, missing, `"true"`, `1`, `null`, `advice` a string or an array: each gives `null`.
  - `acts_only_on_a_top_level_advice`: `advice` nested in another object gives `null`.
  - `does_nothing_on_two_verified_advices`: two blocks each with a verified advice give `null`.
  - `does_nothing_when_the_result_is_not_the_list_form`: string, dict with `content`, `null`, missing `tool_response`; and when the one text block is not JSON, an array, a string, invalid JSON: each gives `null`.
  - `refuses_advice_text_that_is_not_a_plain_single_line`: newline, `\r`, a control character, 1001 characters, the marker `[cs-verified]` inside, an empty string: each gives `null`; 1000 characters of printable text is accepted.
  - `the_host_gate`: the predicate Task 1 decided, with a positive input and one negative input per clause.
  - `a_second_run_on_a_relocated_result_does_nothing`: output blocks fed back as input give `null`.
  - process-level (spawn `node` on the script with stdin): empty stdin, `not json`, a huge `tool_response`, and a thrown error inside (via a malformed `tool_response` that makes the decision code throw) each exit 0 with empty stdout; the acting case exits 0 and prints exactly the JSON the pure function returns.
- [ ] **Step 2: Run to verify they fail** — `node --experimental-strip-types codescout-companion/hooks/cs-verified-relocate.test.mjs` — Expected: module not found.
- [ ] **Step 3: Implement `relocateVerifiedAdvice` and `main`** per the Global Constraints. Edit the one JSON block by `JSON.parse`, deleting the top-level `advice` key, `JSON.stringify` (key order is preserved for string keys); reject a block whose parsed object has an integer-like key or whose text contains a number outside the safe integer range, returning `null` (JSON text round-trips would change them). No second source of truth for "verified".
- [ ] **Step 4: Mutation proof.** For each of these mutants of the script, apply it, run the test file, confirm at least one test fails, revert: accept `verified` truthy; drop the one-block rule; drop the single-line check; drop the marker check; edit every block instead of one; drop the `advice` removal; print `hint` removal too; make `main` exit 1 on error; drop the list-form check. Put the failing test name per mutant in your report; the final `git diff` shows no mutant.
- [ ] **Step 5: Run** the test file and `./tests/run-all.sh`; Expected: pass.
- [ ] **Step 6: Commit** — `git add codescout-companion/hooks/cs-verified-relocate.mjs codescout-companion/hooks/cs-verified-relocate.test.mjs`; message `feat(companion): relocate verified codescout advice to [cs-verified] hook context`.

### Task 3: register the hook and document it

**Files:**
- Modify: `codescout-companion/hooks/hooks.json`, `tests/test-hooks-json-registration.sh`, `codescout-companion/README.md` (the Hooks section only)
- Test: `tests/test-hooks-json-registration.sh` (new case), `tests/test-all-plugins-hooks-json-portable.sh` and `tests/test-cross-platform-hooks.sh` must still pass

**Interfaces:**
- Consumes: the Task 2 script path and Task 1 timeout finding (give the entry a `timeout` only if Task 1 shows the default is unsuitable; otherwise none, like every other PostToolUse hook except `wake-on-job-end`).
- Produces: nothing for later tasks except the registration Task 4 exercises.

- [ ] **Step 1: Write the failing test.** In `tests/test-hooks-json-registration.sh`, a case modelled on case 3c: a PostToolUse entry with matcher `mcp__codescout__.*` whose command matches `^node \${CLAUDE_PLUGIN_ROOT}/hooks/cs-verified-relocate\.mjs$`, and the event is PostToolUse (not PreToolUse, where `mcp__codescout__.*` is already registered for `principal-stamp.mjs`).
- [ ] **Step 2: Run to verify it fails** — `bash tests/test-hooks-json-registration.sh` — Expected: the new case fails.
- [ ] **Step 3: Implement.** Add the entry to the PostToolUse array of `hooks.json`; add a README Hooks entry stating what the hook does, that it never moves an unverified advice, a protocol hint or a safety warning, its fail-open behavior, the measured CLI versions, the tag `[cs-verified]` and that it relocates whenever the gates pass, with or without the consent statement in the user's `CLAUDE.md` (slice 3, not shipped), which only changes how the model treats the context (correction made after the plan was confirmed: the first wording, "a no-op until the consent statement exists", was false), and the two-repo pairing in the style of the README's `principal-stamp.mjs` section ("new hook, old server: safe; new server, old hook: safe").
- [ ] **Step 4: Run** `./tests/run-all.sh` — Expected: pass.
- [ ] **Step 5: Commit** — `git add codescout-companion/hooks/hooks.json tests/test-hooks-json-registration.sh codescout-companion/README.md`; message `feat(companion): register the cs-verified relocation hook and document it`.

### Task 4: prove the shipped hook on the real CLI and close the slice in the design

**Files:**
- Modify: `codescout-companion/contracts/fixtures/cs-verified/results.md` (append the end-to-end section); in the codescout repo, the design `## Rollout` through `doc(action="update")`
- Test: none new (measurement), reusing the Task 1 probe harness

**Interfaces:**
- Consumes: the branch with Tasks 1 to 3, the Task 1 stub server and run script.
- Produces: the end-to-end results and the design update.

- [ ] **Step 1: Run the shipped hook (not the probe hook) through the plugin registration on CLI 2.1.318, and on 2.1.311, with the stub server `codescout`.** Cases: (a) no hook installed: the advice stays in the result and the model sees it; (b) hook installed, verified advice: the model-visible result has no `advice`, a second block intact, and the CLI's own system-role message `PostToolUse:<tool> hook additional context: [cs-verified] <text>` (Task 1 measured this form; it is not a system-reminder tag); (c) `verified:false`: the result keeps its advice, no context; (d) a result with only `hint` (protocol pointer) and one with `hint` plus verified `advice`: `hint` kept, advice relocated; (e) the hook script made to crash (temporarily point the registration at a script that throws): the result is unchanged; (f) COEXISTENCE: a second PostToolUse hook, a stand-in for the `buddy` plugin's unmatched hook, that prints `additionalContext` `[cs-hint] ...` on the same call, with and without ours: record how the CLI merges the two outputs and whether ours still applies; (g) at least one interactive-mode, `/clear` or `--fork-session` run if it can be driven within the cost cap (a pty-driven interactive session is not a `-p` run), else record the gate as not measured there: the gate can fail closed silently (see Task 1 results.md, binding note); (h) the shipped script, not the probe hook, on BOTH CLI versions (the README says the script itself has not yet been run on the CLI until this task does it). Record the transcript lines that show each, the CLI version, the cost of each run.
- [ ] **Step 2: Append to `results.md`** the table of cases and observations; list what was NOT measured (VS Code, Copilot, compaction persistence of the relocated context, subagent calls if Task 1 left it open).
- [ ] **Step 3: Update the design.** Through the catalog (`doc(action="get", id="95abaa31c77daa39", heading="## Rollout")` first, copy the old text exactly): mark item 2 (Hook) done with the claude-plugins commit SHAs and the CLI versions measured, state what is open (consent text, slice 4 measurement, release and cold restart owed, VS Code and Copilot unmeasured, compaction persistence), and correct "Constraints" and "What is left to the builder" where Task 1 settled a point (host signals, hook failure modes beyond `exit 1`, empty stdout).
- [ ] **Step 4: Commit** the claude-plugins changes by name (`test(companion): end-to-end results of the cs-verified relocation on Claude Code 2.1.311 and 2.1.318`), and the design change in the codescout repo by pathspec (`docs(design): slice 2 done - the companion hook relocates verified advice (<sha>)`).
