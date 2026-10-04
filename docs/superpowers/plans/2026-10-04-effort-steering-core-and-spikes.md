# Effort Steering — Core and Spikes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the harness-neutral effort-steering core (features, rules engine, steering text, holdout arm, decision log, CLI) and run spikes S1 to S3, in a new `effort-steering/` directory of the plugins repo.

**Architecture:** A pure core (`features`, `decide`, `steer`, `arm`, `log`) with a thin CLI on top. Spikes S1 to S3 come first, with an operator gate after them. The core comes after the gate, because the spike results decide whether rules have any basis. The Claude Code adapter, spikes S4 and S5 and the plugin registration belong to a second plan.

**Tech Stack:** Node ESM `.mjs`, no dependencies. Tests use `node:assert/strict` and run as `node --experimental-strip-types <file>`, which is the command `tests/run-all.sh` uses for `*.test.mjs` files.

**Spec:** `docs/superpowers/specs/2026-10-04-effort-steering-design.md` in the codescout repo (commit `787d4dcf`).

**Two repos.** Code lives in the plugins repo, `/home/marius/work/claude/claude-plugins`. Paths below are relative to that repo unless marked `[codescout]`. The plan, spec and findings live in the codescout repo.

## Global Constraints

Copied from the spec. Every task includes these.

- The decision vocabulary is `shallow`, `default`, `deep`.
- Confidence classes are `high`, `medium`, `low`. A rule declares its own class. It is not a probability.
- `shallow` needs `high` confidence and no failure signal in the previous turn. `deep` may fire at `medium`. Any other result is `default`.
- When two rules disagree, the higher confidence class wins. A tie between `shallow` and `deep` returns `default`.
- An explicit "ultrathink" always wins, and the core adds no second sentence.
- Any error, timeout or low confidence returns `default` with no steering. The hook fails open.
- Steering phrases, verbatim: `shallow` = "Answer directly without deliberating." `default` = nothing. `deep` = "Please think hard before responding."
- The decision log holds a prompt hash and features. It never holds prompt text. It uses one file per session.
- The holdout arm is a deterministic hash of the session and the turn.
- Node ESM `.mjs`, no dependencies. No settings-file writer, no classifier code, no `llm-proxy`.
- A version bump happens only through `./scripts/release.sh`. This plan bumps nothing.

## Review Focus

Five inputs that the spec implies and no task's main tests exercise. Each has a test in the task that owns the code.

1. An empty or whitespace-only prompt must give `default`, never an exception. (Task 6)
2. A 1 MB prompt, or a prompt built to make a regex backtrack, must not stall the hook. (Tasks 1 and 6)
3. A session id such as `../x` or an absolute path must never write outside the state directory. (Task 9)
4. A malformed ctx, an unreadable rules file or an invalid rule regex must fail open and exit 0. (Tasks 6 and 10)
5. Twenty processes that append to one session log at once must leave every line intact. (Task 9)

## Out of scope (second plan)

The Claude Code adapter, spikes S4 and S5, the registered gate numbers, `.claude-plugin/plugin.json`, the marketplace entry, the README version row and any release. The second plan is written after the Task 5 gate. Task 4 also does not cover how Claude Code orders `additionalContext` from several hooks. That question moves to the second plan.

## Procedure for every commit

Both repos are shared checkouts. Peer sessions run in both.

- Stage explicit file paths only. Never `git add .` and never a directory.
- Run `git diff --cached --name-only`. Every path must be yours.
- Commit with the message in a file: `git commit -F <msgfile> -- <paths>`. Never an inline `-m`.
- Never push.
- Before a commit that touches a file a peer might edit (`tests/run-all.sh`), run `git diff -- <file>`. If any hunk is not yours, stop and ask the operator.
- In the plugins repo, run `./tests/run-all.sh` before each commit.
- Task 4 (Step 5), Task 5 (Step 4) and Task 11 (Step 2) are operator gates. The main session runs those steps. A subagent cannot ask the operator a question.

## File structure

