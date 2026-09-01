---
status: open
opened: 2026-09-01
closed:
severity: medium
owner: marius
related:
  - docs/issues/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md
  - docs/issues/archive/2026-07-10-preview-headings-silent-cap-20.md
tags:
  - cluster/hint-composed-without-the-request
kind: bug
---

# BUG: a scoped `artifact(get)` is billed the full heading map — narrowing the request does not narrow the response

## Summary

`artifact(action="get")` assigns `out["preview"]` **unconditionally** at
`src/librarian/tools/get.rs:536`, two lines *before* `body_selected` is consulted at
`:538`. A caller who narrows the request — `heading=`, `headings=`, `start_line=`/
`end_line=`, `entry_filter=` — still receives the full (20-entry-capped) heading map plus
the whole metadata block. The requested payload is correct and the selector is honoured;
what is wrong is that the **advisory** half of the response ignores the request that
produced it. Derived cost on one real call: **~2,611 bytes of preview against the
3,210-byte section asked for, ≈81%**.

This is the sibling of `2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md`
(IC-22). That file reports the *hint* being composed without the request; this one reports
the *preview* being composed without the request. Same cause, different field.

## Symptom (Effect)

A correctly-narrowed call returns the requested section **and** 20 headings the caller did
not ask for. Observed in an MCP session on 2026-09-01:

```
artifact(action="get", id="1b5a080fe2efcb6b",
         heading="## IC-22 — a next-step hint is composed from the response shape, not from the request")
```

Response carried both:

```
"body":      <the requested section>
"body_meta": { "line_count": 17, "source_line_count": 1190, "bytes": 3210, ... }
"preview":   { "shape": "default",
               "headings": [ {...}, ... 20 entries ... ],
               "summary": "...", "line_count": 1190,
               "total_headings": 32, "headings_truncated": true,
               "last_heading": {...} }
```

No error, no warning. The call succeeded and the extra payload reads as helpfulness.

## Reproduction

Any selector-bearing `artifact(get)` against an artifact with many headings:

```
artifact(action="get", id="1b5a080fe2efcb6b", heading="## Index")
artifact(action="get", id="1b5a080fe2efcb6b", start_line=1, end_line=20)
```

Both return the full `preview.headings` array. `git rev-parse HEAD` at observation:
`bb4688fdad5a3249ba677f51d22ed20e7fb6bedd` (reported as `provenance.head_commit` in the
response itself).

## Environment

