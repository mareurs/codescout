---
id: '950440ec3d9a256e'
kind: bug
status: open
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

Any alias-bearing call to an alias-declaring tool (`read_file`, `create_file`, `edit_file`, `grep`)
whose `call()` returns `Err`. A nonexistent path is the cheapest trigger.

Not reproducible against the currently-built binary: the live `cargo rb` artifact predates the
alias wiring, so a probe returns no advisory on the success path either. Needs `cargo rb` + `/mcp`
to observe end-to-end. The control-flow claim below does not depend on that.

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

Thread the corrections through the error path — `route_tool_error` needs access to them, or
`call_content` must attach them to the error before routing. The design question this needs: a
`RecoverableError` already carries `Guidance`, so there is an existing carrier and the choice is
whether the advisory joins it or arrives as a separate field. Deciding that is what makes this a
filed bug rather than a one-line fix.

## Tests added

None. The guard is a test driving an alias-bearing call whose `call()` errs, asserting the error
carries the advisory. No such test exists for any of the three working paths' error twin either, so
this is a whole missing axis rather than one missing case.

## Resume

Found by an Opus scoped re-review of fix round 3, which enumerated the advisory's consumption sites
and noticed they were all below one `?`. Three prior reviews of this same mechanism — including two
that traced all three render paths deliberately — did not surface it, because each was checking
that the three enumerated paths were covered rather than whether three was the right number.

**The enumeration itself was the blind spot.** The spec said three, the ADR said three, every
review checked three, and the fourth was never a candidate. A count published as a scope is not a
scope that was verified.
