---
kind: bug
status: zombie
title: 'BUG: symbols search mode occasionally 0-matches then succeeds on retry (Bug A fixed in b2344aab; Bug B mitigated + instrumented)'
tags:
- symbols
- include_body
- overview-mode
- list_overview
- parameter-ignored
closed: null
last_observed: 2026-08-07
last_verified: 2026-08-26
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
> **Status: `zombie` as of 2026-08-07, by maintainer decision.** Bug A is fixed (`b2344aab`); Bug B
> is mitigated and instrumented, has not recurred, and its root cause is unconfirmed. There is no
> work available — it resolves only by firing again, and the instrumentation is already in place to
> catch it when it does. Kept open in the ledger rather than archived, which is what `zombie` is for
> (`docs/issues/_TEMPLATE.md`).
>
> **Re-open trigger — any ONE of these:**
>
> 1. A `symbols` search-mode call 0-matches and then succeeds on retry, in any session. The
>    instrumentation added for Bug B is what to read first; do not re-derive from scratch.
> 2. The diagnostic added for Bug B fires in `.codescout/diagnostic-*.log` without a
>    user-visible 0-match — that is the same defect caught earlier, and a better starting point
>    than the symptom.
> 3. An LSP-backed lookup elsewhere shows the same shape: an empty result that a retry fixes.
>    2026-08-07 established that this shape has a general cause — a language server answers
>    before it has finished indexing, and callers take the empty answer as authoritative. That
>    mechanism is now fixed in `audit_doc_refs` specifically (`resolve_file_symbol` cross-checks
>    tree-sitter before reporting a symbol absent), so if `symbols` search mode has the same
>    shape, the same remedy applies and this bug becomes actionable rather than trigger-gated.
>
> Trigger 3 is the one to watch: it converts this from "wait for a flake" into "apply a known fix",
> and it is the reason this is `zombie` rather than `wontfix`.

> **2026-08-06 — verify-open pass. Bug A confirmed closed at the source; Bug B still open, one more non-repro recorded.**
>
> Bug A re-verified independently of the 07-28 retraction: all three overview branches in `src/tools/symbol/list_overview.rs` (lines 231-232, 413-414, 566-567) read `optional_bool_param(&input, "include_body")` and fall back to the guard only when the caller passed nothing — the exact pattern `symbols.rs` uses. Nothing left to fix. The title has been narrowed to Bug B accordingly.
>
> Bug B repro attempt: 8 parallel search-mode `symbols(name=…)` calls fired in a single batch — `looks_like_path`, `nodes_to_chunks`, `apply_body_edits`, `upsert_tracker`, `build_basename_index`, `coalesce_small_chunks`, `is_placeholder`, `unique_basename_path`. **8/8 resolved**, including the last two, which had been *created minutes earlier in the same session* — so neither the LSP nor the symbol index was lagging behind fresh writes.
>
> **This is weak evidence and does not narrow the bug.** The batch was NOT fired immediately after a `workspace(action="activate")`, which both original occurrences had in common and which the root-cause hypothesis names as a precondition. A single non-repro of an intermittent fault proves nothing regardless; recorded only so the next session does not repeat the same inconclusive test and mistake it for progress. The controlled repro this file asks for — N parallel calls in a loop, immediately post-activation, counting 0-match rate — is still owed and is not expressible through hand-issued MCP calls.
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
> anti-pattern `docs/PROGRESSIVE_DISCOVERABILITY.md` lists by name — *"Treating the summary
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
> naming the constraint (per `docs/PROGRESSIVE_DISCOVERABILITY.md`) rather than honoring
> the flag. A directory of bodies would routinely blow the inline budget, so the hint
> is probably the right fix; that should be decided before any code changes.
>
> The second half of this bug — "search mode occasionally 0-matches then succeeds on
> retry" — is **not** addressed by this pass. It is intermittent by nature, so a
> single non-reproduction proves nothing. Kin to
> `docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md` (mitigated) and
> `docs/issues/archive/2026-07-18-grep-glob-literal-path-false-negative-unconfirmed.md`
> (fixed 2026-08-27). **Correction 2026-08-27:** that one is now evidence *against* the
> shared-root reading, not for it. Its cause was confirmed and it is not a staleness
> window: `OverrideBuilder` anchors globs at the resolved `search_path`, so a
> project-root-relative glob is unsatisfiable the moment `path` narrows the root. That is
> deterministic and reproducible on demand — the opposite of intermittent. It presented as
> a staleness flake only because a silent zero looks the same whatever produced it.

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

