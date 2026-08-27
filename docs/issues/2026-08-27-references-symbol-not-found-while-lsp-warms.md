---
status: open
opened: 2026-08-27
closed:
severity: low
owner: marius
related: []
unverified: 'Observed once and not reproducible afterward; the warming-LSP mechanism is inferred from call ordering, never instrumented. No git state captured — shell was disabled in this project at observation time.'
tags: ["references", "lsp", "cold-start", "misleading-error", "unreproduced"]
kind: bug
---

# BUG: `references` answers a warming LSP with `symbol not found` — a resolution error, which the false-zero guard cannot see

## Summary
`references(symbol, path)` returned a hard `symbol not found` error for a symbol
that plainly exists, on the first call after `activate_project`. The same call
later in the same session returned 31 references in 11 files. This is the known
warming-LSP class, but with a symptom the existing mitigation does not cover:
`docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md` guards the
case where resolution SUCCEEDS and yields zero external callers, whereas here
resolution itself fails, so no reference lookup is ever attempted.

## Symptom (Effect)
First call, immediately after `workspace(action="activate")` on the home project:

```
references(symbol="ToolCapabilities", path="src/tools/core/types.rs")
→ {
    "ok": false,
    "error": "symbol not found: ToolCapabilities",
    "hint": "Use symbols(path) to list symbols. Trait impl methods use format 'impl Trait for Struct/method'."
  }
```

Same arguments, later in the same session:

```
references(symbol="ToolCapabilities", path="src/tools/core/types.rs")
→ 31 references in 11 files
```

The hint is actively misleading in two ways: it suggests the caller used the
wrong name form, and it points at the trait-impl syntax — neither of which
applies to a plain top-level struct that `symbols(name=...)` resolves fine.

## Reproduction
**Not reproducible on demand** — a timing window, not a deterministic input.

Best lead: call `references` on a top-level type as the *first* LSP-touching
call after `workspace(action="activate")`, before any `symbol_at` / `symbols`
traffic has warmed rust-analyzer. In the observed case the file had also just
been edited and committed, so a `ContentModified`-style race is not excluded.

## Environment
- Project: codescout (Rust, rust-analyzer), branch `experiments`
- Transport: MCP stdio, Claude Code
- Binary: `target/release/codescout` built 2026-08-27 21:02
- Last HEAD observed this session: `14aa0a08` (not re-checked at observation
  time — `shell_command_mode = "disabled"` was in effect, so no `git` available)

## Root cause
**Unknown — inferred, not measured.** The call ordering is consistent with
rust-analyzer still loading when the first `references` landed:

- `symbols(name_path=..., include_body=true)` succeeded in the same batch — but
  that is the AST/tree-sitter index, not LSP.
- `semantic_search` and `read_file` succeeded in the same batch — neither is
  LSP-backed.
- `symbol_at` (LSP-backed, `def` + `hover`) succeeded in the *next* batch, which
  is the first positive evidence LSP was up.
- `references` on two other symbols — `check_tool_access` (function) and
  `Availability` (enum, **same file**) — succeeded after that.

So no LSP-backed call is known to have succeeded before the failure, and every
LSP-backed call after it succeeded. That is suggestive, not conclusive: nothing
instrumented whether project-load was actually in flight.

Why the existing guard misses it —
`docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md` added
`corroborate_zero_references` in `src/tools/symbol/references.rs`, which fires
when `external_refs == 0`. That requires the symbol to have resolved. Here
resolution failed first, so the guard is structurally unreachable for this
symptom.

## Evidence
### Ordering of the four batches, single session
```
batch 1 (right after activate):
  symbols(name_path="impl Tool for RunCommand/availability")  → ok
  references(symbol="ToolCapabilities", …)                    → symbol not found
  semantic_search("gate a tool out of the advertised list")    → ok
  read_file(".codescout/project.toml", toml_key="security")    → ok

batch 2:
  symbols(name="ToolCapabilities")                             → Struct 464
  symbol_at("src/tools/core/types.rs", line=464)               → ok (def + hover)
  artifact(action="find", kind="bug")                          → ok

batch 3:
  references(symbol="check_tool_access", …)                    → 12 refs / 2 files
  references(symbol="Availability", …)                         → 43 refs / 10 files

batch 4:
  references(symbol="ToolCapabilities", …)                     → 31 refs / 11 files
```

