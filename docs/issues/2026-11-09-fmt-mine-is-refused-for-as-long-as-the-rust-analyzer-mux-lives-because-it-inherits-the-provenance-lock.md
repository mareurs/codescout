---
id: '312cde2f4698874b'
kind: bug
status: open
title: 'BUG: fmt-mine is refused for as long as the rust-analyzer mux lives, because the mux inherits the provenance lock'
owners:
- marius
tags:
- cluster/transient-shared-state-lies-to-readers
opened: 2026-11-09
owner: marius
related:
- docs/trackers/issue-clusters/IC-12-transient-shared-state-lies-to-readers.md
severity: medium
unverified: 'Which run_command started the mux is not established, nor whether closing the inherited descriptors in the mux would weaken a guarantee the fence gives. The flock exit codes were taken inside a run_command, which itself holds a shared lock, so they do not isolate the mux, and the record no longer relies on them. What is established without them: the kernel records a shared lock owned by a pid that no longer exists, and the mux and its children hold that same description. The refusal count of five is from this session only. Owner 1731442 was first misidentified as a peer and is corrected in the body.'
---

## Summary

`./scripts/fmt-mine.sh` through `run_command` is refused with `native provenance coordination refused dispatch: lock acquisition failed because the operation would block`, and stays refused while no tool call is running. The formatter needs the provenance lock exclusively. A shared copy of that lock is held, through inherited file descriptors, by the long-lived `codescout mux` and its `rust-analyzer` children. They outlive the call that took the lock, and the process that took it no longer exists. The refusal names no holder, so it reads the same as "a peer is mid-call".

## Class fit

This is the class "one session's tooling holds shared state for an operation, and every other session's read is wrong for that window, reported as truth": here the shared state is a lock, and the refusal looks like a transient collision. It differs from the class's other members in that the window does not end with the operation. A daemon spawned inside a fenced shell keeps the lock for as long as it lives, which was about three hours at the time of measurement.

## Symptom (Effect)

Observed 2026-11-09, session 677278c4: `./scripts/fmt-mine.sh` refused five times through `run_command`, including a baseline call with no job of mine running. `./scripts/gate.sh` ran normally in the same period, because inside the gate `fmt-mine` is run by a script, not through a fenced `run_command`.

## Reproduction

Two parts, and the second is the stronger one.

1. Look at the lock without trying it:

```
F=/home/marius/.local/state/codescout/native-provenance-v2/.codescout/native-provenance-v1.lock
grep <inode of $F> /proc/locks                 # the owners, as the kernel records them
ps -p <owner pid>                              # an owner that no longer exists
grep -E '^(lock|flags)' /proc/<mux pid>/fdinfo/<fd>   # the mux holds that same lock
```

   Measured 2026-11-09: owner 3325677 does not exist, and `fdinfo` of the mux, its `rust-analyzer` and its `proc-macro-srv` children each names that lock.

2. A unit test rebuilds the mechanism without any peer: `a_lock_kept_by_a_child_of_a_dead_owner_is_named_as_such` in `src/tools/native_provenance_fence/tests.rs` runs `flock -s <file> sh -c 'sleep 30 & echo $!'`, so the owner exits and a background child keeps the lock, then calls `Fence::try_acquire`.

**Do not use `flock -n -x` run from inside `run_command` as a reproduction.** I did, and got exit 1, but the `run_command` that ran it holds a shared lock itself while it runs, so the exclusive try fails whatever else is true. That result is confounded and proves nothing about the mux. Run it from a terminal outside codescout, and read `/proc/locks` first to see which locks exist.

## Environment

Linux, codescout at `53a7d8ec6`, rust-analyzer 1.99.1 (toolchain path in the process list).

## Root cause

Established by reading code and by measurement:

