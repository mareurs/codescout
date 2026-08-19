---
kind: bug
status: fixed
tags:
- tests
- hermeticity
- guide-ledger
- xdg
closed: 2026-08-18
opened: 2026-08-18
owner: marius
related: []
severity: medium
unverified: no regression test — the fix IS a change to a test, so deleting the two .env calls in tests/cross_process_write_lock.rs leaves the suite green; archive trigger deliberately not met (this file says so itself). Fix SHA 45918ca8 is already the master SHA — promotion is fast-forward, do not wait for a second one.
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

Fixed on `experiments` in **`45918ca8`** — "fix(tests): redirect spawned-binary guide ledger off the real state dir". Two files, no production behaviour change.

`tests/cross_process_write_lock.rs` — the spawn now carries both overrides:

```rust
let state_dir = tempfile::tempdir().unwrap();
let mut child = Command::new(&bin)
    .args(["start", "--project", project.to_str().unwrap()])
    .env("XDG_STATE_HOME", state_dir.path())
    .env_remove("CLAUDE_CODE_SESSION_ID")
```

`state_dir` is bound to a **named local declared before `child`**, so it drops after it (Rust drops in reverse declaration order) and the directory outlives the process reading it. A `TempDir` bound to a temporary would be deleted at the end of that statement, leaving the child writing into a deleted path.

`Command::env` / `env_remove` set the *child's* environment. Neither is `std::env::set_var`, so neither violates the no-env-mutation rule.

`src/server.rs:278-289` — the comment above the fallback now records that spawned-binary tests are covered by overriding `XDG_STATE_HOME` on the child rather than by `ServerEnv` injection, so the universal it states stays true and a future reader does not re-derive the same hole.

**Sibling spawn sites swept and cleared.** `grep "Command::new" tests/**/*.rs` → 9 hits in 6 files. Only this one spawns a codescout server; `tests/librarian/mcp_integration.rs:91,105` spawn `librarian-mcp`, a binary that **no longer exists** (dissolved into the codescout crate — single `[[bin]]` target, no `src/bin/librarian-mcp.rs`, and that test carries `#[ignore = "requires standalone librarian binary which no longer exists post-dissolution"]`). The rest are `git`/`cargo`/`which`.

No master-side SHA line: `git rev-list --left-right --count master...experiments` = `0 1022`, so `master` is a strict ancestor and promotion is a **fast-forward**. The `experiments` SHA above already is the master SHA; there is no second one to record.

### Verification

Gate green at the fix commit: 4137 passed, 0 failed, 45 ignored.

The before/after `ls -la ~/.local/state/codescout/guide_hints/` captures are byte-identical across the target test — necessary but **not sufficient**, because a read + GC *scan* of the real directory leaves no filesystem trace either, and the pre-fix code passed this test too.

So the fix carries **positive** evidence as well. The target test's own child only calls `edit_file` against a locked project, which is unlikely to trigger a guide hint, so a scratch probe drove the real binary through `initialize` → `notifications/initialized` → `tools/call workspace(activate)` with the same two overrides applied:

```
$ find <scratch-state-dir> -maxdepth 5
<scratch-state-dir>/codescout/guide_hints/b496898b-af03-4f16-b956-568ea57c5c99.json
```

The ledger landed entirely inside the injected `XDG_STATE_HOME`, and under a **fresh uuid** rather than the developer's session id — which independently demonstrates that `env_remove("CLAUDE_CODE_SESSION_ID")` took effect. The real per-user directory gained no seventh file.
## Tests added

**None — and this is a real residual gap, not a formality.** Stated plainly because the archive trigger requires a regression test and this fix does not have one.

The fix *is* a change to a test, so there is nothing asserting the fix stays applied. Delete the two `.env` calls at `tests/cross_process_write_lock.rs` and the suite still passes: the target test's child never triggers a guide hint, so nothing observable changes, and the real-directory listing is unchanged either way because a read + 35-day GC scan leaves no trace.

What a genuine regression test would need: spawn the binary with `XDG_STATE_HOME` pointed at scratch, complete an MCP handshake, make a call that *does* trigger a guide hint, and assert the ledger appears under the scratch path. The machinery exists — `mcp_handshake` is already in this test file, and the scratch probe under Verification is that test in script form. It was not committed as a test because doing so was outside the scope of the fix.

**This file is therefore deliberately NOT archived.** `CLAUDE.md`'s archive trigger is "gate green plus a regression test"; the gate is green and the test is absent, so archiving it would misreport the state.
## Workarounds

Set `XDG_STATE_HOME` to a scratch path in the shell before running the suite, which redirects the
spawned child along with everything else.

## Resume

The defect is fixed and verified; what remains is the guard.

Promote the scratch probe under *Verification* into a committed regression test in `tests/cross_process_write_lock.rs`, reusing that file's existing `mcp_handshake` helper: spawn with `XDG_STATE_HOME` at a scratch dir, drive one tool call that triggers a guide hint, and assert a ledger appears under scratch and the real per-user directory is untouched. Then archive this file via `artifact(action="move", id=…, new_rel_path="docs/issues/archive/2026-08-18-spawned-binary-test-points-guide-gc-at-real-state-dir.md")` — never a bare `git mv` — and re-point any citation of this path or of id `ef800712655f97a4` in the same commit.

Do **not** go looking for a master-side SHA: promotion is a fast-forward, so `45918ca8` is already it.
## References

- `tests/cross_process_write_lock.rs:112-132` — the spawn
- `src/server.rs:81` — `from_env` hardcodes `guide_hints_dir: None`
- `src/server.rs:285-286` — the fallback to the real per-user directory
- `src/tools/guide_ledger.rs:82-84` — `load` runs `gc` unconditionally
- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` § 2 — the storage move
- `.superpowers/sdd/2026-08-18-guide-ledger-phase-a-storage/progress.md` — Ruling 21 (the
  injection seam) and the enumeration history
