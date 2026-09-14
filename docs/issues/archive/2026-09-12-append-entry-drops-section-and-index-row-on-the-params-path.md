---
kind: bug
status: fixed
tags:
- cluster/accepted-parameter-silently-dropped
closed: null
groundwork: 063681ed
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

Applied: **HONOUR**, in two commits.

Fix SHA: `a1ca3baa` — patch-id `3c7905291d88bba06c280d5f4870d9256e2cde46`
Groundwork: `063681ed` — patch-id `e592ca340fd8bd2f702255bd1557215c5ebb876a`

`063681ed` extracted `splice_pending_section` from `allocate_entry_id` and gave
`augmentation::append_entry` a `section: Option<&PendingSection>` written BEFORE
`tx.commit()`, so a failed splice rolls the transaction back and the id is not consumed.
`a1ca3baa` hoisted the construction and wired the call site.

**What the filing could not know, and it inverts § Fix's own cost estimate:**
`allocate_entry_id` already had both the `Option<&PendingSection>` signature and the
write-then-commit ordering. Honour was therefore the *symmetric* change against exercised
code, not the expensive one — which is the ground the decision actually rests on.

**One silent defect avoided:** `snapshot_missing` is derived from a body read taken BEFORE
the write, so it still named the id whose row the call had just added. Left alone it would
have asked the caller to do by hand the exact thing just done for them — this bug, one
field over.

### A retraction, because the wrong version reached a commit message

`a1ca3baa`'s message says the construction *"needed no hoist — it was already above the
branch"*. **That is false.** At `063681ed` the `if a.entry_collection.is_none()` sits at
`:74` and the two constructions at `:169` and `:192`, **eight-space indented, inside the
prose branch**. The hoist was real and structural: three things moved together — the
both-or-neither `index_row` check, the refusal of a row with no section triple, and the
`PendingSection` construction.

**How the false correction was produced is the reusable part.** The claim came from reading
`:88`/`:118` in the WORKING TREE and taking them for the original — but the tree held
sessionId `8bd791df`'s uncommitted hoist, which had already moved them. Re-checking a stale
description against the live file is the right instinct, and here it returned a confidently
wrong answer, because on a shared checkout *"current"* includes work in flight.
`git show HEAD:<path>` separates them, and the four-space-versus-eight-space indent is the
tell that survives. Caught by `8bd791df`, whose delete edit had specified the eight-space
block as its `old_string` and succeeded — which it could not have done had the code already
been hoisted.

The reading matters beyond the record: *"no hoist needed"* makes this look like a
one-character oversight, when it was the same structural shape as the bug itself — the code
that honours the fields living inside the branch that excludes them.
## Tests added

`a_params_append_writes_its_section_and_index_row_in_the_same_call`
(`src/librarian/tools/append_entry.rs`), the params twin of
`the_tool_writes_the_index_row_in_the_same_call`.

**It asserts against the FILE, never the response**, and that is the whole design. The
response carries an allocated id whether or not anything was written — that IS the defect —
so a test reading `result["id"]` passes under the mutation it exists to catch while feeling
like a test of the write. The first draft here reached for exactly that, which is worth
recording: the wrong version is the one that comes naturally.

It also asserts `snapshot_missing` does not name the id whose row the call just wrote, and
that `section_written` is surfaced.

**MUTATION, observed:** the params call site reverted to `None` reds it with the ledger
byte-for-byte unchanged — no section, no row, `Ok` with an id. The filed defect verbatim,
and the evidence a green compile could not give.

Gate green: 9640 passed, 0 failed, both lanes.
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
- `docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md`
- `docs/trackers/issue-clusters/IC-15-accepted-parameter-silently-dropped.md`