- `is_foreground_formatter` (`src/tools/provenance_fence_context.rs:20`) matches exactly `./scripts/fmt-mine.sh` or `bash scripts/fmt-mine.sh`, in the foreground. For that call `scope_root` takes `Fence::try_acquire`, which is `file.try_lock()`: exclusive and non-blocking. It takes it twice: on the per-user coordination file (`~/.local/state/codescout/native-provenance-v2/.codescout/native-provenance-v1.lock`) and on the checkout's own fence. Every other write and `run_command` takes `try_participate`, which is `try_lock_shared`. A `flock` exclusive try fails while any shared holder exists.
- `apply_to_shell` (called only from the `run_command` spawn sites) duplicates both lock descriptors into the shell and clears close-on-exec in the child (`apply_to_command_named`, `src/tools/native_provenance_fence.rs`). Every descendant of the shell shares the same open file description, and a `flock` lasts while any descriptor on that description lives.
- Measured: `/proc/locks` shows three shared locks on the inode, owned by pids 3325677, 1520813 and 1731442. `ps` shows 3325677 does not exist. It was this session's own codescout server earlier: its pid is in the names of this session's earlier background-job records. The lock it took is still held by `codescout mux … -- rust-analyzer` (pid 3690455, up about 3 hours), its `rust-analyzer` (3690525) and two `rust-analyzer-proc-macro-srv` (3690992, 3690996): `/proc/<pid>/fdinfo/24` of each names that lock.
- The mux's environment carries `CODESCOUT_PROVENANCE_COORDINATOR_FD=24`, `CODESCOUT_PROVENANCE_FENCE_FD=25`, both roots and generations (5743 and 1353). So it was started through `apply_to_shell`, from a `run_command` shell.
- The other two owners: 1520813 is a live `codescout start` of another session, with a `python3 …/live_smoke.py` child 34 s old at the time. 1731442 is this session's own server (its pid prefixes the names of this session's background-job records): the `run_command` that made these measurements held a shared lock itself while it ran. I first called 1731442 a live peer; that was wrong.
- The mux is shared by design (a socket under `/run/user/1000`, `--idle-timeout 180`), so it lives while any session uses rust-analyzer in this checkout.

Not established: which `run_command` started the mux (the generations 5743 and 1353 would identify it if the generation log can be queried; I did not look). Whether closing the inherited descriptors in the mux would weaken any guarantee the fence gives; the tests named `formatter_excludes_a_foreign_workspace_shell_targeting_its_checkout` and `the_actual_shell_inherits_the_request_scoped_fence` show the inheritance is intended for shells.

## Evidence

- `lsof` on the lock file: six processes (the two codescout servers, the python3 smoke test, the mux and its two children).
- `/proc/locks` and `/proc/<pid>/fdinfo/<fd>` as above.
- `ps -o pid,ppid,etimes,cmd` for the pids named.
- `flock -n -x` (exit 1) and `flock -n -s` (exit 0), run from inside `run_command`: CONFOUNDED, see Reproduction. They show only that some shared lock existed, and the measuring call supplied one itself.


