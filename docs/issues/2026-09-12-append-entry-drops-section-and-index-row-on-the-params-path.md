---
kind: bug
status: open
tags:
- cluster/accepted-parameter-silently-dropped
closed: null
opened: 2026-09-12
owner: marius
related: []
severity: medium
---

# BUG: `append_entry` accepts `index_row` / `title` / `body` / `anchor_heading` on the params path and never reads them

## Summary

`doc(action="append_entry")` has two branches. Passing `entry_collection` takes the **params**
branch, which writes a row into the augmentation and returns. Omitting it takes the **prose**
branch, which reserves an id and — given `title` + `body` + `anchor_heading` — writes the section,
optionally with an `index_row` in the same `fs::write`.

Every one of those five fields is declared on the shared `Args` struct. On the params path they
are parsed, accepted, and **never read**. The call returns `Ok` with an allocated id, no section,
no row, and no diagnostic.

## Symptom (Effect)

A caller maintaining a params-backed ledger that also keeps a body table — 21 of 49 guarded
ledgers do, measured 2026-09-07 — has no way to close the two-call window, and the parameter that
exists precisely to close it is accepted without effect. The caller has positive evidence it was
applied, because nothing refused it.

The result is the capture window this repo already tracks: params row written, body section and
index row written by a second call, and the entry row-less in between. The allocator's own comment
states the bound — *"a caller doing two calls always leaves the entry row-less for the interval
between them, and no discipline available to them shortens it."*

## Reproduction

1. Pick a params-backed ledger declaring an `entry_collection`
   (`docs/trackers/open-issue-work-queue.md`, collection `tasks`).
2. Call `doc(action="append_entry", id=…, id_prefix="BL", entry_collection="tasks", entry={…},
   index_row="| {id} | … |", index_after_line="|----|", title="…", body="…",
   anchor_heading="…")`.
3. The call returns `Ok` with the allocated id. No section is written, no row is inserted, and
   neither `index_row` nor the section triple is mentioned in the response.
4. The response *does* carry `snapshot_missing` and `undefined_in_body` — asking the caller to do
   by hand the two things the ignored parameters requested.

## Root cause

The section value, including its `index_row`, is constructed **inside** the
`if a.entry_collection.is_none()` block in `src/librarian/tools/append_entry.rs`. The params path
calls `augmentation::append_entry(&mut cat, &target, entry_collection, &a.id_prefix, a.entry,
&a.cites)` — a signature with no section parameter.

That the prose branch always returns is not inferred: the params call site reads
`a.entry_collection.as_deref().expect("the None case returned above")`, which is only sound
because the `None` case cannot reach it. So the five fields are unreachable on this path by
construction.

**The file already knows this class.** Its own comment on the both-or-neither check says *"`Args`
has no `deny_unknown_fields`, so before this existed a caller passing `index_row` alone got `Ok`
with no row and no error — a silent drop."* That defect was fixed **within** the prose branch,
where the check sits. The identical silence on the params branch was not covered, because the
guard lives inside the branch it guards.

## Evidence

### The five fields are declared once and read on one path

`Args` carries `entry_collection`, `title`, `body`, `anchor_heading`, `index_row`,
`index_after_line`. `PendingSection` (with its `index_row`) is built only in the prose branch.

### The response asks for exactly what it ignored

On a params append the tool returns `snapshot_missing` (*"add the row(s) to the body's
table/section"*) and `undefined_in_body` (*"has no `## <ID> — <title>` heading in the body"*).
Both are satisfiable by the parameters that were dropped, which is what makes the silence
expensive rather than merely untidy: the remedy text and the ignored input describe the same two
writes.

### Four first-hand instances, this session

`BL-73`, `BL-74`, `BL-75` and `BL-76` were each appended to `docs/trackers/open-issue-work-queue.md`
as a params row, then given a body section and a table row by a separate `doc(action="update")`.

## Hypotheses tried

1. **Hypothesis:** the params path refuses the section fields, like the prose path refuses a
   half-pair.
   **Test:** read the branch; `section` is constructed inside `entry_collection.is_none()` and the
   params call site takes no section argument.
   **Verdict:** rejected — there is no refusal, and no code to reach one.

2. **Hypothesis:** it is documented as prose-only, so the caller is at fault.
   **Test:** the tool schema says `index_row` is *"written in the SAME file write as the section
   … only with a section"*, which describes a constraint, not a branch. Nothing states that
   `entry_collection` and a section are mutually exclusive.
   **Verdict:** partly — the constraint is stated, the silence on violating it is not.

## Fix

Not applied. Two shapes, and the choice is a design decision rather than a detail:

- **Refuse.** On the params path, reject any of the five with a message naming the branch. Cheap,
  closes the silence, and leaves the two-call window open — so it fixes this bug and not the one
  that motivated the scout.
- **Honour.** Let the params path take a `PendingSection` and write the params row, the section
  and the index row in one file write. Closes both. This is *direction (3)* of
  `2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md`, whose own Resume
  recommends it for the prose path and whose measurement — 21 of 49 ledgers keep entry rows — is
  the same population.

**Refusing first is not a smaller version of honouring** — it would make every params-backed
ledger's maintenance louder without making it correct, so prefer honouring if both are on the
table.

## Tests added

None yet. The discriminating test is a params append passing `index_row` + the section triple and
asserting the body carries both afterwards; the mutation it must kill is dropping the section
argument at the `augmentation::append_entry` call site.

Note the existing `the_tool_writes_the_index_row_in_the_same_call` covers the prose path only, and
its own doc comment names REACHABILITY as the property it exists to pin — the same property this
bug reports missing one branch over.

## Workarounds

Write the section and the index row in a following `doc(action="update", patch={body_edits: […]})`.
That is the two-call protocol, and it is what every params-backed ledger does today.

## Resume

Decide refuse-vs-honour. If honouring, the change is to `augmentation::append_entry`'s signature
and the params call site in `src/librarian/tools/append_entry.rs`; the section-writing and
row-splicing helpers (`PendingSection`, `PendingIndexRow`, `insert_index_row`) already exist and
are exercised by the prose path.

## References

- `src/librarian/tools/append_entry.rs` — both branches, and the both-or-neither guard inside one
- `src/librarian/catalog/augmentation.rs` — `PendingSection`, `PendingIndexRow`, `insert_index_row`
- `docs/issues/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md`
- `docs/trackers/issue-clusters/IC-15-accepted-parameter-silently-dropped.md`
