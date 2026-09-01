---
kind: bug
status: fixed
tags:
- cluster/hint-composed-without-the-request
- progressive-disclosure
- librarian
- hint
closed: 2026-09-01
opened: 2026-09-01
owner: marius
related: []
severity: low
unverified: "The preview.headings suppression proposed in the original Fix section was NOT measured and NOT done - whether envelope metadata pushes otherwise-inlinable sections over the 9KB inline budget is still unknown. Nothing in the shipped fix depends on it; recorded because a reader may otherwise assume the whole Fix section landed. Also: the gate's default lane showed 1 failure at commit time (tests/issue_clusters.rs::every_declared_class_has_an_index_row), which was a peer's uncommitted work on issue-clusters.md - three classes declaring a Slug with no Index row yet, all three verified absent from HEAD - and not this change."
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

**Root cause, MEASURED 2026-09-01 — and it is sharper than the hypothesis this section
originally carried.** The `inferred from the envelope's own output` note below was replaced by
reading the code.

`default_json_path_hint` (`src/tools/core/types.rs`) selects **the largest array anywhere within
a bounded depth**. It is not argument-blind by oversight — it is *array-selecting by design*,
and a scoped `get` answers with a `body` **string**. A string is never a candidate, so the
heuristic **structurally cannot** return `$.body`, whatever the caller passed. `preview.headings`
wins by being the largest array present.

The irony is load-bearing rather than decorative: that heuristic was itself the fix for an
earlier form of this same class (`docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md`),
whose own docs say *"a hint that cannot work for the result it is attached to is worse than no
hint: it converts a lookup into a failed call."* This is that class surviving in the one shape
its remedy cannot express — the payload whose useful field is not a list.

`json_path_hint` is a `Tool` trait method with a default, so no signature change was needed: the
result itself carries the evidence (`body` + `body_meta`). Note the two-trait detail, since it
cost a compile cycle here — `src/librarian/tools/*.rs` implement the **librarian's** `Tool`
trait, which has no `json_path_hint`; the overridable one is `crate::tools::Tool`, implemented
by `LibrarianAdapter` in `src/librarian/adapter.rs`.

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

**SHIPPED 2026-09-01** at `bb4688fd` (**`experiments`**), patch-id
`5e6ff450ad5eaf822283499492288b7ded15faf3`.

`LibrarianAdapter` — the layer that already overrides `format_compact` — now overrides
`json_path_hint`, delegating to a free `scoped_body_hint(&Value) -> Option<String>` so the
decision is testable without building an adapter. When the payload carries **both** a
`body_meta` object and a `body`, the hint is `$.body`; otherwise the general heuristic stands.

**Keyed on `body_meta`, not on `body` alone — that is the whole discrimination.** `body`
appears on a *full* artifact read too, and there the largest-array rule is still the better
answer: an augmented tracker's `$.augmentation.params.<collection>[*]` is worth far more than
a body the caller already holds entire. `body_meta` is emitted only when the server *scoped*
the read (`heading`, `headings`, or a line slice), so it is the narrowest available signal for
*"the caller named a part, and this is that part"*.

**Scoped to the adapter rather than to `default_json_path_hint`.** That heuristic is right for
`find`, `graph`, `state_at`, `link_scan` and the rest; one action's shape is not a reason to
move a rule the others depend on.

**The `preview.headings` suppression was NOT done, and is not owed.** The bug file proposed
measuring whether envelope metadata pushes otherwise-inlinable sections over the 9 KB budget.
That measurement was not run, so nothing here rests on it — and once the hint points at the
right field, the remaining cost is bytes rather than a wasted call, which is a different and
smaller problem. Recorded as unmeasured rather than dropped silently.
## Tests added

Two tests in `src/librarian/adapter.rs`'s `mod tests`, both **mutation-verified rather than
assumed green**.

1. `a_scoped_read_is_hinted_at_its_body_and_a_full_read_is_not` — a four-row table.
2. `the_scoped_hint_overrides_what_the_default_heuristic_would_have_said` — the end-to-end
   shape, carrying a **precondition assert** that `default_json_path_hint` really does return
   `$.preview.headings[*]` for this payload. Without that assert the test could pass while
   being about nothing, and it is also what keeps it honest if the heuristic changes.

**Mutations run:**

| mutation | effect |
|---|---|
| always `None` (pre-change behaviour) | kills **both**; the second reports `left: "$.preview.headings[*]"` vs `right: "$.body"` — the reported defect as an assertion |
| key on `body` alone, dropping the `body_meta` check | kills **exactly** the full-read row |

The second is why row 2 of the table is annotated as the discriminator: **do not delete it as
redundant.** Without it, a fix keyed on `body` alone passes and every *full* read silently gets
a worse hint — a strictly larger regression than the bug being fixed. Row 3 (`body_meta` with
no `body`) pins the same boundary from the other side.
## Workarounds

On any buffered `artifact(get, heading=…)`, ignore the hint and go straight to
`read_file("@tool_*", json_path="$.body")`; if that re-buffers, `read_file("@file_*",
start_line=1, end_line=N)`.

## Resume

N/A — shipped.

One thing deliberately left unmeasured rather than left implied: whether `preview.headings`
metadata pushes otherwise-inlinable sections over the inline budget. If it does, suppressing it
on a scoped read would *remove* calls rather than re-point them. Nothing in this fix depends on
the answer.
## References

- `src/librarian/adapter.rs:427-488` — the compact-summary ordering rule and BL-19
- `get_guide("progressive-disclosure")` — budgets, `@ref` buffers, and the
  "**Treating the summary as authoritative**" anti-pattern this hint walks the caller into
- `docs/trackers/2026-09-01-fable-system-review.md` SR-13 item 2 — the original, overclaimed
  form ("three calls to read a tracker section"), corrected here