Re-measured a few minutes later in the same session: `/proc/locks` listed FIVE shared locks on the inode (owners 662110, 1520813, 3325677, 3647863, 1731442; the last is this session's own server), and `flock -n -x` still exited 1, which is confounded as above. Two owners were new, so other sessions take this lock continuously. The mux's lock (owner 3325677, a dead pid) was among the five. So the exclusive try could not be isolated to the mux by waiting. What the evidence does establish is the narrower claim: while the mux lives, the exclusive try cannot succeed, whatever the other holders do. A caller who gets "would block" cannot tell a peer that is mid-call from a lock that no call owns.

## Hypotheses tried

- "My own background job holds it": ruled out for the baseline call (no job of mine was running); it can still explain one refusal that overlapped my clippy job.
- "A peer's call is mid-flight": possible for the two live peers, but not for the mux's lock, whose owner is dead.

## Fix

Partly done. **Option 2 is in; option 1 is not, and this bug stays open.**

- Option 2, `4eceee5e7` and `0d47f5898`: a refused fence lock now names its holders (Linux): the processes that have the lock file open, inherited descriptors included, and the owners the kernel records, each marked "not running" when gone. A list cut by its 12-entry cap or by the 400 ms scan limit ends in "and more". Only the process name and a plain lowercase subcommand are shown, never other arguments.
- Verified: unit tests on the pure parts; a test that rebuilds the dead-owner case with `flock -s <file> sh -c 'sleep 30 & echo $!'`; a test that starts 14 holders; mutation runs for the holder text and for the cut flag; and live on the rebuilt server. A refused `./scripts/fmt-mine.sh` named the running gate and its children, a dead owner (this session's previous server, whose gate was still running on inherited descriptors, the mechanism of this bug reproduced within minutes), and the dead owner of the mux's lock.
- Option 1, drafted and not merged, **not recommended** (see "Measured before the option 1 decision"): branch `fix/mux-closes-provenance-leases-20261109`, commit `8f18f6689`, worktree `/home/marius/work/claude/codescout.worktrees/mux-closes-provenance-leases`. The mux closes the inherited leases at startup, only when the descriptor is the lock file the variables name (by device and inode), and strips the seven variables from its language server's environment. The fence is another effort's design, so merging stays a decision for its owner.
- What the draft gives up: a mux that closed its leases is outside the fence's promise that no process descended from an earlier fenced call is alive when the formatter holds the lock exclusively. The shell's own copy is untouched, so the lock still lasts as long as the shell.
- Measured 2026-11-09 before the draft: of four real muxes, one (rust, pid 3690455, 3h50m old) held both descriptors, with `FENCE_FD=25` and `COORDINATOR_FD=24` in its environment; the other three held none. So the lock reaches a mux only when a process inside a fenced shell starts it. Which command started that mux is still not known.

What option 2 does and does not change: a caller who is refused can now tell a peer that is mid-call from a lock that no call owns. The refusal itself is unchanged, and the mux can still keep the lock for hours.

- Option 3, proposed after the measurements and the owner's answers, drafted and not merged: branch `fix/formatter-exclusive-only-with-proof-20261109`, commit `8a3738b28`, worktree `/home/marius/work/claude/codescout.worktrees/formatter-exclusive-with-proof`. `scope_root` takes the exclusive leases only for a trusted formatter (a foreground `fmt-mine` call from `codex-mcp-client` with `fence_capability`); every other formatter call shares the leases. The marker, the certificate and the trusted-closure check are unchanged. Tests: the two exclusion tests now use a native formatter, and `a_formatter_without_a_native_proof_shares_the_leases` is new; mutation `if formatter` fails only the new test, `if false` fails only the two exclusion tests; `cargo test --lib provenance` 37 passed; `./scripts/gate.sh --check` green. Given up: a formatter without a proof may run beside a concurrent writer, a protection the owner found undocumented for those clients but nobody has shown unused. Not covered: the real MCP dispatch path and a Codex native formatter end to end. The fence is the owner's design, so merging is the owner's and the operator's decision.
  - Owner's review of `8a3738b28` (2026-11-09T22:16:06Z, static, not an approval): no new ordering defect in `scope_root`. Option 3 fits a capability-scoped reading of the spec, but the Builder ruling's literal sentence ("a foreground formatter owns it exclusively") is broader, and the base implementation and tests followed it. So the change deliberately narrows protected behavior (the two base tests used formatters without a proof as exclusive holders). The scope decision must be stated, not presented as following from the old wording. An absent rationale is not proof that nothing relies on the old exclusion. Formatters without a proof still cannot run beside a native formatter, because the native one holds the exclusive coordinator.
  - Follow-up `a119095e6`: the code comment states the scope decision and the deliberate removal; the test comment no longer says "nothing else"; new `only_the_codex_client_with_the_fence_capability_takes_the_leases_exclusively` covers five near misses (mutations that drop the client check or the capability check each fail it); a race this branch had introduced (an exclusive generation read just before a test's own lease) is removed by construction, and 10 of 10 runs passed afterwards. `cargo test --lib provenance` 38 passed; clippy `-D warnings` clean; the full gate was not re-run for this tests-and-comments commit.

## Measured before the option 1 decision (2026-11-09)

**What the proof certifies (read, not run).** `scripts/native_provenance_fence.py::verify_fence` checks only the two leases: generation, root, device and inode, an exclusive whole-file `FLOCK`, the counter, and the absence of `unverified-shell-v1`. It looks at no file. `scripts/native_provenance_history.py::apply_checkpoint` uses it to accept one native history as complete, which `file-provenance.py` then uses to attribute bytes; `fmt-mine.sh` formats only the `.rs` paths `cargo fmt --check` lists and the attribution allows. So a daemon matters when it writes working-tree bytes no recorded call explains. The mux itself never applies `workspace/applyEdit`: `handle_server_request` forwards it to the edit-lock owner, or answers success without writing.

**M1 — who holds a lease now (`/proc` scan, 20:45 local).** No mux held one; the rust mux measured earlier (pid 3690455) is gone. Holders: this session's codescout server and shell (expected during the call), and `sh -c … .venv/bin/python engine.py` with two python children in `/home/marius/work/stefanini/invest-europe/ie-pal-engine`, 346 minutes old, started from a fenced shell of Claude session `be7356ea-b0e8-4ba3-8375-9ca446490fb5`, parent now `systemd --user`. `/proc/607933/fdinfo` shows `FLOCK ADVISORY READ` on that checkout's lock and on the account-wide coordinator lock (inode 404635102), owner pid 250705, which no longer exists. A non-blocking exclusive attempt from a clean child was refused on both; for the coordinator that probe is confounded by this session's own server holding a shared lease during the call, for `ie-pal-engine` it is not. Consequence: the class is "any long-lived descendant of a fenced shell", not the mux; a mux-only release does not help with this holder, and a holder in one project blocks the coordinator for every project.

**M2 — what a real mux and rust-analyzer write (scratch workspace, `strace -f` of `~/.cargo/bin/codescout mux … -- rust-analyzer`, one run).** Script: `m2_mux_writes.py` in this session's scratchpad. Phase A (`didOpen`, `documentSymbol` answered, `references` 0 locations, `rename` no edits, `didSave`, 40 s): `git status --porcelain --ignored` shows only `!! target/`. Phase B (the session appends a path dependency to `app/Cargo.toml`, then `didSave` and `didChangeWatchedFiles`, 45 s): ` M Cargo.lock`. The writer was pid 768298, `cargo check --workspace --message-format=json --manifest-path <checkout>/Cargo.toml --keep-going --all-targets`, a descendant of the mux (strace started only the mux), which opened `<checkout>/Cargo.lock` `O_RDWR|O_CREAT` at 20:48:42 and added `dependencies = ["dep"]` to `app`. No `.rs` file and no other non-ignored path was written. Not established: whether that cargo run was rust-analyzer's build-script pass or flycheck; any other language server; a crate with a `build.rs`; `peer-serve`; more than one run.

What the two together say about option 1: the owner's concern is real and measured: a mux descendant writes a tracked file without a tool call. For `fmt-mine` itself the observed write is harmless, because `Cargo.lock` is not a `.rs` file, but the fence's claim is "the history is complete", which this write breaks.

**M2 variants (same script, `M2_MODE`, one run each, 20:56–20:59).**
- Two `cargo check` forms appear in every trace. `cargo check --quiet --workspace --message-format=json …` (rust-analyzer's build-script pass, by its flags) reads a temporary lock copy (`$TMPDIR/rust-analyzer*/Cargo.lock`). `cargo check --workspace --message-format=json --manifest-path <checkout>/Cargo.toml --keep-going --all-targets` works on the checkout's own manifest; in all three runs it is the one that opened `<checkout>/Cargo.lock` `O_RDWR|O_CREAT`.
- `nosave` (phase B sends only `workspace/didChangeWatchedFiles`, no `didSave`): ` M Cargo.lock` again, written by that second form (pid 894870, 20:57:37, ~42 s after the mux started, i.e. at phase B). So the writer does not need a save; my earlier label "flycheck on save" is refuted as stated. Which rust-analyzer feature issues that form is not established.
- `buildrs` (`dep/build.rs` writes `OUT_DIR/gen.rs` and appends to `dep/generated.rs`): `?? dep/generated.rs` after phase A. The project's `build-script-build` ran at 20:58:33.6, 2.1 s after the mux started, under the `--quiet` pass (pid 904544, started 20:58:32.96), before the first non-`--quiet` run (20:58:38.98, which matches the session's phase A save). So rust-analyzer runs the project's build scripts when it loads the workspace, with nothing asked of it; whatever they write is a mux-descendant write. The location `dep/` was this script's choice.
- This repository's `build.rs` writes only `OUT_DIR`, but it runs `git status --porcelain --untracked-files=all -- <DIRTY_SCOPE>` without `--no-optional-locks`, which git documents may refresh `.git/index`. Not measured.

**M1 follow-up (20:55).** The `ie-pal-engine` server's worker restarts and each new worker inherits both leases: workers started at 20:52:37 (pid 825888) and 20:53:08 (pid 833697), the second 2 s after the session wrote `table_metadata.py`, `index_metadata.py` and `test_table_measurement.py` (20:53:06.90), consistent with a dev-server reloader (two restarts observed). The owning session `be7356ea` is alive (`claude` pid 3448215, `ie-pal-engine-47`, 3 h old); the server is ~6 h old, so an earlier process of that session started it. The server writes `logs/app.log`, ignored by `.gitignore:93`; other writes not measurable without ptrace.

**Census running.** `census_sampler.py` (scratchpad), every 300 s for 24 h from 2026-11-09T20:52 local, into `census.jsonl` in this session's scratchpad; it closes its own inherited leases at start and does not appear as a holder. It is a child of this session's codescout server and may stop with it.

**Census result (stopped by hand at 23:32 after 33 samples, 20:52:42 → 23:32:46).**
- No mux held a lease in any sample.
- In 33 of 33 samples, at least one detached process (parent `systemd --user`) older than 10 minutes held the account-wide coordinator lock.
- Long-lived holders seen: the `ie-pal-engine` dev server (`engine.py`, up to 513 minutes, its worker restarting with the leases); a peer session's read-only instruments (`codescout-17`, `sample.sh` 55 min and `resident.sh` 53 min, both `sqlite3 -readonly` against `.codescout/usage.db`, which write nothing in the checkout); several detached scripts from the `ie-pal-engine` session's scratchpad (10–20 min); an `adb -L tcp:5037 fork-server server` (55 min), forked by a first `adb` command of a session in `~/work/personal/home/ha`; a `sleep 895`.
- Short-lived holders were gate runs, shells and codescout servers during their calls.
- Caveat: one 160-minute window on one evening; a 5-minute interval misses holders that live less than that.

**Code facts that bear on the decision (read in `src/tools/provenance_fence_context.rs` and `scripts/native_provenance_fence.py`; not run).**
- `scope_root` writes `unverified-shell-v1` for every `run_command` that is not a trusted formatter, and no code removes it. In the coordinator domain it exists since 2026-11-07 21:31:04. While it exists, `_coordination_root` raises `UNKNOWN`, so no native proof can succeed for any checkout of the account.
- `is_foreground_formatter` decides the exclusive lease from the command text only. `trusted_formatter` (the only path that uses the proof) also needs `codex-mcp-client` with `fence_capability`. So a Claude client's `fmt-mine` waits for an exclusive lease that certifies nothing for it. Whether the exclusive lease has another purpose (for example, formatting with no concurrent writer) is not stated in the code and is a question for the fence's owner.
- Only the exact strings `bash scripts/fmt-mine.sh` and `./scripts/fmt-mine.sh` (not background, not interactive) take the exclusive lease; `./scripts/fmt-mine.sh --context <file>` takes a shared one. That is not a safe workaround: the argument-bearing form formats with only shared leases, so nothing excludes concurrent writers during it (owner's reply, 21:48Z).

**Option 1: not recommended.** M2 measured a mux descendant writing a tracked file (`Cargo.lock`) and running project code (`build.rs`) with no tool call, so the precondition the draft relies on is false. The census found no mux among the long-lived holders, so the draft would not have changed the refusals seen in this window. Branch `fix/mux-closes-provenance-leases-20261109` (`8f18f6689`, `9a9c1959f`) stays unmerged as a reference.

## Tests added

N/A — not fixed.

## Workarounds

Format the files you changed with `rustfmt --edition 2021 <file>` after reading `git diff` for each, then `rustfmt --check`. Used five times in this session. The commit hook runs `rustfmt --check` on the committed bytes either way.

## Resume

0. Review requested 2026-11-09T11:47Z from Codex thread `01a1b59b-676f-7881-b7eb-4d8e1687c3f8` (app-server pid 646545, title "Verifica starea repo-ului"), read only, with three questions: is the mux's exclusion acceptable, does any fence test or proof rely on a daemon keeping the lock, and should `peer::server::run` change too. The send is attributed by one fact, the `Session-Id` trailer of `9cab1095e`; nobody confirmed that thread owns the fence today. The rollout file grew, so a turn ran; the request was sent again at the operator's request at 2026-11-09T11:58:02Z. The thread answered at 2026-11-09T13:49:48Z (static, read-only review; it states it is not an approval and cannot be answered by address, so reply through the operator). Its findings:
   - It does not recommend integrating the draft yet. The exclusion is acceptable only if the whole descendant tree of the daemon never writes the checkout. `mux::process::run` starts an arbitrary server command with `current_dir(workspace_root)`, so that precondition is not enforced. It calls this a contract concern, not a reproduced formatter bypass, because the persistent `unverified-shell-v1` marker still prevents certification.
   - Existing tests pin descendant retention: `the_actual_shell_inherits_the_request_scoped_fence` (`provenance_fence_context_tests.rs`) and `dropping_parent_guard_does_not_release_a_live_formatter` (`native_provenance_fence/tests.rs`). Any exemption needs an explicit contract boundary that keeps ordinary child protection. I confirmed the three tests it names exist, and that `a_terminal_shell_result_does_not_certify_detached_descendants` spawns no daemon (it runs `scope_root` with a `bash scripts/fmt-mine.sh` request and expects an "uncertified shell" refusal). I did not re-check the first two bodies.
   - `peer::server::run` needs its own inheritance audit, not a copy of the helper: its tool handling appends `.codescout/peer-audit.jsonl`, so a read-only tool surface does not mean the daemon never writes the checkout.
   - Before integration it asks for a process-level test of the real `mux::process::run` path: both inherited leases released, all seven variables absent in the child, the shell parent's leases still held, and ordinary descendant protection preserved.
   A reply is not an approval.
   - Follow-up sent 2026-11-09T21:38:23Z to the same thread (queued message `01a1c23f-3011-70b1-9426-ab42cd9db72b`, rollout grew 37208246 → 37221869 bytes), read only, from `claude/.claude "codescout-06"` pid 704722: the measurements, option 1 marked not recommended, and three questions: (a) is the exclusive lease meant for clients that never use the proof; (b) should it still refuse `fmt-mine` while `unverified-shell-v1` exists; (c) is the exact-text formatter match intended.
   - Owner's reply 2026-11-09T21:48:21Z (static review of source and of the approved design, `claude-plugins/docs/superpowers/specs/2026-11-07-native-execution-provenance-design.md` § Formatting and concurrent writers; the measurements were not reproduced; not an approval). (a) Exclusivity for clients without a proof is implemented and tested (`ordinary_edits_coexist_with_commands_but_formatting_requires_quiescence`, `client=None`, `meta=None`), but the design motivates it only by native checkpoint validity, so its extension to those clients has no documented rationale. (b) Persistent refusal of native certification while `unverified-shell-v1` exists is intended, with no automatic reset (Builder refinement 2026-11-07). The lease refusal is a separate condition: a formatter without a proof dispatches when the leases are free, despite the marker; no documented reason was found for blocking it globally when certification is unavailable anyway. (c) The exact allowlist is intended for the trusted-formatter exception, but the same predicate also selects general exclusivity, so argument-bearing forms get shared leases; do not advertise them as a workaround. It agrees option 1 stays not recommended, on the strength of the reported experiments. Checked here: `certificate` returns `Ok(None)` early for non-proof clients and checks the marker only for proof clients; the named test asserts the refusal with no client; the spec text matches the two Builder rulings it cites.
   - Receipt and review request for option 3 sent 2026-11-09T22:14:21Z (queued `01a1c260-1dd8-7c60-aafe-3f92f05059e5`, rollout grew 38095972 → 38149727 bytes). Review received 22:16:06Z; see `## Fix`, option 3.
1. Decide whether to merge the draft (`8f18f6689`). Open points: the call site is now tested by `tests/mux_releases_provenance_leases.rs` (draft branch commit `9a9c1959f`; two mutations of `process.rs` each fail it, and an ordinary-child control keeps the locks; it emulates the fence with plain `flock`, so it covers no real fenced shell and no formatter proof). `peer::server::run` is a second daemon of the same shape and is not changed; audit below. The owner's other point stands: the exclusion is safe only if the mux's descendants never write the checkout, and `mux::process::run` starts an arbitrary server command there. One run of `cargo test --lib provenance` on the branch failed `a_terminal_shell_result_does_not_certify_detached_descendants` (`provenance_fence_context_tests.rs:451`, error text not captured), while 6 of 6 repeated runs on the branch and 6 of 6 on the base passed. That test does not use the real account-wide coordination directory: under `cfg(test)`, `coordination_root` returns a fresh `/tmp/codescout-native-provenance-test-<pid>-<thread>` per process and thread (corrected 2026-11-10; the earlier text said the opposite). So live peer locks on the account cannot explain that failure; its cause is still unknown.
2. For option 1, add a test that spawns a daemon inside a fenced shell and checks the formatter can still be taken after the shell ends. The existing `a_lock_kept_by_a_child_of_a_dead_owner_is_named_as_such` rebuilds the dead-owner setup and can be extended.
3. Close this bug when the mux no longer keeps the lock, not when the refusal is more readable.
4. Audit of `peer::server::run` (read only, 2026-11-09; nothing changed). Read: `src/peer/launch.rs` `ensure_peer_serve` starts `codescout peer-serve` with a plain `tokio::process::Command` (no `env_remove`, no descriptor handling), so a codescout process that runs inside a fenced shell hands the daemon both leases the same way the mux got them. `peer::server::run_with_lock` takes only its own `peer.lock` and then serves. `build_server_for_with_env` builds an `Agent` whose home is the served workspace, read-write unless marked read-only, and every served call appends `{id, tool, ok}` to `<served workspace>/.codescout/peer-audit.jsonl`. `PEER_EXPOSED_TOOLS` is an allow-list of read tools, enforced in `handle_tool_call_inner`. Not read: what `Agent::new` and the exposed tools write beside the audit file, and whether an exposed tool can start a language server. Consequence: the daemon does write into the checkout it serves, so the mux rule ("never writes the checkout") is false for it whenever the served workspace is the leased checkout. The leased root and the served workspace are different values, so a release helper would have to compare them, per lease, and the coordinator lease (account-wide) needs its own answer. Not decided, not built.

## References

- `src/tools/provenance_fence_context.rs` (`is_foreground_formatter`, `scope_root`, `apply_to_shell`), `src/tools/native_provenance_fence.rs` (`try_mode`, `apply_to_command_named`, `coordination_root`).
- `docs/trackers/local-embed-opt-in-session-log.md` F-8.
- `docs/trackers/issue-clusters/IC-12-transient-shared-state-lies-to-readers.md`.
