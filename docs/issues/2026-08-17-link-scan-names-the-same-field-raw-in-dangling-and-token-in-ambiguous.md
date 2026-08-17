---
status: open
opened: 2026-08-17
closed:
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

1. **Hypothesis:** `raw` and `token` mean different things — `raw` being the
   pre-normalisation text and `token` the parsed id.
   **Test:** compare values across both arrays in one scan.
   **Verdict:** deferred, and it is the question the fix hinges on. Observed values are
   plain entry tokens in both (`"BUG-40"`, `"F-3"`), so they *look* interchangeable —
   but `dangling` also reports `kind: "EntryToken"`, which hints `raw` is the generic
   citation text across several citation kinds (md links, 16-hex ids), where `token`
   is entry-specific. If so the right fix is to name them apart *deliberately* and
   document it, not to unify them.

## Fix

Not implemented, and the choice depends on hypothesis 1:

**A. Unify** on one name (`token`) with `kind` present on both arrays, if the values
really are the same concept. Cleanest for callers; a breaking output change.

**B. Keep both names and document the distinction** in `get_guide("librarian")`, if
`raw` is genuinely the wider concept. Then also state the querying consequence: a
resolvability sweep must read both keys, and any count taken from one array alone is a
sample.

Either way, the guidance belongs next to whatever D12 ends up being (HY-9), since that
detector consumes exactly these arrays.

## Tests added

None. Wanted regardless of A or B: a test asserting both arrays' entry shapes in one
place, so the two cannot drift further without a failure. Today nothing compares them.

## Workarounds

Query both keys, always:

```
grep -oE '"(raw|token)":"HY-[0-9]+"' @tool_<id>
```

Or read the arrays separately with `json_path` and treat any single-array count as a
floor rather than a total.

## Resume

Read the finding structs in `src/librarian/tools/link_scan/` (the serialization site) and
settle hypothesis 1 — whether `raw` spans citation kinds that `token` does not. That
answer picks A or B; do not unify the names before checking, because a deliberate
distinction destroyed is worse than an undocumented one.

## References
- `src/librarian/tools/link_scan/` — finding serialization
- `get_guide("tracker-conventions")` § *Detecting these fields* — the anchor-on-structure
  rule this defeats
- `docs/trackers/tracker-hygiene-log.md` — HY-9 (proposed D12, built on these arrays),
  HY-12
