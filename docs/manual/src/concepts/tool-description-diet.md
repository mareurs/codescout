
# Tool Description Diet & Tool Guide Resource

Keeps every tool's MCP description at ≤ 300 characters and exposes long-form
usage notes on demand via the `doc://codescout-tool-guide` resource.

## Motivation

The MCP tool list is sent to the LLM on every turn. Bloated descriptions waste
context tokens with prose that is only occasionally useful. This feature separates
*what the tool does* (short, always paid) from *how to use it well* (long, paid
only when the agent asks).

## Tool::long_docs()

A new optional method on the `Tool` trait:

```rust
fn long_docs(&self) -> Option<&str> { None }
```

Tools with long documentation override it. Currently populated for the five most
complex tools: `symbols`, `symbols`, `semantic_search`, `run_command`,
`memory`.

## doc://codescout-tool-guide

A generated MCP resource that renders each registered tool's `long_docs()` (or
falls back to its short `description()`) into a single Markdown page.

Fetch it with:

```
resources/read doc://codescout-tool-guide
```

The guide is re-generated on every `activate_project` call so it always reflects
the current tool set.

## Guard test

`tool_descriptions_stay_under_budget` in `src/server.rs` asserts that every
registered tool's `description().len() <= 300`. The test fails at compile time if
a future tool exceeds the budget.
