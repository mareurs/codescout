---
status: fixed
opened: 2026-05-24
closed: 2026-05-24
severity: low
owner: marius
related: [docs/issues/2026-05-24-ci-test-matrix-undercount.md]
tags: [ci, librarian, ulid, ordering, timing]
kind: bug
---

# BUG: librarian::tools::timeline tests flaky on fast ULID generation

## Summary

Two `librarian::tools::timeline::tests` cases fail on ubuntu-latest CI
runners because they create multiple events in a tight loop and assume
strict-monotonic ordering. ULIDs encode millisecond timestamps + random
suffix; when two events are generated within the same millisecond, the
random suffix becomes the tiebreaker — not the creation order. On a fast
CI runner the loop body finishes inside one millisecond and the tests
panic with the wrong ordering. Pre-existing flakiness, masked while CI was
dormant (2026-04-13 → 2026-05-24).

## Symptom (Effect)

```
thread 'librarian::tools::timeline::tests::returns_events_newest_first'
panicked at src/librarian/tools/timeline.rs:142:9:
assertion `left == right` failed
  left: String("n2")
 right: "n3"
```

```
thread 'librarian::tools::timeline::tests::intent_verdict_pair_flattens_resolves_edges'
panicked at src/librarian/tools/timeline.rs:209:9:
assertion `left == right` failed
  left: String("01KSCK8S4F95HPYXP9DA5917ZZ")
 right: "01KSCK8S4F6N97C1RJQ06N8DMD"
```

Both ULIDs share the timestamp prefix `01KSCK8S4F` — same millisecond.

## Reproduction

```bash
# Fast loop hitting sub-ms event creation:
for i in $(seq 1 200); do
  cargo test --lib librarian::tools::timeline::tests::returns_events_newest_first 2>&1 \
    | grep -E "panicked|test result"
done
```

Reproduces probabilistically on fast machines, deterministically on most
GHA ubuntu-latest runners (per CI run 26356842338).

## Environment

- ubuntu-latest GHA runner with default features (librarian enabled)
- Not observed on macos-latest (different timing characteristics) or
  Windows (different mechanism — see CRLF bug)

## Root cause

ULIDs are sortable by timestamp granularity of ~1 millisecond. When two
ULIDs share a millisecond, their relative order is determined by the
random suffix (cryptographically random per ULID spec). The test
`returns_events_newest_first` creates 3 events in a back-to-back loop
expecting `arr[0] = "n3"`, but if all 3 events land in the same ms, the
sort order may be `n2, n3, n1` (or any permutation).

```rust
for i in 1..=3 {
    crate::librarian::tools::event_create::call(&ctx, json!({
        "artifact_id": "a", "kind": "note",
        "payload": {"text": format!("n{i}")}
    })).await.unwrap();
}
// assumes arr[0]["payload"]["text"] == "n3" — false when all 3 share ms.
```

## Evidence

CI run 26356842338 — `Test (ubuntu-latest / default)`:
> `test result: FAILED. 2463 passed; 6 failed; 7 ignored`

Failing pair:
- `librarian::tools::timeline::tests::returns_events_newest_first`
- `librarian::tools::timeline::tests::intent_verdict_pair_flattens_resolves_edges`

The other 4 failures (`server::guide_hint_tests::*`) are a separate
environmental issue — they need `~/.config/librarian/workspace.toml`
seeded. Fixed as part of the CI seed step in the same engagement.

## Hypotheses tried

N/A — ULID timing mechanics are well-documented.

## Fix

Three options, in increasing invasiveness:

**A. `tokio::time::sleep(Duration::from_millis(2))` between event_create
calls.** Cheapest, but pure latency tax. Sleeps run during every test
execution.

**B. Inject a clock into event_create so tests can advance time
deterministically.** Most correct, biggest change — touches the
event_create signature.

**C. Use ULID monotonic-mode generation.** The `ulid` crate supports
monotonic-generation (increment last bit when timestamps match) via
`Generator`. Switching `event_create` to a thread-local Generator gives
strict monotonicity within the same ms without sleeps.

Recommend C. It's a 1-shot change in `event_create::call` (or wherever the
event id is allocated), produces real-world value (any rapid event burst
gets stable ordering), and removes the test flakiness.

## Tests added

The two failing tests are the regression cases. Add a stress test that
loops 100x and asserts ordering holds — would have caught the flake
locally.

## Workarounds

Currently the tests pass locally on slower dev machines because more time
elapses between event_create calls. The flake is CI-specific.

## Resume

1. Locate `event_create::call` or wherever the ULID is allocated.
2. Switch to `ulid::Generator` thread-local for monotonic generation.
3. Verify the two failing tests pass on ubuntu-latest CI.
4. Add stress test (100x loop) to ensure no regression.

## References

- `src/librarian/tools/timeline.rs:122-144, 209` (the failing tests)
- `src/librarian/tools/event_create.rs` (probable allocation site)
- Sibling rot surfaced same CI run:
  - `docs/issues/2026-05-24-ci-macos-tempdir-canonicalization.md`
  - `docs/issues/2026-05-24-ci-test-matrix-undercount.md` (fixed in `621732a6`)
