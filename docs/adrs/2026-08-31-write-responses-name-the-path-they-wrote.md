---
id: '437223096ca67296'
kind: adr
status: draft
title: ADR-2026-08-31 — Write responses name the path they wrote
owners:
- marius
tags:
- tool-contracts
- no-echo
- operator-rules
- response-shape
topic: tool-contracts
---

## Context

codescout's write tools answer `json!("ok")` — the **no-echo convention**, named in
`CLAUDE.md` and in the `conventions` memory. Its purpose is that a write must not echo
the *content* it wrote: `call_content` refuses to clone its own input for exactly this
reason, because `edit_file` and `create_file` carry whole file bodies in it.

The convention was never absolute. Two exceptions already shipped:

- `memory/mod.rs:844` returns `{"status":"ok","warnings":[…]}` when a best-effort side
  effect failed, on the grounds that the caller cannot otherwise know.
- `annotate_write_root` promotes a bare `"ok"` to `{"status":"ok","wrote_to":<root>}` for
  every unpinned write, so a call that resolved against the session default is
  distinguishable from one that landed where the caller meant.

`OP-4` declares `**Serves:** edit_file(path~/.claude), create_file(path~/.claude)`. Its
predicate runs through `names_path_containing`, which scans the **response**. So the rule
was structurally dead: the only path-shaped field a write response carried was
`wrote_to`, which names the project root — for a write to `~/.claude/settings.json` that
is the codescout checkout, and matches no `~/.claude` needle.
`docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md` recorded it, with mutations
showing that widening the scan to `wrote_to` still does not fire while a response carrying
a real `abs_path` does.

## Decision

**A write response names the path it wrote**, via `annotate_write_path` in
`call_content` — a sibling to `annotate_write_root`, not a fold into it.

- Keyed by absoluteness: `abs_path` for an absolute path, `rel_path` otherwise.
  `names_path_containing` scans both, and a relative path filed under `abs_path` would be
  a lie the matcher happens not to notice.
- Gated on `is_write` and the existing annotation exemption list — **not** on
  `workspace_override`. That gate exists on `annotate_write_root` because a pinned call
  already named the checkout it meant; which *file* it wrote is a fact about the call
  either way.
- Never overwrites a path the tool set itself.
- Captured from `&input` before `self.call` consumes it, cloning one string rather than
  the input.

**This is a narrow widening of no-echo, not its repeal.** A path is bounded, is already
half-present as `wrote_to`, and tells the caller where the write landed. The content
prohibition is untouched.

## The alternative, and why measurement rejected it

The other route was to match the predicate against the call **input** instead of the
response — leaving no-echo entirely intact. It was rejected on blast radius, measured
rather than argued:

| | this decision | input-threading |
|---|---|---|
| touches | `call_content` only | `route()` → `Shape::matches` → predicate |
| shared with | nothing | **the shipped guide section-grain feature**, `guide_index.rs:194` |
| tool `call` returns changed | none | none |
| `json!("ok")` assertions broken | **0 of 44** | 0 |

`Shape::matches` is the same matcher the guide section-grain feature uses, and its
Phases 2–3 are open as `GG-N`. Threading an input through it widens a signature two
subsystems depend on while one of them is mid-flight.

Note the row that inverted the original framing: this was first described as "bend a
convention" versus "bend a signature", implying the signature change was the contained
one. Measured, it is the reverse.

## Confidence

High on the mechanism, and it is asserted end-to-end rather than by construction:
`op_4_routes_on_a_write_response_the_pipeline_produced` feeds `route()` a response that
came back out of `call_content` past the path-stripper and both annotations, with a
negative control for a path outside `~/.claude`.

The pin that was supposed to detect this fix landing **could not**, and that is the
strongest evidence for asserting on produced values rather than fabricated ones:
`op_4s_path_predicate_cannot_fire_against_a_write_response_today` asserted against a
hand-written response bound to a variable named `observed`, and so could see neither the
bug nor its repair. It is removed, per its own instruction.

## Revisit-when

- A write tool needs to answer with a path it did **not** take from `input["path"]` —
  the capture is a single well-known key, not a general mechanism.
- `names_path_containing` grows beyond `abs_path`/`rel_path`/`items[]`/`violations[].path`,
  at which point the absoluteness keying should be re-checked against what it scans.
- The no-echo convention is revisited as a whole. This ADR widens it by one bounded field
  and does not license a second widening by analogy.

## Sites (initial)

- `src/tools/core/types.rs` — `annotate_write_path`, and its capture in `call_content`
- `src/util/librarian_response.rs` — `names_path_containing`, the consumer
- `src/operator_rules/route.rs` — `op_4s_predicate_is_itself_sound_given_a_path_bearing_response`
- `src/tools/core/tests.rs` — `op_4_routes_on_a_write_response_the_pipeline_produced`
- `docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md`

