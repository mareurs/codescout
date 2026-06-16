# Session Log — VDI Reliability Hardening

> Two-sided observation log for the VDI reliability work stream
> (spec: `docs/superpowers/specs/2026-06-08-vdi-reliability-hardening-design.md`;
> plan: `docs/superpowers/plans/2026-06-08-vdi-reliability-hardening.md`).

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-06-08 | med | plan-prose | fixed-verified | Plan Task 2 test code used a non-existent test harness shape |
| F-2 | 2026-06-08 | med | subagent | mitigated | Broad `git add <file>` swept pre-existing WIP into the Task 1 commit |
| F-3 | 2026-06-08 | low | architectural | documented | Task 7 spawn-level timeout deferred (`initialize()` already bounded; `spawn()` needs `spawn_blocking`) |
| F-4 | 2026-06-09 | med | diagnostic | fixed-verified | Hidden-file test misdiagnosed as gitignore (red-herring `git init`) — real cause was an open write handle blocking `metadata()` on Windows |
| F-5 | 2026-06-09 | low | triage | fixed-verified | `companion_surfaces` failure mis-triaged as peer-gating (Group A); actually stale-tool-name drift (Group B) |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-06-08 | med | Pre-dispatch scout of the test harness before subagent run | Task 2 subagent would fail cargo check + flail | validated |
| W-2 | 2026-06-09 | high | Treated Windows "environmental" test failures as suspects, not noise | dismissing them would have shipped a silent deny-list bypass + project-root shadowing | validated |
| W-3 | 2026-06-09 | med | Adversarial architecture review (Snow Lion) before ship | 2 real gaps (taskkill-in-Drop, foreground bypassing the builder) would have shipped | validated |
| W-4 | 2026-06-12 | med | Diffed the review surface against `git merge-base`, not the `experiments` tip | reviewing `experiments..vdi-windows` would have scrutinized ~1800 phantom "deletions" (the LSP-mux refactor that lives on experiments, never touched by this branch) | validated |
| W-5 | 2026-06-12 | low | Pure platform logic kept in `platform::mod` (cross-platform compiled), not cfg-gated submodules, so its tests run on every CI | the 5 `lsp_binary_name_with` tests ran only on Windows despite testing pure string logic; the move added +5 Linux-gate assertions covering WIN-19 | validated |

---

## Category conventions

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool |
| `subagent` | Subagent diverged from instructions |
| `plan-prose` | Plan document drift vs reality (wrong paths, fictional code, mismatched counts) |
| `architectural` | Structural property the plan/docs didn't surface |
| `self-friction` | Predicted friction that was a false alarm |
| `release-pipeline` | Deployment-time gap |

---

## F-1 — Plan Task 2 test code used a non-existent test harness shape

**Observed:** 2026-06-08, pre-dispatch reconnaissance for the VDI reliability
plan (`2026-06-08-vdi-reliability-hardening.md`), before dispatching any task.

**When:** About to choose subagent-driven vs inline execution; scouting the
seams the plan's test code depends on.

**Expected (plan):** Task 2's `#[cfg(windows)]` test called
`run_command_inner(...)` with 10 positional args and built context via an
invented `test_ctx()` helper exposing `ctx.root()` / `ctx.security()`.

