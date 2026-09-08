---
status: open
opened: 2026-09-08
closed:
severity: high
owner: marius
related:
  - docs/conventions/test-env-isolation.md
  - docs/issues/2026-09-08-the-shell-suites-lane-is-flaky-and-ci-endpoint-sampling-misattributes-it.md
kind: bug
tags:
  - cluster/repro-env-diverges-from-gate-env
---

# BUG: the embedder tests read an ambient env var every local shell has and CI has none of, so `experiments` has been red for 36 hours with no owner and every local gate run green

## Summary

`src/retrieval/embedder.rs:285` reads the dense model name straight from process
environment:

```rust
let dense_model_name = std::env::var("CODESCOUT_EMBEDDER_MODEL_NAME").unwrap_or_default();
```

`CODESCOUT_EMBEDDER_MODEL_NAME` is set in every shell on this machine (it reaches the process
env from the gitignored `.env`, `.gitignore:41`). **CI has it nowhere** — no workflow step
exports it, and `.env` cannot travel. So `unwrap_or_default()` yields `""` in CI and a real
name locally.

That divergence was **latent and harmless until `9c03b32f`** (*"feat(embed): require a model
name, and refuse the blank one at the boundary"*, ET-3's last residual) turned a tolerated
blank into a hard refusal at the constructor. From that commit on, the same test suite passes
for every developer and fails for CI.

**This is the "experiments CI has been red, no owner" item** that
`docs/plans/2026-09-06-stale-ledger-and-shared-state-fix-queue.md` § *Not in this queue* records
and explicitly declines to own. It now has a cause.

## Symptom (Effect)

`Test (ubuntu-latest / default)` — and the macOS, Windows and `server-stack` lanes with it —
red on every run since 09-06T19:13, while `cargo test --workspace` is green on every developer
machine. **Last green ubuntu/default: `c9e6cb6b`, 09-06T17:35.**

The second-order cost is larger than the lane: with no green CI for 36 hours, nobody can cite a
CI result for *anything*, so every other red is unreadable too.

## Reproduction

Deterministic, non-destructive, and it does not touch `.env`:

```
CODESCOUT_EMBEDDER_MODEL_NAME= cargo test --lib retrieval::embedder::tests
    →  FAILED. 16 passed; 12 failed

cargo test --lib retrieval::embedder::tests          # control, var as normally set
    →  ok. 28 passed; 0 failed
```

## Environment

- Local: `CODESCOUT_EMBEDDER_MODEL_NAME` present in the process env of every shell here.
- CI: `ubuntu-24.04`, image `20260831.293.1`. `grep -nE 'CODESCOUT_EMBEDDER_MODEL_NAME|EMBED'
  .github/workflows/ci.yml` finds only `FASTEMBED_CACHE_DIR` — the variable is never set.
- `.env` is gitignored at `.gitignore:41`, so it can never reach a runner.

## Root cause

**A unit test reading ambient process environment.** The value is not supplied by the test, so
the test asserts about whatever the developer's shell happens to hold. `docs/conventions/test-env-isolation.md`
exists for this class.

Note what makes it survive review: **the test is not wrong and the commit is not wrong.**
`9c03b32f` is a correct hardening — refusing a blank model at the boundary is the right
contract. The defect is that the tests never declared their dependency, so hardening the
contract silently reclassified an unstated precondition as a failure, in one environment only.

## Evidence

CI `Test (ubuntu-latest / default)` failures are a **byte-identical set across three runs**
spanning ~21 hours (`ae7fe563`, `226bd69f`, `999553cf`) — systematic, not flaky:

```
retrieval::embedder::tests::a_sparse_error_status_surfaces_the_servers_body
retrieval::embedder::tests::dense_and_sparse_legs_run_concurrently
retrieval::embedder::tests::embed_batch_uses_discovered_batch_size_end_to_end
retrieval::embedder::tests::embed_one_batch_errors_on_sparse_exhaustion
retrieval::embedder::tests::sparse_429_retries_then_succeeds_within_a_few_attempts
retrieval::embedder::tests::the_dense_only_path_skips_empties_and_zero_fills_them
tools::semantic::semantic_search::classify_search_error_tests::the_batch_sparse_producers_real_error_reaches_an_embedder_bucket
```

Bisected by CI verdict, not by guess: last green `c9e6cb6b` (09-06T17:35), first red
`751c9cd1` (09-06T19:13). Fourteen commits in that window; **exactly one touches
`src/retrieval/embedder.rs` — `9c03b32f`.**

**Set arithmetic NOT reconciled, stated rather than smoothed over:** CI reports `13 failed` in
that lane and 7 distinct names were extracted from its `failures:` block; blanking the variable
locally fails **12** in `retrieval::embedder::tests` alone. Blank-string and absent-variable are
not the same input, and CI runs the whole workspace where the repro ran one module. The
*mechanism* is confirmed by the control; the exact populations are not the same measurement and
should not be quoted as one.

## Hypotheses tried

- *"A runner-image roll."* **Refuted** — image version byte-identical on the green and red runs.
- *"The `Shell suites` red is the same problem."* **Refuted** — that one is intermittent (3 of
  12) and green again at `226bd69f`; filed separately.
- *"The tip that CI first reported red is the cause."* **Refuted** — CI samples endpoints, and
  the window held 14 commits from 5 sessions.

## Fix

Not applied — it belongs to the tests, not to `9c03b32f`, and the direction is a choice:

1. **Have the tests supply the name.** Construct the embedder with an explicit model rather than
   letting it read the environment. Correct, and the only option that removes the ambient
   dependency instead of satisfying it.
2. **Export the variable in CI.** Cheap, and wrong in the way this class always is: it makes CI
   match one developer's shell rather than removing the dependency, and the next variable
   repeats it.

(1) is the fix; (2) would unblock the branch today if someone needs a green urgently. **Do not
revert `9c03b32f`** — the contract it added is right, and reverting trades a visible red for the
silent blank-model behaviour it was written to stop.

## Tests added

None yet. What is owed is a guard that fails when a test reads a `CODESCOUT_*` variable it did
not set — the class-level answer, since this is the second env-divergence bug filed today.

## Workarounds

None needed locally; the tree is green here by construction, which is precisely the problem.

## References

- `src/retrieval/embedder.rs:285` — the ambient read
- `9c03b32f` — the hardening that made the latent divergence fatal
- `.gitignore:41` — why `.env` cannot reach CI
- `docs/plans/2026-09-06-stale-ledger-and-shared-state-fix-queue.md` § *Not in this queue* — the
  ownerless-CI item this explains
- `docs/conventions/test-env-isolation.md`