| Path | Responsibility |
|---|---|
| `effort-steering/core/testkit.mjs` | `t(name, fn)`: runs one test, prints `ok - name`, rethrows on failure |
| `effort-steering/core/features.mjs` | Prompt features. Pure. |
| `effort-steering/core/decide.mjs` | Rules compiler and the decision function. Pure. |
| `effort-steering/core/rules.json` | The tunable rules. Starts empty. |
| `effort-steering/core/steer.mjs`, `steer.table.json` | Steering text per level |
| `effort-steering/core/arm.mjs` | Holdout assignment |
| `effort-steering/core/log.mjs` | Record building and per-session JSONL append |
| `effort-steering/bin/effort-policy.mjs` | CLI: `decide` |
| `effort-steering/spikes/` | Spike scripts S1 to S3. Throwaway, but tested. |
| `[codescout] docs/research/2026-10-04-effort-steering-spike-findings.md` | Spike results |

---

### Task 1: Test discovery, `features.mjs`

**Files:**
- Create: `effort-steering/README.md`, `effort-steering/core/testkit.mjs`, `effort-steering/core/features.mjs`
- Test: `effort-steering/core/features.test.mjs`
- Modify: `tests/run-all.sh` (the `SUITES=(...)` array)

**Interfaces:**
- Produces: `t(name: string, fn: () => void): void`.
- Produces: `extractFeatures(prompt: string): Features` where `Features = { chars: number, words: number, isQuestion: boolean, hasStackTrace: boolean, startsWithSlash: boolean, explicitDeep: boolean }`. A non-string input returns the features of `''`.

- [ ] **Step 1: Record the baseline.** Run `./tests/run-all.sh`. Save the list of failed suites, if any. A later failure must be new to count as caused by this task.
- [ ] **Step 2: Write the failing test** `features.test.mjs`. Test names and assertions:
  - `counts`: `'fix the bug'` gives `chars 11`, `words 3`. `''` and `'   '` give `chars 0`, `words 0`, all booleans false.
  - `isQuestion`: `'why does this fail?'` is true. `'fix it'` is false.
  - `hasStackTrace`: a Python `Traceback (most recent call last):` block is true. A Node line `    at run (/a/b.js:1:2)` is true. The prose `'meet me at noon'` is false.
  - `startsWithSlash`: `'/legible:explain'` is true. `'see /etc/hosts'` is false. `'/etc/hosts is wrong'` is false. The pattern is `^\s*\/[A-Za-z][\w:-]*(\s|$)`.
  - `explicitDeep`: `'ultrathink about this'` and `'ULTRATHINK'` are true. `'ultrathinking'` is false.
  - `linear time`: a 1 MB string of `'a'` and a string of 100000 repeats of `'  at '` each finish in under 2000 ms.
- [ ] **Step 3: Run it and see it fail.** `node --experimental-strip-types effort-steering/core/features.test.mjs`. Expected: fail with a module-not-found error.
- [ ] **Step 4: Implement** `testkit.mjs` and `extractFeatures` in `features.mjs`. Use plain regexes without nested quantifiers.
- [ ] **Step 5: Run it and see it pass.** Same command. Expected: every `ok - ...` line, exit 0.
- [ ] **Step 6: Add the discovery glob.** In `tests/run-all.sh`, add `EFFORT_TESTS_DIR="$SCRIPT_DIR/../effort-steering"` and append `"$EFFORT_TESTS_DIR"/*/*.test.mjs` to `SUITES`. A new plugin directory is otherwise never scanned, and the runner would still print "All suites passed".
- [ ] **Step 7: Prove discovery.** Run `./tests/run-all.sh`. Expected: the output contains `▶ features.test.mjs`, and the failed suites match the Step 1 baseline. Then flip one assertion in `features.test.mjs`, rerun, and confirm the runner now lists `features.test.mjs` as failed. Revert the flip.
- [ ] **Step 8: Write `effort-steering/README.md`.** Three lines: what the directory is, that it is not yet an installable plugin, and the spec path.
- [ ] **Step 9: Commit** per the procedure. Paths: the five new files and `tests/run-all.sh`. Message: `feat(effort-steering): prompt features and test discovery`.

### Task 2: Spike S2, transcript usage

**Files:**
- Create: `effort-steering/spikes/s2-usage-report.mjs`
- Test: `effort-steering/spikes/s2-usage-report.test.mjs`

