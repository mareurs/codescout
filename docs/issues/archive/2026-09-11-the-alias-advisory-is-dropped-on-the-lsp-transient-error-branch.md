---
kind: bug
status: fixed
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-11
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

**The title's own count is wrong, and it is left in place only because the slug is cited
elsewhere.** Derived at fix time: `call_content` has **four** render paths (buffered envelope,
`OutputForm::Text` compact render, pretty-JSON value, error path), and this branch is not a fifth
one — it is the **second of three outcomes inside the fourth**. That the file describing a
miscount published one in its own headline is the class holding about itself; see § Fix for the
four counts and the surfaces they each belong to.

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

**Applied, 2026-09-11, commit `4629a95b3d7970fbc1906288f442247ae3d40117` (patch-id
`6e572a051ef63d359d1347df5250d32f2d7ceb70`).**

**Structural, not a second `downcast_ref` in the arm that was missing one** — which is what this
file's own § Fix asked for, and the reason it asked. `route_tool_error` no longer builds a
`CallToolResult` inside its arms. `select_error_render` returns an `ErrorRender`, and
`route_tool_error` attaches the advisory once after arm selection. An arm added to the chain
**cannot** skip the advisory: it has nothing to return early with.

`ErrorRender` has **two** variants, not three, and the count is the point — they are the two
CARRIERS the advisory has, not the three arms:

- `Body(Value)` — an object-shaped body (`RecoverableError` and the LSP-transient arm), rendered
  `isError: false`. The advisory lands at `corrections.param_aliases` via the same
  `merge_param_corrections` the success path calls. Because it MERGES under `corrections`, the
  transient arm's own `hint` — the LSP retry guidance — is preserved beside it rather than
  overwritten; overwriting it would have reproduced this defect in the other direction.
- `Fatal(String)` — plain wire text, `isError: true`. No body to splice into, so it keeps the
  `⚠ {hint}\n\n` prefix.

`attach_param_corrections_to_error` and `merge_param_corrections` became `pub(crate)` so the new
gates drive the production carrier selection rather than re-building an `AdvisedError` by hand.

Its doc comment's count-as-scope paragraph — the *"Both error types now carry it"* sentence this
file's § Evidence named — now derives four numbers at the four surfaces they belong to instead of
publishing one as the scope of another: **2 carriers** here (one per arm of
`e.downcast::<RecoverableError>()`), **3 outcomes** and **2 response shapes** in `route_tool_error`,
**4 render paths** in `call_content`. Each moves independently; only the shape count bounds the
advisory's addresses.
## Tests added

Three, in `src/server.rs`'s `route_tool_error` block — **one per ARM, not one per feature**, since
the three arms have different carriers and a mutation kill on one says nothing about the others:

- `the_advisory_reaches_the_recoverable_arm_of_route_tool_error` — arm 1, carrier `extra`.
  POSITIVE CONTROL.
- `the_advisory_reaches_the_lsp_transient_arm_of_route_tool_error` — arm 2, **this bug**. Runs both
  `-32800` and `-32801`, and asserts BOTH facts arrive: the arm's own `Wait and retry` guidance
  preserved in `hint`, and the advisory beside it at `corrections.param_aliases`.
- `the_advisory_reaches_the_fatal_arm_of_route_tool_error` — arm 3, carrier `AdvisedError` + text
  prefix. SECOND POSITIVE CONTROL, holding a different thing fixed than arm 1: arm 1 checks the
  `RecoverableError` carrier, this one checks the wrapper arm 2 shares.

**Observed RED before the fix**, both controls green — which is what makes it a measurement:

```
test the_advisory_reaches_the_fatal_arm_of_route_tool_error ... ok
test the_advisory_reaches_the_recoverable_arm_of_route_tool_error ... ok
test the_advisory_reaches_the_lsp_transient_arm_of_route_tool_error ... FAILED
LSP request failed: code -32800 (RequestCancelled): corrections.param_aliases.hint missing
on the LSP-transient arm — this is bug 696f3be9902ebf17: {"error":"LSP request failed: code
-32800 (RequestCancelled)","hint":"The LSP server returned a transient error ..."}
test result: FAILED. 2 passed; 1 failed
```

The id in that panic text is the artifact's **pre-archive** id. Archiving re-keys a row
(`id = sha256(abs_path)`), so the live assertion in `src/server.rs` now cites `85856feb201949bc`
while the RED above is quoted verbatim as observed. Both name this file.

**Mutated once per guarded SITE**, each killing exactly its own test:

| site | mutation | reds |
|---|---|---|
| `merge_param_corrections(&mut rec.extra, c)` (`Ok` branch) | drop `corrections` from `extra` after the merge | arm 1 only |
| `route_tool_error`'s `Body` attach | suppress the advisory there | arm 2 only |
| `route_tool_error`'s `Fatal` prefix | suppress the advisory there | arm 3 only |
| the shared `AdvisedError` downcast | suppress it | arms 2 **and** 3 |

The fourth row is the carrier claim, not a leak: arms 2 and 3 share one wrapper, so a green arm 1
beside two reds localises the defect to the carrier rather than to an arm.
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
