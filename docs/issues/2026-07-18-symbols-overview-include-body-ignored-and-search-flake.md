---
kind: bug
status: open
tags:
- symbols
- include_body
- overview-mode
- list_overview
- parameter-ignored
closed: null
opened: 2026-07-18
owner: marius
related:
- '2026-06-11-symbols-search-include-docs-and-focus.md'
severity: medium
---

> **2026-07-19 update:** Bug A fixed — all three `list_overview.rs` branches now read
> `optional_bool_param(&input, "include_body")` before falling back to the guard default,
> matching `symbols.rs`. Regression test `symbols_overview_honors_explicit_include_body_true`
> added covering both the single-file and directory-scan overview branches. Bug B (search-mode
> intermittent 0-matches under parallel calls) remains open/unconfirmed — no harness built yet.

# BUG: `symbols` overview mode silently ignores `include_body=true`; search mode occasionally 0-matches then succeeds on retry

## Summary

Two distinct frictions hit while using `symbols` against a real project (Mercury MRP
Automation, a sibling workspace project) during normal reconnaissance work, not a
targeted bug hunt. **Bug A** (confirmed via source read): overview mode (`symbols(path=...)`
with no `name`/`query`/`symbol`) never applies the caller's `include_body=true` — it always
falls back to `guard.should_include_body()` instead, so bodies are silently dropped even
though the parameter schema advertises `include_body` as a general input. Search mode
(`symbols(name=...)`) reads the explicit param correctly, so this is a one-sided regression/gap
in the overview code path, not a global "feature not implemented." **Bug B** (reproduced twice,
root cause unconfirmed): search-mode `symbols(name=<X>, include_body=true)` returned "0
matches" for a symbol that unambiguously exists in the given file, then succeeded on an
identical solo retry a few tool-calls later. Both occurrences were part of a batch of several
parallel `symbols` calls fired in the same turn, shortly after the target project was
(re-)activated via `workspace(action="activate")`.

## Symptom (Effect)

**Bug A** — overview mode, `include_body=true` passed explicitly, no bodies returned:

```
symbols(path="src/mrp/engine/past_due.py", depth=1, force_mode="symbols", include_body=true)
→ Function open_balance (43-45)
    Variable open_balance/orders
  Function is_past_due (48-51)
    Variable is_past_due/orders
    Variable is_past_due/as_of
    ...
(23 symbols listed with line numbers, zero body text anywhere in the response)
```

Same file, same function, **search mode** with the same `include_body=true` — bodies ARE
returned:

```
symbols(name="is_past_due", path="src/mrp/engine/past_due.py", include_body=true)
→ def is_past_due(orders: pd.DataFrame, as_of) -> pd.Series:
      """ETA-0008: Open Supply Order Date is before ``as_of`` (Past Due)."""
      as_of_ts = pd.Timestamp(as_of)
      return orders[DOCK_DATE].notna() & (orders[DOCK_DATE] < as_of_ts)
```

**Bug B** — intermittent "0 matches" then success on retry, verbatim:

```
[call, one of 4 parallel `symbols` calls in the same turn]
symbols(include_body=true, name="is_past_due", path="src/mrp/engine/past_due.py")
→ 0 matches

[a few tool-calls later, solo call, identical params]
symbols(include_body=true, name="is_past_due", path="src/mrp/engine/past_due.py")
→ src/mrp/engine/past_due.py (1)
  Function  48-51  is_past_due
      def is_past_due(orders: pd.DataFrame, as_of) -> pd.Series:
      ...
```

Second, independent occurrence in the same session, different file/symbol, same pattern:

```
[call, one of 5 parallel `symbols` calls in the same turn]
symbols(include_body=true, name="next_working_date", path="src/mrp/engine/shutdown.py")
→ 0 matches

[later, solo call, identical params]
symbols(include_body=true, name="next_working_date", path="src/mrp/engine/shutdown.py")
→ src/mrp/engine/shutdown.py (1)
  Function  58-68  next_working_date
      def next_working_date(iso_date: str, plant: str, calendar: ShutdownCalendar) -> str:
      ...
```

## Reproduction

Git commit: `beba4a7033cd174a898f30777c1fd58c91814a4b` (codescout, this session).

**Bug A (100% reproducible, verified twice independently — once by a subagent, once by me):**
1. Open any project with a Python (or presumably any-language) source file containing
   multiple functions — used `src/mrp/engine/past_due.py` in the Mercury MRP Automation
   sibling project.
2. Call `symbols(path=<file>, include_body=true)` with NO `name`/`query`/`symbol` argument
   (overview mode). Tried `depth=1` and `depth=2`, with and without `force_mode="symbols"` —
   all combinations reproduce.
3. Observe: full symbol tree with line numbers, zero body text.
4. Contrast: `symbols(path=<file>, name=<any function in that file>, include_body=true)` on the
   same file correctly returns the body.

**Bug B (reproduced twice, not yet reliably reproducible on demand):**
1. Recently activate a project via `workspace(action="activate", path=...)`.
2. In the same agent turn, fire several `symbols(name=..., include_body=true)` calls in
   parallel (as part of a normal multi-file reconnaissance batch — not a synthetic stress
   test).
3. One or more of the parallel calls may return "0 matches" for a symbol that demonstrably
   exists (confirmed by an immediate, unbatched retry with identical parameters succeeding).
4. Not yet confirmed whether parallelism, recent project activation, or both together are
   required to trigger it — only 2 data points, both matching this pattern, no controlled
   isolation attempted yet.

