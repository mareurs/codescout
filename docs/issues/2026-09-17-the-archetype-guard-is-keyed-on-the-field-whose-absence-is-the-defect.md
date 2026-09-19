---
kind: bug
status: fixed
tags:
- cluster/selector-narrower-than-its-population
- librarian
- trackers
- tracker_design
- archetypes
- agent-ergonomics
closed: null
opened: 2026-09-17
owner: marius
related: []
severity: medium
---

# The archetype guard is keyed on the field whose absence is the defect

`librarian(tracker_design, archetype="audit_issues")` serves a shape that
`doc(update_entry)` cannot address, `doc(get, entry_filter=...)` is not enabled for,
and whose entries define no citable token. Each of those was fixed for `task_list` on
2026-08-18. `audit_issues` keeps entries in params exactly as `task_list` does, and
was not corrected.

The test that exists to catch this cannot see it, because it iterates archetypes that
declare `entry_collection` — and the missing `entry_collection` is one of the defects.

## Observed

Building a real tracker to the `audit_issues` archetype on 2026-09-16
(`docs/trackers/innovaplan-catalog-reconciliation.md` in the `backend-kotlin` repo,
artifact `5c8a613173e032a6`), following `tracker_design`'s served example verbatim:

```
doc(action="update_entry", id=..., entry_collection="issues", entry_id="3", fields={...})
→ update_entry: no entry `3` in `issues`
   hint: Known ids:  (+10 more)
```

Ten entries existed. None was addressable. The served `params_shape_example` numbers
them `"n": 1`, and `update_entry` resolves entries by `id`. The hint prints an empty
known-ids list followed by `(+10 more)`, which is itself confusing — it reports both
that nothing is known and that ten things are.

Recovering meant re-keying all ten entries `n` → `id` **and** rewriting
`params_schema` in the same call, since the schema served alongside the example pins
`n` as `required`. A `patch={params:...}` alone is refused:

```
merge_params: patch violates params_schema: /issues/0: "n" is a required property (+9 more)
```

## Three defects, one root

`tracker_design.rs` declares `entry_collection` for exactly three archetypes —
`failures` (:174), `tasks` (:315), `rules` (:484). Comparing `audit_issues` against
`failure_table`, which is the same shape of tracker:

| | `failure_table` | `audit_issues` |
|---|---|---|
| entry key in `params_shape_example` | `"id": "F-1"` | **`"n": 1`** |
| `entry_collection` declared | `"failures"` | **absent** |
| per-entry heading in `body_skeleton` | `## F-N — <title>`, with "NOT optional: this heading is the only thing that makes `F-N` citable" | **none** — only `## Per-issue detail` |

Consequences, in order of how quickly they bite:

1. **`update_entry` cannot address any entry.** Observed above. The standard
   entry-editing tool is unusable against a tracker built to the served example.
2. **`entry_filter` is not enabled.** `doc(get, entry_filter=...)` requires the
   augmentation to declare `entry_collection`; the archetype does not serve one, so a
   caller who follows the example exactly gets a tracker they cannot filter. *(Not
   hit in this session — `entry_collection: "issues"` was set by hand, which is why
   `entry_filter` worked here. It is latent, not observed.)*
3. **Entries define no citable token.** `link_scan` binds a token to
   `## <ID> — <title>`; the `audit_issues` body skeleton has no such heading, so its
   entries are exactly the `ledger_defines_nothing` case that
   `docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`
   was opened for.

## Why no guard caught it

`tracker_design.rs:1033-1043`:

```rust
/// Keyed on each archetype's OWN `entry_collection` declaration rather than a
/// [hardcoded list]
fn every_archetype_with_an_entry_collection_teaches_where_the_defining_heading_goes() {
    let Some(collection) = arch["entry_collection"].as_str() else {
        // skipped
```

The `let ... else` skips any archetype that declares no `entry_collection`. That
choice is deliberate and reasonable — keying on the archetype's own declaration
avoids a hardcoded list going stale — but it makes the guard **self-exempting**: an
archetype missing `entry_collection` is silently excluded from the test that would
have reported it missing. The two per-archetype assertions that do exist
(`:1059 failure_table_archetype_has_entry_collection_field`,
`:1069 constitution_archetype_has_entry_collection_field`) name their subjects
individually, and no equivalent exists for `audit_issues`.

The guidance text has the same blind spot. `tracker_design.rs:550`:

> Only archetypes keeping entries in params (`failure_table`, `task_list`) support
> it; `reflective` keeps entries in prose — retrofit first

`audit_issues` keeps entries in params too. It is in neither list, so a reader
checking whether their archetype supports filtering finds it named nowhere and cannot
tell which side of the line it falls on.

## Reproduction

```
librarian(action="tracker_design", archetype="audit_issues")
# params_shape_example -> issues[].n, no entry_collection key in the response

librarian(action="tracker_design", archetype="failure_table")
# params_shape_example -> failures[].id, "entry_collection": "failures" present
```

Then build a tracker from the first and call `update_entry` on any entry.

## Suggested fix

**FIXED 2026-09-19** — `38c171786ab80e9088c7b4e3ce7aa7d8dc9182c6`, patch-id `7b38047f3d0489161cb215f7a005b8b75d48bc35`. Both halves shipped: the archetype keys on `id` with an `^AI-\d+$` pattern, declares `entry_collection: "issues"`, and its skeleton carries the `## AI-N — <title>` heading; and `every_archetype_without_an_entry_collection_has_no_ledger_shaped_array` closes the self-exemption, so an array-of-objects params shape must declare an entry_collection. Both reds observed in the order that proves it discriminates — hardened guard against the unfixed archetype first, then the heading guard against a partial fix.

Smallest correct change: re-key `audit_issues`' `params_shape_example` and
`params_schema_example` from `n` to `id`, add `"entry_collection": "issues"`, and give
`body_skeleton` a `## <N> — <title>` per-issue heading carrying the citability
sentence `failure_table` already has. That is the 2026-08-18 `task_list` fix applied to
the archetype it skipped.

Worth more than the one-archetype fix: make the guard non-self-exempting. A test that
asserts *every* archetype whose `params_shape_example` contains an array of objects
declares an `entry_collection` and keys those objects on `id` would have caught this
one, and will catch the next archetype added without them. As written, the guard's
coverage shrinks silently every time an archetype omits the field.

## Notes

Found while building a tracker in another repo, not while working on codescout —
which is why it is filed here rather than in that repo's session log: the workaround
(re-key ten entries by hand) is local, but the defect is in the tool, and burying it
in a consumer's log puts it where nobody maintaining librarian would look.
