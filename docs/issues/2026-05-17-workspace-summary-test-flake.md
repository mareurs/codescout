---
status: fixed
opened: 2026-05-17
closed: 2026-05-18
severity: low
owner: marius
related: []
tags: ["test-isolation", "agent", "config", "tmpdir", "order-dependent"]
kind: bug
---

# BUG: `workspace_summary_returns_none_for_single_project` flakes under full-workspace runs

## Summary

`cargo test --lib -p codescout workspace_summary_returns_none_for_single_project`
passes when run in isolation but fails when run as part of the full workspace
test suite (`cargo test --workspace`). The failure panics on `Result::unwrap()`
inside `agent/mod.rs:1771` because it tries to read
`/tmp/.tmpXXXX/.config/codescout/config.toml` which doesn't exist. Likely a
test-pollution issue where another test creates/sets `XDG_CONFIG_HOME` to a
tmpdir that doesn't survive to this test, OR the test itself sets up a
tmp env that the agent reads from but doesn't pre-seed the config file.

Surfaced 2026-05-17 during M5/M6/#52 workspace test runs. Did not block any
shipped work (each completed feature has its targeted test suite passing
in isolation).

## Symptom (Effect)

```
thread 'agent::tests::workspace_summary_returns_none_for_single_project' (1247065) panicked at src/agent/mod.rs:1771:70:
called `Result::unwrap()` on an `Err` value: reading global config /tmp/.tmpM9ajBa/.config/codescout/config.toml

Caused by:
    No such file or directory (os error 2)
```

## Reproduction

```
cargo test --workspace
# → workspace_summary_returns_none_for_single_project FAILS

cargo test --lib -p codescout workspace_summary_returns_none_for_single_project
# → passes (isolation)
```

Stash + retry confirms the failure is independent of any in-flight changes
on `experiments` HEAD as of 2026-05-17.

## Environment

- Date observed: 2026-05-17
- Tool: `cargo test --workspace`
- Branch: experiments @ 6aa570cd / 7d012ce4
- Affected test: `agent::tests::workspace_summary_returns_none_for_single_project`

## Root cause

Unconfirmed. Two leading hypotheses:

1. **Env pollution.** Another test sets `XDG_CONFIG_HOME` or `HOME` to a
   tmpdir that gets dropped before this test runs. Test order in the
   workspace suite matters.
2. **`agent/mod.rs:1771` unconditionally unwraps** the global config read.
   Should probably treat "no config file" as `Ok(default)` instead of `Err`.

## Evidence

- Stashed all in-flight changes, ran the test in isolation → passes
- Pre-existing on HEAD before any of M5/M6/#52 work landed
- The path `/tmp/.tmpM9ajBa/.config/codescout/config.toml` indicates the
  test (or its caller) set `HOME=/tmp/.tmpXXXX/`, but didn't pre-create
  the codescout config subtree

## Hypotheses tried

(none yet — opened during #52 session for parking-lot follow-up)

## Fix


**Root cause:** TOCTOU race in `GlobalConfig::load` at
`src/config/global.rs:55-80`. Old shape:

```rust
if !path.exists() { return Ok(None); }       // window 1
let metadata = std::fs::metadata(&path)...?; // racy: ENOENT here panics
let text = std::fs::read_to_string(&path)...?;  // window 2
```

Under parallel test runs, another test sets `HOME=/tmp/.tmpXXX/`,
this test's `path.exists()` returns true while the file is briefly
present, then the other test's `TempDir` drops, the path disappears,
and `metadata()` fails with ENOENT — bubbled as the global config
read error.

**Fix:** removed the `path.exists()` short-circuit and added explicit
`std::io::ErrorKind::NotFound → Ok(None)` arms on both `metadata()` and
`read_to_string()`. Each I/O call now race-tolerantly returns `Ok(None)`
when the file vanishes mid-load. Other errors (permission, IO) still
bubble with the original context message.

The fix is consistent with the function's contract — `load()` returns
`Result<Option<Self>>`, with `None` meaning "no usable global config".
"File deleted out from under us" is functionally identical to "file
never existed" for the caller, so the broader `None` interpretation
is correct.

**Verification:**
- Isolated: `cargo test --lib workspace_summary_returns_none_for_single_project` → pass
- Parallel: `cargo test --workspace` → both `workspace_summary_*` tests
  pass. Only failures are 2 unrelated retrieval_integration HTTP-mock
  tests (501 Not Implemented at localhost:36501).
- Total: 2443 passed / 2 failed (unrelated) / 41 ignored.

**Commit:** `<tba>` on `experiments`.

## Tests added


The pre-existing `workspace_summary_returns_none_for_single_project`
test now exercises the race-tolerant path implicitly — it runs in
parallel with other tempdir-using tests and no longer panics. No
new dedicated test added; the race is hard to reproduce
deterministically (depends on test scheduling + tempdir lifecycle),
and the existing test in parallel-suite mode is the load-bearing
regression check.

## Workarounds

Skip the workspace run for now:

```
cargo test --lib -p codescout audit_doc_refs   # narrow scope passes
cargo test --lib -p codescout workspace_summary_returns_none_for_single_project   # also passes alone
```

## Resume

Concrete next action: read `agent/mod.rs` around line 1771; check what the
unwrap target is. If it's a config file read, refactor to `unwrap_or_default()`
or surface the error as `RecoverableError`. Re-run `cargo test --workspace`
to verify.

## References

- Discovered during M5/M6/#52 session work
- Co-failing on stashed `experiments@6aa553f1` rules out the in-flight changes
- Sibling pre-existing flake: `docs/issues/2026-05-17-grep-integration-workflow-test.md`