**Interfaces:**
- Produces: `usageRecords(lines: string[]): UsageRecord[]` with `UsageRecord = { line: number, ids: Record<string, string>, thinkingTokens: number | null, outputTokens: number | null, cacheCreation: number | null, cacheRead: number | null }`. `ids` holds every id-like field the inspection in Step 1 finds. Lines that do not parse, or that carry no usage, are skipped.
- Produces: `dedupeKeyReport(records: UsageRecord[]): { key: string, groups: number, consistent: boolean }[]`. `consistent` is true when every record that shares the key has the same `thinkingTokens`.

- [ ] **Step 1: Inspect real records.** Take three assistant records from `~/.claude-sdd/projects/-home-marius-work-claude-codescout/9403d62d-116b-46ea-ac9b-004acff2b1cb.jsonl`. Print their key paths to depth 4. Expected: the list includes `message.usage.output_tokens_details.thinking_tokens`. Write down every id-like key path you see (for example a message id, a request id and a record uuid). Use the names you observe in the fixture and the code.
- [ ] **Step 2: Write the failing test.** The fixture is three JSON lines: two share one message id and carry identical usage, and one has a different id. Replace content text with a placeholder. Assertions:
  - `usageRecords` returns 3 records.
  - For the message-id key, `groups` is 2 and `consistent` is true.
  - For a key unique per line, `groups` is 3.
  - A line with invalid JSON and a line without usage are skipped.
- [ ] **Step 3: Run it and see it fail.** Expected: module not found.
- [ ] **Step 4: Implement** both functions.
- [ ] **Step 5: Run it and see it pass.**
- [ ] **Step 6: Run the spike on all transcripts** of the codescout project directory. Report, per candidate key: group count and consistency. Report also whether any key path that matches `/effort/i` appears in any record, with one example path. Write the output to `${SPIKE_OUT:-${TMPDIR:-/tmp}/effort-steering-spikes}/s2-report.json`. The output must hold no prompt text.
- [ ] **Step 7: Commit** per the procedure. Message: `feat(effort-steering): spike S2 transcript usage report`.

### Task 3: Spike S3, offline replay

**Files:**
- Create: `effort-steering/spikes/s3-replay.mjs`
- Test: `effort-steering/spikes/s3-replay.test.mjs`

**Interfaces:**
- Consumes: `extractFeatures` (Task 1). `usageRecords` and the dedupe key found in Task 2.
- Produces: `turnsFromTranscript(lines: string[], dedupeKey: string): Turn[]` with `Turn = { promptSha256: string, features: Features, thinkingTokens: number, outputTokens: number, cacheCreation: number }`. A turn starts at a real user prompt and ends at the next one. Its token fields sum the deduped assistant messages in between.
- Produces: `bucketReport(turns: Turn[]): { name: string, n: number, median: number, p90: number }[]`. `p90` is the value at index `ceil(0.9 * n) - 1` of the sorted list. Buckets: `all`, `chars<=40`, `chars41-400`, `chars>400`, `isQuestion`, `hasStackTrace`, `startsWithSlash`, `explicitDeep`.

- [ ] **Step 1: Find what marks a real user prompt.** From real records, find which fields separate a typed prompt from a tool result, a command echo and a sidechain record. Record the distinguishing fields.
- [ ] **Step 2: Write the failing test.** The fixture has two prompts. A tool-result record sits between them. One assistant message is duplicated. Assertions:
  - Two turns come back.
  - The tool-result record does not start a turn.
  - The duplicated message counts once in the sums.
  - `median([1,2,3])` is 2. `p90` of ten values `1..10` is 9.
  - No turn field holds prompt text.
- [ ] **Step 3: Run it and see it fail.**
- [ ] **Step 4: Implement** both functions. Read each transcript line by line, not whole.
- [ ] **Step 5: Run it and see it pass.**
- [ ] **Step 6: Run the spike on all transcripts** of the codescout project directory. Write `s3-report.json` to the spike output directory. Print `n`, `median` and `p90` for each bucket. Print the confounders as a fixed note: adaptive thinking is on, and the effort level is known only if Task 2 found it recorded.
- [ ] **Step 7: Commit** per the procedure. Message: `feat(effort-steering): spike S3 offline replay`.

### Task 4: Spike S1, hook input fields (operator gate)

**Files:**
- Create: `effort-steering/spikes/s1-dump-hook.mjs`
- Test: `effort-steering/spikes/s1-dump-hook.test.mjs`

