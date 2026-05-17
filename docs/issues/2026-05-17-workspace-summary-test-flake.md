---
status: open
opened: 2026-05-17
closed:
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

Not yet applied. Two viable paths:

1. **Loosen `agent/mod.rs:1771`** to treat ENOENT as `Ok(default_config)`.
   Most natural fix; preserves test isolation.
2. **Ensure the test seeds an empty config file** in its tmpdir before
   activating an agent. More invasive; only fixes this one test.

Recommend option 1 — code shouldn't crash on absent config files anywhere
else either.

## Tests added

N/A — this bug *is* about a failing test.

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