**Got (scouted reality):** `src/tools/run_command/tests.rs:364` defines the real
harness `async fn project_ctx() -> (tempfile::TempDir, ToolContext)`. Existing
tests (e.g. `run_command_cwd_works`, tests.rs:1364) drive the tool via the
public entry point `RunCommand.call(json!({...}), &ctx)`, **not** the
`run_command_inner` positional signature. There is no `test_ctx` / `ctx.root()`
/ `ctx.security()`. Also `src/platform/mod.rs` has **no** `#[cfg(test)] mod tests`
(Task 1's `build_windows_cmdline` test must create one).

**Probable cause:** Plan test code was written from the function signature in
`inner.rs` without scouting how the existing `tests.rs` suite actually
constructs context and invokes the tool.

**Workaround:** Revised plan Task 2 to use `project_ctx().await` +
`RunCommand.call(json!({"command":..., "run_in_background":true}), &ctx)` and
poll the `@bg_*` ref via a second `RunCommand.call(json!({"command":"type @bg_..."}), &ctx)`
on the same `ctx`. Task 1 step made explicit about adding the `mod tests` block.
Task 5 (Win32) test needs no ctx and was already correct.

**Severity:** med — a subagent given Task 2 verbatim would fail `cargo check`
(undefined `test_ctx`, wrong `run_command_inner` arity) and likely flail across
retries; controller would absorb the drift mid-dispatch.

**Status:** fixed-verified — plan revised before any subagent dispatch.

**Fix idea / Pointer:** Plan Task 1 + Task 2, this session.

---

## W-1 — Pre-dispatch scout of the test harness caught fictional test code

**Observed:** 2026-06-08, before selecting an execution mode for the VDI
reliability plan.

**Pattern:** Before dispatching the first subagent of a plan whose tasks
contain *test code*, scout how the target test module actually builds context
and invokes the unit under test (`grep "fn project_ctx"`, read one sibling
test) — not just the production function signature.

**Counterfactual:** Without this scout, the Task 2 subagent would have written
a test against `test_ctx()` / `ctx.root()` / a 10-arg `run_command_inner(...)`,
failed `cargo check` on the first run, and burned ≥1 retry cycle (likely more,
guessing at the real harness) before discovering `project_ctx()` +
`RunCommand.call(json!(...))`. On this VDI each build verification also costs
the move-exe-aside + background-build + `/mcp` dance, multiplying the cost of a
failed task.

**Confirming data points:**
1. F-1 (this session) — `test_ctx`/`ctx.root()`/positional `run_command_inner`
   cited by the plan do not exist; real harness is `project_ctx()` + `.call(json!)`.
2. Mirrors `code-explorer` W-2 (jsonpath plan): pre-dispatch scout of *type
   shape* caught fictional `.hint` field before any subagent ran.

**Impact:** med — saves ≥1 failed subagent task plus, on this VDI, a build/reload
cycle per failure.

**Promote-when:** A third pre-dispatch scout (any project) catches fictional
test-harness or type shape in plan code → promote to CLAUDE.md: "Before
dispatching the first subagent of a plan, scout the actual test-harness shape
(context constructor + invocation entry point), not just the production
signature."

**Status:** validated — single in-project datapoint, drift caught + fixed
before dispatch.

---

## F-2 — Implementer's broad `git add <file>` swept pre-existing WIP into the Task 1 commit

**Observed:** 2026-06-08, Task 1 spec-compliance review (subagent-driven execution).

**When:** Task 1 implementer ran `git add src/platform/windows.rs` and committed.

**Expected:** Commit contains only the Task 1 deliverable (`shell_command_configured` + `build_windows_cmdline`), ~8 new lines in windows.rs.

**Got:** windows.rs diff was +117/-7 — the commit also swept in pre-existing *uncommitted* WIP for `lsp_binary_name` (.exe/.cmd/.bat probing: `lsp_binary_name_with`/`find_on_path` + 5 tests) that was `M src/platform/windows.rs` in the session-start `git status`. The spec-compliance reviewer caught it (flagged "extra unrequested work") — the two-stage review did its job.

**Probable cause:** Briefing told the implementer to `git add` whole files; windows.rs already had unrelated uncommitted WIP from a prior session. Whole-file `git add` cannot exclude pre-existing hunks, and interactive `git add -p` is unavailable in this environment.

**Workaround:** The swept-in WIP is legitimate and in-scope (it is Task 7's binary-resolution work, bug 2026-06-06). Amended the commit message (`cd31ca76`, was `80fd8e7e`) to honestly describe both changes and re-scoped Task 7 to only the spawn/init timeout + bug-file close.

**Severity:** med — committed another session's WIP into a feature commit; caught + made honest, no data loss, but the commit is non-atomic.

**Status:** mitigated — message amended; not cleanly un-mixed (single file, no hunk staging available).

**Fix idea / Pointer:** Future implementer briefings must (1) run `git status` first and (2) `git add` only the specific files this task created/owns, never a file already showing as pre-existing `M`. If a task must edit a file with pre-existing WIP, the controller stages it. Plan Task 7 re-scoped to spawn/init timeout + bug close.

---

## F-3 — Task 7 spawn-level timeout deferred: `initialize()` already bounded, `spawn()` needs `spawn_blocking`

**Observed:** 2026-06-08, executing Task 7 (LSP spawn hardening) inline.

**When:** Verifying the two remaining Task 7 sub-steps after the binary-resolution
fix had already shipped (cd31ca76).

**Expected (plan Task 7 Step 3):** "Wrap the LSP `spawn()` + initialize handshake
in `tokio::time::timeout(...)`."

**Got (scouted reality):**
1. The **initialize handshake is already bounded** — `LspClient::initialize`
   (`src/lsp/client.rs:762`) calls
   `request_with_timeout("initialize", params, self.init_timeout)` with
   `init_timeout` defaulting to 30s, wrapped in a 5×-retry loop with
   `fatal_stderr_hint()` short-circuiting. No new init timeout is needed.
2. **`cmd.spawn()` cannot be meaningfully wrapped in `tokio::time::timeout`** —
   it is a synchronous call (`cmd.spawn().with_context(...)?`, no `.await`) that
   performs `CreateProcessW` on the calling thread. An EDR-induced hang blocks
   that thread; `tokio::time::timeout` only cancels *futures* and would not
   preempt the blocking syscall. A real spawn timeout requires
   `tokio::task::spawn_blocking` + abandoning the hung thread (the tokio blocking
   pool is bounded, so repeated hangs would exhaust it) — a design decision with
   its own trade-offs.

**Decision:** Per the plan's explicit STOP rule ("If this task grows beyond
'verify abs-path + add a bounded timeout,' STOP and split it into its own
spec/plan; ship Tasks 1-6 first"), the spawn-level timeout is **deferred** to a
dedicated spec. Task 7 ships with: abs-path resolution verified wired (✅,
cd31ca76) + init handshake confirmed already-bounded (✅) + bug
`2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md` (already `fixed`)
committed.

**Severity:** low — no regression; the riskiest sub-task was correctly scoped out
rather than bolted on. The bug the task set out to fix is resolved.

**Status:** documented — spawn-timeout split out; re-open trigger is an observed
LSP spawn hang on this VDI (none seen yet; the binary-resolution fix removed the
known failure mode).

**Fix idea / Pointer:** If a spawn hang is ever observed, open a spec for a
`spawn_blocking`-based bounded spawn with a thread-abandonment budget. Until
then, the cold-start retry budget (`cold_start_max_retries`, 20 on Windows) +
tree-sitter fallback already covers query-time degradation.

---

## W-2 — Treating Windows "environmental" test failures as suspects surfaced 2 real production bugs

**Observed:** 2026-06-09, making the Windows `cargo test` suite green.

**Pattern:** When the full suite showed 8 failures, the easy read was "Windows
test-harness rot — environmental, not our bugs." Instead each failure was
root-caused individually before being filed as environmental.

**Counterfactual:** Two of the failures were **real production bugs** wearing an
environmental disguise:
1. `is_denied` (`src/util/path_security.rs`) compared paths with
   `Path::starts_with`, but Windows `fs::canonicalize` returns `\\?\` verbatim
   paths while a not-yet-existing input stays plain — so `~/.ssh/id_rsa` never
   matched the verbatim deny prefix: a **silent deny-list bypass** (a security
   gate that doesn't fire). Surfaced only because `read_ssh_key_denied` failed.
2. `detect_project_root` (`src/util/fs.rs`) walked each marker across the *full*
   ancestry, so a `.git`/`.codescout` in a distant ancestor shadowed a nearer
   manifest. Surfaced only because the Windows `%TEMP%` tempdir sits under a
   marked home dir.
Filing all 8 as "environmental, won't fix" would have shipped both. Fixed in
`1d8cde48`.

**Impact:** high — one was a security-relevant deny-list bypass.

**Promote-when:** A second instance (any platform) where a "flaky/environmental"
test triage uncovered a real product bug → promote to CLAUDE.md: "Root-cause
each cross-platform test failure before filing it environmental; the env is
often the *trigger* that exposes a latent logic bug."

**Status:** validated — 2 real bugs found + fixed in one triage pass.

---

## W-3 — Adversarial architecture review (Snow Lion) before ship caught 2 spawn-wall gaps

**Observed:** 2026-06-09, `/buddy:summon lion` review of the reliability stream
before any cherry-pick to master.

**Pattern:** After the 8-task plan "completed", an independent architecture lens
re-read the call graph (not the diff) and asked "does the wall actually hold?".

**Counterfactual:** It found two gaps the task-by-task execution missed:
`BackgroundKillGuard::drop` still shelled out to `taskkill` (re-arming the exact
EDR-spawn anti-pattern Task 5 removed, in a cancellation Drop), and the
foreground `run_command` path built its `Command` inline on both platforms,
bypassing the new `shell_command_configured` builder. Both would have shipped as
"done." Fixed in `8c4c738f`.

**Confirming data points:**
1. W-3 (this session) — review caught taskkill-in-Drop + foreground bypass.
2. Mirrors the "cite the import, not the diagram" discipline now saved as the
   `architecture-snow-lion/platform-law-leaks-at-call-sites` buddy memory.

**Impact:** med — the kill-guard gap would stall cancellation under EDR.

**Promote-when:** A third instance where a post-"done" adversarial review catches
a real gap → promote "adversarial review before ship" into the Standard Ship
Sequence.

**Status:** validated — single in-stream datapoint, gaps caught + fixed.

---

## F-4 — Hidden-file test misdiagnosed as gitignore; real cause was an open write handle

**Observed:** 2026-06-09, fixing `check_index_scope_counts_hidden_non_gitignored_files`.

**When:** The test returned `Clear` (file_count 0) on Windows — the dotfile wasn't
counted.

**Expected (my first hypothesis):** a parent-repo `.gitignore` (dotfiles repo
ignoring `AppData/`) excluded the test file. Added a `git init` to isolate the
walker's git scope.

**Got:** The `git init` did **not** fix it. A throwaway diagnostic test
(walking the dir + calling `check_index_scope` directly) proved the `ignore`
walker *did* yield `.config.toml` with `ext="toml"`, and `check_index_scope`
returned `file_count: 1` when the file was written with `fs::write`. The only
difference from the failing test was that the test used `File::create` +
`write_all`, holding the handle **open** during the walk — on Windows the
walker's per-entry `metadata()` fails on an open write handle, so the file was
silently skipped. `git config core.excludesfile` was unset and `%USERPROFILE%`
wasn't even a git repo, so gitignore was never the cause.

**Probable cause:** Theorizing about the failure (gitignore) before running a
diagnostic. The platform-specific behavior (open-handle metadata failure) was
non-obvious and only a direct experiment disambiguated it.

**Workaround / fix:** Switched the test to `fs::write` (closes immediately),
removed the red-herring `git init` and the diagnostic. Fixed in `1d8cde48`.

**Severity:** med — cost one investigation cycle (a build) chasing the wrong
cause; the fix is one line.

**Fix idea / Pointer:** For a platform-specific test failure with a non-obvious
cause, write a 5-line diagnostic that prints the intermediate state *before*
theorizing — the empirical result is cheaper than a wrong hypothesis + its build.

---

## F-5 — `companion_surfaces` failure mis-triaged as peer-gating, then found to be tool-name drift

**Observed:** 2026-06-09, triaging the 6 remaining Windows failures.

**When:** Grouping failures into "caused by the peer cfg(unix) gating" (Group A)
vs "pre-existing environmental" (Group B).

**Expected (my triage):** `companion_surfaces_reference_only_real_tools` failed
because `peer` is absent on Windows, so I added a `peer` insert to the test.

**Got:** The drift output named `replace_symbol`/`insert_code`/`remove_symbol`/
`edit_lines`/`create_or_update_file` — never `peer`. The real cause was
**stale tool names in the codescout-companion hooks** (cross-repo), consolidated
away long ago. Reverted the speculative `peer` insert; fixed the hooks in
`codescout-companion:71aceeb`.

**Probable cause:** Assigning a cause from the failing test's *name* + the
surrounding batch theme (peer gating) instead of reading the assertion output
first.

**Severity:** low — caught and corrected within the same pass; speculative edit
reverted before commit.

**Fix idea / Pointer:** Read the actual assertion/drift output before assigning a
failure to a batch theme — the name and neighbours mislead.

---

## W-4 — Merge-base diff, not sibling-tip diff, scoped the Windows review to real changes

**Observed:** 2026-06-12, scoping the Linux-side review of the vdi-windows stack.

**Pattern:** When reviewing a feature branch that has diverged from its sibling, diff
against `git merge-base <sibling> <branch>`, never `<sibling>..<branch>`. The two-dot
sibling-tip diff conflates *what this branch changed* with *what the sibling advanced
after the fork*, reporting the latter as spurious deletions.

**Counterfactual:** `git diff experiments..vdi-windows` reported `src/lsp/manager.rs`
−230, `src/lsp/mux/process.rs` −155, and five doc files deleted — none of which
vdi-windows touched; they are the mux-single-owner-invariant refactor that landed on
`experiments` *after* vdi-windows forked. Reviewing that surface would have meant
auditing ~1800 phantom deletions (LSP-mux teardown logic) instead of the 559 real
Windows insertions. The merge-base diff (`0c84c1a4..vdi-windows`) showed
`src/lsp/manager.rs` at its true +4.

**Confirming data points:**
1. This session — phantom −1800 vs real +559; `manager.rs` −230 (phantom) vs +4 (real).

**Impact:** med — saves a whole misdirected review pass; also surfaced the
branch-divergence fact relevant to the eventual graduation rebase.

**Promote-when:** a second divergent-branch review where the sibling-tip diff misleads.
At 2 datapoints, promote to CLAUDE.md review guidance: "diff feature branches against
merge-base, not the sibling tip."

**Status:** validated — single datapoint, drift caught before any review effort wasted.

## W-5 — Pure platform logic belongs in `platform::mod`, not cfg-gated submodules

**Observed:** 2026-06-12, after the WIN-19 `.exe`-first fix landed in `cfg(windows) platform::windows`.

**Pattern:** Pure, side-effect-free platform logic (string/decision functions) should live in
the cross-platform `platform::mod`, with only the actual OS side-effect in the `cfg(windows)` /
`cfg(unix)` submodule. Then the logic's unit tests compile and run on *every* platform's CI, not
just the target one.

**Counterfactual:** `lsp_binary_name_with` (pure extension-resolution) + its 5 tests lived in
`#[cfg(windows)] platform::windows`, so the tests only ran on a Windows host. The WIN-19 fix was
therefore "verified by reasoning" on the Linux dev machine — no running assertion. Moving the
function to `platform::mod` (368aa9df) made the 5 tests run on the Linux gate (lib 2685→2690),
turning the fix into a checked behavior.

**Confirming data points:**
1. `build_windows_cmdline` — already in `platform::mod`, pure, Linux-tested (pre-existing; the model for this).
2. `lsp_binary_name_with` — moved this session (368aa9df); +5 Linux assertions covering WIN-19.

**Impact:** low per-instance, compounding — each pure-logic function kept cross-platform is
permanent CI coverage a Linux-only dev box would otherwise never exercise.

**Promote-when:** 2 datapoints reached. Promote to CLAUDE.md / a platform convention: "Pure
platform logic goes in `platform::mod` (cross-platform tested); only the OS side-effect goes in
the `cfg`-gated submodule." The next platform-logic addition should follow this by default.

**Status:** validated — 2 datapoints; ripe for promotion to a written convention.

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## F-2 — Implementer's broad `git add <file>` swept pre-existing WIP into the Task 1 commit

**Observed:** 2026-06-08, Task 1 spec-compliance review (subagent-driven execution).

**When:** Task 1 implementer ran `git add src/platform/windows.rs` and committed.

**Expected:** Commit contains only the Task 1 deliverable (`shell_command_configured` + `build_windows_cmdline`), ~8 new lines in windows.rs.

**Got:** windows.rs diff was +117/-7 — the commit also swept in pre-existing *uncommitted* WIP for `lsp_binary_name` (.exe/.cmd/.bat probing: `lsp_binary_name_with`/`find_on_path` + 5 tests) that was `M src/platform/windows.rs` in the session-start `git status`. The spec-compliance reviewer caught it (flagged "extra unrequested work") — the two-stage review did its job.

**Probable cause:** Briefing told the implementer to `git add` whole files; windows.rs already had unrelated uncommitted WIP from a prior session. Whole-file `git add` cannot exclude pre-existing hunks, and interactive `git add -p` is unavailable in this environment.

**Workaround:** The swept-in WIP is legitimate and in-scope (it is Task 7's binary-resolution work, bug 2026-06-06). Amended the commit message (`cd31ca76`, was `80fd8e7e`) to honestly describe both changes and re-scoped Task 7 to only the spawn/init timeout + bug-file close.

**Severity:** med — committed another session's WIP into a feature commit; caught + made honest, no data loss, but the commit is non-atomic.

**Status:** mitigated — message amended; not cleanly un-mixed (single file, no hunk staging available).

**Fix idea / Pointer:** Future implementer briefings must (1) run `git status` first and (2) `git add` only the specific files this task created/owns, never a file already showing as pre-existing `M`. If a task must edit a file with pre-existing WIP, the controller stages it. Plan Task 7 re-scoped to spawn/init timeout + bug close.

---

## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
