---
id: '25d8de9cf59c0e49'
kind: bug
status: fixed
title: doctor reports a host id that differs per fresh catalog, so the forward param probe may pass vacuously for doctor keys
tags:
- cluster/unclassified
closed: 2026-10-30
opened: 2026-10-30
owner: marius
severity: low
---

# BUG: doctor reports a host id that differs per fresh catalog, so the forward param probe may pass vacuously for doctor keys

## Summary

`doctor` returns `catalog_health.audit.host` (`src/librarian/tools/doctor.rs:1297`, from `resolve_host_id`). Each fresh in-memory test catalog mints a new id. Two otherwise identical `doctor` calls on two fresh contexts therefore never compare equal. `param_probe::sweep` decides "honored" by whether two outcomes differ, and `Librarian`'s probe builds a fresh context per call. For every `doctor`-labelled key the "outcome differs" test may then pass whether or not `doctor` reads the key.

## Class fit

`cluster/unclassified`. It is a green-but-uninformative result: a check that cannot fail for one population. The same shape is documented for `audit_log` in the probe's call site.

## Symptom (Effect)

Observed: the reverse probe added in `9173695c5` reported 22 `doctor:<key>` reads, for keys such as `topic` and `anchor_id` that `doctor` does not read. The first difference was `"host":"ripper-2b775b"` against `"host":"ripper-ff9c14"`.

## Reproduction

Remove the `drop_host` call from `strip` in `no_action_reads_a_key_its_description_does_not_name` (`src/librarian/tools/librarian.rs`) and run `scripts/with-slot.sh cargo test --lib librarian::tools::librarian::tests::no_action_reads`. It fails with the 22 `doctor:*` pairs.

## Environment

Linux, `experiments` at `9173695c5`.

## Root cause

The `host` difference is verified at the bytes. The consequence is now shown (2026-10-30): with `#[serde(skip)]` on `doctor::Args::offset`, the forward probe stayed GREEN. After masking `host` the same mutation fails with `doctor:offset (declared integer)`.

## Evidence

Run on 2026-10-30: the failing assertion listed 22 `doctor:*` pairs and a debug print showed the host strings above. After masking `host` the same sweep covered 259 pairs and passed.

## Hypotheses tried

None beyond the above.

## Fix

Done 2026-10-30. The forward probe's call closure in `librarian::tools::librarian::tests::every_action_labelled_schema_key_is_honored_by_that_action` now masks `host` (`drop_host`, the helper the reverse probe already used) before the outcomes are compared. Fixed at the call site, not in the shared `param_probe::outcome`, because `host` is a `doctor` field and the shared helper serves other tools.

## Tests added

No new test: the existing forward probe is the test. Mutation run: `#[serde(skip)]` on `Args::offset` in `src/librarian/tools/doctor.rs` stays green before the fix and fails after it; mutation reverted.

## Workarounds

The reverse probe masks `host`. The forward probe has none.

## Resume

Mutate a `doctor` argument read (for example `offset`) and run `every_action_labelled_schema_key_is_honored_by_that_action`. Green means the bug is real.

## References

- `docs/issues/2026-10-30-librarian-accepts-a-declared-key-for-an-action-that-does-not-read-it-and-the-response-does-not-say-so.md`
