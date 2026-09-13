---
id: 077308f225dbd8ec
kind: bug
status: fixed
title: 'BUG: two RecoverableError types share a name, and the bridge that makes both correct removes the feedback'
tags:
- cluster/addressing-without-an-escape-hatch
---

## Summary

Two types named `RecoverableError` exist in this crate:

- `crate::librarian::tools::RecoverableError` — `{message, hint}` (`src/librarian/tools/mod.rs:45`)
- `crate::tools::RecoverableError` — `{message, guidance, extra}` (`src/tools/core/types.rs:591`)

Their `Display` output is close enough to be indistinguishable in a test panic, and
`adapter.rs`'s `bridge_recoverable_error` converts the librarian one into the host one at the
boundary — so **both are correct on the wire**. The choice between them is therefore invisible
at runtime and visible only to a `downcast_ref`.

That bridge is what makes this more than a name collision. It removes the feedback that would
otherwise catch the wrong choice immediately. (Framing owed to sessionId `8bd791df`.)

## Symptom (Effect)

Writing a helper *inside* `src/librarian/tools/mod.rs` and referring to bare `RecoverableError`
silently selects the librarian type. Nothing at the call site says which you got.

The failure surfaces somewhere else entirely, as
`every_required_param_failure_names_its_action_and_routes` panicking with:

```
event_create: a required-param miss must be recoverable, not a bare serde error — got:
doc(action="event_create") requires 'id', 'event.kind' and 'event.payload': missing field `kind` (hint: …)
```

**The error in that message is recoverable and is not a bare serde error.** The assertion's
own text names the one cause that is excluded by the evidence printed beside it, because that
test does `use super::*` and then `use crate::tools::RecoverableError`, shadowing the one the
helper produced. A reader goes hunting an un-wrapped serde error that does not exist.

## Reproduction

Measured 2026-09-12 while adding `deser_error` (`cdd71995`).

1. Inside `src/librarian/tools/mod.rs`, write a function returning
   `RecoverableError::with_hint(msg, hint)` — unqualified.
2. `cargo test --lib required_param_routing` → the panic above.
3. Qualify it to `crate::tools::RecoverableError::with_hint(...)` → **now a compile error**,
   `E0308 expected Error, found RecoverableError`, because the two types' `with_hint` differ in
   return type as well: the librarian's returns `anyhow::Error`, the host's returns `Self`.
4. Add `.into()` → green.

Three distinct failure modes for one name, and only the third is loud at the point of the
mistake.

## Environment

`experiments` at `cdd71995`. Both types are behind `feature = "librarian"` for the bridge path;
the host type is unconditional.

## Root cause

A namespace with two members and no disambiguator at the point of use. `use super::*` in the
librarian tool modules brings the librarian type into scope under the bare name, and a sibling
`use crate::tools::RecoverableError` in the same module's test block shadows it — so the *same
token* means different types in two scopes of one file.

## Evidence

The mixture is already live and load-bearing in both directions, so neither type can simply be
deleted:

- `event_create.rs` and `augment.rs` built the **host** type (before `cdd71995`).
- `artifact.rs` (`use super::{RecoverableError, …}`), `get.rs`, `find.rs`, `context.rs`,
  `update.rs`, `temp_write_guard.rs` and `statements.rs` use the **librarian** one.
- `get.rs:913`, `find.rs:3155` and `temp_write_guard.rs:214` each `downcast_ref` to the
  **librarian** type; `mod.rs:580`'s test block downcasts to the **host** type.

`adapter.rs:946-961` documents the bridge and cites
`docs/issues/archive/2026-07-10-librarian-recoverable-error-downcast-never-matches.md` — the
earlier bug where the librarian type reached `route_tool_error` unbridged and every librarian
recoverable condition hard-failed as `isError: true`. The bridge fixed that and, in doing so,
made the choice unobservable.

## Fix

Not attempted. Three options, none free:

- **Rename one** (`LibrarianRecoverableError`, or re-export the host type under a distinct
  alias). Mechanical, wide diff, and it is the only option that removes the ambiguity rather
  than documenting it.
- **Collapse to one type**, with the librarian's `hint` folded into the host's `guidance`. The
  bridge then deletes itself. Largest change; also the one that ends the class.
- **Say so at the point of use** — a module-level note in `src/librarian/tools/mod.rs` naming
  both types and the fact that the bridge hides the difference. Cheapest, and it does not stop
  the next person, which is this class's standing objection to documentation-as-remedy.

**What is already done and is not the fix**: `every_required_param_failure_names_its_action_and_routes`'s
panic message now names BOTH candidate causes rather than only the serde one, and `deser_error`
carries a comment saying why it is qualified. That converts a mystery into a two-minute
correction; it does not make the wrong choice hard to make.

Fix SHA: *(not fixed)*
Patch-id: *(not fixed)*

## Tests added

None for the class. The assertion-message repair landed in `cdd71995`.

## Resume

**FIXED 2026-09-13.** Renamed the librarian type to `LibrarianRecoverableError` via
LSP-aware rename (249 sites, 33 files) rather than collapsing — the user's call,
with the collapse option carried forward to `docs/ROADMAP.md`'s new "Collapse the
Two RecoverableError Types Into One" entry for future investigation rather than
dropped.

One site the rename tool could not reach: `lift_top_level_param!`
(`src/librarian/tools/update.rs:368`), a `macro_rules!` definition referencing
`super::RecoverableError` in its own body — macro token trees aren't part of the
LSP rename's reference graph. Caught immediately by `cargo build` (`E0433`), fixed
by hand. Worth naming as a class: an LSP-based rename over a namespace with a
macro definition owes the same escape-hatch scrutiny as any other parser over a
namespace (`cluster/addressing-without-an-escape-hatch`, already this bug's tag).

Gate green in the mandated order. Verified the two types' constructors and
call-site populations directly (251 refs / 33 files for the librarian type, 491
refs / 69 files for the host type) before choosing rename over collapse — the
reference counts are what make collapse's cost concrete rather than assumed.

**Fix:** `e0b8d235`, patch-id `4e3e0072836d1518f732f107ebce1e2376992580`.

## References

- Found 2026-09-12 while adding `deser_error` (`cdd71995`), which fixed an unrelated
  remedy-text defect on `doc`'s unknown-field refusal.
- Class framing (`IC-6`'s disambiguator half rather than a name collision, because the bridge
  removes the feedback) is sessionId `8bd791df`'s.
- `docs/issues/archive/2026-07-10-librarian-recoverable-error-downcast-never-matches.md` — the
  same pair, the other failure direction, before the bridge existed.
