---
id: 03aef9ae4c1d0f6e
kind: bug
status: zombie
title: Usage database test intermittently fails initialization with SQLite error778 in the default gate
owners:
- marius
tags:
- cluster/unclassified
- testing
last_observed: 2026-10-28
opened: 2026-10-28
severity: medium
unverified: Seen once, in a research worktree, not on experiments. Not reproduced in 60 loaded iterations here (Linux). A disk-full or quota condition was not induced. The cause is unknown.
---

# BUG: Usage database test intermittently fails initialization with SQLite error 778 in the default gate

## Summary

The default library gate failed an existing usage database test during database initialization. The exact test passed once in isolation afterward.

## Class fit

The mechanism and environmental dependency remain unclassified. This records an observed database initialization failure rather than assuming a schema defect or inferring a disk capacity condition from a different temporary filesystem.

## Symptom (Effect)

usage::db::tests::write_record_stores_all_fields panicked in the shared tests::tmp helper at src/usage/db.rs:1334:40 while unwrapping open_db: "disk I/O error", SQLite error code778. The failure precedes the test's field assertions.

## Reproduction

Run ./scripts/gate.sh --check in the named worktree. One observed default library result was 7880 passed, 2 failed, 14 ignored. This is an intermittent observation, not a deterministic recipe. Running usage::db::tests::write_record_stores_all_fields alone passed once in0.31seconds; log gate-probe-write_record_stores_all_fields.log.

## Environment

Observed 2026-10-28 in research worktree /tmp/codescout-real-task-20261024 at f41b306b4c17bdb9bddc5cecf3e1baead8640ee8. Source/build input hashes were unchanged throughout the gate. The gate used a private short /var/tmp directory and its leased Cargo pool.

## Root cause

Unknown. What the error code says, from SQLite's documented extended codes: 778 is `SQLITE_IOERR_WRITE` (10 | 3 << 8), a write the operating system refused, so the failure was an I/O error during the WAL pragma or the table creation in `open_db` (`src/usage/db.rs`), not a schema error. Why the OS refused it is not established. The record's author saw `/tmp` quota exhaustion in the same session, on a different filesystem from the `/var/tmp` this gate used; that does not connect the two.

## Evidence

Private evidence: .codescout/resident-private/independent-transfer-20261028/gate.log, gate-result.json, gate-isolated-probes.json and the named isolated probe log. These are retained locally in the main repository and excluded from Git.

## Hypotheses tried

- Running the exact named test alone with a short private `/var/tmp` directory: passed once (research session).
- 2026-11-04 on `experiments`: the whole `usage::db::tests` module, `--test-threads=32`, 60 iterations, with 12 busy-loop processes competing for CPU (script in the session scratchpad, not committed). 0 of 60 iterations failed. This does not rule out a disk-full or quota condition, which the run did not induce.
- Parallel load and filesystem resource pressure remain unproven.

## Fix

None. No unrelated production source was modified.

## Tests added

None: this record parks an observed gate failure; no fix or reliable reproduction is claimed.

## Workarounds

Keep the failed full-gate result visible. An isolated pass does not turn the full gate green.

## Resume

Zombie: seen once (2026-10-28, a research worktree, default lane), not reproduced since. Re-open if SQLite error 778, or any `disk I/O error`, appears again in a `usage::db` test, and then capture `df` and the quota of the TMPDIR filesystem at that moment, which this record lacks.

## References

- The failure occurred while validating the independent effort transfer plan; that plan exists only on the research branches (commit `747ee5a45`).
