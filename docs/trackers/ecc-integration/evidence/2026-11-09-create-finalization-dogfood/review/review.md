# Independent create finalization review

## Finding — Important: worktree ordering is not guarded by the downstream failure fixtures

At candidate src/librarian/tools/create.rs:741 and :752, the slug and augmentation failure tests pass worktree=false. The registration-failure test alone does not establish registration's placement inside the transaction. Applied M7 moves the unchanged registration block before unchecked_transaction; the original create module still passes all 51 tests.

This survivor is behaviorally significant. After observing survival, the diagnostic retained the exact M7 production bytes and changed only those two fixtures' worktree flags to true. Both then failed the existing registration assertion with observed count 1, expected 0. The preceding SQLite-cause, artifact-count and augmentation-count assertions passed. Thus a later slug or augmentation error can leave committed registration while the artifact rolls back, without the original suite detecting the broken catalog boundary.

Recommendation: exercise each downstream failure with and without worktree context, then rerun exact M7 and the affected mutation set. No production change is recommended. The controller owns the lasting test changes and final gate.

## Reviewed identity and assessment

Base: 038fffb3447b55900e617a251b6472486259e07c.
Uncommitted candidate: src/librarian/tools/create.rs SHA256 68f4fdaa9030bd0bd78939ee7debb37c6dd5b30917db07e43278505fee27d3bd.
Review source: /home/marius/work/claude/codescout/.worktrees/ecc-create-finalization-review.
Candidate production code matches the bounded catalog transaction plan; direct helpers use the same connection, registration is before commit, disk I/O follows commit, and existing best-effort cleanup preserves the original I/O error. No production correctness defect was identified. Ready with the test-gap correction and the controller's required final gate. This verdict applies to the recorded candidate, not later controller edits.

## Executed evidence

Independent baseline: 51 passed, 0 failed, 0 ignored.
Initial applied candidate mutations: exactly 7; 6 killed, 1 survived, 0 inconclusive. Each ran the default-feature create module, 51 selected tests. These are observed mutation verdicts, not a general coverage percentage.

| Mutation | Observed pass/fail | Verdict |
|---|---:|---|
| M1 drop commit | 38 / 13 | Killed |
| M2 commit after artifact and slug | 49 / 2 | Killed |
| M3 omit registration | 50 / 1 | Killed |
| M4 commit before registration | 50 / 1 | Killed |
| M5 omit disk cleanup | 49 / 2 | Killed |
| M6 remove uncertainty warning | 50 / 1 | Killed |
| M7 registration before transaction | 51 / 0 | Survived |

M7 diagnostic after survival: 49 passed / 2 failed; both failures are registration count 1 versus 0. This is a diagnostic rerun of M7, not an eighth mutation.
Final clean rebuilt control: 51 passed / 0 failed; candidate bytes unchanged. Source restoration hashes were verified for all seven initial runs and the diagnostic. git diff --check passed.

The first restored control returned 50/1 while emitting M6's shortened warning although the review source retained its original SHA. The runner had switched source roots inside an outer build-slot lease; Cargo reused the previous mutant binary. The final control touched only the unchanged source's mtime and re-leased slot1, which also automatically cleared the cache because it exceeded its size ceiling; the clean rebuild passed. Retain both outputs. Future replay must acquire with-slot INSIDE mutation-probe's test command so the actual source root is registered, then reacquire from the restored source root for its control. This infrastructure false red does not change the initial seven verdicts: every mutant run compiled its armed source.

## Replay and retained evidence

All paths below are relative to this review directory:
- candidate.rs, candidate.patch: exact reviewed bytes and base diff.
- mutations.json, run-review.py: initial M1–M6 descriptors and historical runner.
- M1–M6 .patch/.log files, results.json: individual outcomes and restoration checks.
- M7-registration-before-transaction.json/.patch/.log, M7-result.json: survivor descriptor and passing result.
- M7-mutated.rs, M7-snapshot-check.json: actual mutated source, verified equal to the recorded patch; SHA256 509aac9a2fb6cb6c973a5d0e346e447b26f119094678670206d009e0762598ce.
- M7-diagnostic.rs, M7-diagnostic-fixtures.patch, M7-diagnostic.log, M7-diagnostic-result.json, diagnose-M7.py, run-M7-diagnostic.py: diagnostic source, two fixture changes, and observed residual registration.
- initial-mutation-summary.json: exact seven-run population and measured totals.
- baseline.log, restored-control.log, restored-control-rebuilt.log, restored-control-rebuilt-result.json: controls, including the retained stale-binary failure.

Retained isolated mutation source trees:
- /home/marius/work/claude/codescout/.worktrees/ecc-create-finalization-review.worktrees/mutation-slot-0
- /home/marius/work/claude/codescout/.worktrees/ecc-create-finalization-review.worktrees/mutation-slot-1

Safe invocation shape: mutation-probe.sh --strict --file src/librarian/tools/create.rs --find <descriptor.find> --replace <descriptor.replace> -- ./scripts/with-slot.sh --slot N cargo test --lib librarian::tools::create::tests:: -- --test-threads=4. Read descriptor values through subprocess argv, not shell interpolation. Use TMPDIR=/home/marius/.cache/codescout-ecc-tmp and CODESCOUT_GATE_TMPFS_BASE=off; do not preset CARGO_TARGET_DIR or hold an outer build lease across source-root switches. Slots1 and2 were used for this review; slot0 was untouched.

## Declined to judge / limits

- Stronger filesystem atomicity, directory removal, or reversal of worktree registration after a disk error: the plan explicitly preserves existing best-effort disk cleanup. The existing cleanup/error/warning paths were reviewed and mutated; no stronger guarantee is claimed.
- Shared-MCP generic call-graph contradiction: source evidence came from opened definitions and executed tests; the controller owns the incidental navigation record.
- Full repository gate, real private MCP checks, integration and shipping: assigned to the controller and not represented by this module-only review.