**Interfaces:**
- Produces: a hook script. It reads the hook input JSON on stdin, writes `{ keys: { [name]: typeof value } }` to the file named by env `S1_OUT`, prints nothing to stdout and exits 0. It never stores prompt text.

- [ ] **Step 1: Write the failing test.** Pipe `{"session_id":"s","cwd":"/x","prompt":"hello"}` into the script with `S1_OUT` set to a temp file. Assertions: exit 0, empty stdout, the file has `keys.session_id === 'string'` and `keys.prompt === 'string'`, and the file does not contain the text `hello`. Invalid JSON on stdin exits 0 and writes nothing.
- [ ] **Step 2: Run it and see it fail.**
- [ ] **Step 3: Implement** the hook.
- [ ] **Step 4: Run it and see it pass.**
- [ ] **Step 5: Ask the operator** for approval to run one small `claude -p` model call. State the cost: one short reply. Do not continue without a yes.
- [ ] **Step 6: Check the flag.** Run `claude --help` and confirm a `--settings` option exists. If it does not, stop and report.
- [ ] **Step 7: Run the live check.** Write a scratch settings file with a `UserPromptSubmit` command hook that runs `node <abs path>/effort-steering/spikes/s1-dump-hook.mjs`. Run `claude -p "reply with the word ok" --settings <scratch file>` from a temp directory, with `S1_OUT` set. Expected: the output file lists keys including `session_id` and `cwd`. If the hook does not fire, stop and report. Edit no real settings file.
- [ ] **Step 8: Commit** the script and test per the procedure. Message: `feat(effort-steering): spike S1 hook input dump`.

### Task 5: Findings and the operator gate

**Files:**
- Create: `[codescout] docs/research/2026-10-04-effort-steering-spike-findings.md`
- Modify: `[codescout] docs/superpowers/specs/2026-10-04-effort-steering-design.md` (section `## Unverified claims register`)

- [ ] **Step 1: Write the findings doc.** Sections: S1 hook input keys. S2 dedupe key, its consistency and whether the effort level is recorded. S3 the bucket table. Add that `session_id` and `cwd` are also read by `codescout-companion/hooks/constitution-brief.mjs`. Facts only. State each limit.
- [ ] **Step 2: Update the register.** Move each item the spikes verified out of the register, with a pointer to the findings doc. Mark each remaining item as still open.
- [ ] **Step 3: Commit both files** in the codescout repo per its commit sequence, with the message in a `-F` file. Do not push.
- [ ] **Step 4: Stop and present the findings to the operator.** Ask for one decision: proceed to Task 6, adjust the design, or stop. Do not start Task 6 without it. If no bucket in S3 separates thinking depth, say so plainly. The rules then have no basis.

### Task 6: `decide.mjs`

**Files:**
- Create: `effort-steering/core/decide.mjs`
- Test: `effort-steering/core/decide.test.mjs`

**Interfaces:**
- Consumes: `extractFeatures` (Task 1).
- Produces: `POLICY_VERSION = 1`.
- Produces: `compileRules(raw: unknown): CompiledRules` where `CompiledRules = { version: number, rules: Rule[], skipped: { id: string, error: string }[] }`. It never throws.
- Produces: `decide(ctx: Ctx, rules: CompiledRules): Decision` with `Ctx = { prompt?: unknown, signals?: { prevTurnFailed?: boolean, retryCount?: number, planMode?: boolean, turnIndex?: number } }` and `Decision = { level: 'shallow' | 'default' | 'deep', confidence: 'high' | 'medium' | 'low', reasons: string[] }`. It never throws.

**Rule file schema.** `{ "version": number, "rules": [{ "id": string, "level": "shallow" | "deep", "confidence": "high" | "medium" | "low", "reason": string, "when": {...} }] }`. `when` keys, all optional: `minChars`, `maxChars`, `isQuestion`, `hasStackTrace`, `startsWithSlash`, `matchesAny` (regex strings), `matchesNone` (regex strings), `prevTurnFailed`, `minRetries`, `planMode`. All present keys must hold. Regexes are case-insensitive. A rule is skipped, with an entry in `skipped`, when its level or confidence is invalid, a regex does not compile, a regex is slow, a `when` key is unknown, or `when` is empty. A rule that matches everything is a footgun. A regex is slow when it runs longer than 50 ms on the probe string `'a'.repeat(40) + '!'`. `compileRules` measures this with `node:vm` (`vm.runInNewContext(..., { timeout: 50 })`), and the skip error is `slow_regex`. Checked on Node v26.10.0: the pattern `(a+)+$` is interrupted after 51 ms, and `foo|bar` takes 1 ms. At decision time, rule regexes see only the first 20000 characters of the prompt.

