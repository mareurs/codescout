---
status: open
opened: 2026-09-01
closed:
severity: low
owner: marius
related: []
tags:
  - cluster/capped-result-presented-as-complete
  - progressive-disclosure
  - librarian
  - hint
kind: bug
unverified: 'No regression test yet. The cluster tag is the weaker half of IC-13 — the response is honestly marked as buffered (nothing is presented as complete), so only IC-13''s SECOND clause applies: the signal is computed correctly and the follow-up route it names is wrong. The Index notes two readers independently hit this same boundary in IC-13''s claim text, so this file is a third datapoint for that pending ruling rather than a clean member.'
---

# BUG: a heading-scoped `artifact(get)` that overflows hints at `$.preview.headings[*]` — the metadata — instead of `$.body`, the section the caller asked for

## Summary

When `artifact(action="get", heading=…)` exceeds the inline budget, the overflow envelope's
`hint` names `read_file(@tool_*, json_path="$.preview.headings[*]")`. The caller passed a
`heading`, so the payload they want is `$.body`. Following the hint returns the heading map
— data the caller did not ask for and already has — costing one wasted call before they
guess the right path. Cheap to fix and it fires on every large section read.

## Symptom (Effect)

```
artifact(action="get", id="1b5a080fe2efcb6b", heading="## Index")
→ {
    "output_id": "@tool_5cb2c106",
    "summary": "sections: ## What this ledger is for · ## The entry shape · …",
    "hint": "read_file(\"@tool_5cb2c106\", json_path=\"$.preview.headings[*]\") to extract a
             specific field, or read_file(\"@tool_5cb2c106\", start_line=N, end_line=M) to
             browse sections",
    "buffered_bytes": 25531
  }
```

`$.preview.headings[*]` is the heading list. `$.body` is the `## Index` section. The
`summary` block leads with the heading list too, so both the summary and the hint answer a
question the caller did not ask.

## Reproduction

At `72484f8d5817e4675191d84caaaad869abf78f71`, any section over ~9 KB:

```
1. artifact(action="get", id="1b5a080fe2efcb6b", heading="## Index")
   → overflow envelope; hint names $.preview.headings[*]
2. read_file("@tool_*", json_path="$.preview.headings[*]")   # following the hint
   → the heading map. Not the section.
3. read_file("@tool_*", json_path="$.body")                   # what was wanted
   → the section (itself re-buffered at this size — see Root cause)
```

**Control — the friction is narrower than it first looked.** A section *under* the budget
returns inline in **one** call, body included:

```
artifact(action="get", id="1b5a080fe2efcb6b", heading="## How a cluster becomes a rule")
→ { …, "body": "## How a cluster becomes a rule\n\n**Threshold: three or more…",
    "body_meta": { "bytes": 1595, … } }
```

So there is no general "three calls to read a tracker section" defect. The 2026-09-01
review's SR-13 item 2 claimed one; this file is the corrected, narrower form.

## Environment

- codescout `experiments` @ `72484f8d5817e4675191d84caaaad869abf78f71`
- Claude Code, `~/.claude-sdd` profile, MCP stdio

## Root cause

`inferred from the envelope's own output — the hint-selection code has not been read yet.`

The overflow hint appears to be derived from the payload's shape without reference to
**which arguments the caller passed**. `preview.headings` is present on every `artifact(get)`
response, so a generic shape-walk finds it first; `heading=` is what makes `$.body` the
answer, and that argument is not consulted.

Two contributing facts, both observed:

1. **The envelope carries `preview.headings` even for a heading-scoped read.** For
   `issue-clusters.md` that is 20 heading objects with `level`/`text`/`line`. It is pure
   overhead on a call that named its section, and it competes with the body for the
   ~9 KB inline budget — so it can push an otherwise-inlinable section over the threshold.
   Not yet measured: how many sections in the live corpus sit in that window. Worth
   measuring before sizing the fix, because if the answer is "several", suppressing
   `preview` on heading-scoped reads removes calls rather than just re-pointing them.
