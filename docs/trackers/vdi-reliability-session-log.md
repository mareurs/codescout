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

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-06-08 | med | Pre-dispatch scout of the test harness before subagent run | Task 2 subagent would fail cargo check + flail | validated |

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