**Decision order.**
1. `features.explicitDeep` gives `deep`, `high`, reasons `['user_override']`.
2. No rule matched gives `default`, `low`, `['no_rule']`.
3. Take the highest confidence class among matched rules. If both `shallow` and `deep` rules sit at that class, give `default`, `low`, `['tie_shallow_deep', ...ids]`.
4. A `shallow` result needs `high` confidence and no `prevTurnFailed`. Otherwise give `default` with the reason `guard_shallow` plus the rule ids. A `deep` result below `medium` gives `default`, `low`, `['low_confidence', ...ids]`.
5. Otherwise return the level, the class and the ids of the matched rules at that class and level.

- [ ] **Step 1: Write the failing test.** Test names and assertions:
  - `guards`: shallow/high/no failure gives `shallow`. Shallow/high with `prevTurnFailed` gives `default`. Shallow/medium gives `default`. Deep/medium gives `deep`. Deep/low gives `default`.
  - `precedence`: a high shallow rule beats a medium deep rule. A high shallow and a high deep rule give `default` with `tie_shallow_deep`. `'ultrathink'` beats a matching high shallow rule.
  - `conditions`: each `when` key has one passing and one failing case. A rule with one failing key does not match (AND). `minRetries: 2` fails at `retryCount` 1 and passes at 2.
  - `fail open`: an invalid regex rule is skipped and listed, and the other rules still apply. An empty `when` is skipped. `prompt` of `undefined`, `42`, `''` and `'   '` each give `default` without throwing.
  - `slow regex`: a rule with `matchesAny: ['(a+)+$']` is skipped with the error `slow_regex`. A valid rule next to it still applies. `decide` on a 1 MB prompt with only safe rules finishes within 2000 ms.
- [ ] **Step 2: Run it and see it fail.**
- [ ] **Step 3: Implement** `compileRules` and `decide` as specified.
- [ ] **Step 4: Run it and see it pass.**
- [ ] **Step 5: Commit** per the procedure. Message: `feat(effort-steering): rules engine and decision function`.

### Task 7: `steer.mjs`

**Files:**
- Create: `effort-steering/core/steer.mjs`, `effort-steering/core/steer.table.json`
- Test: `effort-steering/core/steer.test.mjs`

**Interfaces:**
- Consumes: `Decision` (Task 6).
- Produces: `render(decision: Decision, table: { version: number, phrases: Record<string, string> }): string | null`.

- [ ] **Step 1: Write the failing test.** Assertions:
  - `shallow` gives exactly `"Answer directly without deliberating."`. `deep` gives exactly `"Please think hard before responding."`.
  - `default` gives `null`.
  - A decision with `user_override` in `reasons` gives `null`.
  - A table without the phrase for the level gives `null`.
  - The file `steer.table.json` parses and holds the two phrases above verbatim, with `version` 1.
- [ ] **Step 2: Run it and see it fail.**
- [ ] **Step 3: Implement** `render` and write `steer.table.json`.
- [ ] **Step 4: Run it and see it pass.**
- [ ] **Step 5: Commit** per the procedure. Message: `feat(effort-steering): steering text`.

### Task 8: `arm.mjs`

**Files:**
- Create: `effort-steering/core/arm.mjs`
- Test: `effort-steering/core/arm.test.mjs`

**Interfaces:**
- Produces: `assignArm(sessionId: string, turnId: string, holdoutRate: number): 'treated' | 'holdout'`. It hashes `JSON.stringify([sessionId, turnId])` with sha256, reads the first 8 hex digits as an unsigned 32-bit number, divides by 2^32, and returns `holdout` when the result is below `holdoutRate`. It throws `RangeError` when `holdoutRate` is not a number in `[0, 1]`.

