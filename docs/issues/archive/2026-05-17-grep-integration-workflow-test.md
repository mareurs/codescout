---
status: fixed
opened: 2026-05-17
closed: 2026-05-18
severity: medium
owner: marius
related: []
tags: ["grep", "integration-test", "output-shape", "regression"]
kind: bug
---

# BUG: `workflow_read_search_replace` integration test fails — grep result missing `matches` field

## Summary

`cargo test --test integration workflow_read_search_replace` panics at
`tests/integration.rs:66:55` because `search_result["matches"].as_array()` returns
`None`. The grep call against a directory path returns a result without a
`matches` field. Pre-existing on `experiments` HEAD `c1e46616` (verified by
running the test with all M5 changes stashed). Likely a regression from a recent
grep output-shape change.

## Symptom (Effect)

```
thread 'workflow_read_search_replace' (688839) panicked at tests/integration.rs:66:55:
called `Option::unwrap()` on a `None` value
```

`tests/integration.rs:66` is `let matches = search_result["matches"].as_array().unwrap();`
— the unwrap target is `as_array()` on `search_result["matches"]`. None implies
either `matches` isn't an array, or the key is absent from the object.

## Reproduction

```
cargo test --test integration workflow_read_search_replace
```

Fails on `experiments` HEAD `c1e46616` (M4 closure commit). Workflow:

1. Two-file fixture, both contain "Hello"
2. `Grep.call({ pattern: "Hello", path: dir.path() })` → result
3. Test expects `result["matches"]` to be an array of length ≥ 2

## Environment

- Date observed: 2026-05-17
- Branch: experiments @ c1e46616
- Cargo: stable
- Other integration tests in the same file pass (8 passed; 1 failed).

## Root cause

Unconfirmed. Hypotheses:

1. `Grep` now routes large results through `OutputBuffer` and the test fixture
   trips the threshold, so the response is `{ "output_id": "..." }` instead of
   `{ "matches": [...] }`. The 2-line fixture shouldn't overflow but caps may
   have been tightened.
2. The output shape changed (e.g. `matches` → `results`) without updating this
   integration test.
3. The grep filesystem path now returns a different envelope when input is a
   single directory vs file.

## Evidence

`git log --oneline -- src/tools/grep.rs` recent commits:

- `77c02d65 fix(grep): route @-prefixed buffer refs to grep_in_buffer`

The 77c02d65 commit only added an `@`-prefix early-return branch — should not
affect filesystem paths. Stash-and-retry confirms the failure is independent
of any uncommitted M5 changes.

## Hypotheses tried

(none yet — opened during M5 session for parking-lot follow-up)

## Fix


Hypothesis (2) confirmed — output shape changed without updating the
integration test. `Grep::call` with default `context_lines=0` returns
`{file_groups, total, files}` (grouped-by-file shape, see
`src/tools/grep.rs:188-203`). Only `context_lines > 0` keeps the legacy
flat `matches` array.

**Fix:** updated `tests/integration.rs:66-71` to read
`search_result["total"].as_u64()` instead of
`search_result["matches"].as_array()`. The new shape's `total` field
counts individual matching lines and is the direct semantic equivalent
of the old `matches.len()` assertion.

**Verification:**
- `cargo test --test integration workflow_read_search_replace` — passes.

**Commit:** `1c6f3969` on `experiments`.

## Tests added


`workflow_read_search_replace` itself — the test now reads the
current grep output shape (`total` field) and passes again. Catches
future regressions if grep output drops `total` or returns a
non-numeric.

## Workarounds

Skip the integration test until fixed:

```
cargo test --workspace --lib --quiet  # passes
```

## Resume

Concrete next action: bisect `git log src/tools/grep.rs src/tools/output_buffer.rs`
between the last known-passing commit and `c1e46616`. Add `dbg!(&search_result)`
on line 65 of `tests/integration.rs` to see the actual response shape. If
it's `{ "output_id": ... }`, decide whether the integration test should
dereference the buffer or whether the threshold needs lifting back.

## References

- Discovered during M5 (`call_graph` Phase B fix) full-workspace test run
  on 2026-05-17.
- Co-failing run on stashed `experiments@c1e46616` rules out M5 changes.
