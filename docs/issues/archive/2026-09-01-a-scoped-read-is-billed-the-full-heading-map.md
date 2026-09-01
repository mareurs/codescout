---
kind: bug
status: fixed
tags:
- cluster/hint-composed-without-the-request
closed: 2026-09-01
opened: 2026-09-01
owner: marius
related:
- docs/issues/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md
- docs/issues/archive/2026-07-10-preview-headings-silent-cap-20.md
severity: medium
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

**SHIPPED 2026-09-01 on `experiments`, in three commits.** Each SHA is paired with its
patch-id because the SHA is positional and dies when `experiments` is rebased; every
patch-id below was **re-derived independently** at closeout rather than copied from an
implementer's report.

| SHA (`experiments`) | patch-id | what |
|---|---|---|
| `f3a76f81ded24630411894d8898492a402463f80` | `69d5fda78f7fcaa292f0b1fcc419bd4bd50cefef` | the stub itself |
| `aee9dd6bb7c85ff804ded190c0ccd6cac933bdbe` | `08bba53a99703b16f21aaed1d051fe0566d74e9f` | review round: contract annotation, heading-miss restore, `last_heading` kept, `total_headings` backfill |
| `b9bcfee42ea57c6dd64351efa3e677b1325fedce` | `6102147d08b38d0dbad1f4ee72984870335846ac` | heading-miss restore extended to the plural `headings=` selector |

**What shipped.** `body_selected` (already in scope) now gates the preview: on a
body-selected read `stub_preview` replaces the `headings` array with a note string,
keeping `line_count`, `total_headings` and `headings_truncated`. `entry_filter` is
deliberately excluded — it filters params rows rather than selecting body.

**Three corrections the review forced, each a defect in the original plan rather than in
the implementation:**

1. **The stub must not apply when the selector fails to resolve.** `body_selected` is true
   whenever `heading=` is supplied *including on a typo*, so the first version made a
   mistyped heading lose the map — costing the second round-trip this change exists to
   remove. Restored for both the singular and plural selectors, on missing **and**
   ambiguous. Not extended to `start_line`/`end_line` or `full=true`: a line range has no
   "did you mean" character the map would repair.
2. **`last_heading` is kept, not dropped.** It exists because `append_entry` inserts
   *before* its anchor, so a ledger's append point is its LAST heading. ~60 bytes against
   a ~2,400-byte array — 2% of the saving, protecting a fix that took its own bug file.
3. **`total_headings` is backfilled from the discarded array when absent.** It is stamped
   only above the 20-heading cap, so under-cap reads previously got the note and *no
   count* — strictly worse than before, in the commoner case. This is what makes the
   "magnitude is retained" claim true rather than merely stated.

Design context: `docs/superpowers/specs/2026-09-01-request-aware-response-envelope-design.md`.
## Tests added

Ten tests, all verified present by name at closeout. All exercise real `call(&ctx, …)`
paths against on-disk fixtures — **no hand-built response shapes**, which was a defect
found and corrected earlier in this same work stream
(`response-envelope-session-log:F-1`).

**The gate and its twin** — the absence assertion is monotone under removal (a dead
preview builder satisfies it perfectly), so neither covers the property alone:

- `preview_is_stubbed_when_a_body_selector_is_present` — `src/librarian/tools/get.rs:1269`
- `preview_headings_are_still_shipped_when_no_body_selector` — `:1345`
- `preview_present_by_default` — `:1239` (pre-existing; retained as the shape twin)

**The heading-miss restore** — the regression the review caught:

- `heading_miss_keeps_the_full_preview_not_the_stub` — `:978` (singular)
- `headings_selector_restores_preview_on_missing_member` — `:1167`
- `headings_selector_restores_preview_on_ambiguous_member` — `:1192`
- `headings_selector_stub_applies_when_all_members_resolve` — `:1140` (the control; without
  it, a mutation disabling the plural stub entirely passes the three above)

**Magnitude retained under the cap:**

- `stub_preview_backfills_total_headings_under_the_cap` — `:1314`

**The cross-file contract** — `section_headings_summary` requires `preview.headings` to be
a renderable array, and the stub's string is what suppresses the section map. Stated in
**both** production doc comments, not only in a test:

- `a_body_selected_read_summary_cannot_lead_with_the_heading_map` — `src/librarian/adapter.rs:1023`
- `an_unscoped_read_summary_still_leads_with_the_heading_map` — `:1049`

**Mutation-verified per guarded SITE, not per feature.** Both halves of the plural
`!missing.is_empty() || !ambiguous.is_empty()` condition were mutated separately, each
killing exactly the test for the condition it dropped; the stub gate, the `M5` backfill and
the `I4` drop-list arm were each killed individually. A kill at one site says nothing about
another.

### Verified LIVE after a rebuild, by a session that did not make the fix (2026-09-01)

Every other line in this file rests on `cargo test` green plus the author's own reading — both
of which are evidence about the **source tree**, never about the copy a running MCP server
serves. `reconnaissance-patterns:R-89`: freshness breaks on three independent axes (build,
process, distribution), an `include_str!`'d or long-lived server holds whatever it started with,
and **the session that made the edit is the least representative observer of whether it
shipped**, being the only one reading the copy its own reload handed it.

`codescout-3c` supplied the missing observer. After the operator ran `cargo rb` and reconnected
`/mcp` — a genuinely fresh process, started by neither of us — they called a heading-scoped
`artifact(get)` and reported:

> `preview.headings` returns `"omitted (body selector present) — call artifact(get, id=…) with
> no body selector for the map"`, and the response comes back **inline rather than buffered**.

Both halves matter and they are different claims. The stub proves the gate fires; **inline
rather than buffered** proves the envelope actually shrank below the buffering threshold, which
is the outcome the ~81% figure predicts and which no unit test in this fix asserts.

**Recorded here rather than left in the message, and the timing is the argument.** It arrived as
a peer message; `codescout-3c`'s session had **ended** within the hour, so the message was the
only copy and is now unreachable. That is `reconnaissance-patterns:R-166` — *a finding parked in
a commit message or a chat has no citable home* — firing on the very evidence that closes this
file, roughly sixty minutes after the law was written.

**Logged as a confirmation, not a catch.** CLAUDE.md § *Testing Discipline* asks for the
**denominator**: when a re-derivation confirms, publish the confirmation, because a population
that records only its catches looks self-correcting. This one confirmed. The denominator is
`codescout-3c`'s.
## Workarounds

None needed for correctness — the requested payload is right. To limit the cost, prefer
`read_markdown(path, heading=…)` for artifacts not under librarian guard; guarded ledgers
(`docs/trackers/issue-clusters.md`) have no alternative surface and must pay it.

## Resume

N/A — fixed, reviewed (Opus task review + scoped re-review, all findings addressed), gate
green on `experiments`.

Three **deferred minors** carried to the whole-branch review rather than discarded: a test
cites a gitignored `progress.md` path that will not survive the SDD workspace deletion;
the adapter fixture hard-codes the note string instead of referencing the constant; and
`stub_preview` has no direct unit test, so its "unknown preview shapes keep current
behaviour" claim holds by coincidence of key naming rather than by construction.
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
