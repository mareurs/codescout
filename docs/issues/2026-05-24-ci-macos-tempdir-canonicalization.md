---
status: open
opened: 2026-05-24
closed:
severity: medium
owner: marius
related: [docs/issues/2026-05-24-ci-test-matrix-undercount.md]
tags: [ci, macos, tempfile, path-canonicalization]
kind: bug
---

# BUG: 11 unit tests fail on macOS CI because /var → /private/var symlink

## Summary

Eleven unit tests fail on macos-latest runners with `assert_eq!` mismatches
of the shape `left: "/private/var/folders/...", right: "/var/folders/..."`.
macOS's `/var` is a symlink to `/private/var`. `tempfile::tempdir()` returns
a path under `/var/folders/...`, but the production code (in `Agent::new`,
`activate_project`, etc.) canonicalizes the path via `std::fs::canonicalize`,
which resolves the symlink to `/private/var/folders/...`. The tests assert
the un-canonicalized value, so left/right diverge by exactly the `/private`
prefix. Pre-existing — surfaced when CI started exercising the macOS test
matrix on 2026-05-24 (matrix-undercount fix in `621732a6`).

## Symptom (Effect)

```
thread 'agent::tests::activate_replaces_previous_project' panicked at
src/agent/mod.rs:1268:9:
assertion `left == right` failed
  left: "/private/var/folders/p8/.../T/.tmpduVjfN"
 right: "/var/folders/p8/.../T/.tmpduVjfN"
```

Affected tests (all in macos-latest no-features, identical mechanism):

- `agent::tests::activate_replaces_previous_project` (mod.rs:1268)
- `agent::tests::activate_sets_project` (mod.rs:1257)
- `agent::tests::agent_is_clone_safe` (mod.rs:1342)
- `agent::tests::home_root_set_on_first_activate` (mod.rs:1478)
- `agent::tests::home_root_set_from_initial_project` (mod.rs:1460)
- `agent::tests::home_root_not_changed_by_second_activate` (mod.rs:1492)
- `agent::tests::new_with_valid_project` (mod.rs:1241)
- `tools::config::tests::activate_project_switches_focus_by_id` (501)
- `tools::config::tests::activation_response_emits_legacy_index_when_db_present` (1230)
- `tools::symbol::tests::symbols_propagates_error_when_fallback_also_fails` (5557)
- `util::path_security::tests::write_to_tmp_allowed` (1095)

Same pattern in default and local-embed configs.

## Reproduction

```bash
# On macOS (CI or local):
cargo test --no-default-features 2>&1 | grep -E "panicked|FAILED" | head
```

## Environment

- macOS 15.x ARM64 (macos-latest GHA runner; also reproducible on local
  macOS dev machines)
- All Rust toolchains and feature configs

## Root cause

macOS canonicalization expands `/var` to `/private/var` (file-system level
symlink, see `man hier`). Test assertions compare a `tempfile` result
(unresolved) against a path returned by production code that has been
through `canonicalize`. Both refer to the same on-disk location but as
distinct strings.

## Evidence

CI run 26356842338 — `Test (macos-latest / no-features)`:
> `test result: FAILED. 1959 passed; 11 failed; 7 ignored`

All 11 failures share the `left: /private/var, right: /var` shape.

## Hypotheses tried

N/A — symlink mechanics are well documented.

## Fix

Two viable shapes:

**A. Canonicalize the expected side in tests.** Wrap the temp path in
`.canonicalize().unwrap()` before constructing the expected value:

```rust
let tmp = tempfile::tempdir().unwrap();
let expected = std::fs::canonicalize(tmp.path()).unwrap();
let agent = Agent::new(Some(tmp.path().to_path_buf())).await.unwrap();
assert_eq!(agent.project_path(), expected);
```

Cleanest — production code stays normalizing, test mirrors the same
normalization.

**B. Don't canonicalize in production.** Lossy — breaks any deduplication
across symlinked roots. Not recommended.

Apply shape A across all 11 sites. Look for a shared test helper
(`make_test_agent` or similar) that constructs the expected path.

## Tests added

The 11 failing tests themselves are the regression cases. Fix verifies once
they pass on macos-latest.

## Workarounds

None on macOS. Linux unaffected (no /var symlink); Windows uses different
tempdir mechanics and avoids the issue.

## Resume

1. Search for `tempfile::tempdir` + `assert_eq!` patterns in the 11 cited
   files — the test surgery is mechanical.
2. Or factor a `canonicalized_tempdir()` helper.
3. Verify on macos-latest CI run.

## References

- `src/agent/mod.rs:1241-1492` (7 tests)
- `src/tools/config/tests.rs:501, 1230` (2 tests)
- `src/tools/symbol/tests.rs:5557` (1 test)
- `src/util/path_security.rs:1095` (1 test)
- Sibling rot surfaced same CI run:
  - `docs/issues/2026-05-24-ci-test-matrix-undercount.md` (fixed in `621732a6`)
  - `docs/issues/2026-05-24-ci-ubuntu-default-tests-flaky.md` (separate engagement)
