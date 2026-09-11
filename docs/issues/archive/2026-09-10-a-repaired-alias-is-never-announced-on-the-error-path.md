---
id: 50ac8439bae8b9bf
kind: bug
status: fixed
title: 'BUG: a repaired alias is never announced on the ERROR path — a fourth render path the spec, the ADR and three reviews all enumerated as three'
tags:
- cluster/hint-composed-without-the-request
---

# BUG: a repaired alias is never announced on the ERROR path — a fourth render path nobody enumerated

## Summary

`Tool::call_content` repairs parameter aliases, then attaches an advisory at three render sites.
All three are downstream of `let mut val = self.call(input, ctx).await?`. Any `Err` from `call()`
early-returns past every one of them, so a caller whose parameter name was silently rewritten
learns nothing about it on exactly the calls it is paying most attention to — the ones that failed.

## Symptom (Effect)

`read_file(file_path="src/tool.rs")` where the path does not exist:

- `file_path` is normalized to `path` at the boundary — repair happens.
- `call()` returns a not-found `RecoverableError`.
- `route_tool_error` composes the response from the error alone. It knows nothing about aliases.
- The caller sees a path error and **no indication that the parameter name it sent is not a
  parameter of the tool.**

The design spec enumerates three render paths and the ADR's Consequences paragraph says the
advisory "must be threaded through **all three**". This is a fourth, and it is the only one where
the advisory is not merely mis-addressed but absent.

## Reproduction

Any alias-bearing call to an alias-declaring tool (`read_file`, `create_file`, `edit_file`, `grep`,
and since `8b396343` also `symbols`) whose `call()` returns `Err`. A nonexistent path is the
cheapest trigger; a regex-like `symbols` pattern is cheaper still and needs no fixture.

**CONFIRMED ON THE WIRE 2026-09-11**, against binary `7e08645f` (`index(action="status")` →
`reading_binary_sha`). The "not reproducible" note this section previously carried is discharged:
the `cargo rb` + `/mcp` it asked for has happened, and the control-flow claim below now has a
runtime instance rather than only a grep.

The reproduction is a **two-cell experiment on one tool, one alias, one mechanism** — the only
variable is whether `call()` succeeds:

| call | alias sent | outcome | advisory reached caller? |
|---|---|---|---|
| `symbols(query="OutputGuard")` | `query`→`name` | Ok | **YES** — `⚠ 'query' is not a parameter of symbols — corrected to 'name'.` |
| `symbols(query="Tool\|Doc", symbol="x")` | `query`→`name` | Err | **NO** — regex refusal only, no mention of `query` |

Verbatim second cell:

```json
{
  "ok": false,
  "error": "pattern looks like a regex (found '|') — symbols searches symbol names, not text",
  "hint": "Use grep(pattern=\"...\") for regex text search, or make separate symbols calls for each symbol name"
}
```

**Why this pairing is the evidence and either cell alone is not.** A single failing cell cannot
distinguish "the advisory is absent on error" from "this tool never emits one" — and the sibling
bug `1e11cf9357136e0e` is a live alternative explanation for any single absence, since it drops
advisories by output size. Holding the tool, the alias and the repair fixed while varying only
`Ok`/`Err` rules both out: the same repair announces itself in cell 1 and is silent in cell 2.

**The cost is visible in cell 2 and is worse than a missing note.** The caller is told to fix its
*pattern*, which is true, and is not told that `query` is not a parameter of `symbols`, which is
also true. The obvious next action — fix the regex, resend with `query` — is the one the tool just
declined to correct. So the error path does not merely withhold the advisory; it hands back a
remedy that routes the caller straight past the second defect in their call.
## Environment

`experiments`, 2026-09-10, after `0672ae75`.

## Root cause

`param_corrections` and `param_notice` are consumed at exactly three sites in
`src/tools/core/types.rs` — the buffered envelope, the pretty-JSON insert, and the compact-text
prefix — and every one of them is below the `?` on `self.call(input, ctx).await`. The `?` is the
mechanism: it is not that the error path drops the advisory, it is that the error path *returns
before the advisory is ever attached*.

Established from control flow plus a grep of all consumption sites, **not** from a runtime probe;
see Reproduction for why the probe is unavailable.

**And the boundary normalization disarmed the one thing that used to cover this.**
`src/fs/mod.rs`'s `get_path_param`/`require_path_param` carry a teaching hint that fires when *no*
path resolves. Before the collapse, a caller sending only `file_path` could reach that hint. Now
normalization rewrites `file_path` to `path` before `call()` runs, so a path always resolves and
that hint is unreachable for exactly the alias case it was written for.

## Why this is the ADR's own diagnosis with the polarity flipped

Amendment 2026-09-10 diagnosed the original defect as: *"the hint rides the **error** path, which a
successful repair means you never reach."* The fix moved the note to the success path. The mirror
image was never checked: **the note now rides the success path, which an error means you never
reach.** Same shape, opposite direction, and the amendment that fixed one half is what created the
other.

It is also verbatim the Revisit-when trigger that amendment added: *"a repaired alias call is
observed where the `corrections` note did **not** reach the caller — that is a render-path hole, not
a repair failure."*

## Why this class

