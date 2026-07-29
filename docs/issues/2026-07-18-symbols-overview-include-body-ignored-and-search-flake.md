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
> **RETRACTED 2026-07-28, later the same day — the narrowing below is WRONG, and
> Bug A is closed.**
>
> `symbols(path=<dir>, include_body=true)` **does** honor the flag. Re-ran the exact
> reproduction live against `src/peer`: **43 of 43 symbols carry a `body`**, doc
> comments included. Three independent signals said so and the verify pass consulted
> none of them — the 07-19 regression test
> `symbols_overview_honors_explicit_include_body_true` asserts
> `files[0].symbols[0].body` on the directory-scan branch specifically and has been
> green throughout; all three `list_overview.rs` branches read
> `optional_bool_param(&input, "include_body")` and thread it into `symbol_to_json`;
> and the response's own `buffered_bytes` was **56 915** — 43 symbols of names and
> line numbers are not 56 KB.
>
> What happened: the pass read the **compact summary** and took it for the result. The
> summary is a ~2 KB line-oriented preview that cannot render a body at any size; the
> full result sat in a `@tool_*` buffer named in the same response. This is the
> anti-pattern `docs/PROGRESSIVE_DISCLOSURE.md` lists by name — *"Treating the summary
> as authoritative. It's a preview, not the whole result"* — hit during the one
> activity whose entire purpose is to avoid it.
>
> A real defect survives one layer over, and is now fixed: `Symbols::json_path_hint`
> checked only the search-mode shape, so an overview carrying bodies advertised
> `$.files`, indistinguishable from an overview carrying none. For a buffered result
> that hint is the *only* signal in the envelope that `include_body=true` was honored.
> See *Fix*.
>
> **Bug B — search-mode intermittent 0-matches — remains open and unconfirmed.** It is
> the only reason this file is still open.
>
> The retracted text is kept verbatim below, unedited, for the record.
>
> ---
>
> **SCOPE NARROWED 2026-07-28 by a verify-open pass — still open, but half as wide.**
>
> The `include_body` claim is now **file-vs-directory dependent**, and only the
> directory case reproduces:
>
> | call | `include_body=true` honored? |
> |---|---|
> | `symbols(path="src/retrieval/index_lock.rs", include_body=true)` | **yes** — full bodies returned for all 8 symbols |
> | `symbols(path="src/lsp/mux/coherence_rust.rs", include_body=true)` | **yes** — the whole 74-line test body returned |
> | `symbols(path="src/peer", include_body=true)` | **no** — name/kind/line-range listing only, no bodies, no hint that the flag was dropped |
>
> So single-FILE overview honors the flag; DIRECTORY overview silently drops it.
> Whether dropping it for a directory is a deliberate size guard is unresolved — if
> it is, the defect is the *silence*, not the drop, and the fix is an overflow hint
> naming the constraint (per `docs/PROGRESSIVE_DISCLOSURE.md`) rather than honoring
> the flag. A directory of bodies would routinely blow the inline budget, so the hint
> is probably the right fix; that should be decided before any code changes.
>
> The second half of this bug — "search mode occasionally 0-matches then succeeds on
> retry" — is **not** addressed by this pass. It is intermittent by nature, so a
> single non-reproduction proves nothing. Kin to
> `docs/issues/2026-06-09-references-false-zero-stale-graph.md` (mitigated) and
> `docs/issues/2026-07-18-grep-glob-literal-path-false-negative-unconfirmed.md`
> (zombie) — all three are LSP/index staleness-window shapes and may share one root.

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

## Fix (2026-07-28): make a buffered envelope disclose that bodies are present

Bug A needed no fix — it was fixed on 07-19 and the flag has been honored since. The
defect the retraction exposed is one layer over, in disclosure rather than behaviour:
nothing in a buffered response told the caller that bodies were included.

`Symbols::json_path_hint` already pointed at `$.symbols[0].body` when a *search-mode*
result carried one. Overview mode nests a level deeper (`files[].symbols[].body`), so the
check fell through and the hint read `$.files`. Two responses that differ by 50 KB of
bodies produced byte-identical envelopes: same shape of summary, same hint.

Now it scans the `files` array for the first entry with a body and returns
`$.files[<i>].symbols[0].body`. Scanning rather than checking `files[0]` matters in
practice — a directory scan routinely leads with a `mod.rs` of bare re-exports, or a file
the language detector skipped, and checking only the first entry reproduces the same false
negative one index over.

Deliberately *not* done: adding a body-present note to the compact summary. The summary is
truncated to a hard cap, so a note appended there is exactly what disappears on the large
results that most need it, and `format_compact` also feeds the non-buffered render where
the claim would be wrong. The hint is not truncated and exists to answer "what do I read
next", which is the question being answered.

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

## Tests added (2026-07-28)

Two in `src/tools/symbol/tests.rs`, both asserting the hint *string* rather than the
presence of bodies — the bodies were never missing, the disclosure was:

- `symbols_json_path_hint_points_at_a_body_in_both_response_shapes` — all four
  combinations of {search, overview} × {body, no body}. The overview-with-body assertion
  carries the reason in its failure message: pointing at `$.files` reads as "no bodies".
- `symbols_json_path_hint_scans_past_leading_files_without_bodies` — a leading file with
  an empty `symbols` array, asserting the hint reaches `$.files[1].symbols[0].body`.

The 07-19 regression test `symbols_overview_honors_explicit_include_body_true` was left
alone. It was correct, it covered the directory branch, and it was green the whole time —
changing it would obscure that the test did its job and the reader did not.

## Tests added

**Bug A (fixed 2026-07-19):** `tools::symbol::tests::symbols_overview_honors_explicit_include_body_true`
in `src/tools/symbol/tests.rs` — asserts `symbols(path=<file>, include_body=true)` (no name/
query/symbol) returns non-empty body text for both the single-file overview branch and the
directory-scan branch.

Bug B has no proposed test yet — needs the controlled repro harness described above first.
