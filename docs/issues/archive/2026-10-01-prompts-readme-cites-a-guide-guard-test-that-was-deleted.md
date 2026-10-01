---
id: '5dd5dc0075f93a52'
kind: bug
status: fixed
title: 'BUG: the prompts README and a doc comment cite a guide-guard test that was deleted, so a reader following either searches for a test that does not exist'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
closed: 2026-10-01
opened: 2026-10-01
related: []
severity: low
---

# BUG: the prompts README and a doc comment cite a guide-guard test that was deleted, so a reader following either searches for a test that does not exist

## Summary

`src/prompts/README.md` (rule 8's explanation) and the doc comment on `PULL_ONLY_GUIDE_TOPICS` (`src/prompts/mod.rs`) both said `every_guide_topic_is_triggered_or_declared_pull_only` "fails the build" for a guide topic that is neither triggered nor declared pull-only. That test was deleted in `94396e00` (2026-08-27) and replaced by Gate 2; its two halves were restored under new names. The guard was intact. Only the citations named a test that does not exist.

## Symptom (Effect)

Nothing fails. A contributor told by the README to *"give the topic a trigger or record it in `PULL_ONLY_GUIDE_TOPICS`"*, and that a named test enforces the choice, searches for that name and finds only comments calling it *the deleted* test (`src/prompts/mod.rs` and `src/server.rs` test doc comments), which reads as the guard having been lost.

## Reproduction

Measured 2026-10-01 on the working tree before `a49c3c1e`: `grep` for `every_guide_topic_is_triggered_or_declared_pull_only` over `*.rs` returns only doc comments, none a `fn`. The live test is `every_guide_topic_is_triggered_xor_declared_pull_only` (`src/server.rs`, `#[cfg(feature = "librarian")]`), which asserts triggered-or-declared and not-both. The "topic no longer exists" half is `pull_only_guide_topics_are_registered_with_real_reasons` (`src/prompts/mod.rs`).

## Environment

Branch `experiments`. Found while re-verifying `BL-25` in the open-issue work queue.

## Root cause

A rename-by-splitting: the combined test was removed and its halves restored elsewhere, and the two prose surfaces that named it were not in the commit's diff. Inferred from `94396e00`'s subject and the two doc comments describing the split; not measured beyond the grep above.

## Evidence

The restored test's doc comment: *"the triggered-xor-pull-only half of the deleted `every_guide_topic_is_triggered_or_declared_pull_only`"*.

## Hypotheses tried

1. **Hypothesis:** the guard was lost in the deletion. **Test:** read the live test body. **Verdict:** rejected: it asserts both directions and its fixture probes every tool's `relevant_guide_topic`.

## Fix

Fixed on `experiments`.

- **SHA:** `a49c3c1ef7981bc7451ae6a678bbdc522367b34e`
- **patch-id:** `df6a1d9366f51dc0772ae3eed613aeeae163cf63`

The README and the doc comment now name `every_guide_topic_is_triggered_xor_declared_pull_only`, and the doc comment says the no-longer-exists half is the other test. Gate green: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

## Tests added

None: the change is prose citing existing tests, and a test pinning a citation would be a sentence-pinning test of the kind this repo avoids.

## Workarounds

N/A, fixed.

## Resume

N/A, fixed.

## References

- `docs/issues/archive/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`, the bug whose recurrence gate this is.
- `docs/trackers/open-issue-work-queue.md` § BL-25.
