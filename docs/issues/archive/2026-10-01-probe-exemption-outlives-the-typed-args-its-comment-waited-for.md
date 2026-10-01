---
id: '6de6196b7684b6b7'
kind: bug
status: fixed
title: 'BUG: the schema-honesty probe still exempted doctor''s fix and offset after doctor''s Args was typed, and the comment justifying the exemption said it was waiting for exactly that'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
closed: 2026-10-01
opened: 2026-10-01
related: []
severity: low
---

# BUG: the schema-honesty probe still exempted doctor's fix and offset after doctor's Args was typed, and the comment justifying the exemption said it was waiting for exactly that

## Summary

`every_action_labelled_schema_key_is_honored_by_that_action` (`src/librarian/tools/librarian.rs`, `mod tests`) carried `accepts_any_json: &["fix", "offset"]`, an admission that the probe cannot reach those two keys on `doctor`. The comment above it gave the reason: `doctor` read them through untyped accessors, so no ill-typed value is rejected, and *"a typed `Args` for `doctor` would let the probe reach them."* `doctor`'s `Args` was typed in `26b60af8` (patch-id `39641840397a72f257450e7d36b72e197a1c67bf`), so the condition the comment waited on had already been met, and the exemption was left behind.

## Symptom (Effect)

Nothing fails. That is the defect: the exemption is a standing admission of blindness for two keys, and after the typed `Args` landed the admission was false. A future regression that made `doctor` stop honouring `fix` or `offset` would have been excused by the list rather than caught.

## Reproduction

Measured 2026-10-01 against the bytes of `642acd3e^`, in an isolated worktree via `scripts/mutation-probe.sh --file src/librarian/tools/librarian.rs --find 'accepts_any_json: &["fix", "offset"],' --replace 'accepts_any_json: &[],' -- cargo test --lib every_action_labelled_schema_key_is_honored_by_that_action`: **SURVIVED**, 3 tests passed. Removing the exemption changes no verdict, so the clause cannot be false on any input the current code produces.

## Environment

Branch `experiments`, default feature lane (the test is a librarian test, so the lean lane never compiles it).

## Root cause

A comment recorded a precondition for retiring an exemption, and the sibling change that satisfied the precondition (`26b60af8`, `fix: Option<String>` at `src/librarian/tools/doctor.rs:497`, `offset: Option<usize>` at `:503`) did not touch the file holding the comment. The two sites are in different modules, so nothing connected them. Measured 2026-10-01: reading `Args` in `doctor.rs` shows both fields typed.

This is the *semantically inert* reading of a mutation survivor (`CLAUDE.md` § *Testing Discipline*): reachable and tested, but unable to be false on any input the fixed code produces, because a sibling repair removed its domain. The repair is to delete the clause. The discriminator is the same mutation on the pre-`26b60af8` bytes, which would be expected to kill; it was not run here, because the typed `Args` is read directly out of the source above.

## Evidence

Gate on the fix tree, 2026-10-01: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, with `librarian::tools::librarian::tests::every_action_labelled_schema_key_is_honored_by_that_action ... ok` in the default lane.

## Hypotheses tried

1. **Hypothesis:** the exemption still excuses a real blindness. **Test:** remove it and run the probe. **Verdict:** rejected: the probe passes without it (SURVIVED above).

## Fix

Fixed on `experiments`.

- **SHA:** `642acd3e7d7df6dbb67717cc68fa3d4e9b5be237`
- **patch-id:** `a931ac624aa15832ff3090ab069858baa3cba63d`

Deleted the `["fix", "offset"]` exemption and the comment block that justified it. `accepts_any_json` is now `&[]` for `doctor`'s `Spec`.

## Tests added

None new: the existing probe is the regression test, and it now has nothing to excuse. Its verdict on `doctor` is now unconditional for every labelled key. Mutation evidence above stands in for a new test, because the claim is that a clause is inert, not that a behaviour is missing.

## Workarounds

N/A, fixed.

## Resume

N/A, fixed.

## References

- `src/librarian/tools/doctor.rs` (`Args`) and commit `26b60af8`, the change that made the exemption false.
- `docs/trackers/issue-clusters/IC-11-doc-contradicted-by-code.md`, the class: a comment true when written, contradicted by code that has since changed.
