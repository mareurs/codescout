---
status: open
opened: 2026-08-30
closed:
severity: medium
owner: marius
related: []
tags:
  - tests
  - librarian
  - temp-guard
  - environment-sensitivity
  - false-attribution
kind: bug
---

# BUG: three temp-guard tests fail from any checkout under `/tmp`, and blame the guard for it

## Summary

Three `librarian` tests assert that writing a temp-rooted workspace into a **real
(outside-temp) catalog** is refused. Each builds its "outside-temp" catalog under
`std::env::current_dir()`. When the checkout is itself under the OS temp dir that
catalog is *inside* temp, the guard correctly declines to fire, and `expect_err`
panics with a message naming the guard.

The assumption is stated in a code comment and enforced by nothing. The failure
message describes a guard defect; the actual condition is "this test cannot
establish its premise from here."

This matters because **the scratchpad this project instructs agents to use for
temporary work is under `/tmp`**. Verifying a branch or a merge in a scratch
worktree — the obvious, sanctioned way to do it without disturbing a shared tree —
lands squarely in the failing configuration.

## Symptom (Effect)

```
librarian::tools::create::tests::create_refuses_temp_workspace_into_real_catalog ... FAILED
librarian::tools::reindex::tests::reindex_refuses_temp_root_into_real_catalog ... FAILED
librarian::tools::temp_write_guard::tests::wrapper_refuses_temp_workspace_into_real_outside_temp_catalog ... FAILED

panicked at src/librarian/tools/temp_write_guard.rs:145:14:
temp workspace + real (outside-temp) catalog must be refused: ()

panicked at src/librarian/tools/create.rs:1133:10:
temp workspace + real (outside-temp) catalog must be refused: Object {"id": String("..."), "abs_path": String("/tmp/.tmpRM3dYP/docs/specs/x.md")}

panicked at src/librarian/tools/reindex.rs:1198:47:
reindexing a temp root into a real (outside-temp) catalog must be refused: Object {"added": Number(0), ...}
```

Every message asserts the guard **must** refuse. None says the catalog it built
was not outside-temp — which is the actual reason.

## Reproduction

```
git worktree add --detach /tmp/<anywhere> experiments
cd /tmp/<anywhere>
cargo test --lib -- \
  librarian::tools::create::tests::create_refuses_temp_workspace_into_real_catalog \
  librarian::tools::reindex::tests::reindex_refuses_temp_root_into_real_catalog \
  librarian::tools::temp_write_guard::tests::wrapper_refuses_temp_workspace_into_real_outside_temp_catalog
```

All three fail. The identical command in a checkout outside `/tmp` passes in
0.11s.

## Environment

Linux, any. Depends only on whether `std::env::current_dir()` is under
`std::env::temp_dir()`. Not feature-, toolchain-, or timing-dependent.

## Root cause

`should_refuse` (`src/librarian/tools/temp_write_guard.rs:14-21`) is correct and
is not the defect:

```rust
let under_temp = |p: &Path| p.starts_with(temp_dir);
let catalog_is_real = catalog_db.is_some_and(|c| !under_temp(c));
under_temp(root) && catalog_is_real
```

The tests are what break. `wrapper_refuses_temp_workspace_into_real_outside_temp_catalog`
(`:136-155`) builds its catalog like this:

```rust
// (Assumes the repo checkout is not itself under the OS temp dir, which holds here.)
let outside = tempfile::TempDir::new_in(std::env::current_dir().unwrap()).unwrap();
```

From a `/tmp` checkout, `outside` is under `/tmp`, so `catalog_is_real` is
**false**, `should_refuse` returns false, the write is allowed — correctly — and
`expect_err` panics. The other two build their catalogs the same way.

**The assumption is written down and unenforced.** That parenthetical is an
accurate description of a precondition with no assertion behind it, so when it
stops holding the suite reports a guard failure instead of an unmet premise.

**measured 2026-08-30**, and the control is the part that matters:

| where | merge applied | result |
|---|---|---|
| `/tmp` worktree | operator-rules Phase 2 merged in | 3 FAILED |
| `/tmp` worktree | **plain `experiments`, no merge** | **3 FAILED** |
| main checkout (`~/work/...`) | — | 3 passed, 0.11s |

The middle row is the control. Without it the three failures read as "the merge
broke three librarian tests", which is what they were about to be reported as.

## Evidence

### The near-miss that surfaced it

Found while verifying whether `sdd/operator-rules-phase-2` could be merged. The
merge probe ran in a scratch worktree under the session scratchpad — i.e. under
`/tmp` — and `cargo test --workspace` returned `4706 passed; 3 failed`. The three
failures are in `librarian`, a subsystem that branch does not touch, which is what
prompted the control rather than a bug report against the merge.

Had the control not been run, the likely outcomes were a false "this branch breaks
three tests" (blocking a clean merge) or an afternoon bisecting a guard that was
working correctly the whole time.

### Why `#[ignore]` is not the answer

These three are the only coverage that `guard_temp_workspace_write` is *wired
into* `create` and `reindex` rather than merely correct in isolation —
`should_refuse` already has pure unit tests. Silencing them would delete the wiring
proof, which is the same trade `ET-9` T7's row refuses for the loopback-guard test.

## Hypotheses tried

1. **Hypothesis:** the operator-rules Phase 2 merge broke three librarian tests.
   **Test:** re-ran the same three in the same `/tmp` worktree at plain
   `experiments`, merge not applied.
   **Verdict:** rejected — identical failures without the merge.

2. **Hypothesis:** the guard itself regressed.
   **Test:** ran the same three in the main checkout, outside `/tmp`.
   **Verdict:** rejected — 3 passed in 0.11s. The guard is fine; its tests are
   location-sensitive.

## Fix

Not applied — filed on notice while merging a different branch.

Make the precondition **loud instead of assumed**. Two shapes, and they are not
equivalent:

- **Assert it.** At the top of each of the three, check
  `!std::env::current_dir()?.starts_with(std::env::temp_dir())` and panic with a
  message naming the real cause ("this test needs a checkout outside the OS temp
  dir; found …"). Cheapest, and turns a misleading failure into an accurate one —
  but the test still fails, so a `/tmp` run is still red.
- **Remove the dependency on `current_dir()`.** The tests need a catalog path the
  guard classifies as outside-temp; they do not need it to be genuinely outside
  temp on disk. `should_refuse` is already pure and takes `temp_dir` as a
  parameter — the wiring tests could inject a synthetic `temp_dir` the same way,
  making all three location-independent. Larger change, and the only one that
  makes a `/tmp` run green.

Prefer the second. Whichever is taken, **verify it from a `/tmp` worktree**, which
is the configuration the current tests cannot survive and the one an agent
following this project's own scratchpad instruction will be in.

## Tests added

None — the fix is the test change.

## Workarounds

Run the suite from a checkout outside the OS temp dir. If a `/tmp` worktree is
required, treat these three failures as expected and confirm with the control (run
them at the unmodified base in the same worktree) before attributing them to any
change under test.

## Resume

Implement fix option 2 in `src/librarian/tools/temp_write_guard.rs` and its two
siblings (`create.rs:1133`, `reindex.rs:1198`): give the wiring tests an injectable
`temp_dir` so they stop deriving "outside-temp" from `current_dir()`. Then verify
from a worktree under `/tmp` — a green run there is the whole point, and a run in
the main checkout proves nothing, since it is green today.

## References

- `src/librarian/tools/temp_write_guard.rs:14-21` — `should_refuse`, correct.
- `src/librarian/tools/temp_write_guard.rs:136-155` — the test and its unenforced
  assumption.
- `src/librarian/tools/create.rs:1133`, `src/librarian/tools/reindex.rs:1198` —
  the two siblings with the same shape.
