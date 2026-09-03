---
id: da299bf4cb8d7664
kind: bug
status: fixed
title: 'BUG: an overflowing doc() call never routed to the progressive-disclosure guide'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-09-04
opened: 2026-09-03
owner: marius
related:
- docs/trackers/tool-usage-patterns.md:T-33
severity: medium
---

## Summary

`LibrarianAdapter::relevant_guide_topic` (`src/librarian/adapter.rs`) chose only between
`"librarian"` and `"tracker-conventions"`, unconditionally — unlike `symbols`/`references`/
`call_graph`, which check for overflow before their own topic split. So any `doc(action="find"|"get")`
call that buffered into an `@tool_*` handle could never surface `progressive-disclosure`, the one
guide that explains how to read that handle. An agent hitting the overflow had no way to learn the
`@ref` mechanics from the auto-injected guide at the exact moment it mattered.

## Symptom (Effect)

`doc(action="find", kind="bug", filter={"status": {"in": ["open","taken","investigating","zombie"]}}, limit=100)`
overflowed into `@tool_69062af3` and injected `tracker-conventions` (because its `abs_path` fields
matched `docs/issues/`), never `progressive-disclosure`. Moments later, in the same session, native
`Read("@tool_69062af3")` was called on the handle and failed with `File does not exist` — the exact
failure `progressive-disclosure`'s existing "don't treat `output_id` as a filename" anti-pattern
bullet was meant to prevent, but the agent never saw that guide because the tool that produced the
overflow could not select it.

Logged as `T-33` in `docs/trackers/tool-usage-patterns.md`.

## Reproduction

At `dd9abd25` (`experiments`), via MCP:

```
doc(action="find", kind="bug",
    filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}}, limit=100)
```

Any call whose result both overflows (>~10 KB / `MAX_INLINE_TOKENS`) and whose `abs_path` matches
`docs/issues/` or `docs/trackers/` reproduces it — `tracker-conventions` wins every time,
`progressive-disclosure` never fires.

## Environment

Linux, codescout `experiments`, MCP transport (`codescout start --debug` spawned per Claude Code
session), rust workspace build.

## Root cause

Two layered defects, found and fixed in sequence.

**First layer.** `relevant_guide_topic(&self, result: &Value)` picked from a closed two-way split
(`names_tracker_path(result)` → `"tracker-conventions"`, else `"librarian"`) with no overflow branch
at all — unlike `Symbols`/`References`/`CallGraph`, none of which this adapter's original author had
cross-checked against. `src/librarian/adapter.rs:360-392` (pre-fix).