- [ ] **Step 1: Write the failing test.** Assertions:
  - The same inputs always give the same arm.
  - Rate 0 gives `treated` for turns `'1'..'500'`. Rate 1 gives `holdout` for all of them.
  - Rates `-0.1`, `1.1` and `NaN` throw `RangeError`.
  - Over turns `'1'..'2000'` at rate 0.2, the holdout share is between 0.15 and 0.25.
  - Mutation guards: at rate 0.5, the arm sequence for session `'a'` over turns `'1'..'200'` is not constant, and differs from the sequence for session `'b'` in at least one position. The sequence `assignArm('a:b', String(i), 0.5)` differs from `assignArm('a', 'b:' + i, 0.5)` in at least one position. A plain `':'` join would make these two collide.
- [ ] **Step 2: Run it and see it fail.**
- [ ] **Step 3: Implement** `assignArm`.
- [ ] **Step 4: Run it and see it pass.**
- [ ] **Step 5: Commit** per the procedure. Message: `feat(effort-steering): deterministic holdout arm`.

### Task 9: `log.mjs`

**Files:**
- Create: `effort-steering/core/log.mjs`
- Test: `effort-steering/core/log.test.mjs`

**Interfaces:**
- Consumes: `Features` (Task 1), `Decision` (Task 6), `POLICY_VERSION` (Task 6).
- Produces: `hashPrompt(prompt: string): string`, the sha256 hex digest.
- Produces: `stateDirFor(env: Record<string, string | undefined>): string`. It returns `<XDG_STATE_HOME>/effort-steering`, or `<home>/.local/state/effort-steering` when `XDG_STATE_HOME` is unset or empty.
- Produces: `buildRecord(a: { sessionId: string, turnId: string, harness: string, prompt: string, features: Features, decision: Decision, arm: 'treated' | 'holdout', steered: boolean, rulesVersion: number, steerVersion: number, now?: Date }): LogRecord`. `LogRecord` keys: `ts` (ISO string), `session_id`, `turn_id`, `harness`, `policy_version`, `rules_version`, `steer_version`, `level`, `confidence`, `reasons`, `features`, `prompt_sha256`, `arm`, `steered`. It has no field that holds the prompt.
- Produces: `appendRecord(stateDir: string, record: LogRecord): string`. It appends one JSON line to `<stateDir>/<session_id>.jsonl`, creates the directory if needed and returns the file path. It throws `RangeError` unless `session_id` matches `/^[A-Za-z0-9._-]{1,128}$/` and is not `.` or `..`.

- [ ] **Step 1: Write the failing test** with a temp state directory that the test removes at the end. Assertions:
  - A record built from the prompt `'my secret token abc123'` gives a log line that does not contain `abc123`, and `prompt_sha256` equals `hashPrompt` of that prompt.
  - Every `LogRecord` key listed above is present.
  - `stateDirFor({XDG_STATE_HOME: '/s'})` ends in `/s/effort-steering`. Empty and unset values fall back to the home path.
  - `appendRecord` throws `RangeError` for `'../x'`, `'/abs'`, `''`, `'a/b'`, `'..'` and a 129-character id. No file appears outside the temp directory.
  - Two session ids give two files. Two appends to one id give two lines.
  - Concurrency: spawn 20 child processes with async `spawn`. Each appends 50 records to one session file. Afterwards the file has 1000 lines and every line parses as JSON.
- [ ] **Step 2: Run it and see it fail.**
- [ ] **Step 3: Implement** the four functions. Use one `appendFileSync` call per record.
- [ ] **Step 4: Run it and see it pass.**
- [ ] **Step 5: Commit** per the procedure. Message: `feat(effort-steering): per-session decision log`.

### Task 10: CLI

**Files:**
- Create: `effort-steering/bin/effort-policy.mjs`, `effort-steering/core/rules.json` (content: `{"version":1,"rules":[]}`)
- Test: `effort-steering/bin/effort-policy.test.mjs`

