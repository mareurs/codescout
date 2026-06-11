---
status: fixed
opened: 2026-06-11
closed: 2026-06-11
severity: medium
owner: marius
related: []
tags: [symbols, include_docs, progressive-disclosure, tool-ux]
kind: bug
---

# BUG: symbols search silently ignored include_docs; single-symbol search returned a bare locator

## Summary
Calling `symbols(symbol="X", include_docs=true)` returned only a locator with no
docstring — `include_docs` was consumed solely by the overview path, and the
search path dropped it silently. Separately, a search resolving to one large
symbol returned a bare `kind/range/name` line with no way to see the code without
a second `include_body=true` round-trip.

## Symptom (Effect)
Observed against `backend-kotlin`:

```
symbols(symbol: "LegalLimitConstraints", path: ".../LegalLimitConstraints.kt", depth: 1, include_docs: true)
→ Object  51-524  LegalLimitConstraints
```

The file has a 30-line object-level KDoc plus per-member KDoc, yet no docs were
returned, and the 473-line object showed neither body nor member shape.

## Reproduction
- `git rev-parse HEAD` at fix: `5927b65d` (experiments).
- Any documented symbol, search mode (a `name`/`symbol`/`query` arg present):
  `symbols(query="Foo", include_docs=true)` — pre-fix: no `docs` field. Post-fix:
  `docs` attached. A single match also auto-shows its code (leaf body, or large
  container members).

## Environment
codescout v0.15.0, MCP stdio, Linux. (Live Kotlin repro was blocked by the
Kotlin LSP cold-start — `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`
— but the cause was confirmed statically from source and reproduced on the
`tests/fixtures/kotlin-library` fixture + Rust integration test.)

## Root cause
`symbols` is two code paths behind one tool name, split on whether a name arg was
passed (`src/tools/symbol/symbols.rs:148` — `if !has_name_arg { return list_overview(...) }`):

1. **Overview** (`path` only) → `src/tools/symbol/list_overview.rs`, which reads
   `include_docs` (line 201) and attaches a file-level `docstrings` array.
2. **Search** (name given) → `symbols.rs` call logic, which read `include_body` /
   `depth` / `kind` but **never `include_docs`** — so it was silently dropped.

Compounding: `symbol_to_json` (`src/symbol/query.rs:102`) has no `docs` field —
symbols never carried their own docstring; and a single search match got only a
locator unless it was ≤2 matches **and** ≤40 lines (`auto_inline_small_bodies`).
A 473-line object satisfied neither, so: bare locator.

## Evidence
- File has rich KDoc — confirmed by reading source lines 21-50 (object) + 60-72
  (member `dailyHoursInRange`).
- Search path never references `include_docs` — grep of `symbols.rs` shows it only
  in the description (line 96) and schema (line 128), not the `call()` body.
- `format_search_symbols` (`src/tools/symbol/display.rs`) rendered only
  `kind/range/name` + an inlined `body`; it had no notion of `docs` or `children`,
  so even an enriched JSON would not display in compact view.

## Hypotheses tried
1. **`include_docs` is a dead param everywhere.** Test: grep consumers. Verdict:
   **rejected** — consumed in `list_overview.rs`, just not in the search path.
2. **The container has no extractable children (extraction bug).** Test: overview
   on `tests/fixtures/kotlin-library`. Verdict: **rejected** — Kotlin members nest
   correctly (`Object BookRegistry` → `Property/Method`); the call-1 empty-children
   was a cold-LSP artifact.
3. **Misconfiguration (debug off on some projects).** Test: `.claude.json` on all
   profiles. Verdict: **rejected** (separate finding) — uniform `--debug`.

## Fix
Implemented in `src/tools/symbol/symbols.rs` + `src/tools/symbol/display.rs`.
Experiments-side commit `5927b65d` — **not yet on master**; update to the
master-side SHA after cherry-pick (CLAUDE.md § "After cherry-pick").

Two post-passes on the search `matches` vec (mirroring `auto_inline_small_bodies`,
so `symbol_to_json`'s signature is untouched):
- **`attach_docstrings`** (gated on `include_docs`) — attaches each matched
  symbol's own docstring as a `docs` field; associates by `DocstringInfo.symbol_name`
  with a ≤3-line proximity fallback. Makes `include_docs` work in search mode.
- **`focus_single_symbol`** (single match, no explicit `include_body`) — leaf →
  inline full body (oversized bodies overflow to `@ref` via progressive disclosure);
  large container (>80 enclosing lines) → direct-member signatures (`children`) via
  `extract_symbols` + a drill-in `members_hint`; small container → body.

`format_search_symbols` now renders `docs` (as `//` comment lines), member
`children`, and `members_hint`. Schema/description updated to drop "overview only"
and document the single-symbol focus. No `ONBOARDING_VERSION` bump (live tool
description/schema; onboarding slice + `builders.rs` untouched).

## Tests added
In `src/tools/symbol/tests.rs`:
- `include_docs_attaches_docs_in_search_mode` — **end-to-end** via `Symbols::call`
  (proves the post-pass runs on the live path, not just the helper).
- `focus_single_symbol_inlines_large_leaf_body`
- `focus_single_symbol_shows_members_for_large_container`
- `attach_docstrings_attaches_symbol_doc`
- `format_search_symbols_renders_docs_members_and_hint`
All pass (290 `tools::symbol` tests green); clippy clean; release build verified.

## Workarounds
Pre-fix: pass `include_body=true` to see a body; for docs, use overview mode
(`symbols(path=file, include_docs=true)`) and correlate the `docstrings` array by
`symbol_name`.

## Resume
N/A — fixed. Possible follow-ups: (1) overview mode still returns docs as a
separate file-level array rather than per-symbol; (2) `symbols` search ignored a
`workspace=` pin in one observation (overview path-existence check resolved against
the active project) — worth a dedicated tracker if it recurs.

## References
- Fix: `src/tools/symbol/symbols.rs` (`focus_single_symbol`, `attach_docstrings`),
  `src/tools/symbol/display.rs` (`format_search_symbols`); commit `5927b65d`.
- Path split: `src/tools/symbol/symbols.rs:148`; overview: `src/tools/symbol/list_overview.rs:201`.
- Session log: `docs/trackers/symbols-focus-session-log.md` (W-1).
