---
status: mitigated
opened: 2026-05-21
closed: 2026-05-21
severity: medium
owner: marius
related: []
tags: [references, call_graph, lsp, navigation]
kind: bug
last_observed: 2026-05-21
---

# BUG: references undercounts call sites that call_graph and grep both find

## Summary
`references(symbol)` returns far fewer usages than actually exist. For
`format_read_file`, it returned 3 references while `call_graph` (callers) and a
literal `grep` both found 17 call sites in a single test file. references missed
16 real, LSP-resolvable call sites — a navigation tool silently returning a
fraction of the truth is dangerous (refactors that trust it will miss callers).

## Symptom (Effect)
Warm session (LSP exercised by a prior call_graph), `code-explorer`, branch
`experiments`.

`references(symbol="format_read_file", path="src/tools/read_file.rs", limit=100)`:
```
3 references in 2 files

src/tools/read_file.rs (2)
    120  Some(format_read_file(result))
    676  pub(super) fn format_read_file(val: &Value) -> String {

src/tools/edit_file/tests.rs (1)
   4121  assert_eq!(format_read_file(&val), "");
```

`call_graph(symbol="format_read_file", path="src/tools/read_file.rs", direction="callers")`:
```
callers: 20 edges across 3 files
    src/tools/edit_file/tests.rs (17)   ← 4073, 4087, 4101, 4114, 4121, 4135,
                                          4160, 4193, 4209, 4232, 4258, 4276,
                                          4294, 4313, 4330, 4349, 4363
    src/tools/read_file.rs (1)          ← 120 format_compact → format_read_file
    src/tools/run_command/tests.rs (2)  ← depth-2 transitive
```
All 20 edges tagged `lsp`.

Ground-truth `grep "format_read_file" src/tools/edit_file/tests.rs`: 17 call
sites (lines listed above) + 1 import (line 4) + test-fn-name matches. So
call_graph and grep agree on 17 calls in that file; references reports 1.

## Reproduction
1. `/mcp` reconnect (fresh server).
2. Warm the LSP: `call_graph(symbol="format_read_file", path="src/tools/read_file.rs", direction="callers")` → 20 edges.
3. `references(symbol="format_read_file", path="src/tools/read_file.rs", limit=100)` → only 3.
4. `grep "format_read_file" src/tools/edit_file/tests.rs` → 17 call sites confirm references is the undercounter.

Re-running references is stable at 3 (not a transient warming artifact within the
session). NOTE: the very FIRST references call after reconnect returned 0
(separate coldstart behavior, see gotchas memory) — this bug is the steady-state
undercount AFTER warming, which is the more serious issue.

## Environment
codescout v0.13.0 (release, via `~/.cargo/bin/codescout` symlink →
`target/release/codescout`), Linux, MCP stdio, rust-analyzer backend.

## Root cause