2. **A `$.body` extraction of a large section re-buffers**, so the chain really is three
   calls for a 25 KB section (`get` → `read_file($.body)` → `read_file(start/end)`).
   That part is progressive disclosure working as designed; only the hint is wrong.

`src/librarian/adapter.rs:427-488` (`librarian_compact_summary`) documents the ordering
rule for what a compact summary leads with — "**the answer the action was asked for** —
matched titles for `find`, **section headings for `get`**". For an *unscoped* `get`, section
headings are indeed the answer. The rule was written before `heading=` was the common case
and does not branch on it.

## Evidence

### The docstring states the intent this violates

`src/librarian/adapter.rs:471-476`:

> /// 2. **The answer the action was asked for** — matched titles for `find`, section
> ///    headings for `get`, and a head-preview of whichever field actually holds the prose.
> ///    Previously the body-cap warning was the *only* case, so every other response fell
> ///    through to the generic envelope and the call returned no payload at all: 104
> ///    `artifact` overflows in the live corpus, each a guaranteed wasted turn. BL-19.

"The answer the action was asked for" is exactly the right principle. `heading=` changes
what that answer is, and the mapping is fixed rather than argument-aware — which is BL-19's
own failure mode (a wasted turn per overflow) surviving in a narrower form.

### Observed cost, this session

Three wasted calls across the session — once on `## Index` and twice on other large
sections — each following the hint before falling back to `$.body`.

## Hypotheses tried

1. **Hypothesis:** every tracker section read costs three calls.
   **Test:** `artifact(get, heading="## How a cluster becomes a rule")`.
   **Verdict:** rejected — returned inline with `body` in one call. The defect is
   size-gated, not universal.
2. **Hypothesis:** the buffering of the 25 KB `## Index` section is itself wrong.
   **Test:** `buffered_bytes: 25531` against `TOOL_OUTPUT_BUFFER_THRESHOLD = 10_000`.
   **Verdict:** rejected — buffering is correct at that size.
3. **Hypothesis:** the hint is generic and ignores `heading=`.
   **Test:** compare the hint text on a heading-scoped overflow vs the documented
   ordering rule at `src/librarian/adapter.rs:471`.
   **Verdict:** confirmed at the output; **not yet confirmed in the hint-selection code**
   (see Root cause's `inferred from` line).

## Fix

Make the hint argument-aware: when the `get` call carried `heading` / `headings` /
`start_line`, lead with `json_path="$.body"` — and for a body that will itself overflow,
name the line-slice form directly (`read_file("@tool_*", start_line=1, end_line=120)`),
which skips a whole hop.

Consider also suppressing `preview.headings` on a heading-scoped read, or reducing it to a
count. Measure item (1) in *Root cause* first: if sections routinely sit just over the
budget because of envelope metadata, this removes calls; if not, it is only tidiness.

SHA: not yet fixed.
patch-id: not yet fixed.

## Tests added

None yet. Shape: a table test over `(args, expected_hint_json_path)` — `heading` present →
`$.body`; no `heading` → `$.preview.headings[*]`. **The `heading` row must fail against
today's tree** before the fix lands, or it pins nothing.

## Workarounds

On any buffered `artifact(get, heading=…)`, ignore the hint and go straight to
`read_file("@tool_*", json_path="$.body")`; if that re-buffers, `read_file("@file_*",
start_line=1, end_line=N)`.

## Resume

Read the hint-selection code — start at `src/librarian/adapter.rs:427`
(`librarian_compact_summary`) and find where `hint` is composed for the overflow envelope
(it may live in the shared `ToolContext::call_content` path rather than the librarian
adapter; `grep(pattern="to extract a specific field")` will locate the literal). Confirm
hypothesis 3 in the code, then add the failing table-test row.

## References

- `src/librarian/adapter.rs:427-488` — the compact-summary ordering rule and BL-19
- `get_guide("progressive-disclosure")` — budgets, `@ref` buffers, and the
  "**Treating the summary as authoritative**" anti-pattern this hint walks the caller into
- `docs/trackers/2026-09-01-fable-system-review.md` SR-13 item 2 — the original, overclaimed
  form ("three calls to read a tracker section"), corrected here
