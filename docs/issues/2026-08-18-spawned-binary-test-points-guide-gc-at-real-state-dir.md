---
status: open
opened: 2026-08-18
closed:
severity: medium
owner: marius
related: []
tags: [tests, hermeticity, guide-ledger, xdg]
kind: bug
---

# BUG: a spawned-binary test points the guide-ledger GC at the developer's real state directory

## Summary

`tests/cross_process_write_lock.rs` spawns the real `codescout` binary, which since the
guide-ledger Phase A storage move resolves its hint ledger to the **user's real**
`$XDG_STATE_HOME/codescout/guide_hints/` instead of the temporary project it is given. Running
`cargo test` therefore makes a child process read that directory, run the 35-day file-deleting
GC over it, and — if any tool call in that test triggers a guide hint — write into the
developer's *live* session ledger.

Every in-process test injects `ServerEnv.guide_hints_dir` and is hermetic. This one is not,
because it reaches the production path by **process spawn**, which no in-process injection can
intercept.

## Symptom (Effect)

No user-visible failure. The observable effects are side effects on the developer's machine
during `cargo test`:

1. a `read_dir` + age scan of `~/.local/state/codescout/guide_hints/`;
2. deletion of any ledger there whose newest stamp is older than 35 days;
3. potentially a write to the ledger of the developer's **currently live** Claude Code session,
   because the spawned child inherits `CLAUDE_CODE_SESSION_ID`.

Effect 3 is the one with a real cost: a guide the live session should have received would be
recorded as already-delivered and silently suppressed.

## Reproduction

```bash
cargo build            # tests/cross_process_write_lock.rs self-skips unless target/debug/codescout exists
cargo test --test cross_process_write_lock
```

Observed at commit `ed4e5e6d` (branch `experiments`). The test's own `bin.exists()` self-skip does
**not** fire on a normal developer machine, so this runs in practice.

## Environment

Any platform. Reached whenever `target/debug/codescout` exists and `CARGO_TARGET_DIR` is unset.

## Root cause

`tests/cross_process_write_lock.rs:120-121` spawns the binary:

```rust
let mut child = Command::new(&bin)
    .args(["start", "--project", project.to_str().unwrap()])
```

with no environment overrides. The child runs `run` → `from_parts` →
`from_parts_with_env(.., ServerEnv::from_env())`, and `from_env` hardcodes
`guide_hints_dir: None` (`src/server.rs:81`). The binding then takes its fallback arm
(`src/server.rs:285-286`), resolving `crate::util::fs::per_user_state_dir()` straight off the
**inherited** `XDG_STATE_HOME`/`HOME` (`src/util/fs.rs:97-116`). `GuideLedger::load` runs
`gc(d, p)` unconditionally before reading (`src/tools/guide_ledger.rs:82-84`).

*Measured 2026-08-18: `read_file tests/cross_process_write_lock.rs:112-132` confirms the spawn
carries no `.env(..)`; `read_file src/server.rs:248-262` and `:285-286` confirm the fallback
arm.* The runtime side effect itself was **not** observed — the before/after
`ls -la ~/.local/state/codescout/guide_hints/` captures taken across two full `cargo test` runs
were byte-identical, which is consistent with a scan that deleted nothing (no file there is 35
days idle) and with that test triggering no guide hint. So effects 1 and 2 are inferred from the
code path, and effect 3 is a hazard rather than a sighting.

## Evidence

### This is a regression from the Phase A storage move, not pre-existing

The whole-branch fix-wave re-review characterised it as "pre-existing, untouched by the diff".
That is wrong, and the distinction matters for whose bug it is.

Before Task 6 the binding was:

```rust
let guide_hints_dir = guide_project_root
    .as_ref()
    .map(|r| r.join(".codescout").join("guide_hints"));
```

For a child spawned as `codescout start --project <tmpdir>`, `agent.project_root()` resolves to
that tmpdir, so the ledger landed in `<tmpdir>/.codescout/guide_hints/` and the test **was**
hermetic. Task 6 (`17119957`) replaced the project-derived path with a session-keyed per-user
path, which is correct for production and removed this test's incidental isolation. The *test
file* is untouched; its behaviour changed underneath it.

### Why five successive enumerations missed it

The injection-site count was revised six times during Phase A execution: 1 → 8 → 11 → 15 → 16,
each correction from widening the method (one traced test → all callers of one builder →
constructor-tracing → whole-branch duplication view → actually editing every site). This site is
invisible to **all** of those, because it is not an in-process construction at all — no
`ServerEnv` literal, no constructor call, just a `Command::spawn`. It took a seventh method,
looking for spawned binaries, to find it.

### The comment this falsifies

