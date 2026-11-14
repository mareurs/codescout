---
id: '5f9659f9e20a2224'
kind: bug
status: open
title: 'BUG: the prose append_entry path never sets key_note, so a ledger with a non-key file name gets no warning'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-14
severity: low
unverified: 'Read from the code, not run. Not established: the live response of a prose append on a ledger with a non-key file name (no call made), and whether the omission was a deliberate choice. A reserve-only prose call writes no section, so the params rule (note only when a section was written) would also give it no note.'
---

# BUG: the prose append_entry path never sets key_note, so a ledger with a non-key file name gets no warning

## Summary
When a ledger's file name is not a canonical key, the params path of `append_entry` returns a `key_note` saying that no `[[key:TOKEN]]` can name the entry until the file is renamed. The prose path writes the same kind of section and returns no such note.

## Class fit
cluster/unclassified: two write paths for one tool, and only one of them carries a notice the caller needs. The prose outcome type has no field for it.

## Symptom (Effect)
A prose ledger named, for example, `Ledger_One.md` takes the entry silently. The caller learns nothing about the file name. A params ledger with the same name gets `key_note`.

## Reproduction
Not run. Read path:
- Params: `src/librarian/catalog/augmentation.rs:1153` sets `key_note` with `ledger_key_note(path, &new_id)` when a section was written. `src/librarian/tools/append_entry.rs:529` copies it into the response. Tests `the_response_carries_a_key_note_when_the_ledger_name_is_not_a_key` (tool) and `a_ledger_whose_own_name_is_not_a_key_still_takes_the_entry_and_says_so` (catalog) pin it.
- Prose: `AllocateOutcome` (`augmentation.rs:1171`) has no `key_note` field. The prose branch of `call` (`append_entry.rs`, from line 351) builds its JSON from `id`, `reserved`, `section_written`, `body_max`, `reserved_max`, `frontmatter_max`, `next_step`, `adopted` and `cites`, and returns. `ledger_key_note` is called from `append_entry` only.

## Environment
experiments at fe5370c2f.

## Root cause
Task 7 of Plan C1 added `key_note` to `AppendOutcome` only. `allocate_entry_id_with_cites` returns `AllocateOutcome`, which was not extended, and the prose branch of the tool does not call `ledger_key_note`.

## Evidence
- Grep for `key_note` in `src/` shows it in `AppendOutcome`, `append_entry` and the params tool branch, and in no prose code.
- The C1 execution-state document lists "prose path sets no `key_note`" as finding N-4.

## Hypotheses tried
None. Found by the Plan C1 whole-branch review (N-4), read from the code.

## Fix
Not started. Add `key_note: Option<String>` to `AllocateOutcome`, set it with `ledger_key_note` when `section_written`, and copy it into the prose JSON. Or decide the prose path should not carry it and say so in the tool description. Add a test with a non-key file name on the prose path.

## Tests added
None.

## Workarounds
None needed for data. A caller can check the file name against the key rule by hand.

## Resume
Open. Not started.

## References
- `src/librarian/catalog/augmentation.rs` (`AppendOutcome`, `AllocateOutcome`, `ledger_key_note`)
- `src/librarian/tools/append_entry.rs` (`call`)
- `docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md`