## Environment

- codescout, commit `beba4a7033cd174a898f30777c1fd58c91814a4b`
- MCP stdio transport, VS Code Copilot Chat host
- Windows
- Target project: Mercury MRP Automation (sibling workspace project, Python), files
  `src/mrp/engine/past_due.py` and `src/mrp/engine/shutdown.py`

## Root cause

**Bug A — confirmed via source read.** `src/tools/symbol/list_overview.rs` never reads the
caller's `include_body` input in any of its three overview branches:

```
list_overview.rs:229:  let include_body = guard.should_include_body();
list_overview.rs:404:  let include_body = guard.should_include_body();
list_overview.rs:556:  let include_body = guard.should_include_body();
```

All three ignore whatever the caller passed and use the `OutputGuard`'s default instead. Grep
for `include_body` in that file turns up only these three assignments plus their immediate
downstream uses (`source = if include_body {...}`, `symbol_to_json(..., include_body, ...)`) —
the input parameter itself is never consulted.

Contrast with the correct pattern in `src/tools/symbol/symbols.rs:220-221` (search-mode entry
point):

```
let include_body_explicit = optional_bool_param(&input, "include_body");
let include_body = include_body_explicit.unwrap_or_else(|| guard.should_include_body());
```

This explicitly reads the input first and only falls back to the guard default when the
caller didn't pass anything. `list_overview.rs`'s three call sites should follow the same
pattern.

**Bug B — not confirmed.** Hypothesis only: some race between parallel `symbols` search-mode
calls and either (a) LSP/index warm-up shortly after a project activation, or (b) a shared
cache/index that isn't yet coherent across concurrent readers. Both observed failures
happened inside a parallel batch shortly after `workspace(action="activate")`; both recoveries
were solo, non-batched retries. Two data points is not enough to isolate parallelism vs.
recency-of-activation vs. coincidence — needs a controlled repro (e.g. script N parallel
`symbols(name=X)` calls immediately after activation in a loop and see how often "0 matches"
appears) before treating this as more than a hypothesis.

## Evidence

**Bug A:**
```
grep "include_body|should_include_body" src/tools/symbol/list_overview.rs
→ 13 matches, all either `let include_body = guard.should_include_body();` (3x, lines 229/
  404/556) or downstream uses of that same local — no read of the input parameter anywhere.

grep "include_body|should_include_body" src/tools/symbol/symbols.rs
→ line 220-221 explicitly reads `optional_bool_param(&input, "include_body")` before falling
  back to the guard default — the correct pattern, present in search mode only.
```

**Bug B:** session transcript — two occurrences of "0 matches" immediately followed by an
identical-params retry succeeding, both inside a batch of 4-5 parallel `symbols` calls issued
shortly after a `workspace(action="activate")` call in the same turn sequence.

## Hypotheses tried

1. **Bug A: `include_body` is an unimplemented/dead parameter everywhere.** Rejected — search
   mode (`symbols.rs:220`) honors it correctly; only the three overview-mode branches in
   `list_overview.rs` ignore it.
2. **Bug A: overview mode intentionally omits bodies for size/performance reasons, and this is
   by design rather than a bug.** Possible, but if so it's undocumented — the tool's own
   `input_schema` advertises `include_body` as a general boolean with no carve-out noting it's
   ignored in overview mode, and the overview mode's own hint text (`list_overview.rs:445,461`)
   tells callers to use `symbols(symbol='...', include_body=true)` for a body, which reads as
   "overview mode doesn't do bodies, ask search mode" — but the caller has no way to know that
   without hitting this exact failure mode once. At minimum this needs either honoring the
   param or a clearer error/rejection when `include_body=true` is combined with no `name`/
   `query`/`symbol`, instead of silently succeeding without the requested data.
3. **Bug B: the symbol genuinely didn't exist at call time (e.g. stale file).** Rejected — the
   file was read moments earlier via other tools in the same session and the function was
   unambiguously present; the identical retry succeeding with no code changes in between rules
   out a real absence.
4. **Bug B: name-matching is case/whitespace-sensitive and the first call had a typo.**
   Rejected — verified the exact same string was used in both the failing and succeeding
   calls (copy-pasted from the earlier failing call for the retry).

## Fix

**Bug A:** In `list_overview.rs`, replace all three occurrences of
`let include_body = guard.should_include_body();` with the same explicit-param-first pattern
already used in `symbols.rs:220-221`:
```
let include_body_explicit = optional_bool_param(&input, "include_body");
let include_body = include_body_explicit.unwrap_or_else(|| guard.should_include_body());
```
If overview mode is intentionally meant to never include bodies regardless of caller intent,
the alternative fix is to reject/ignore-with-a-visible-hint rather than silently drop the
data — but the more likely intended behavior, given the parameter is caller-visible and
generic, is to honor it.

**Bug B:** Not proposed — root cause unconfirmed. Needs a controlled, scripted reproduction
(N parallel `symbols(name=X)` calls immediately post-activation, repeated across several
projects/runs) before a fix can be targeted. Recommend `status: open` rather than
`investigating` until someone has bandwidth to build that harness.

## Tests added

**Bug A (fixed 2026-07-19):** `tools::symbol::tests::symbols_overview_honors_explicit_include_body_true`
in `src/tools/symbol/tests.rs` — asserts `symbols(path=<file>, include_body=true)` (no name/
query/symbol) returns non-empty body text for both the single-file overview branch and the
directory-scan branch.

Bug B has no proposed test yet — needs the controlled repro harness described above first.
