---
id: 1e11cf9357136e0e
kind: bug
status: fixed
title: 'BUG: the buffered envelope drops the tool''s own `corrections`, so a caller learns their request was reinterpreted only if the answer was small'
tags:
- cluster/hint-composed-without-the-request
---

# BUG: the buffered envelope drops the tool's own `corrections`, so a caller whose request was reinterpreted is told only if the response happened to be small

## Summary

`Tool::call_content` attaches its parameter-alias advisory on all three render paths, including the
buffered overflow envelope. A tool's OWN `corrections` — `find.rs`'s report that it repaired the
caller's filter or lifted a top-level param — reaches only the two inline paths. On overflow the
envelope is rebuilt from a fixed key set and the tool's advisory stays inside the buffer, so whether
a caller learns their request was reinterpreted depends on how big the answer was.

## Symptom (Effect)

Observed live, two calls on 2026-09-10, same tool, same reinterpretation:

```
doc(action="find", rel_path="docs/trackers/observer-blindness", limit=3)
  -> inline, and carries  corrections: {filter: [...], hint: "..."}

doc(action="find", rel_path="docs", limit=400)
  -> {output_id: "@tool_8b23a749", summary: ..., hint: ..., buffered_bytes: ...}
     the lift advisory is nowhere in the envelope
```

`rel_path` is not a `find` parameter — it is lifted into `filter={"rel_path": {"contains": …}}` and
that lift is reported under `corrections`. In the second call the caller is told nothing about it
unless they spend a second round-trip on the buffer, which is precisely the cost the governing ADR
exists to remove.

## Reproduction

Any `doc(action="find", rel_path=…)` whose result set exceeds the inline budget. `rel_path` is the
cheapest trigger because the lift is unconditional; an inverted filter leaf works too.


**RE-VERIFIED 2026-09-11 at `13859878`, still live.** Run against a release binary built from
that commit — identity confirmed from compiled-in schema strings (`index.scope`'s
`{"const": "project"}`, the nested `oneOf` on `symbols`/`references`, and the deleted
`librarian.project` param), not from an mtime.

```
doc(action="find", rel_path="docs/issues", limit=200)
```

The envelope the caller receives carries `output_id`, `summary`, `hint`, `buffered_bytes`. Its
shape listing reads `5 keys: count, items, scope, hints, corrections` — the key is NAMED and its
value is not. Pulled out of the buffer afterwards, here is what the caller never saw:

```json
{"filter": ["top-level rel_path lifted into the filter: {rel_path: {contains: docs/issues}}"],
 "hint": "rel_path is a create-time param; on find it was read as a filter clause. ..."}
```

**THE CONTROL, recorded here because without it a probe CONFIRMS this bug is fixed when it is
not.** The FRAMEWORK's param-alias advisory *does* reach the envelope — measured the same day on
one tool in both response shapes, which controls for tool-specific behaviour: `grep` with a bad
param name returning a small result emits a prose warning banner, and the same call overflowed
emits a structured `corrections.param_aliases` object inside the envelope. Both survive.

So *"I called an overflowing tool with a bad param name and saw `corrections` in the envelope"*
is a TRUE observation and is **no evidence whatever about this bug**. Two unrelated mechanisms
share the key name `corrections`, and § *Summary* already says the framework half was never the
defect — but a reader who probes that half gets a clean green and closes this file wrongly. That
nearly happened on 2026-09-11: the probe was run, the confirmation was read, and only re-reading
this file's own § *Summary* before reporting caught that it had tested the wrong half.

The generalisation, since it is not specific to this key: **CLAUDE.md's "run the reproduction"
rule is written for the FIXING phase, and this was the CLOSING phase.** Confirming a bug is
fixed has the same requirement and feels lower-stakes, because the expected answer is the
reassuring one — so a substituted probe goes unquestioned in a way it would not if the answer
had been alarming. Run the reproduction the FILE states, not one composed from its title.
## Environment

`experiments`, 2026-09-10. Reachable in the shipped binary — not latent, and not introduced by the
parameter-alias-collapse plan.

## Root cause

`src/tools/core/types.rs:1207-1212` builds the envelope from a fixed key set —
`output_id` / `summary` / `hint` / `buffered_bytes` — and the value the tool returned, with any
`corrections` it wrote, becomes the buffer's contents rather than part of the envelope. The
framework's own alias advisory survives because `:1219-1221` explicitly re-attaches it into the
freshly-built envelope. Nothing does that for the tool's.

So this is not a missing feature: **one of the two writers of that field is re-attached at this
site and the other is not**, and the asymmetry is invisible from either end. `find.rs:1290` cannot
see the render path; `call_content` does not know the key is already meaningful to a tool.

