---
id: b284cd70c19eee0f
kind: bug
status: fixed
title: 'BUG: doc drops undeclared top-level keys, so update with dry_run=true performs the write'
owners:
- marius
tags:
- cluster/accepted-parameter-silently-dropped
closed: 2026-11-09
opened: 2026-11-09
owner: marius
related:
- docs/trackers/issue-clusters/IC-15-accepted-parameter-silently-dropped.md
severity: medium
unverified: 'RESOLVED 2026-11-09. Was: the refusal was tested only by calling Artifact::call directly, so the wire path was unseen. It is now shown on the rebuilt server: dry_run=true on doc update is refused and the file is unchanged. Not covered by this fix and neither tried nor ruled out: option 2, refusing any root key declared in no action''s schema.'
---

## Summary

`doc(action="update", dry_run=true, …)` performs the write. `dry_run` is not a parameter of `doc`: nothing reads it, and nothing tells the caller so. Any undeclared top-level key is dropped the same way (`bogus_param_xyz=true` was accepted too). Ignoring undeclared root keys is deliberate in `doc` (see Root cause), but `dry_run` is a real parameter on a sibling tool (`librarian`, action `merge_worktree`), so a caller has a good reason to send it, and the cost of being ignored is a write the caller believed was a preview.

## Class fit

This is the class "accepted at the boundary and silently dropped": the call succeeds, so the caller has positive evidence that `dry_run` was honoured, and no later observation separates "applied as a preview" from "ignored". It differs from the class's other members in where the drop happens. Here nothing downstream discards the value: the boundary never declared it. And the root-level leniency is a recorded design constraint, not an oversight, so the defect is narrower than "unknown keys are ignored".

## Symptom (Effect)

Observed 2026-11-09, session 677278c4, twice, on purpose the second time:

1. A real record. `doc(action="update", dry_run=true, id=<a bug record>, patch={status:"fixed", extra:{…}, body_edits:[{action:"replace", content:"x", heading:"## Fix"}]})` returned `{"updated": true}`. The `## Fix` section of that record held `x` afterwards and `status` was `fixed`. It was caught in `git diff` before any commit and repaired.
2. A throwaway document (`docs/scratch/dryrun-probe-20261109.md`, since deleted). The same shape with `dry_run=true` replaced the `## Probe` body. A second call with `bogus_param_xyz=true` in place of `dry_run` replaced it again. Both responses were `{"updated": true}` with no warning field.

## Reproduction

```
doc(action="create", kind="note", title="probe", rel_path="docs/scratch/probe.md", body="## Probe\n\noriginal\n")
doc(action="update", dry_run=true, id=<id>, patch={body_edits:[{action:"replace", heading:"## Probe", content:"CHANGED"}]})
read the file: the body is CHANGED
```

## Environment

Linux, codescout at `ef7fe2ee9`.

## Root cause

Established by reading:

- `dry_run` is declared for `merge_worktree` in the `librarian` tool's schema (`src/librarian/tools/librarian.rs:111`) and read in `LibrarianAdapter::is_destructive` for that action only (`src/librarian/adapter.rs:539`). For `doc update` the gate keys on `force` (`adapter.rs:534`), so a `dry_run` on `update` is read by nothing.
- Objects inside `doc`'s schema that declare `additionalProperties:false` (`augment`, `event`, `event.source`) are refused by `#[serde(deny_unknown_fields)]`. The patch object is strict too: an unknown key inside `patch` was refused by name in this session (`unknown field \`closed\``).
- The root object is excluded on purpose. The comment on `every_schema_object_promising_to_refuse_unknown_keys_actually_refuses` (`src/librarian/tools/artifact.rs:1723`) says the dispatcher passes one shared argument blob with every sibling action's keys aboard, so the root type cannot carry `deny_unknown_fields`, and that it was tried once and "broke every `doc(update)` call". I did not re-run that attempt.
- `grep` has the precedent for the narrow remedy: `refuse_native_keys_that_change_the_result` (`src/tools/grep.rs:809`) refuses one named key because ignoring it would change what the caller thinks they got.

Not established: whether a check against the schema's declared `properties` (as opposed to serde's `deny_unknown_fields`) would avoid the breakage that attempt hit. Sibling-action keys are declared in `properties`, so it might.

## Evidence

- `usage.db`, `tool_calls`: `doc` update calls carrying `dry_run`: ids 282775 (refused for another reason, an unknown key inside `patch`), 282776 and 283606. All three are this session. No other session in the log sent one, so this is a demonstrated hazard and not a measured habit.
- The file states described under Symptom.

## Hypotheses tried

None needed for the symptom.

## Fix

Fixed in `53a7d8ec6` with option 1, the narrow refusal.

- `Artifact::call` (`src/librarian/tools/artifact.rs`) runs `refuse_dry_run` right after the unknown-action check, before the audit stamp and the dispatch. `dry_run` true (the boolean, `"true"` or `"TRUE"`) is refused for every action with a recoverable error: `'dry_run' is not a parameter of doc`. The hint says what to do instead, and ends with "Nothing was changed."
- `dry_run` false is not refused.
- Other undeclared top-level keys are still dropped. That is the deliberate root leniency described under Root cause. Option 2 (refuse any root key declared in no action's schema) is not done and not ruled out.

## Tests added

In `src/librarian/tools/artifact.rs`:

- `dry_run_true_on_update_is_refused_and_leaves_the_file_unchanged`: three spellings; asserts the refusal, the hint, and that the file is byte-identical afterwards. A test that only checked the response would have passed on the old behaviour.
- `dry_run_true_is_refused_before_any_action_runs`: `find`, `get`, `update`, `move`, `delete`, `graft`, `append_entry`.
- `dry_run_false_is_not_refused`: pins how narrow the rule is.

Mutation, run: commenting out the call made the first two fail, and the update test failed with `dry_run on update must be refused: {"id": ..., "updated": true}`, which is this bug as filed. Reverted.

## Workarounds

Do not send `dry_run` to `doc`. To preview an `update`, read the record with `doc(action="get", id=…)` and make the edit with `force` unset, which keeps the body-shrink guard active.

## Resume

Nothing outstanding for this fix. The wire check was done on the rebuilt server on 2026-11-09: `doc(action="update", dry_run=true, ...)` on a throwaway document returned `'dry_run' is not a parameter of doc — doc(action="update") was not run` with the hint, and the file still held its original body. The throwaway was deleted.

Option 2 (refuse any root key declared in no action's schema) is not part of this fix. It stays a separate decision: it needs a check on the schema's declared `properties`, and a trial against the update call that broke the earlier `deny_unknown_fields` attempt.


## Fix provenance

- **SHA:** `53a7d8ec6eae4ba12c291d52a0f8ca50adc51b77` (`experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `8c9f770c48df0c29fe6c47a9c4b7fe42ffc893c5` — content hash of the diff; survives rebase and cherry-pick.

## References

- `src/librarian/adapter.rs:534-541` (`is_destructive`), `src/librarian/tools/librarian.rs:111`.
- `src/librarian/tools/artifact.rs:1723` (the root exclusion and its reason), `src/tools/grep.rs:809` (the precedent).
- `docs/trackers/issue-clusters/IC-15-accepted-parameter-silently-dropped.md`.