**Bug A — was real, and is FIXED in `b2344aab`** ("fix(symbols): honor explicit include_body in
overview mode"). All three overview branches now use the explicit-param-first pattern this
section proposed, so the code quoted just below **no longer exists** — it is kept as the record
of what was wrong. The earlier title called Bug A *retracted*, which reads as "the report was
mistaken": it was not. The defect was real and it was fixed.

**Original finding, as written:** `src/tools/symbol/list_overview.rs` never reads the
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

**Bug B:** Still not proposed — but **narrowed substantially by reading the search path**
(2026-08-06), without needing the harness first.

### Three silent-degradation paths, all confirmed in code

`search_project_symbols` (`src/tools/symbol/symbols.rs`) wraps each language in an 8 s
`PER_LANG_BUDGET`. Every failure mode yields *an empty vector that is indistinguishable
from "no matches"*:

1. **Budget exceeded** → `Ok(Vec::new())`, with the only signal a `tracing::warn!`. That
   goes to the log, not to the response, so the agent cannot see it.
2. **`get_or_start` error** → the `?` makes the task return `Err`, and the collector
   `let Ok(Ok(symbols)) = task_result else { continue };` skips it silently.
3. **Join error** → same `continue`.

### The author already named the mechanism

The comment above that budget says it outright:

> Per-language hard timeout: a pathological LSP state (**silent workspace/symbol on a
> still-indexing server**, init retry loop on a server that keeps crashing) must not hang
> the whole tool call past the MCP 60 s ceiling. On timeout we yield an empty result for
> that language; the tree-sitter fallback below still runs if every language produces
> nothing.

"Silent workspace/symbol on a still-indexing server" is exactly the observed symptom: a
freshly-started server answers `workspace/symbol` with `[]` — a *valid* response meaning
"nothing indexed yet" — and a retry after indexing completes succeeds.

### The one thing that does NOT yet add up, and it is the next question

There **is** a tree-sitter fallback, gated on the aggregate being empty. If it runs
whenever total matches are zero, a 0-match on an existing symbol in a parseable file
should have been covered by it. So either the fallback's gate is narrower than that
comment implies, or the flake arises when **one language returns matches while another
degrades** — a non-empty aggregate that suppresses the fallback while still being wrong.

**Answered, same session — and it rules the LSP-warming story OUT.** The gate is
`if matches.is_empty()`, where `matches` holds only symbols that already passed the
name/kind/in-root/in-walk filters. So the fallback runs whenever *the final answer would
be zero*, regardless of which languages degraded or how many raw symbols came back. The
partial-success hypothesis is dead too: non-matching LSP results do not populate `matches`
and therefore do not suppress the fallback.

That is load-bearing, because it means **a 0-match result implies tree-sitter also found
nothing** — and tree-sitter does not touch the LSP. No amount of "the server was still
indexing" explains it.

### The hypothesis that does fit, and it is a different bug class

Both paths key off the same `root`, resolved once at
`src/tools/symbol/symbols.rs:225` via
`ctx.agent.require_project_root_for(ctx.workspace_override.as_deref())`. If that root is
not yet the intended project at the moment of the call — the symptom is specifically
*"immediately post-activation"* — then the LSP is queried against one tree and the
tree-sitter walker walks the same wrong tree, and **both legitimately return nothing**. A
retry once activation has settled then succeeds. That is the only story so far that
explains a zero from *both* independent paths.

It also has precedent in this repo: `docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md`
(fixed, archived) is the same class — active-project state read before it settled on a
shared server.

### What this changes for whoever picks it up

The originally-proposed harness — "N parallel `symbols(name=X)` calls immediately
post-activation" — is still the right experiment, but instrument it for the **root**, not
for LSP readiness: log the resolved `root` alongside each 0-match and compare it to the
intended project. If they differ on the failing calls, the fix is in activation ordering,
not in `symbols` at all, and the disclosure work below becomes secondary.

Building an LSP-warming harness would have measured the wrong variable.

### Fix direction once that is settled

There is already a house convention for exactly this shape, used twice, so the disclosure
question needs no invention:

- `references.rs` sets `completeness_warning` when the LSP returns 0 references but the
  identifier appears elsewhere by grep — *"the reference index may still be warming after
  a reindex. Re-run, or corroborate with grep…"*;
- `list_overview.rs` sets `"lsp": "warming"` plus a hint, and `display.rs` renders
  `[lsp warming]`.

Search mode should do the same: a zero-or-suspect result that coincided with a degraded
language must say so in the **response**, not only in `tracing`. The harm this bug causes
is not the retry — it is an agent concluding "this symbol does not exist" from an answer
that actually meant "not indexed yet".

**Deliberately not implemented here.** `symbols` is the most-used tool in the server, and
the root cause is narrowed but not confirmed; patching the disclosure before knowing which
of the paths above fires risks papering over the real one. Confirm the fallback gate first.

### Bug B — the walk was discarding its own errors, and that is now fixed (2026-08-07)

Reading the two walks turned up a defect neither hypothesis had named. Both used
`walker.flatten()`:

- the accepted-files walk that every LSP match is gated on via `in_walk`, and
- the tree-sitter fallback walk.

`ignore::Walk` yields `Result<DirEntry, Error>`, so **`.flatten()` silently discarded every
I/O error**. A walk truncated by fd exhaustion, a permission error, or any transient made a
*partial* tree look like a *complete* one — and because both search paths key off that same
walk, one truncation zeroes both at once.

That fits the observed conditions better than the root race this file previously favoured: the
failures happened inside a **parallel batch**, where N concurrent recursive walks plus LSP
servers compete for file descriptors, and every recovery was a **solo** retry. The root race
remains possible and is still covered — a wrong root yields `accepted == 0`, which now reports
itself distinctly.

**What changed.** A `WalkAudit` (`errors`, `accepted`) is threaded through
`search_project_symbols`; both `Err` arms count and log instead of dropping. On a zero result
the response carries `completeness_warning` — the existing house convention from
`references.rs` — rendered by `format_search_symbols` in **both** its branches:

| walk state | what the caller sees |
|---|---|
| clean, sources accepted | bare `0 matches` — a trustworthy answer about the symbol |
| entries unreadable | warning naming the root, the count, and "may be a false negative" |
| 0 source files accepted | warning pointing at the active project rather than the symbol |

The **`None` case is the load-bearing half**: warning on every zero would be noise, and the
reader would learn to skip the warning that matters.

**Two things deliberately NOT done.**

- **The two walks were not merged**, despite applying an identical filter (`is_file` +
  `detect_language`). They are a *recovery path*, not duplication: if walk 1 truncates, every
  LSP symbol is filtered out, `matches` goes empty, and that is precisely what *triggers* the
  fallback — so a complete walk 2 can still find the symbol. Collapsing them would remove the
  second chance and make this bug strictly worse.
- **The zero branch of `format_search_symbols` returns early**, so the warning is appended
  before that return. `references.rs` already paid for this lesson (its BUG 2026-05-21 comment:
  surface the warning in both the zero and normal branches) — a warning that renders only
  alongside results explains nothing about the result that needed explaining.

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

Ten, on `experiments`.

**Unit — `walk_audit_tests` in `src/tools/symbol/symbols.rs`:**

- `a_clean_walk_over_a_populated_tree_produces_no_warning` — the over-match guard that keeps
  the warning meaningful.
- `an_unreadable_entry_makes_the_zero_suspect_and_names_the_root` — including singular/plural,
  since an agent reads this string to decide whether to trust a zero.
- `zero_accepted_files_points_at_the_root_rather_than_the_symbol`.
- `a_failed_and_empty_walk_reports_the_failure_not_the_root` — precedence, so a filesystem
  failure does not send the reader off to check activation.

**Renderer — `src/tools/symbol/tests.rs`:**

- `format_search_symbols_surfaces_completeness_warning_on_zero_matches`
- `format_search_symbols_leaves_a_trustworthy_zero_bare` (over-match guard)
- `format_search_symbols_surfaces_completeness_warning_alongside_results`

**End-to-end through the real walk — `src/tools/symbol/tests.rs`:**

- `search_on_a_tree_with_no_source_files_says_so_instead_of_a_bare_zero`
- `search_on_a_populated_tree_returns_a_bare_zero_for_a_missing_symbol` — with a positive
  control proving the fixture really is searchable, so the bare zero is a statement about the
  symbol rather than a broken fixture.
- `an_unreadable_directory_is_counted_rather_than_silently_dropped` (`#[cfg(unix)]`) — the
  reproducible stand-in for the transient, guarded to skip where mode bits are ignored (root)
  instead of asserting a precondition it cannot establish.

**Verified by mutation, not by passing.** Reverting the early-return warning in
`format_search_symbols` kills the zero-branch test and nothing else. Neutralising the error
counting kills the unreadable-directory test.

**One coverage limit, stated rather than glossed.** The unreadable-directory test pins that
errors are counted *somewhere* in the search, not that *both* walk sites count. That surfaced
because an incomplete mutation — the two `Err` arms sit at different indentation depths, so a
single-indentation `replace_all` matched only one — left one counter live and the test still
passed. Both sites are the same two-line construction exercised in the same call, so it is
left as is. Worth remembering that "the mutation did not kill it" can mean the mutation was
incomplete, not that the test is bad.

Gate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, 3515 passed / 0 failed
/ 44 ignored.

## Resume

**Status is deliberately `open`, not `mitigated` or `fixed`.** A real defect was found and
fixed (walk errors discarded) and the *harm* is neutralised (a false zero now announces
itself) — but the originating symptom has never been reproduced, so which path actually fired
in the two observed failures is unconfirmed. `mitigated` would be defensible; it would also
drop this file out of `artifact(find, kind="bug", status in [open, investigating])`, and a
recurring flake with an unconfirmed cause is better left visible to triage.

**What would close it.** The next occurrence is now self-diagnosing — that is the whole point
of the change. When a 0-match happens again, read the `completeness_warning`:

- *"could not read N entries"* → the walk-truncation hypothesis is confirmed. Bound the cause
  next: compare `ulimit -n` against the number of concurrent `symbols` calls plus running LSP
  servers. Flip to `fixed` if the count explains it.
- *"accepted 0 source files"* → the **root race** is confirmed instead, and the fix belongs in
  activation ordering, not in `symbols`. Precedent:
  `docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md`.
- **No warning at all, on a symbol that provably exists** → both hypotheses are wrong and a
  third mechanism is at work. That is the most informative outcome of the three, and it is one
  this file previously could not distinguish from either.

**Bookkeeping.** The fix SHA is on `experiments` only. The current promotion is a fast-forward,
which mints no new SHAs, so the citation stays valid as written once `master` catches up.
