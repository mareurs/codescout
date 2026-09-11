---
id: b4bf27e5a1c5b50c
kind: bug
status: open
title: 'BUG: the alias advisory reaches RecoverableError but not a plain anyhow error — the error path was closed for one of its two error types'
tags:
- cluster/hint-composed-without-the-request
---

## Summary

`295a928e` fixed `950440ec3d9a256e` by attaching the parameter-alias advisory to
`RecoverableError::extra`, so `route_tool_error` splices it and a caller reaches
`corrections.param_aliases` at the same address as on the success path.

**That covers one of the two error types `call()` can return.** A plain `anyhow` error — the
`isError: true` branch — is untouched. A caller whose parameter name was silently rewritten and
whose call then failed with an `anyhow` error still learns nothing about the rewrite, which is
verbatim the defect `950440ec3d9a256e` described, surviving on the half its fix did not reach.

## Symptom (Effect)

Any alias-bearing call to an alias-declaring tool (`read_file`, `create_file`, `edit_file`, `grep`,
`edit_code`, `symbol_at`, `references`, `call_graph`, `symbols`) whose `call()` returns
`anyhow::bail!` rather than a `RecoverableError`. The alias is repaired, the call fails, and the
response carries the error message with no indication the parameter name is not a parameter of the
tool.

## Environment

`experiments`, 2026-09-11, at `4ba169bd`. Not yet observed on the wire: the built binary is
`7e08645f`, which predates `295a928e`, so a probe cannot currently distinguish this from the
already-fixed half. Needs `cargo rb` + `/mcp`. The control-flow claim does not depend on it.

## Root cause

`route_tool_error` deliberately sends only the **outermost** message for a plain `anyhow` error and
logs `.source()` server-side. That is not an oversight — an error oracle over HTTP leaks filesystem
layout, so the narrow response is the security-relevant choice and must stay.

The consequence is that there is no splice point on this branch. Attaching the advisory the way
`295a928e` did is unavailable, and the two obvious substitutes both lose information:

- `anyhow::Context` **prepends**, so wrapping overwrites the position the tool's own message
  occupies — the same "both facts must arrive together" failure the parent bug named.
- A `format!` into the message drops the `.source()` chain.

## Fix

Not a one-liner, which is why this is filed rather than folded into `295a928e`. It needs a wrapper
error type that carries the advisory as structured data while re-exporting `source()`, so the
chain survives and the response composition stays in `route_tool_error`'s hands. Deciding whether
that type is worth introducing for this alone is the open question.

**Or close it by construction instead:** if every alias-declaring tool's failure modes are
`RecoverableError` by policy, this branch becomes unreachable for the population that can trigger
it, and the fix is a gate asserting that rather than a new error type. That is plausibly cheaper
and has not been checked.

## Why this class

`cluster/hint-composed-without-the-request` (`IC-22`). On this branch the response is composed from
the error value alone; nothing request-derived reaches it, including the fact that the request was
rewritten. Same class and same remedy shape as its parent.

**Discriminator against the parent** (`950440ec3d9a256e`, archived as `50ac8439bae8b9bf`): there a
splice point existed and the advisory was simply never built into anything that reached it. Here
the advisory cannot be spliced without either overwriting the tool's own message or truncating the
error chain, and the narrowness is a deliberate security property. Same class, materially different
remedy, so a separate file rather than reopening the archived one.

## Why it is filed rather than fixed

Surfaced by the subagent that fixed the parent, which stated the boundary at the call site and
flagged it for a decision rather than widening scope. Recording it because a boundary documented
only as a source comment is invisible to the ledger — and because the parent bug's own lesson was
that **a count published as a scope is not a scope that was verified**. "The error path is fixed"
is true of one error type and false of the other; without this file, the next reader inherits the
sentence and not the qualifier.

## Tests added

None. The guard would be a test driving an alias-bearing call whose `call()` returns
`anyhow::bail!`, asserting the response carries the advisory — currently red by construction.
`295a928e` added three error-path tests and all three use `RecoverableError`, so the axis is
covered for one type and untested for the other.