**Interfaces:**
- Consumes: `extractFeatures`, `compileRules`, `decide`, `render`, `assignArm`, `buildRecord`, `appendRecord`, `stateDirFor`, `hashPrompt`.
- Produces: `node effort-policy.mjs decide [--rules <file>] [--table <file>] [--holdout-rate <0..1>] [--log]`. It reads one JSON ctx from stdin: `{ prompt, signals?, session_id?, turn_id?, harness? }`. It prints one JSON line `{ level, confidence, reasons, steer, arm }` and always exits 0. `steer` is `null` when `arm` is `holdout`. The defaults are `core/rules.json`, `core/steer.table.json` and rate 0. A rate of 0 means no holdout until the operator registers the value.
- With `--log`, it appends one record. When `session_id` or `turn_id` is missing, it skips the log and adds the reason `log_skipped_no_ids`. When the append throws, it adds the reason `log_failed` and writes the error to stderr.
- Invalid stdin JSON gives `{ level: 'default', confidence: 'low', reasons: ['bad_input'], steer: null, arm: 'treated' }`. An unreadable rules file adds `rules_unreadable` and gives `default`.

- [ ] **Step 1: Write the failing test.** The test calls the CLI with `spawnSync`, sets `XDG_STATE_HOME` to a temp directory and removes it at the end. Assertions:
  - A valid ctx with the shipped empty rules gives `default` with `no_rule`.
  - A temp rules file with a high shallow rule gives `shallow` and the exact shallow phrase in `steer`.
  - Invalid JSON on stdin exits 0 with `bad_input`.
  - `--holdout-rate 1` gives `arm: 'holdout'` and `steer: null`, even when a rule says `shallow`.
  - `--log` with ids creates one file with one line, and that line does not contain the prompt text.
  - `--log` without ids creates no file and adds `log_skipped_no_ids`.
  - `--log` with the session id `'../x'` exits 0, writes nothing outside the temp directory and adds `log_failed`.
  - A rules file with one invalid regex rule and one valid rule still applies the valid rule.
  - An unreadable `--rules` path gives `rules_unreadable`.
- [ ] **Step 2: Run it and see it fail.**
- [ ] **Step 3: Implement** the CLI and write `rules.json`.
- [ ] **Step 4: Run it and see it pass.**
- [ ] **Step 5: Run `./tests/run-all.sh`.** Expected: all effort-steering suites run and pass, and the failed suites match the Task 1 baseline.
- [ ] **Step 6: Commit** per the procedure. Message: `feat(effort-steering): effort-policy CLI`.

### Task 11: First rules from S3 (operator gate)

**Files:**
- Modify: `effort-steering/core/rules.json`
- Test: `effort-steering/core/rules.test.mjs`

**Interfaces:**
- Consumes: the S3 report (Task 3), the findings (Task 5), `compileRules` and `decide` (Task 6).

- [ ] **Step 1: Propose candidate rules in chat.** Use a table: rule id, `when`, level, confidence, and the S3 evidence (bucket `n`, `median`, `p90`). Propose only rules that the S3 buckets support. Do not edit `rules.json` yet.
- [ ] **Step 2: Stop.** The operator chooses which rules to keep and may change them. Continue only with an explicit answer.
- [ ] **Step 3: Write the failing test** `rules.test.mjs`. It holds a `CASES` table keyed by rule id, each entry with a `positive` list and a near-miss `negative` list. A near-miss text must resemble another rule's cue. Assertions:
  - Every rule id in `rules.json` has an entry with both lists non-empty.
  - `CASES` has no id that is missing from `rules.json`.
  - `compileRules(rules.json).skipped` is empty.
  - Each positive text gives the rule's level through `decide`. Each negative text does not.
- [ ] **Step 4: Run it and see it fail.**
- [ ] **Step 5: Write `rules.json`** with the approved rules.
- [ ] **Step 6: Run it and see it pass.** Then run `./tests/run-all.sh`.
- [ ] **Step 7: Commit** per the procedure. Message: `feat(effort-steering): first rules from the offline replay`.

---

## Decisions beyond the spec

The operator should confirm these. Each fills a gap the spec leaves open.

- `explicitDeep` detection lives in the core features, so every harness gets it.
- A rule with an empty `when` is skipped.
- A tie between `shallow` and `deep` gives `default` with confidence `low`.
- A `deep` rule below `medium` gives `default`.
- The CLI has a `--log` flag, so a harness hook that shells out can also write the log.
- The holdout rate defaults to 0 until the operator registers a value.
- The holdout key is `JSON.stringify([sessionId, turnId])`, not a joined string.
- The plugin manifest, marketplace entry and README row wait for the second plan, so no empty plugin can be installed.
