---
kind: bug
status: fixed
tags:
- cluster/declared-not-wired
- operator-rules
- routing
closed: 2026-08-31
opened: 2026-08-28
owner: marius
related:
- docs/issues/2026-08-28-op-4-path-predicate-can-never-fire.md
severity: medium
unverified: 'OP-2 is NOT covered and never can be by this mechanism. It declares `Serves: Agent, Task`, which are Claude Code HARNESS tools; codescout has no such tools (verified against the served surface and `ls src/tools/`), so they never enter call_content and no selector_key work can route them. For OP-2 the selector_key gap named as this bug''s root cause is not the binding constraint. Of the three triggered rules: OP-3 now routes; OP-4 has its routing precondition only and still cannot fire for an independent reason (see related); OP-2 is structurally unreachable. Also NOT verified end-to-end: the fix is covered by two tests meeting at a verified point — the real tools return Some(key), and route() delivers given a Some selector — but no test drives a real memory(action="write") call through call_content and asserts the OP-3 block appears.'
---

# BUG: Triggered operator rules route nothing in production — no non-librarian tool ever supplies a selector_key

## Summary
Every `binding: triggered` operator rule currently in the ledger (OP-2, OP-3, OP-4)
targets a tool that never produces a `selector_key` in production. `route()`
(`src/operator_rules/route.rs`) is therefore never invoked with a selector that could
match any of them on a real call. The Task 5 routing mechanism exists and is tested in
isolation, but nothing in production wires a selector to it for any tool these three
rules `Serves`.

