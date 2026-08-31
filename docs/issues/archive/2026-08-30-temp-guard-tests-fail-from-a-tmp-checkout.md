---
kind: bug
status: fixed
tags:
- tests
- librarian
- temp-guard
- environment-sensitivity
- false-attribution
closed: 2026-08-31
opened: 2026-08-30
owner: marius
related: []
severity: medium
unverified: 'No CI lane runs from a cwd under the OS temp dir, so nothing would catch a REGRESSION of this fix by re-deriving the premise from current_dir(). The protection is structural rather than tested: TempGuardEnv has no default, so every construction site must state its temp root and the compiler enumerates any that do not. A deliberate revert would still pass CI.'
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

**Fixed 2026-08-31 on `experiments` at `3ec8e500`**, patch-id
`a09e8ef8809f8ccf2a7d3b0d52f50dce2cf58ad4`. Option 2, as this section preferred.

`TempGuardEnv { temp_dir, opted_in }` resolves the process environment **once, at the edge**
(`from_env`), and `guard_temp_workspace_write` takes it as an argument. `ToolContext` carries it,
so the `create`/`reindex` wiring tests inject a synthetic temp root via
`TestToolContextBuilder::with_temp_guard`. Both "inside" and "outside" then live physically under
the OS temp dir, and which is which is a property of the **fixture** rather than of the machine.

**Not** the `TMPDIR`-plus-`#[serial]` shape, which was the first design I reached for.
`docs/conventions/test-env-isolation.md` records that as option B — *NOT VIABLE*, "do not
reintroduce this pattern" — because `set_var` is process-global and `#[serial]` only locks
against tests that opt in; the class was driven from 119 occurrences to 0. This is option A.

The new field deliberately carries **no default**, so the compiler enumerated all nine remaining
construction sites rather than letting any silently inherit the machine's temp dir.

### Correction — the trigger is the CWD, not the checkout

This file's title and Reproduction say "any checkout under `/tmp`". Measured at fix time with the
**same binary and same source**, cwd as the only variable:

| cwd | result |
|---|---|
| `/tmp/...` | **3 FAILED** |
| `~/work/claude/codescout` | 3 passed |

The tests never consult where the source lives, so the checkout location is irrelevant; a `/tmp`
worktree failed because its cwd was also under `/tmp`. The original framing is not wrong — it
describes the observed incident — it is narrower than the defect.

That distinction changed the fix. A `CARGO_MANIFEST_DIR`-based repair (compile-time crate root,
immune to cwd) would have cleared the case reproduced here and left the *reported* one — a `/tmp`
worktree, where checkout and cwd are both under temp — still broken. Fixing the probe's artifact
instead of the incident was a live risk, avoided only because the control was run.

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

The three that were failing now pass from a `/tmp` cwd — `10/10` where it was `3` red — and one
new test guards the guard:

- `the_same_fixture_is_allowed_once_the_caller_opts_in`
  (`src/librarian/tools/temp_write_guard.rs`) — **non-vacuity**. The *same* fixture must be
  ALLOWED with `opted_in: true` and still REFUSED without it. Without that pairing, a fixture
  that quietly stopped satisfying the refusal precondition — precisely this bug — turns every
  refusal test green again, and green is also what they report when they are working.
- `synthetic_temp()` is the shared fixture, with the load-bearing detail annotated on it: both
  directories are physically under the OS temp dir *on purpose*, and both paths are canonicalized
  because the guard canonicalizes what it compares — on a host whose temp dir is a symlink an
  uncanonicalized prefix would not match and the fixture would silently stop discriminating.

The non-vacuity check is a **fixture flip rather than a source mutation**, deliberately: the tree
is shared with concurrent sessions, and a transient edit to `should_refuse` would surface in their
runs as an unexplained red — the failure mode this repo has been recording all day.
## Workarounds

Run the suite from a checkout outside the OS temp dir. If a `/tmp` worktree is
required, treat these three failures as expected and confirm with the control (run
them at the unmodified base in the same worktree) before attributing them to any
change under test.

## Resume

N/A — fixed and verified from the configuration that mattered. Gate green in the documented order:
`fmt` 0 diffs, `clippy --workspace --all-targets --features local-embed` 0 warnings, lean
`--no-default-features` 3404 passed / 0 failed (third), default `--workspace` 4964 passed / 0
failed (last).

**One honest gap, recorded in `unverified:` so a query can read it.** No CI lane runs from a cwd
under the OS temp dir, so nothing would catch a *regression* — someone re-deriving the premise
from `current_dir()` would pass CI exactly as before. The protection is **structural, not tested**:
`TempGuardEnv` has no `Default`, so every construction site must state its temp root and the
compiler names any that do not. That is a stronger guard than a test for accidental drift and no
guard at all against a deliberate revert.
## References

- `src/librarian/tools/temp_write_guard.rs:14-21` — `should_refuse`, correct.
- `src/librarian/tools/temp_write_guard.rs:136-155` — the test and its unenforced
  assumption.
- `src/librarian/tools/create.rs:1133`, `src/librarian/tools/reindex.rs:1198` —
  the two siblings with the same shape.
