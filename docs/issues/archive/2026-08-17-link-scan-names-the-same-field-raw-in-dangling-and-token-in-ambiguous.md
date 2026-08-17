---
status: fixed
opened: 2026-08-17
closed: 2026-08-17
severity: low
owner: marius
related: []
tags: [librarian, link-scan, tool-output-shape, measurement-hygiene]
kind: bug
---

# BUG: `link_scan` calls the cited token `raw` in `dangling` and `token` in `ambiguous`, so one query across both silently answers half

## Summary

`librarian(action="link_scan")` returns parallel finding arrays whose entries describe
the same thing — a citation that failed to resolve — under two different key names.
`dangling[]` entries carry `raw`; `ambiguous[]` entries carry `token`. Any single grep
or `json_path` written for one array returns nothing from the other, and returns it
*successfully*, so the caller reads a partial answer as a complete one.

## Symptom (Effect)

Measured 2026-08-17 while verifying that no `HY-N` token had broken. The query looked
right and the zero was meaningless:

```
grep -o '"token":"HY-[0-9]*"' @tool_0f032c9d   ->  (no output, exit 1)
```

Read naively that says "no HY token is dangling or ambiguous". What it actually says is
"no HY token is *ambiguous*" — the `dangling` array was never searched, because its key
is `raw`. The two shapes:

```json
// dangling[0]
{ "src_id": "e0802ffca04e9bf7", "raw": "BUG-40", "kind": "EntryToken", "line": 157 }

// ambiguous[1]
{ "src_id": "e0802ffca04e9bf7", "token": "F-3", "line": 156,
  "candidates": [...], "candidates_total": 23 }
```

Note also that `dangling` carries `kind` and `ambiguous` does not, so the divergence is
more than one key.

## Reproduction

Commit `dc2d1dd8`, branch `experiments`.

```
librarian(action="link_scan", scope="project")
read_file("@tool_<id>", json_path="$.dangling[0]")    # -> has "raw"
read_file("@tool_<id>", json_path="$.ambiguous[0]")   # -> has "token"
```

## Environment

Linux, `experiments` @ `dc2d1dd8`. Affects any caller querying link_scan findings —
the tracker-hygiene sweep's proposed D12 (citation-resolvability, HY-9) is built
directly on these arrays.

## Root cause

The two finding types are separate structs serialized independently, and nothing forces
their shared field to share a name. Not yet traced to the line — the divergence is
visible in the output, and the serialization site is in
`src/librarian/tools/link_scan/`. *Measured 2026-08-17 from the live tool output above;
the struct definitions are inferred, not yet read.*

The interesting part is why it bites rather than merely being untidy. The repo's own
measurement discipline says to anchor detection on **structure, not vocabulary** —
`get_guide("tracker-conventions")` § *Detecting these fields*, learned from
`grep -c 'Status:'` counting prose *about* Status. A structural anchor is exactly what
`"raw":` and `"token":` are, and here the structure itself is inconsistent, so the
correct technique still produces a wrong count. That makes this a hazard for the
detector class that is being built on top of it, not just a style wart.

## Evidence

### The tool's own zero-warning fires for the wrong reason

codescout's `grep` helpfully warns that *"this zero describes what was searched, not the
pattern"* — advice aimed at pruned hidden paths. Here the search space was right and the
**key name** was wrong, which the warning cannot distinguish. A caller who heeds the
warning still concludes correctly-shaped nonsense.

## Hypotheses tried

1. **Hypothesis:** `raw` and `token` mean different things — `raw` being the generic
   citation text across several kinds, `token` the entry-specific parsed id. The `kind`
   field appearing only on `dangling` was the circumstantial support.
   **Test:** read the three `json!` literals at the serialization site instead of
   inferring intent from the output.
   **Verdict:** **rejected.** All three arms serialize the same expression:

   ```rust
   "ambiguous":  { "src_id": row.id, "token": c.raw, "line": …, "candidates": … }
   "dangling":   { "src_id": row.id, "raw": c.raw, "kind": …, "line": … }
   "cross_repo": { "src_id": row.id, "raw": c.raw, "line": … }
   ```

   No distinction existed to preserve — which settles the A-or-B question in § Fix and is
   why the § Resume warning against unifying before checking could be discharged rather
   than heeded. `kind`'s absence from two arms was equally accidental: `c.kind` is in
   scope for all three.
## Fix