Partially mitigated, and the mitigation is what makes it a `hint-composed-without-the-request`
instance rather than a capped-result one: `describe_payload_shape` does list `corrections` among the
buffer's keys, so the KEY NAME leaks while the content does not. A caller who reads the shape
listing carefully can infer that something was corrected and still cannot learn what.

Measured 2026-09-10 by running the two calls above; the code path was then read at
`types.rs:1207-1221`. Not inferred from the source alone.

## Why this class and not its neighbour

Tagged `cluster/hint-composed-without-the-request`. The envelope's `hint` is composed from the
response's SHAPE — how many bytes, which buffer, how to page it — and carries nothing derived from
the REQUEST, including the fact that the request was not the one executed.

Deliberately not tagged `cluster/capped-result-presented-as-complete`, though it is adjacent: that
class is about a truncated result read as whole, and here the truncation IS announced
(`buffered_bytes`, `output_id`, and the shape listing that names `corrections`). What is
unannounced is specifically the request-side reinterpretation. Per the one-tag rule, if a reader
judges the capped-result claim the better fit, that is a retag rather than a second file.

## Fix

Fixed on `experiments` — SHA `2183a058d14e25589403c666f8166b48e4873a00`, patch-id
`f1fa07c475e33d75ae5b615539c483f8c090caeb`.

`src/tools/core/types.rs`, `Tool::call_content`'s overflow branch, two changes:

1. The tool's own `corrections` is carried out of `val` into the freshly-built envelope.
   The envelope is assembled from a fixed four-key literal, so nothing the tool wrote
   reaches the caller unless it is re-attached there by name.
2. The framework advisory stopped being a wholesale
   `buffered["corrections"] = {param_aliases}` assignment and now routes through
   `merge_param_corrections` — the same function the small-output and error paths call,
   making this site its THIRD caller. **This was a second defect, not a tidy-up**: with
   the tool's value carried in at step 1, the old assignment would have overwritten it on
   every call where an alias ALSO fired. The tests found that; this file had not recorded
   it, because § *Symptom* was observed on a call where no alias fires.

**The envelope-contract decision this section asked for: carried VERBATIM, no cap.** The
inline path already carries this same value uncapped, so capping it only on the overflow
path would make the advisory's CONTENT depend on the RESULT size — the exact coupling
being removed, in a quieter form. Its size is bounded by the caller's own REQUEST
(`find.rs` emits one entry per repaired filter leaf, `update.rs` one per lifted top-level
param), never by the result, so it cannot grow on an axis the caller could not predict.

Both alternatives this section floated were considered and rejected. A byte cap plus a
pointer into the buffer costs a classified cap constant, a `cap_probe` row and a
truncation annotation (`every_cap_constant_is_classified`, `truncation_sites`) — and
reintroduces the coupling. "Name the key only, as the shape listing already does" is what
the code did before this fix: § *Root cause* records that the shape listing naming
`corrections` is precisely what makes the omission unreadable.
## Tests added

Three, in `src/tools/core/tests.rs`, each observed RED before the fix and green BY NAME in
both gate lanes — the changed code is not librarian-gated, so the lean lane is real
coverage here rather than vacuous:

- `the_tools_own_corrections_reaches_the_caller_on_the_buffered_path` — the production
  shape. Called with the CANONICAL param name so no alias repair fires and
  `param_corrections` is `None`, which is how `doc(action="find", rel_path=…)` behaves.
  **It carries the control § *Reproduction* demanded**: it asserts
  `corrections.param_aliases` is ABSENT, so it cannot be satisfied by the framework half
  that was never broken. Without that assertion the test passes against unfixed code.
- `the_buffered_envelope_keeps_both_the_tools_own_hint_and_the_framework_alias_hint` — the
  overwrite twin, object shape (`find.rs`'s `{filter, hint}`, where both writers use the
  key `hint`).
- `a_bare_array_corrections_survives_the_buffered_path_with_the_framework_advisory` — the
  non-object arm (`update.rs`'s bare array), promoted to `{tool, param_aliases}`.

The two existing fixtures gained a `big` field rather than being duplicated; their
inline-path tests pass `big: false`. That field is annotated load-bearing ON THE FIXTURE
LINE, because setting it `false` at every call site retires all three buffered-path guards
while leaving the suite green — a removal no assertion here can catch.
## Resume

Found by an Opus re-review of an unrelated fix round, which ran the tool rather than reading it —
the source-level trace of the same site had been read twice that day by two other parties without
anyone noticing this asymmetry, because from the source the re-attach at `:1219` looks like the
site handling `corrections`.

Recorded deliberately: two LATENT siblings of this exact class — an advisory dropped by a render
path — were fixed in `aed56c8d` the same day, while this one, the only PRODUCTION-REACHABLE one,
was left open. The reason is the envelope-contract decision above, not priority. A later reader
seeing the two fixes should not conclude the class was swept.