Linux, MCP stdio transport, project `codescout`, branch `experiments`, profile
`~/.claude-sdd`. Not transport-specific — the defect is in the response builder, and the
CLI does not exercise it (see Hypotheses tried #2).

## Root cause

`src/librarian/tools/get.rs:536` executes `out["preview"] = preview::extract(...)`
inside `if let Some(body) = parsed_body.as_deref()`. The very next statement, `:538`, is
`if body_selected { ... }`. So the selector *is* known to the function; it is simply
consulted after the preview has already been built and inserted. The `Args` struct
(`:88–116`) carries every selector the gate would need — `full` `:101`, `heading` `:103`,
`occurrence` `:107`, `headings` `:109`, `start_line` `:111`, `end_line` `:113`,
`entry_filter` `:115`.

Measured 2026-09-01: observed directly in a live MCP response (quoted above), and the
ordering confirmed by reading `src/librarian/tools/get.rs:489–540`.

The class-level cause is shared with IC-22 and is worth stating separately because it
predicts further instances: **codescout composes advisory payload at the response layer,
where the request is out of scope.** The strongest evidence is not in this file's code path
at all but in a trait signature — `fn relevant_guide_topic(&self, result: &Value) ->
Option<&str>` (`src/tools/core/types.rs:1453`, 12 implementors) admits only the result, so
request-awareness is *unrepresentable* for guide injection rather than merely unimplemented.

## Evidence

### The 81% figure, and exactly how much of it is derived

The response's own `body_meta.bytes` gives the payload exactly: **3,210 bytes**.

The preview block was reconstructed from the real file rather than measured on the wire —
the MCP response is not byte-countable from inside the session:

```
python3 -c "
import json,re
hs=[]
for i,l in enumerate(open('docs/trackers/issue-clusters.md'),1):
    m=re.match(r'^(#{1,6}) (.+)',l)
    if m and len(m.group(1))==2: hs.append({'level':2,'text':m.group(2).strip(),'line':i})
cap=hs[:20]
prev=json.dumps({'shape':'default','headings':cap,'summary':'x'*140,'line_count':1190,
                 'total_headings':len(hs),'headings_truncated':True,'last_heading':hs[-1]})
print(len(hs), len(cap), len(prev))
"
→ 30 20 2611
```

**Exact:** the 20-entry cap, the heading texts, and the 3,210-byte payload.
**Approximate:** the `summary` field is modelled at 140 bytes, and the script counts only
`##` headings (30) where the response reported `total_headings: 32` across all levels. So
81% is a derivation with a stated error bar, not a measurement. It is not load-bearing —
the defect stands at any ratio.

### It is not the already-filed cap bug

`docs/issues/archive/2026-07-10-preview-headings-silent-cap-20.md` reports the cap being
*silent*. It no longer is: the response carries `headings_truncated: true` and
`total_headings: 32`. That fix is intact and this is a different defect — the complaint
here is that the array is sent **at all** on a narrowed request.

## Hypotheses tried

1. **Hypothesis:** this is IC-13 (*a capped result presented as complete*).
   **Test:** read the returned `preview` for truncation signalling.
   **Verdict:** rejected — `headings_truncated: true` and `total_headings: 32` are both
   present and honest. Nothing is presented as complete.
   **Evidence:** § *It is not the already-filed cap bug*.

2. **Hypothesis:** the CLI can measure the envelope-to-payload ratio on the wire.
   **Test:** `codescout artifact get <id> --heading '…' > scoped.json; wc -c` → 3,377 bytes,
   suspiciously close to the 3,210-byte payload, suggesting no preview at all.
   **Verdict:** rejected — the CLI emits **markdown**, not the MCP JSON envelope;
   `json.load()` failed with `Expecting value: line 1 column 1 (char 0)`. The two surfaces
   are not comparable and the 3,377 figure measures a different object. Recorded because
   quoting it would have published a fabricated number: this is `IC-5` (*the reproduction
   environment is not the gating environment*) caught mid-flight.

3. **Hypothesis:** the selector is being dropped (IC-15).
   **Verdict:** rejected — the `heading=` argument was honoured and the correct section
   returned. IC-15's own *Falsified by* clause excludes a parameter that was honoured.

## Fix

**Plan.** Compute `body_selected` *before* `:536` and branch the preview on it. When any
selector is present, emit a stub rather than the array:

```json
"preview": { "shape": "default", "line_count": 1190, "total_headings": 32,
             "headings": "omitted (selector present) — call with no selector for the map" }
```

Retaining `total_headings` is deliberate: it reports the **magnitude** the caller did not
receive, rather than merely its absence.

The change lives at `src/librarian/tools/get.rs:535-540`. Design context, and the two
sibling changes that share this invariant, are in
`docs/superpowers/specs/2026-09-01-request-aware-response-envelope-design.md`.

SHA and patch-id to be recorded here at fix time, per `get_guide("tracker-conventions")`.

## Tests added

None yet — the fix is not written. When it lands, the gating assertion
(`preview_stubbed_when_selector_present`) is **monotone under removal**: a dead preview
builder satisfies it perfectly. It must therefore ship paired with its positive twin, and
that twin already exists — `preview_present_by_default` at
`src/librarian/tools/get.rs:1024`. Neither alone covers the property.

## Workarounds

None needed for correctness — the requested payload is right. To limit the cost, prefer
`read_markdown(path, heading=…)` for artifacts not under librarian guard; guarded ledgers
(`docs/trackers/issue-clusters.md`) have no alternative surface and must pay it.

## Resume

Edit `src/librarian/tools/get.rs`: hoist the `body_selected` computation from `:538` above
the `out["preview"]` assignment at `:536`, then branch. Add
`preview_stubbed_when_selector_present` next to the existing
`preview_present_by_default` (`:1024`) and mutate **this site specifically** — a kill on
the hint builder or the guide trigger proves nothing about this one.

## References

- `docs/issues/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md` — the
  hint-side sibling; seed of IC-22.
- `docs/trackers/issue-clusters.md` § IC-22 — the class. **Grain question, unresolved:**
  IC-22's claim says "a system-authored *next-step hint*". A `preview` block is advisory
  payload but not a next-step hint, so admitting this file either widens the claim to
  "system-authored advisory" or wants a sibling class. Tagged IC-22 because the mechanism,
  the blind party, and the *Falsified by* clause all match exactly; flagged here rather
  than silently widening the class, which is the ledger owner's call.
- `docs/superpowers/specs/2026-09-01-request-aware-response-envelope-design.md` — the design.
- `src/tools/core/types.rs:1453` — `relevant_guide_topic`, the signature-level form of the
  same cause.
