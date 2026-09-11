---
kind: bug
status: open
tags:
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-11
owner: marius
related: []
severity: medium
---

# BUG: the parameter-alias advisory is dropped on `route_tool_error`'s LSP-transient branch — the fifth render path nobody enumerated

## Summary

`attach_param_corrections_to_error` (`src/tools/core/types.rs`) wraps a plain `anyhow` failure
in `AdvisedError` so the alias advisory survives the error path. `route_tool_error`
(`src/server.rs`) consults `AdvisedError` in its **final `else` only**. Its middle branch — the
LSP-transient `-32800` / `-32801` arm — composes a fresh body from the error string and never
looks, so an aliased call to any of the five LSP-backed alias-declaring tools loses the advisory
during a cold-index window. Worse than a bare drop: that branch also **occupies `hint`** with LSP
boilerplate whose remedy is *"Wait and retry"*, which routes the caller straight back through the
alias they were never told about.

## Symptom (Effect)

Body returned for an `AdvisedError` whose inner message contains `code -32800`:

```
{
  "error": "LSP request failed: code -32800 (RequestCancelled)",
  "hint": "The LSP server returned a transient error (RequestCancelled -32800 or ContentModified -32801). ... Wait and retry; or for non-idempotent methods (rename, applyEdit) re-issue manually after confirming server state."
}
```

No `corrections` key, no `param_aliases`, no `⚠` prefix. The identical input through the final
`else` branch returns `⚠ 'file_path' is not a parameter of references — corrected to 'path'.`
followed by the tool's own message.

## Reproduction

At `a8e8a91a` (branch `experiments`), in a scratch copy of the tree, three probes added to
`src/server.rs`'s test module next to the existing `route_tool_error` unit tests:

1. control — `AdvisedError` over `anyhow::anyhow!("kaboom in the tool")`, assert the wire text
   names `file_path`. **PASSED.**
2. `AdvisedError` over `anyhow::anyhow!("LSP request failed: code -32800 (RequestCancelled)")`,
   same assertion. **FAILED.**
3. same with `-32801 (ContentModified)`. **FAILED.**

`cargo test --lib --no-default-features probe_` → `8 passed; 2 failed`. The control passing is
what makes the two reds a measurement rather than a broken harness.

On the live wire the same shape is reachable with no probe: any of `references`, `call_graph`,
`symbol_at`, `edit_code`, `symbols` called with a declared alias (e.g. `file_path`) while
rust-analyzer is cold and the retry budget in `src/lsp/client.rs` is exhausted.

## Environment

Linux, branch `experiments` at `a8e8a91a`, stdio MCP transport, project codescout. Both feature
lanes affected — the branch is not feature-gated.

## Root cause

`route_tool_error` (`src/server.rs`) is an `if` / `else if` / `else` chain over the error value:

1. `downcast_ref::<RecoverableError>()` — splices `rec.extra` (where
   `merge_param_corrections` put the advisory) into the body. Covered.
2. `e.to_string().contains("code -32800") || e.to_string().contains("code -32801")` — builds
   `json!({"error": …, "hint": <LSP boilerplate>})` from scratch. **Never consults
   `AdvisedError`.**
3. `else` — the only arm that computes the `⚠ {hint}` prefix from
   `e.downcast_ref::<crate::tools::AdvisedError>()`.

`AdvisedError`'s `Display` delegates to its inner error (`src/tools/core/types.rs`, deliberately,
so the tool's own message survives), so a wrapped LSP-transient error still satisfies branch 2's
`contains` and is captured there. The wrapper's whole design — invisible delegation — is what
makes it invisible to the selector.

Measured 2026-09-11: the three probes above, run in a copy of `a8e8a91a` under a private
`CARGO_TARGET_DIR`; control green, both transient cases red.

## Evidence

The failing assertion's payload, verbatim from the probe run, is the block quoted under
*Symptom* above. The passing control returns the same inner message **with** the advisory.

The enumeration that missed it is in `attach_param_corrections_to_error`'s own doc comment:
*"**Both error types now carry it, via two different carriers.**"* Two error **types** is right;
the dispatch it feeds has **three** outcomes, and the count was published as the scope.

## Hypotheses tried

1. **Hypothesis** — the transient branch is unreachable for an alias-declaring tool.
   **Test** — enumerated `param_aliases()` impls; five of the ten are LSP-backed
   (`references`, `call_graph`, `symbol_at`, `edit_code`, `symbols`), and
   `is_retryable_lsp_error` in `src/lsp/client.rs` exists precisely because those codes
   propagate as plain `anyhow` errors past the retry budget. **Verdict:** rejected.
2. **Hypothesis** — the gate `the_dispatch_boundary_prefixes_the_repair_onto_a_fatal_error_too`
   already covers it. **Test** — read it: it drives `symbol_at` on an unsupported-language file,
   an `anyhow::anyhow!("unsupported language")` that lands in branch 3. **Verdict:** rejected —
   nothing drives branch 2 with an advisory attached.

## Fix

Not fixed. The shape that matches the rest of the design is to compute the prefix (or the
`corrections` splice) **once, above the chain**, rather than inside one arm — the same argument
`merge_param_corrections`'s own doc comment makes for being one function called from both
object-shaped render paths. A second `downcast_ref` inside branch 2 would close this instance and
leave the next arm open.

Note branch 2 returns `CallToolResult::success`, so the `RecoverableError`-style
`corrections.param_aliases` address is available there; it does not need the text prefix.

## Tests added

None yet. The regression test is the probe above, minus the temporary scaffolding: an
`AdvisedError` over an inner message containing `code -32800`, asserting the advisory reaches the
wire — paired with the plain-`anyhow` control, since only the pair distinguishes "the branch was
fixed" from "the wrapper stopped wrapping".

## Workarounds

None for the caller. An agent that receives the LSP-transient envelope after sending an alias
learns nothing about the alias and, following the envelope's own `hint`, retries with the same
key.

## Resume

Add the two probes from *Reproduction* to `src/server.rs`'s `route_tool_error` test block
(alongside `recoverable_error_routes_to_success_not_is_error`), observe the red, then hoist the
`AdvisedError` downcast above the `if` chain in `route_tool_error` so every arm can use it.

## References

- `src/server.rs` — `route_tool_error`
- `src/tools/core/types.rs` — `attach_param_corrections_to_error`, `AdvisedError`,
  `merge_param_corrections`
- `src/lsp/client.rs` — `is_retryable_lsp_error`
- `docs/issues/archive/2026-09-11-the-alias-advisory-still-does-not-reach-the-plain-anyhow-error-path.md`
  — the fix that created this wrapper, and whose scope paragraph stops at two error types
- `docs/adrs/2026-07-10-repair-and-continue-input-handling.md` — the governing ADR