### The symbol was resolvable by other means at failure time
`symbols(name="ToolCapabilities")` reported `Struct 464`, and `symbol_at` at
line 464 returned a full hover including the struct's fields. So the name and
path passed to `references` were correct.

## Hypotheses tried
1. **Hypothesis:** Wrong symbol name or `name_path` form (what the hint claims).
   **Test:** `symbols(name="ToolCapabilities")` and `symbol_at(path, 464)`.
   **Verdict:** rejected — both resolve the bare name at that path.
2. **Hypothesis:** Caused by `shell_command_mode = "disabled"`, set moments
   earlier. **Test:** `references` on two other symbols in the same session with
   shell still disabled. **Verdict:** rejected — both succeeded;
   `shell_command_mode` is read only by `run_command` and
   `current_capabilities`, and `references` is gated on `RequiresLsp`.
3. **Hypothesis:** `references` cannot resolve `struct` symbols, only functions
   and enums. **Test:** re-ran the identical call in batch 4.
   **Verdict:** rejected — returned 31 references.
4. **Hypothesis:** rust-analyzer had not finished project-load when the first
   call landed. **Test:** none — inferred from batch ordering only.
   **Verdict:** deferred, unmeasured. This is the leading hypothesis.

## Fix
N/A — not attempted. Two candidate directions, neither implemented:

- **Cheap:** when symbol resolution fails and the language has a live LSP that
  has not confirmed project-load, say so in the error instead of suggesting the
  caller mistyped the name. Same shape as the fix in
  `docs/issues/archive/2026-08-16-audit-doc-refs-calls-a-warming-lsp-offline.md`,
  which stopped `audit_doc_refs` calling a warming LSP "offline".
- **Fuller:** retry resolution on the cold-start retry budget. Note the archived
  false-zero bug found the budget already covers `textDocument/references` but
  never fires there, because a definition-only response is a *success*. A
  resolution failure is an error, so it may already be retriable — worth
  checking before writing anything.

## Tests added
None. Justified: the trigger is a project-load timing window with no mock-LSP
fixture in the suite for `References::call` (the archived false-zero bug records
the same gap and tested its text-scan helper in isolation instead). A test
asserting the *error text* would be possible without reproducing the race, but
only after deciding what the message should say.

## Workarounds
- Re-run `references` once. It is a warming window, not a persistent state.
- Warm LSP first, or corroborate with `grep "\bSYMBOL\b"` /
  `call_graph(direction="callers")`.
- Treat `symbol not found` from `references` as "unknown, retry" rather than
  "the name is wrong" — especially early in a session, and *especially* when
  `symbols(name=...)` finds the same symbol.

## Resume
Instrument first, then decide. Add a debug log in
`src/tools/symbol/references.rs` at the resolution-failure branch recording
whether the language's LSP client has confirmed project-load, then reproduce by
`workspace(action="activate")` followed immediately by `references` on a
top-level type in a large file. If project-load is in flight, compare the branch
against the cold-start retry budget in `src/lsp/client.rs` to see whether a
resolution error is already retriable and merely not retried here.

## References
- `docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md` — same
  root-cause class, different symptom; its `corroborate_zero_references` guard
  cannot reach this one.
- `docs/issues/archive/2026-08-16-audit-doc-refs-calls-a-warming-lsp-offline.md`
  — precedent for correcting a warming-LSP misdiagnosis in the message.
- `docs/issues/archive/2026-05-07-symbols-empty-lsp-cold-start.md`,
  `docs/issues/archive/2026-04-24-find-symbol-cold-start-hang.md`,
  `docs/issues/archive/2026-08-21-mux-lsp-cold-starts-not-recorded.md` — the
  broader cold-start family.
- Noticed while verifying `shell_command_mode = "disabled"` end-to-end, commit
  `6058dad6` (`feat(tools): hide run_command when shell_command_mode is
  disabled`).