**Second layer, found only after live-testing the first fix.** The first fix (`e128816c`) copied
`Symbols::relevant_guide_topic`'s condition verbatim: `result.get("overflow").is_some() ||
result.get("output_id").is_some()`. That check works for `Symbols`/`References`/`CallGraph` only
because those three tools self-report `result["overflow"]` in their OWN truncation logic (see
`src/tools/symbol/symbols.rs`, `list_overview.rs`) before `relevant_guide_topic` is ever called.
`LibrarianAdapter` sets neither field on its raw pre-buffer value — `overflow` and `output_id` only
exist on the envelope `Tool::call_content` builds AFTER `relevant_guide_topic` returns, from the
generic byte-threshold path (`src/tools/core/types.rs::exceeds_inline_limit`, independently
re-checked by `PostCtx::overflowing` in `src/engines/coordinator.rs`). So the condition was `false`
on every call to `doc()`, overflowing or not — dead code.

*Measured 2026-09-04:* rebuilt (`cargo rb`) and reconnected (`/mcp`) after `e128816c` landed, then
re-ran the exact overflow-plus-tracker-path repro above. It still routed to `tracker-conventions`.

## Evidence

Live transcript, this session, after `cargo rb` + `/mcp` reconnect on `e128816c`:

```
doc(action="find", kind="bug", filter=..., limit=100)
→ output_id: "@tool_694a2d05", abs_path fields under docs/issues/
→ _guide_hint: "First call this session for topic 'tracker-conventions'."
```

`workspace(post_compact=true)` was used to reset the per-session guide-delivery ledger between
attempts, ruling out "already delivered earlier this session" as the explanation before concluding
the fix itself was inert.

`src/tools/symbol/symbols.rs:88` (`Symbols::relevant_guide_topic`) and
`src/tools/symbol/list_overview.rs:325,478,631,701` (`result["overflow"] = ...`) — the self-reporting
mechanism the first fix assumed `doc()` also had.

`src/engines/coordinator.rs` (`PostCtx::overflowing` doc comment) and `src/engines/emitters.rs`
(`emit_guide_sections`) — the coordinator's own independently-computed overflow gate, which only
ever sees `"progressive-disclosure"` as a candidate if `relevant_guide_topic` names it; nothing at
the coordinator layer supplies it on the tool's behalf.

## Hypotheses tried

1. **Hypothesis:** copying `Symbols`' `result.get("overflow") || result.get("output_id")` check into
   `LibrarianAdapter` is sufficient, since it works for the three symbol tools.
   **Test:** unit test constructing a `json!({"abs_path": ..., "output_id": "@tool_x"})` value and
   asserting `relevant_guide_topic` returns `"progressive-disclosure"`.
   **Verdict:** rejected, but only after this test passed and a full gate run passed — the test
   itself was the defect. It hand-inserted an `output_id` key no real `doc()` call ever produces at
   the point `relevant_guide_topic` runs, so it validated the function's logic against an input the
   production path never supplies. Caught only by live-testing against the rebuilt binary.
2. **Hypothesis:** the coordinator's separately-computed `ctx.overflowing` (`PostCtx`) already
   handles overflow-routing centrally, making any per-tool overflow check redundant.
   **Test:** read `emit_guide_sections` in full (`src/engines/emitters.rs`).
   **Verdict:** rejected — `ctx.overflowing` only ever *gates* `"progressive-disclosure"` once a
   tool's `relevant_guide_topic` (or `topic_declaring`'s section match) has already named it as a
   candidate. It supplies no candidate on its own.
3. **Hypothesis:** `LibrarianAdapter` can detect overflow the same way `Symbols` does, by checking
   for a self-reported `overflow`/`output_id` field.
   **Test:** grep `src/librarian/tools/find.rs` and sibling files for any `"overflow"` key
   construction on the raw pre-buffer result.
   **Verdict:** rejected — no such self-reporting exists in librarian's own result construction. The
   only place the condition can be computed correctly is by re-deriving `exceeds_inline_limit` on
   `result` directly, matching the formula `call_content` already uses for `PostCtx::overflowing`.

## Fix

Two commits, because the first one shipped the wrong condition and the second corrects it.

**`e128816c`** (superseded logic, kept for history) — added the overflow-first branch to
`relevant_guide_topic`, but with the copied-from-`Symbols` condition that never evaluates true for
`doc()`. Landed with a green gate and a passing unit test, both of which were blind to the defect
for the reasons in Hypothesis 1 above.

**`d71e0e08`** (the actual fix) — replaced the condition with
`crate::tools::exceeds_inline_limit(&serde_json::to_string(result)...) || result.as_object().and_then(|o| o.get("output_id"))...is_some()`,
the same formula `PostCtx::overflowing` computes in `src/engines/coordinator.rs`, recomputed here
because `relevant_guide_topic` only holds `result`, not the serialised `json` `call_content` already
has in scope. Also rewrote the regression test to overflow for real (an 11 KB padded payload,
asserted via `exceeds_inline_limit` directly) instead of a hand-inserted `output_id` key.

Both at `src/librarian/adapter.rs`.

- **SHA (superseded):** `e128816ca29ca65e5a7603d36b8166b5ef6cf472`, branch: `experiments`.
  **patch-id:** `410d780ff863e1ce12efe6b53eea09fe4610563e`
- **SHA (fix):** `d71e0e08c5083ea9a771821fbf4860f2ddc66a66`, branch: `experiments`.
  **patch-id:** `a59a758a15c69f4058cce11f149c5cd2ca9e8b8d`

## Tests added

- `librarian::adapter::tests::overflow_wins_the_guide_slot_even_on_a_tracker_path` — `src/librarian/adapter.rs`.
  Constructs a genuinely-overflowing (11 KB padded) result whose `abs_path` also matches the
  tracker-path branch, asserts `exceeds_inline_limit` confirms real overflow, then asserts
  `relevant_guide_topic` returns `"progressive-disclosure"` (not `"tracker-conventions"`).
  Mutation-verified twice: reverting to `e128816c`'s condition reds this rewritten test; the
  original (fake-`output_id`) version of the same test stayed green under that identical revert,
  which is the proof the first test never exercised the real defect.
- `librarian::adapter::tests::non_overflowing_tracker_path_still_gets_tracker_conventions` — companion
  test pinning that the pre-existing non-overflow split is untouched.

## Workarounds

None needed for callers — `get_guide("progressive-disclosure")` can always be fetched explicitly
regardless of auto-injection routing.

## Resume

N/A — fixed and gate-verified.

## References

- `docs/trackers/tool-usage-patterns.md:T-33` — the friction that started the investigation.
- `src/tools/symbol/symbols.rs`, `references.rs`, `call_graph/mod.rs` — the three tools whose
  self-reported `overflow` field the first (wrong) fix assumed `doc()` shared.
- `src/engines/coordinator.rs`, `src/engines/emitters.rs` — the coordinator layer that gates
  `"progressive-disclosure"` on `ctx.overflowing` regardless of what a tool names.
- Live verification: `cargo rb` + `/mcp` reconnect, this session, both before (still broken) and
  after (`d71e0e08`, fixed) the second commit.