`src/server.rs:281-284` now asserts: *"every test constructs its server with an injected
`guide_hints_dir`, so no test reads, writes, or garbage-collects the real per-user state
directory."* True of every in-process test, false repo-wide because of this one.

### The class of harm was observed live — but from the pre-fix code, not from this bug's path

Measured 2026-08-18 13:35, during the post-`cargo rb` + `/mcp` wire verification of Phase A.

`~/.local/state/codescout/` and its `guide_hints/` subdirectory were **created at 12:11:39
local**, and the first file in them was `55515bc5-…json` — the ledger of the *live Claude Code
session* orchestrating this plan — stamped `2026-08-18T09:11:39Z`, the same instant in UTC.

Task 6, which introduced the per-user binding, committed at **12:16:07** — five minutes *later*.
So the write came from Task 6's **uncommitted working tree** during its TDD cycle, while the
binding was still the plan's defective Step 3 (a bare `per_user_state_dir()` with no injection
seam). Ruling 21's `ServerEnv.guide_hints_dir` seam did not exist until the review at ~12:39.

**What this establishes:** the harm Ruling 21 was written to prevent is not hypothetical. A test
run really did reach out of its tempdir, create the developer's real state directory, and write
the live session's ledger under the live session's own id.

**What it does NOT establish — an earlier guess of mine that the timeline refuted:** it is **not**
attributable to `tests/cross_process_write_lock.rs`. I first supposed the spawned child wrote it,
but `target/debug/codescout` was rebuilt at 13:21:01 and Task 6 landed at 12:16, so no post-Task-6
binary existed at 12:11 for that test to spawn. The spawn path documented above remains real —
traced in the code, `.env`-free at `tests/cross_process_write_lock.rs:120-121` — but **no observed
write is attributed to it.** Effects 1-3 stay inferred.

**Post-fix behaviour, verified on the wire the same minute:** with the shipped code the live server
wrote only `~/.local/state/codescout/guide_hints/55515bc5-….json` at 13:34:31, in the timestamped
map shape, preserving the pre-existing entry's original stamp; the project-local
`.codescout/guide_hints/55515bc5-….json` stayed frozen at its pre-rebuild 13:20:24 legacy-array
content.
## Hypotheses tried

1. **Hypothesis:** pre-existing, unrelated to the Phase A storage move.
   **Test:** read the pre-Task-6 binding at `17119957^` and trace what
   `agent.project_root()` yields for a child spawned with `--project <tmpdir>`.
   **Verdict:** rejected — pre-change the ledger resolved inside the temporary project, so the
   test was hermetic. This is a Phase A regression.

## Fix

One line, in the test, no production change:

```rust
let mut child = Command::new(&bin)
    .args(["start", "--project", project.to_str().unwrap()])
    .env("XDG_STATE_HOME", &<a tempdir the test already holds>)
```

`Command::env` sets the **child's** environment; it does not call `std::env::set_var` and so does
not violate the project's no-env-mutation-in-tests rule (concurrent `set_var` is UB —
`docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`).
Clearing `CLAUDE_CODE_SESSION_ID` on the child as well would close effect 3 independently of the
directory.

Then widen the sentence at `src/server.rs:281-284` from "every test" to "every in-process test",
or drop the qualifier once this is fixed and the universal is true again.

Also worth a sweep while here: any other test that spawns the real binary inherits the same
exposure. `grep -n "Command::new(&bin)" tests/` is the search.

## Tests added

None — bug is open. A fix should assert the child's ledger landed under the injected
`XDG_STATE_HOME` rather than merely that the real directory was untouched, since the latter
passes vacuously whenever nothing there is 35 days idle.

## Workarounds

Set `XDG_STATE_HOME` to a scratch path in the shell before running the suite, which redirects the
spawned child along with everything else.

## Resume

Apply the `.env("XDG_STATE_HOME", ..)` fix at `tests/cross_process_write_lock.rs:120`, run
`cargo test --test cross_process_write_lock`, and confirm a ledger appears under the injected
tempdir and not under `~/.local/state/codescout/guide_hints/`. Then run
`grep -n "Command::new" tests/` to check whether any sibling test spawns the binary with the same
exposure, and qualify or restore the universal at `src/server.rs:281-284`.

## References

- `tests/cross_process_write_lock.rs:112-132` — the spawn
- `src/server.rs:81` — `from_env` hardcodes `guide_hints_dir: None`
- `src/server.rs:285-286` — the fallback to the real per-user directory
- `src/tools/guide_ledger.rs:82-84` — `load` runs `gc` unconditionally
- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` § 2 — the storage move
- `.superpowers/sdd/2026-08-18-guide-ledger-phase-a-storage/progress.md` — Ruling 21 (the
  injection seam) and the enumeration history