**CORRECTED (see Hypothesis #9):** the undercount is rust-analyzer index-STATE
dependent and TRANSIENT — a fresh RA process returns the complete set for the
same symbol. It is NOT the symbol-shape quirk theorized below; that theory was an
artifact of observing all controls inside one degraded RA process. The mechanism
detail below still holds (references uses `textDocument/references`; call_graph
uses `callHierarchy/incomingCalls` + a persisted cache, which is why call_graph
stayed complete while RA's live reference index was stale).

---

**Proximate cause (mechanism, still valid):** the two tools use different LSP
requests at the same correct identifier position.
- `references` (`src/tools/symbol/references.rs:57`) → `client.references()` →
  **`textDocument/references`** (`src/lsp/client.rs:1032-1058`).
- `call_graph` callers → `resolve_one_hop` (`src/tools/symbol/call_edges/resolver.rs:47`)
  → `prepare_call_hierarchy` + **`callHierarchy/incomingCalls`** → `resolve_via_lsp`,
  persisted in a SQLite `EdgeCache` and served by name on later hits
  (`src/tools/symbol/call_graph/mod.rs:222-234`).

For `format_read_file`, rust-analyzer's `textDocument/references` returns 3 (1 of 17
call sites in the cfg(test) file) while `callHierarchy/incomingCalls` returns all 17.
RA has fully analyzed the test file (incomingCalls proves it), so this is a
`textDocument/references`-specific incompleteness on RA's side — NOT our position
logic, NOT our filtering, NOT a missing-analysis problem.

**Symbol-specific, not universal:** for `RecoverableError/with_hint`, references
returns 198 (> call_graph's 109) — complete. The undercount correlates with
references concentrated inside a `#[cfg(test)]` module in a different file + low
non-test usage. Why RA's reference search is incomplete for that shape is
rust-analyzer-internal and not yet root-confirmed (see Hypotheses #6).
## Evidence
1. references output (3) — quoted above.
2. call_graph output (20 edges, 17 in tests.rs) — quoted above.
3. grep ground truth (17 calls in tests.rs) — `grep "format_read_file"
   src/tools/edit_file/tests.rs`.

Cross-tool: call_graph == grep == 17; references == 1 for the same file. The
two LSP-backed tools disagree, and the literal text search sides with call_graph.

## Hypotheses tried

1. **Hypothesis:** LSP coldstart / partial index. **Test:** re-ran references
   twice after warming via call_graph; raised limit to 100. **Verdict:** rejected
   as the explanation for the steady-state count — stable at 3 across repeats,
   not growing. (Coldstart DID cause the initial 0; that is separate.)
2. **Hypothesis:** result limit truncation. **Test:** `limit=100`. **Verdict:**
   rejected — only 3 returned, well under the cap.
3. **Hypothesis:** wrong query position — `references` queries LSP at the item
   start (`pub` keyword) not the identifier. **Test:** read the position-resolution
   path. **Verdict:** REJECTED — `SymbolInfo.start_line/start_col` come from
   `selection_range.start` (the identifier), `src/lsp/client.rs:159-161`, pinned by
   test `convert_document_symbols_uses_selection_range`. Position is correct.
4. **Hypothesis:** our filtering/capping drops refs. **Test:** read references.rs;
   `excluded=0` (no build-dir drops); `total=3` from `cap_grouped` before any cap;
   no overflow shown. **Verdict:** REJECTED — the live `textDocument/references`
   itself returned 3.
5. **Hypothesis:** `textDocument/references` universally underreports vs
   `callHierarchy/incomingCalls`. **Test:** ran references + call_graph on a second
   symbol, `RecoverableError/with_hint`. **Verdict:** REJECTED — references found
   **198 in 39 files**, MORE than call_graph's 109 (references counts non-call refs
   too). references is complete for with_hint. The bug is **symbol-specific**, not
   a blanket feature failure.
6. **Hypothesis (current, not yet root-confirmed):** RA's `textDocument/references`
   returns an incomplete set for symbols whose usages are concentrated inside a
   `#[cfg(test)]` module in a *different* file, with little non-test usage —
   `format_read_file` is `pub(super)`, called 17× in `edit_file/tests.rs` + once in
   src. RA HAS analyzed that file (callHierarchy/incomingCalls returns all 17), so
   this is a `textDocument/references`-specific incompleteness on rust-analyzer, not
   a missing-analysis or our-code defect. **Test still owed:** a symbol called many
   times ONLY from non-test src (predict: references complete) vs one called only
   from a cfg(test) module (predict: references undercount). Until run, the cfg(test)
   correlation is a strong association, not proof.
7. **Discriminating experiment (RUN):** compared references vs call_graph/grep on 4 control
   symbols. **Verdict:** references is COMPLETE in 4 of 5 cases — `RecoverableError/with_hint`
   (198 refs), `OutputGuard/from_input` (12 = def + 11 callers, incl. same-file cfg(test) refs),
   `test_ctx` in edit_file/tests.rs (69 refs, all same-file cfg(test) callers), `ReadFile`
   (64 refs, 55 in edit_file/tests.rs). cfg(test) alone REJECTED (test_ctx complete); cross-file
   alone REJECTED (from_input complete); high-fan-in REJECTED (test_ctx 69 complete).
8. **The decisive control — `ReadFile`:** imported on the *same line 4* into the *same* sibling
   cfg(test) module as `format_read_file`, yet references found it completely (55 in-file +
   the import at line 4). `format_read_file` references found NEITHER its import NOR 16/17 calls.
   The only differences: `ReadFile` is `pub struct` (type) vs `format_read_file` `pub(super) fn`
   (free fn); and `format_read_file` uniquely has prefix-colliding siblings (`format_read_file_summary`,
   `format_read_file_auto_chunked*`). **Conclusion:** trigger is one of {`pub(super)` free fn,
   prefix-colliding sibling identifiers} — a rust-analyzer `textDocument/references` quirk for that
   shape. Distinguishing the two is RA-internal and not pursued. references is reliable for the
   overwhelming majority of symbols; this is a rare edge case, NOT a systemic failure.
9. **CORRECTION — the symbol-shape theory (#6/#8) was wrong.** After a `/mcp` reconnect
   (which spawns a FRESH rust-analyzer process), `references(format_read_file)` returned
   the COMPLETE set (20: 17 test calls + import + def + src caller) — the exact symbol
   that was stuck at 3 earlier. Nothing about the symbol changed; only the RA process did.
   **Verdict:** the undercount is rust-analyzer index-STATE-dependent and TRANSIENT, not a
   stable property of `pub(super)` free fns / prefix-colliding names. The 5 control symbols
   in #7/#8 were all observed inside the SAME degraded RA process — a shared confound. Most
   likely trigger: `read_file.rs` had just been edited (the OutputForm::Text flip) and
   repeated `cargo build`/`test`/`clippy` churned RA's watched files, leaving a stale
   partial reference index for that symbol that didn't recover in-session; call_graph masked
   it via its persisted EdgeCache. The completeness guard remains correct as defense-in-depth
   (it keys on the symptom — call sites > references — not on the disproven cause), and was
   verified live to stay silent when references is complete (20 refs ≥ 17 call sites).
## Fix

**Mitigation shipped (the root cause is in rust-analyzer, not fixable here).**
Added a completeness cross-check to `references`: after computing the reference
count, it runs one `prepare_call_hierarchy` + `incoming_calls` (skipped for
non-callable symbols, where `prepare_call_hierarchy` returns `None`) and counts
call sites. Call sites are a strict subset of references, so if call-hierarchy
finds MORE call sites than references found references, references is provably
incomplete — and a `completeness_warning` is attached and rendered, steering the
caller to `call_graph(direction="callers")`.

This cannot produce false positives (the subset relation is exact) and adds no
warning in the common case (references usually returns ≥ call sites because it
also counts non-call refs). It does NOT change the reference count — references
still under-reports for the pathological symbol; the warning ensures the
undercount is never silent.

- `src/tools/symbol/references.rs`: `references_completeness_hint(refs_total, call_sites)`
  (pure decision helper) + cross-check wired into `call()` + warning rendered in
  `format_compact` (both the normal and zero-refs branches).
## Tests added

In `src/tools/symbol/tests.rs`:
- `references_completeness_hint_warns_only_when_calls_exceed_refs` — pure decision
  logic: warns for (3,17) and (0,5); silent for (198,109), (12,11), (5,5).
- `references_format_compact_appends_completeness_warning` — warning renders in the
  normal branch.
- `references_format_compact_appends_warning_on_zero_refs` — warning renders even
  when references found 0 (the most dangerous silent case).

All 13 `references` lib tests pass; clippy clean. The RA-side undercount itself is
not unit-testable (needs the live rust-analyzer quirk); the guard logic + rendering
are.
## Workarounds
Use `call_graph(direction="callers")` instead of `references` for
"who calls X" — it found all 17 here. For non-call references (mentions in
comments/strings), fall back to `grep`. Treat a low `references` count with
suspicion until this is fixed.

## Resume
Diff seed resolution + result collection between `src/tools/symbol/references.rs`
and `src/tools/symbol/call_graph/`. Reproduce with the format_read_file fixture
above. Check whether references issues `textDocument/references` at the
definition position (676) vs the call position, and whether RA returns the full
set for a `pub(super)` fn referenced from a sibling test module. Confirm against
grep ground truth (17 calls in edit_file/tests.rs).

## References
- `src/tools/symbol/references.rs`, `src/tools/symbol/call_graph/`.
- codescout memory `gotchas` (LSP cold-start section) — for the separate
  first-call-returns-0 behavior.
