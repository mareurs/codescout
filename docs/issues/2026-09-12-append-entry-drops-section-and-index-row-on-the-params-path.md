---
kind: bug
status: investigating
tags:
- cluster/accepted-parameter-silently-dropped
closed: null
groundwork: 063681ed
handed_to: 8bd791df
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

**DECIDED 2026-09-12 — honour, not refuse.** Refusing is not a smaller version of honouring: it
would make every params-backed ledger's maintenance louder without making it correct, closing the
silence while leaving the two-call window that motivated finding it.

**Groundwork landed at `063681ed`** (sessionId `f3c594ce-c424-40d3-a603-9693cfef3f63`), and it is
behaviour-preserving today:

- `splice_pending_section` extracted from `allocate_entry_id`. It was inline there, **which is this
  bug's root cause stated as code**: the routine that honours `title`/`body`/`anchor_heading`/
  `index_row` lived inside the branch that excluded the params path. Shared rather than copied,
  because a second copy reproduces the defect the moment either drifts. All 81 augmentation tests
  pass against the extraction, which is what makes it behaviour-preserving rather than plausible.
- `augmentation::append_entry` gains a `section` slot and writes it **before** `tx.commit()`,
  mirroring `allocate_entry_id` — a failed splice rolls back, the id is not consumed, and the
  refusal's *"nothing was written"* stays true.
- `AppendOutcome` gains `section_written`, so the hint can stop telling a caller to write a section
  the server already wrote.

**What remains is the tool-layer wiring**, and the call site says so in place rather than leaving a
reader to infer it from a parameter that is always `None`: hoist the `index_row` + `PendingSection`
construction out of the prose branch so both branches build it from **one** place. A second,
copied construction is the thing this change exists to avoid.

**One detail that would be a silent defect when the wiring lands, recorded because only the
restructuring author sees it:** `snapshot_missing` is derived from a body read taken BEFORE the
write, so it must drop the new id when that id's index row was just written — otherwise the
response asks the caller to do by hand the exact thing the call just did. That is **this bug, one
field over**.

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

**Decided: HONOUR.** Groundwork landed in `063681ed`; the tool-layer wiring is handed to
sessionId `8bd791df`, who filed this. Status is `investigating` rather than `taken` because
the session that did the groundwork stopped — worked, no live owner.

Done in `063681ed`, all behaviour-preserving:

- `splice_pending_section` extracted from `allocate_entry_id` and shared. It was inline
  there, which is *why* this bug exists — the code that honours the five fields lived inside
  the branch that excludes them. 81 augmentation tests pass against the extraction.
- `augmentation::append_entry` takes `section: Option<&PendingSection>` and writes it BEFORE
  `tx.commit()`, mirroring `allocate_entry_id`: a failed splice rolls the transaction back,
  so the id is not consumed and "nothing was written" is true. This deliberately inverts
  part of that function's existing contract — the body read it already does must never fail
  the call, and this one must.
- `AppendOutcome::section_written`, the mirror of `AllocateOutcome`'s.
- `abs_path` read by reference (consuming it is what made a section write impossible to add
  without restructuring), and `snapshot_missing` drops the new id when its row was just
  written — that list is derived from a body read taken BEFORE the write, so leaving it
  alone re-introduces this bug one field over.

**Still owed, and the traps in it:**

1. Hoist the `index_row` + `PendingSection` construction out of the prose branch so both
   build it from one place. **Carry the two REFUSALS with it** — the both-or-neither check
   on `index_row`/`index_after_line`, and the refusal of `index_row` without a section
   triple. Hoisting only the construction gives the params path a way to write a row whose
   id nothing defines, which is the dangling-citation shape that refusal exists to prevent.
2. Pass `section.as_ref()` at the params call site — build it BEFORE `a.entry` is moved.
3. Surface `section_written` in the response, and reconcile it with the two existing hints:
   `undefined_in_body` self-corrects (it re-reads after commit), `snapshot_missing` does
   not. They look interchangeable and are derived at different times.
4. Discriminating test: params append with `index_row` + the section triple, asserting the
   BODY carries both afterwards. Mutation that must red: revert the call site to `None`. If
   it still passes, the test is reading the params row rather than the file.

**Do not run § Reproduction as written** — it appends to a live ledger and allocates a real
`BL-N` that can never be reused. The four first-hand instances below already establish the
behaviour; use a fixture.

The original signature note stands: the helpers already exist and are exercised by the prose
path. What the filing could not know is that `allocate_entry_id` already had both the
`Option<&PendingSection>` signature and the write-then-commit ordering — so honour is the
**symmetric** change against exercised code, not the expensive one, which inverts the cost
comparison § Fix had to guess at.

## References

- `src/librarian/tools/append_entry.rs` — both branches, and the both-or-neither guard inside one
- `src/librarian/catalog/augmentation.rs` — `PendingSection`, `PendingIndexRow`, `insert_index_row`
- `docs/issues/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md`
- `docs/trackers/issue-clusters/IC-15-accepted-parameter-silently-dropped.md`
