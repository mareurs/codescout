---
id: 7eacbdf17fc6f1eb
kind: bug
status: fixed
title: doc's id parameter routing omits the augment action, which requires one
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
- doc
- tool-surface
- schema
topic: tool schemas
opened: 2026-09-03
severity: medium
---

## Summary

`doc`'s `id` parameter description enumerates the actions that take a document id — eleven
of them — and **omits `augment`**, which requires one. The `augment` parameter itself leads
with `create:`. So an agent reading the schema to decide whether `augment` takes an `id`
finds that it does not, while CLAUDE.md and `get_guide("librarian")` both call
`doc(action="augment", id=…)`.

## Symptom (Effect)

From the live `tools/list` at `e5307ba2`, the `id` param's routing prefix:

```
get/update/move/delete/graph/state_at/append_entry/update_entry/event_create/
event_list/gather: document id (16-hex). ...
```

`augment` is absent. Meanwhile CLAUDE.md § *Session Intelligence Trackers* instructs:

```
doc(action="augment", id=…, merge=true, augment={params:{observations: [...]}})
```

and `get_guide("librarian")` § *Augmentation Lifecycle* likewise uses
`doc(action="augment", id=…, augment={prompt: …, …})`.

## Reproduction

Mechanical, no model needed — derive each action's parameter set from the routing prefixes
in `doc`'s own param descriptions:

```
python3 prompt-engineering/scenarios/tool-shape/fixtures/gen_fixtures.py
```

It refuses to write, reporting `augment` with fewer than two parameters. Its derived set for
`augment` is `{merge}` alone.

## Environment

`experiments` @ `e5307ba2`, codescout release binary, live `tools/list` capture.

## Root cause

The action-to-parameter binding on this tool is carried **entirely in prose** — each param's
description opens with an `action:` or `action/action:` routing prefix, which is the only
machine- or agent-readable statement of which actions accept it. There is no structural
binding (no per-action schema, no `if/then`), so the prefix list is the contract, and it has
drifted from the accepted set. Nothing checks the two against each other.

*Measured 2026-09-03 by deriving all 17 actions' parameter sets from the captured schema: 58
of 60 params route to at least one action, the two that route nowhere are `action` (the
dispatcher) and `workspace` (universal), and `augment` is the only action whose derived set
is missing a parameter its documented call form requires.*

## Evidence

Derived parameter counts per action, from the capture:

```
doc_augment       1 param   <- merge only; `id` and `augment` both missing
doc_gather        1 param   <- genuinely single-param, verified separately
every other      2..12 params
```

`gather` is the control here: it also derives one parameter, and that one is **correct** —
only `id` routes to it and the tool description confirms it takes nothing else. So a thin
derived set is not by itself the defect; `augment`'s is thin because the prose omits params
the call needs.

## Hypotheses tried

1. **Hypothesis** — the routing parse is too strict and misses a later `augment:` clause in
   `id`'s description. **Test** — the parse matches routing tokens at every sentence start,
   not just the leading prefix, which is what correctly recovers `title` and `body` for
   `append_entry` (both lead with `create:`). Applied to `id`, it finds no `augment`.
   **Verdict** — rejected; the omission is in the text.

## Fix

Not fixed. Add `augment` to the `id` parameter's routing prefix, and give the `augment`
parameter its own `augment:` clause rather than leading with `create:`.

The wider question is worth separating from the one-line repair: **the action-to-parameter
binding is prose-only on a 17-action tool**, so this class of drift is silent by construction
and cannot be gated. A per-action derivation like `gen_fixtures.py`'s, run as a test, would
turn it into a failing build instead of a reading error.


**Fixed 2026-09-03.** Added `augment` to `id`'s routing prefix, and reordered `augment`'s own
description to lead with `augment:` instead of `create:`. Also added a narrow regression test
(`id_param_routing_names_augment`) pinning both. Left the wider question — a per-action
derivation gate in codescout's own test suite — unaddressed, as this bug's own Resume section
separated it from the one-line repair.

**Hit `tool_surface_under_budget` on the first attempt** (56499 vs. budget 56476, 23 over): my
first wording added `named by \`id\`` to `augment`'s description, which cost more bytes than
necessary. Reordering without that addition nets **-1 char** vs. the original — the budget
pressure caught a description that was carrying the wrong emphasis anyway, so the fix stayed
under budget without raising it.

Committed at `7ee62dff`, patch-id `f00716189d547cef9913399580c64278491a063d`. Gate green on
`experiments`: `cargo fmt --check`, `cargo clippy --workspace --all-targets --features
local-embed -- -D warnings`, `cargo test --workspace --no-default-features`, `cargo test
--workspace` — exit 0 on all four.
## Tests added

None — not fixed. A regression test is the derivation above: assert every action's derived
parameter set is non-empty and contains `id` for the actions whose call form requires one.

## Workarounds

Pass `id` to `doc(action="augment")` regardless of what the parameter description lists —
CLAUDE.md's documented call form is correct and the schema prose is not.

## Resume

Decide whether to repair the two descriptions only, or to add the per-action derivation as a
gate. The derivation already exists and refuses on drift:
`prompt-engineering/scenarios/tool-shape/fixtures/gen_fixtures.py`.

## References

- `prompt-engineering:scenarios/tool-shape/fixtures/gen_fixtures.py` — the derivation, and
  its `MANUAL_ROUTE_FIXUPS` override naming this exact gap
- `docs/trackers/resume-tool-surface-structural-mechanisms.md` § SM-4 — found while building
  the split-surface arm; left unfixed there it would have biased the experiment