Tagged `cluster/hint-composed-without-the-request`. On the error path the response is composed by
`route_tool_error` from the error value alone; nothing request-derived reaches it, including the
fact that the request was rewritten. The remedy is to consult the request when composing the
response, which is this class's remedy shape.

**Discriminator against its sibling filed the same day**
(`the-buffered-envelope-drops-the-tools-own-corrections`): there the advisory exists in the object
and is dropped by a rebuild from a fixed key set, so the remedy is a re-attach at a site that
already handles the field. Here the advisory is never built into anything, because a different
function composes the whole response and has no access to it. Same class, different remedy, so two
files rather than one paragraph.

## Fix

**FIXED** on `experiments` — `295a928e`, patch-id `7432ae74fe64be3c4821616a44fec5f6abe48db7`.

`call_content`'s `let mut val = self.call(input, ctx).await?` is now a `match`, and the `Err` arm
runs the advisory through `attach_param_corrections_to_error` (`src/tools/core/types.rs`) on its way
out. Kept local to `call_content`: `route_tool_error`'s signature is unchanged, because nothing
outside `call_content` holds the advisory.

**The design question this bug existed to force, decided.** The advisory attaches to
`RecoverableError::extra` under `corrections`, which `route_tool_error` already splices into the
response body at the top level — so it arrives at `corrections.param_aliases`, the SAME address the
success path uses. One shape, not two. Joining the existing `Guidance` was rejected: a
`RecoverableError` carries at most one `Guidance`, so joining means either overwriting the tool's
own recovery text — the exact failure this file describes, where both facts must arrive together —
or concatenating two registers into one field, against this file's own model that "the field name
itself carries the register".

The three success-path merge arms were extracted into `merge_param_corrections` and are now called
from both render paths, so the advisory's address is a property of the mechanism rather than of
which branch the call landed on.

**Scope, stated rather than left to be inferred from the cases fixed.** Only `RecoverableError`
(`isError: false` — "bad input, self-correct and retry") carries the advisory. A plain `anyhow`
error (`isError: true` — "stop and surface to the user") is deliberately untouched:
`route_tool_error` sends only the outermost message on that branch and logs the `.source()` chain
server-side, because an error oracle over the HTTP transport can leak filesystem layout to an
authenticated-but-untrusted client. Widening the message there either overwrites the original error
text (`anyhow::Context` prepends) or drops the chain the log depends on; closing it properly needs
a wrapper error type that re-exports `source()`, not a `format!`. Documented at the site and pinned
by `a_fatal_error_is_left_untouched_and_that_is_the_documented_boundary`.

**Not a hole, for the record:** `call_content`'s ambiguous-write refusal returns before
`param_corrections` is ever built, and needs no advisory — its message already names both alias
keys and both discarded values.
## Tests added

Five, across two levels, covering the axis rather than the case.

`src/server.rs` — `the_dispatch_boundary_announces_the_repair_when_call_errs`: the ERROR twin of
`the_dispatch_boundary_normalizes_and_announces_for_real_tool_calls`, driving `call_tool_inner` (the
real MCP entry point) for two real registered tools whose `call()` errs — `read_file`
(`OutputForm::Text`, path not found) and `create_file` (`OutputForm::Json`, refusing to overwrite).
Both assert the IDENTICAL `corrections.param_aliases` address AND that the original error text
survives; each tool's `output_form()` is pinned so the both-forms claim cannot silently become one.
The sibling success gate's doc comment, which previously declared this fourth path uncovered, now
points here.

`src/tools/core/tests.rs` — an `AliasErr` fixture, deliberately a sibling of the existing
`AliasEcho` rather than a flag on it, holding the same tool name and the same alias so that Ok/Err
is the only variable (the two-cell design this file's own Reproduction used). It covers both
`OutputForm`s on the error path, the merge arm when the error already carries its own `corrections`
key, and the fatal-error boundary.

**Mutated once per guarded CALL SITE, not once per feature.** Reverting `call_content`'s `Err` arm
to a bare re-raise reds all 3 error-path tests and leaves all 3 success-path tests green; removing
the success path's `merge_param_corrections` call reds 3 success-path tests and leaves the
error-path tests green. Neither kill implies the other, which is why one mutation would not have
established the shared helper is load-bearing at both sites.

**A THIRD site existed and is now CLOSED:** the buffered-envelope insert wrote `corrections`
wholesale rather than through the shared helper, and survived both mutations green. That is the
sibling bug `1e11cf9357136e0e`, not this one — fixed 2026-09-11 in `2183a058` (patch-id
`f1fa07c4…`), which routes that site through `merge_param_corrections` as well, making it the
helper's third caller. The wholesale write is gone, so the mutation-survival noted here no longer
describes the tree.

The fatal-boundary test's absence assertion is monotone under removal and is NOT coverage on its
own — it is a boundary marker, meaningful only next to the positive test that fails in exactly that
direction. Its own comment says so, so nobody credits it with more than it provides.
## Resume

Found by an Opus scoped re-review of fix round 3, which enumerated the advisory's consumption sites
and noticed they were all below one `?`. Three prior reviews of this same mechanism — including two
that traced all three render paths deliberately — did not surface it, because each was checking
that the three enumerated paths were covered rather than whether three was the right number.

**The enumeration itself was the blind spot.** The spec said three, the ADR said three, every
review checked three, and the fourth was never a candidate. A count published as a scope is not a
scope that was verified.
