# Session Log — VDI Reliability Hardening

> Two-sided observation log for the VDI reliability work stream
> (spec: `docs/superpowers/specs/2026-06-08-vdi-reliability-hardening-design.md`;
> plan: `docs/superpowers/plans/2026-06-08-vdi-reliability-hardening.md`).

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-06-08 | med | plan-prose | fixed-verified | Plan Task 2 test code used a non-existent test harness shape |

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

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