## Symptom (Effect)
None user-visible — this is an absence, not a crash or error. The imperatives for OP-2
("Sonnet is the subagent-dispatch floor"), OP-3 ("Durable facts go to codescout memory,
never Claude Code's built-in memory"), and OP-4 ("Config changes apply to all three
Claude Code profiles") are never surfaced inline by the router on the calls they are
meant to guard, even though the ledger declares all three `status: active`.

## Reproduction
Not applicable in the usual sense — this is reproduced by inspection of the routing
mechanism's inputs, not by triggering a failure. `cargo test --lib operator_rules::` and
`cargo test --lib tools::core::tests::` both pass; the gap is that neither suite's
green result implies these three rules ever route on a *real* call, because both drive
`route()`/`call_content` with a hand-supplied selector rather than one produced by an
actual tool invocation. See Evidence.

## Environment
Branch `sdd/operator-rules-phase-2`, this worktree
(`/home/marius/work/claude/codescout/.claude/worktrees/operator-rules-phase-2`).
Not runtime/OS/transport-specific — a property of the tool trait implementations.

## Root cause
Walk the mechanism end to end:

1. `Tool::selector_key` (`src/tools/core/types.rs:1251-1253`) defaults to `None`:
   ```rust
   fn selector_key(&self, _input: &Value) -> Option<String> {
       None
   }
   ```
   The comment immediately above (`types.rs:1248-1250`) explains why: computing a
   selector costs a clone/scan on every call, and only ~3% of calls need to inject a
   guide.

2. `Shape::matches` (`src/prompts/guide_index.rs:179-180`) refuses to match at all when
   the selector is `None`:
   ```rust
   pub fn matches(&self, sel: Option<&str>, result: &Value) -> bool {
       let Some(sel) = sel else { return false };
   ```
   so a `None` selector short-circuits before any rule can match, regardless of tool or
   action.

3. `op_content` in `call_content` (`src/tools/core/types.rs:1135-1138`) only calls
   `route()` at all when the calling tool's own `selector_key()` returned `Some`:
   ```rust
   let op_content: Vec<Content> = if selector.is_some() {
       ...
       for r in crate::operator_rules::route::route(selector.as_deref(), &val) {
   ```
   (the `else` branch, added this session as a minor hot-path fix, skips the mutex
   lock and corpus scan entirely for the common case).

4. The only production override of `selector_key` is `LibrarianAdapter::selector_key`
   (`src/librarian/adapter.rs:190-195`):
   ```rust
   fn selector_key(&self, input: &Value) -> Option<String> {
       match input.get("action").and_then(Value::as_str) {
           Some(action) => Some(format!("{}.{}", self.name(), action)),
           None => Some(self.name().to_string()),
       }
   }
   ```
   `LibrarianAdapter` wraps every tool `crate::librarian::tools::all_tools()` returns —
   `adapters_for` (`src/librarian/adapter.rs:77-88`) maps each one through the adapter.
   That is a fixed, closed set: the librarian-crate tools (`artifact`, `get_guide`,
   `librarian`, and siblings), not every tool codescout serves.

5. The only other override is `RoutedEchoTool::selector_key`
   (`src/tools/core/tests.rs:1410-1415`), a `#[cfg(test)]`-only stub built specifically
   to exercise the router path inside `call_content`'s own tests. It is not reachable
   from any real call.

6. No other tool in the registry overrides `selector_key`. In particular:
   - `Agent`/`Task` are Claude Code's own tools — they are not `crate::tools::Tool`
     implementors in this process at all, so no override written in this codebase could
     ever reach them. Not "doesn't currently" — structurally cannot.
   - `Memory` (registered directly, `src/server.rs:342`, `Arc::new(Memory)`) is a real,
     local `Tool` impl, but it is not part of the `lib_all_tools()`/`adapters_for`
     wrapped set, so it inherits the `None` default — confirmed by `grep selector_key
     src/tools/memory/mod.rs` returning zero matches (measured 2026-08-28).
   - `edit_file`/`create_file` are likewise unwrapped and inherit the default. The
     companion bug file below found this independently, from the write-response-shape
     angle, before this file existed to name the selector-key blocker underneath it.

## Evidence
**`grep 'fn selector_key' -r src` (this session, 2026-08-28):** exactly two
non-test-double implementations exist in the whole tree — `LibrarianAdapter` and the
trait default — plus the one `#[cfg(test)]` stub (`RoutedEchoTool`).

**`grep selector_key src/tools/memory/mod.rs` (this session, 2026-08-28):** 0 matches —
`Memory`'s `impl Tool for Memory` (`src/tools/memory/mod.rs:585`) has no
`selector_key` method.

**Ledger `Serves` lines, quoted verbatim (this session, 2026-08-28):**
- `docs/trackers/operator-rules.md:63` — `**Serves:** Agent, Task` (OP-2)
- `docs/trackers/operator-rules.md:86` — `**Serves:** memory.write` (OP-3)
- `docs/trackers/operator-rules.md:107` — `**Serves:** edit_file(path~/.claude),
  create_file(path~/.claude)` (OP-4)

## Hypotheses tried
1. **Hypothesis:** `route()` might be called from somewhere else with a
   synthetically-constructed selector, bypassing `Tool::selector_key` entirely.
   **Test:** searched for `route(`/`route_in(` call sites.
   **Verdict:** rejected — the only production call site is the `op_content` block
   inside `call_content`, gated by `if selector.is_some()` where `selector` is the
   calling tool's own `selector_key()` result (`types.rs:1135-1138`).
   **Evidence link:** see Root cause point 3.
2. **Hypothesis:** `Memory` (or `EditCode`/`CreateFile`) might override `selector_key`
   somewhere not caught by a naive `impl Tool for X` search (e.g. a blanket impl, a
   macro-generated method).
   **Test:** `grep selector_key src/tools/memory/mod.rs`.
   **Verdict:** rejected — zero matches; the method is absent, so it resolves to the
   trait default.
   **Evidence link:** see Evidence, second entry.

## Per-rule conclusion
- **OP-2** (`Serves: Agent, Task`) — unreachable structurally. `Agent`/`Task` never
  enter this process as `Tool` implementors; no in-process `selector_key` override
  could reach them regardless of how it's written.
- **OP-3** (`Serves: memory.write`) — unreachable today, but not structurally:
  `Memory` is a real, local `Tool` impl that currently inherits the `None` default.
  Overriding `selector_key` on it would let `route()` see `Some("memory.write")` on
  the write path — see "Smallest fix" below for why this is a partial answer, not a
  full one.
- **OP-4** (`Serves: edit_file(path~/.claude), create_file(path~/.claude)`) —
  unreachable for the same reason as OP-3 (no `selector_key` override on
  `edit_file`/`create_file`), compounded by a second, independent blocker: even a
  synthetic `Some("edit_file")` selector cannot satisfy the `path~` predicate, because
  no write-tool response shape carries the written path. Full detail:
  `docs/issues/2026-08-28-op-4-path-predicate-can-never-fire.md`.

## Total anti-correlation
The set of tools that can ever produce a non-`None` selector today (the
`lib_all_tools()`/`adapters_for` closed set — librarian-crate tools only) and the set
of tools every currently-`triggered` ledger rule targets (`Agent`, `Task`, `memory`,
`edit_file`, `create_file` — `docs/trackers/operator-rules.md:63,86,107`) are disjoint.
Zero overlap: every `triggered` rule in the shipped ledger targets a tool outside the
only family that can ever produce a selector. This is not partial coverage where some
triggered rules route and others don't — it is total: as of this ledger, the Task 5
routing mechanism has no live `triggered` rule it can ever deliver in production.

(`OP-1`, for contrast, is `Binding: always` — delivered unconditionally by `op_content`
on every call, not selector-gated. It is unaffected by this bug.)

## Fix

**Fixed 2026-08-31 at `2447f709`** (patch-id `f83c6439691efb24ca790d00752e7cc7a43a74fe`),
with a follow-up at `a4968a13` correcting two comments the fix falsified.

`memory`, `edit_file` and `create_file` now override `selector_key`, projecting
`<tool>.<action>` via a shared `core::types::action_selector_key` so the opted-in tools
cannot drift apart. The `call_content` guard is `if selector.is_some()`, so supplying the
key is sufficient to reach `route()` — no change to the router was needed.

**What the suite was doing instead, which is the finding.** The routing tests were green
against `RoutedEchoTool`, a stub *named* `"memory"` projecting `{tool}.{action}` exactly as
`LibrarianAdapter` does, while the real `Memory` took the trait default and returned `None`.
A green suite and a dead feature were consistent with each other for as long as the stub was
the only caller. The two new tests assert against the real tools for that reason.

**Scope — read the `unverified:` field before treating this as closed.** OP-3 routes; OP-4
gets only its precondition and still cannot fire; OP-2 is unreachable from here for good.

*Not implemented — filing only, per this task's scope.* Any fix here is a design
decision (which tools get selector overrides and how, whether `Agent`/`Task` need an
entirely different delivery mechanism since they never enter this process) that
deserves its own task, not a fix folded into a routing-mechanics bug filing.

**Smallest fix, scoped as a note only:** overriding `selector_key` on `Memory` to
project `{tool}.{action}` (mirroring `LibrarianAdapter`'s own projection) is the
smallest change that would let OP-3 start routing. **Caveat, stated honestly:** this
only reinforces compliance for an agent that already calls the real `memory` tool with
`action: "write"` — it does not, and cannot, intercept the actual violation OP-3 exists
to prevent, which is an agent reaching for Claude Code's *built-in* memory feature
instead of ever calling codescout's `memory` tool at all. An agent that skips
codescout's `memory` tool entirely produces no call for `route()` to see, regardless of
any selector-key fix on this side. The rule can only ever catch agents that are already
halfway compliant (right tool, wrong action/target) — not the ones the imperative is
actually worried about. No equivalent smallest fix exists for OP-2 (structurally
unreachable) or OP-4 (two independent blockers — see the companion bug file).

*(Superseded — this pair read `N/A, no fix commit exists yet` while the file was still
`open`, and the fix landed afterwards. The declared anchors are in § *Fix provenance*
below. Kept rather than deleted because the paragraph above it is the scope decision that
made the N/A true at the time.)*

## Fix provenance

- **SHA:** `2447f709` (`experiments`)
- **patch-id:** `f83c6439691efb24ca790d00752e7cc7a43a74fe`
- **SHA:** `a4968a13` (`experiments`)
- **patch-id:** `01ee708b8db35c1918f463dc0cf642c2fe5b99a9`

`2447f709` is the fix; `a4968a13` corrects two comments it falsified. Scope is unchanged
from `unverified:` — OP-3 routes, OP-4 gains only its precondition, OP-2 is structurally
unreachable from here.
## Tests added
None — this is a filing-only bug report; no code changes accompany it. The existing
`op_4s_*` tests in `src/operator_rules/route.rs` already pin the predicate-level
behavior in isolation (their doc comments were extended this session to note the
synthetic-selector gap this file describes), but they cannot, without a fix, exercise
the real production call path this bug describes.

## Workarounds
None available at the tool-routing level. The imperatives OP-2/OP-3/OP-4 encode are
still enforced the old way — an agent reading `CLAUDE.md`/memory directly, exactly as
before Phase 2's routing work existed.

## Resume
Decide, as its own task: (a) whether `Memory` should get a `selector_key` override
(accepting the "reinforces, doesn't intercept" caveat above), (b) whether OP-2 needs a
different delivery mechanism entirely since it targets tools outside this process
(e.g. a Claude Code-side hook rather than an in-process one), and (c) whether the
write-tool-response-shape fix in the companion OP-4 bug file should land first, after,
or alongside any selector-key fix here. No code was touched filing this bug; `cargo
test` is unaffected.

## Related gap: `LEDGER_SRC` vs `LEDGER_PATH` staleness
Even a fully-wired selector-key fix would not make a ledger edit take effect without a
rebuild. `route()` reads from the compiled-in `OPERATOR_RULES` static
(`src/operator_rules/corpus.rs:23-25`), sourced from `LEDGER_SRC`
(`corpus.rs:16`):
```rust
pub const LEDGER_SRC: &str = include_str!("../../docs/trackers/operator-rules.md");
```
an `include_str!`, baked in at compile time. `compile`/`check` (the CLI-facing
validation path, `src/main.rs:463-468`) instead read `LEDGER_PATH`
(`src/operator_rules/mod.rs:24`, `"docs/trackers/operator-rules.md"`) fresh off disk on
every invocation. So editing the ledger and running `codescout operator-rules check`
validates against current bytes, while the routing path in an already-built binary
keeps using whatever was compiled in at the last build — a staleness window between
"the ledger file says X" and "the running binary routes X." This is a pre-existing,
deliberate property of the design (`corpus.rs:10-15` documents the *why*: routing must
be pinned to build time so a malformed ledger fails `cargo test` before any binary
ships, not surface as a runtime read failure) — not something this bug introduces —
but it compounds any future selector-key fix: shipping the fix and editing the ledger
in the same change is not sufficient on its own to make a new rule route without a
rebuild landing too.

## References
- `src/tools/core/types.rs:1251-1253` (`Tool::selector_key` trait default)
- `src/tools/core/types.rs:1135-1138` (`op_content` — the sole production call site of
  `route()`, gated on `selector.is_some()`)
- `src/prompts/guide_index.rs:179-180` (`Shape::matches` — a `None` selector never
  matches)
- `src/librarian/adapter.rs:190-195` (`LibrarianAdapter::selector_key`, the only
  production override)
- `src/librarian/adapter.rs:77-88` (`adapters_for` — wraps every librarian-crate tool)
- `src/tools/core/tests.rs:1410-1415` (`RoutedEchoTool::selector_key`, test-only
  double)
- `src/server.rs:342` (`Memory` registered directly, not wrapped by `adapters_for`)
- `src/tools/memory/mod.rs:585` (`impl Tool for Memory` — no `selector_key` override)
- `src/operator_rules/corpus.rs:16,23-25` (`LEDGER_SRC`, compiled in at build time)
- `src/operator_rules/mod.rs:24` (`LEDGER_PATH`, read fresh by `compile`/`check`)
- `src/main.rs:463-468` (`OperatorRules` CLI command reads `LEDGER_PATH`)
- `docs/trackers/operator-rules.md:63,86,107` (OP-2/OP-3/OP-4 `Serves` lines)
- `docs/issues/2026-08-28-op-4-path-predicate-can-never-fire.md` (companion bug — the
  OP-4-specific second blocker, and the write-response-shape angle on this same
  routing gap)