**Fixed 2026-08-17 in `7c218338` (`experiments`), replayed on the wire.** Fast-forward
promotion, so this SHA *is* the master SHA once promoted.

**Wire replay** against the release binary built 15:56, project-scope `link_scan`:

| Array | Keys | token-bearing rows |
|---|---|---|
| `ambiguous` | `candidates`, `candidates_total`, `kind`, `line`, `raw`, `src_id` | **0** |
| `dangling` | `kind`, `line`, `raw`, `src_id` | **0** |
| `cross_repo` | `kind`, `line`, `raw`, `src_id` | **0** |

All three share the same four keys, `ambiguous` keeps its two extras, and no row carries
`token`. `kind` reads `"EntryToken"` — the `as_str()` value, byte-identical to the old
`Debug` rendering, which is the check that the fragility fix changed nothing observable.

This is what the unit test could not show: it calls `finding()` directly, so it proves the
constructor's shape but not that the three call sites were rewired to use it. Only a real
scan does that.

**Option A (unify) — but implemented as a constructor, not a rename.** A rename leaves
three `json!` literals free to diverge again, and `link_scan/mod.rs` had **no tests at
all**, which is how they drifted side by side in the first place. One construction site
makes divergence impossible rather than merely corrected:

```rust
fn finding(src_id: &str, c: &extract::Citation, extra: Value) -> Value
```

Arm-specific fields (`candidates`, `candidates_total`) merge in on top, so unifying the
shared shape does not flatten what makes `ambiguous` useful.

**Wire change, deliberately.** `ambiguous[].token` is now `ambiguous[].raw`, and `kind`
is present on all three arrays instead of one. Nothing in-repo consumes these — the
audience is agents reading a report, and HY-9's proposed D12 is not built — so this is
the cheapest moment it will ever be.

**One adjacent fragility removed rather than propagated.** `kind` was serialized as
`format!("{:?}", c.kind)`. `Debug` is a developer-facing rendering with no stability
promise, so a variant rename silently changed the API — and the failure mode is a
consumer's filter quietly matching nothing, which is *this bug's own shape* one field
over. `CitationKind::as_str()` now spells the wire values explicitly, with strings
identical to what `Debug` produced.

Not done, and deliberately: a `legend` field of the kind
`audit_doc_refs` gained in `f908e883`. The array names (`ambiguous`, `dangling`,
`cross_repo`) already carry their meaning, and the harm here was a field NAME splitting
in two, not a value nobody could interpret. Adding a legend would be answering a
different bug.
## Tests added

Two in `src/librarian/tools/link_scan/mod.rs` — which had none before, and that absence is
part of the root cause:

- **`every_finding_array_names_the_cited_text_the_same_way`** — the RED test. Asserts all
  three shapes carry `raw`, `kind`, `src_id`, `line`, **and** that none carries `token`.
  The absence assertion is the point: `token` is the name that split the vocabulary, so
  the test pins that no arm can reintroduce it.
- **`finding_carries_arm_specific_fields_alongside_the_shared_shape`** — guards against
  over-unifying, i.e. a shared shape that drops `candidates`.

One in `extract.rs`:

- **`citation_kind_wire_values_match_what_debug_emitted`** — asserts `as_str()` equals
  `format!("{kind:?}")` for every variant, which is what makes the `Debug` swap provably
  behaviour-preserving. It also fails if a variant is renamed without updating the
  mapping — the silent-API-change that using `Debug` invited.

**Mutation-verified:** reintroducing `"token"` in the constructor fails the RED test with
`left: Null, right: "F-3"` on the ambiguous row.
## Workarounds

No longer needed. Historical, for anyone reading a report from a build before this fix:
query both names, `grep -oE '"(raw|token)":"HY-[0-9]+"'`, and treat a single-array count
as a floor.

The general lesson outlives the fix and is worth keeping: **absence from a findings array
proves nothing** until you have checked the key name AND the cap. `n_refs_found` against a
50-entry `findings` window is the other half of the same trap
(`docs/issues/archive/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md`).
## Resume

N/A — fixed, replayed on the wire, archived.
## References
- `src/librarian/tools/link_scan/` — finding serialization
- `get_guide("tracker-conventions")` § *Detecting these fields* — the anchor-on-structure
  rule this defeats
- `docs/trackers/tracker-hygiene-log.md` — HY-9 (proposed D12, built on these arrays),
  HY-12
