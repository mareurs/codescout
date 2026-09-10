---
id: '50aed1562ca29abc'
kind: bug
status: open
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

Re-attach the tool's own `corrections` into the envelope alongside the framework's, or promote both
under one key at that site. The design question this needs and does not yet have: the envelope is a
deliberate progressive-disclosure budget, so adding an unbounded advisory to it needs an
envelope-contract decision (truncate the hint? name the key only, as the shape listing already
does?) rather than an unconditional insert. That is why this is filed rather than fixed inline.

## Tests added

None yet. The guard that would have caught it is a fixture returning its own `corrections` from
`call()` while producing an over-budget payload; the analogous fixtures for the two inline paths
were added 2026-09-10 and stop at the inline branch.

## Resume

Found by an Opus re-review of an unrelated fix round, which ran the tool rather than reading it —
the source-level trace of the same site had been read twice that day by two other parties without
anyone noticing this asymmetry, because from the source the re-attach at `:1219` looks like the
site handling `corrections`.

Recorded deliberately: two LATENT siblings of this exact class — an advisory dropped by a render
path — were fixed in `aed56c8d` the same day, while this one, the only PRODUCTION-REACHABLE one,
was left open. The reason is the envelope-contract decision above, not priority. A later reader
seeing the two fixes should not conclude the class was swept.
