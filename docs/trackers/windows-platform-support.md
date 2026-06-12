---
id: '42dfdfc8b1522192'
kind: tracker
status: active
title: Windows Platform Support — WIN-N Issue Index
owners:
- marius
tags:
- windows
- platform
- vdi
- portability
- ci
topic: windows
time_scope: null
---

# Windows Platform Support — WIN-N Issue Index

Living index of Windows-platform issues for codescout: what is broken, fixed,
mitigated, or deferred when running the MCP server + test suite on Windows. The
primary driver is a locked-down VDI whose EDR/AV injects into spawned processes
and stalls them. This tracker is the durable cross-session **map**; per-incident
detail lives in `docs/issues/` bug files, which get archived once their fix ships
to master — the tracker outlives them.

## Scope & boundary

- **Belongs here:** any Windows-specific defect, portability gap, build/install
  quirk, or platform-gated-code decision — as a one-line WIN-N row pointing at
  the bug file or commit that holds the detail.
- **Does NOT belong here:** full incident detail (→ `docs/issues/<date>-<slug>.md`),
  the scoped narrative of one work stream (→ `docs/trackers/<topic>-session-log.md`),
  or design docs (→ `docs/superpowers/specs|plans/`).
- **Largest contributor:** the VDI reliability work stream (spec + plan +
  session-log under Relationships); its bug files are indexed here as WIN-N rows.

## Status legend

| status | meaning |
|---|---|
| `fixed` | root cause addressed + verified (on `experiments` unless noted) |
| `mitigated` | workaround in place; root cause not fully addressed |
| `open` | known, unaddressed |
| `deferred` | scoped out to its own spec/plan with a re-open trigger |
| `wontfix` | intentionally not fixing |

## Areas

- **process-spawn** — child-process creation/kill under EDR (the core VDI hazard).
- **lsp** — language-server binary resolution + spawn.
- **platform-gated** — code/deps that are Unix-only and must be `cfg(unix)`-gated.
- **path-handling** — canonicalization, 8.3 short names, verbatim `\\?\`, HOME/USERPROFILE.
- **build-install** — building / reloading the live binary on Windows.
- **test-portability** — unit/integration tests that bake in Unix assumptions.
- **companion** — codescout-companion plugin (cross-repo) surfaced on Windows.

## Issue index

<!-- Rendered mirror of the augmentation `issues` params (tool-usage-patterns
     style). Maintain via:
       artifact_augment(id="42dfdfc8b1522192", merge=true, params={issues:[...]})
     then re-sync this table. Filter rows live with:
       artifact(action="get", id="42dfdfc8b1522192", entry_filter={"status":{"eq":"open"}}) -->

| id | area | status | summary | ref | since |
|----|------|--------|---------|-----|-------|
| WIN-1 | process-spawn | fixed | run_command spawn hang + cmd.exe quote mangling (EDR grandchild holds pipe; .arg() MSVC-CRT quoting; inherited stdin REPL block) | docs/issues/2026-06-08-windows-run-command-child-process-hang.md | 2026-06-08 |
| WIN-2 | process-spawn | fixed | process kill/liveness shelled out to taskkill/tasklist (spawn under EDR); now Win32 OpenProcess/TerminateProcess/GetExitCodeProcess | 9de846d4 | 2026-06-09 |
| WIN-3 | process-spawn | fixed | all 3 run_command spawn sites + BackgroundKillGuard routed through one platform::shell_command_configured; stdin=null default both platforms | 8c4c738f | 2026-06-09 |
| WIN-4 | lsp | fixed | LSP binary name hardcoded .cmd (npm shim only); now PATH-probes .cmd/.exe/.bat | docs/issues/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md | 2026-06-06 |
| WIN-5 | lsp | deferred | bounded LSP spawn timeout under EDR — spawn() is sync CreateProcessW, needs spawn_blocking; init handshake already bounded | docs/trackers/vdi-reliability-session-log.md (F-3) | 2026-06-09 |
| WIN-6 | platform-gated | fixed | peer module (Unix domain sockets) does not compile on Windows; gated behind cfg(unix) | 5f8911b2 | 2026-06-09 |
| WIN-7 | platform-gated | fixed | tikv-jemalloc-sys fails to build on Windows MSVC; gated as cfg(unix) target dep | docs/issues/archive/2026-05-24-ci-windows-jemalloc-build-fail.md | 2026-05-24 |
| WIN-8 | path-handling | mitigated | librarian umbrella members need verbatim \\?\ prefix on Windows (canonicalize().starts_with); workspace.toml written with verbatim + plain fallbacks | %APPDATA%\librarian\workspace.toml | 2026-06-08 |
| WIN-9 | test-portability | fixed | Windows lib suite green: fixed 5 in-repo failures (detect_project_root marker-shadowing + is_denied \\?\ verbatim deny-bypass = 2 real bugs; plus open-file-handle, /tmp seed, canonical-root test fixes) | 1d8cde48 (experiments) | 2026-06-09 |
| WIN-10 | build-install | mitigated | running .exe is locked during rebuild; no ~/.cargo symlink on Windows; workflow = move exe aside, background rebuild, /mcp reload | CLAUDE.md | 2026-06-08 |
| WIN-11 | test-portability | fixed | server tool-count tests hardcoded Unix count of 22 (incl. peer); made cfg(unix)-aware (21 on Windows) | 5881ed09 | 2026-06-09 |
| WIN-12 | companion | fixed | codescout-companion hooks referenced consolidated-away tool names; updated to edit_code/edit_file/edit_markdown/create_file | codescout-companion:71aceeb | 2026-06-09 |
| WIN-13 | path-handling | mitigated | librarian + guide_hint emitted mixed-slash paths on Windows (18 historical test failures) | docs/issues/archive/2026-05-24-ci-windows-default-feature-failures.md | 2026-05-24 |
| WIN-14 | process-spawn | fixed | resolve_head_sha shelled out to `git rev-parse --short HEAD` at every project activation (unbounded .output(), no timeout → EDR hang risk); now libgit2 revparse_single().short_id() like sibling probe_has_git_remote | bcc712ae (experiments) | 2026-06-09 |
| WIN-15 | process-spawn | fixed | hardware GPU probe spawned nvidia-smi/rocm-smi at onboarding (sync CreateProcessW in 2s timeout that can't preempt a hung spawn → tokio-worker stall); now skipped on Windows unless CODESCOUT_GPU_PROBE set, via pure gpu_probe_enabled() | 8ceb908f (experiments) | 2026-06-09 |
| WIN-16 | build-install | mitigated | windows-gnu (MinGW), not MSVC: jobs=64 caused 16x oversubscription on the 4-core VDI (64 EDR-taxed rustc on 4 cores); removed the hardcoded cap so cargo auto-detects logical CPUs. lld fast-linker deferred (bundled-lld link on windows-gnu unverified + fresh-binary EDR quarantine risk — WIN-18) | 20aa7df3 (experiments) | 2026-06-09 |
| WIN-17 | test-portability | fixed | cargo clippy -D warnings failed on Windows: cfg(unix)-gated peer + mux left is_codescout_kotlin_home + 5 peer-serve CodeScoutServer methods + parse_env_kv dead; plus missing_const_for_thread_local FP (clippy 1.96); annotated cfg_attr(not(unix), allow(dead_code)) + scoped allow | 2f6e35c3 (experiments) | 2026-06-09 |
| WIN-18 | build-install | open | CrowdStrike EDR quarantines freshly-built unsigned binaries: a 3-line hello-world test exe was deleted as malware seconds after rustc produced it. Large cargo outputs (codescout.exe, test deps) survive — ML heuristic targets tiny isolated PEs. Avoid throwaway standalone exes; spurious build/test-failure risk; AV unchangeable (target/ exclusion out of our control) | session 2026-06-09 observation | 2026-06-09 |
| WIN-19 | lsp | fixed | lsp_binary_name preferred `.cmd` over `.exe` when both on PATH → forced an implicit cmd.exe shim spawn (WIN-1 EDR grandchild hazard); now probes `.exe` first. Resolution logic + 5 tests moved to platform::mod (368aa9df) so they run on the Linux gate (lib 2685→2690), not just Windows | vdi-windows 9cba50cb, 368aa9df | 2026-06-12 |
| WIN-20 | process-spawn | fixed | run_command_inner Windows foreground arm leaked out/err temp files on the spawn-error path (TmpfileGuards built inside the future; an early `?` from spawn() orphaned the `.keep()`d files in %TEMP%); guards now created before spawn + moved in | docs/issues/2026-06-12-windows-runcmd-tempfile-leak-spawn-error.md | 2026-06-12 |
| WIN-21 | build-install | fixed | local-embed + local-embed-dynamic are mutually-exclusive ONNX backends (static-link vs dlopen); enabling both handed `ort` conflicting features → cryptic link error. Added cfg(all(...)) compile_error! guard (Linux-verified) | vdi-windows 9cba50cb | 2026-06-12 |
## Currently stable on Windows

What works now (post the VDI reliability stream, on `experiments`):

- `cargo build` (lib + bin) compiles clean.
- `run_command` foreground / background / interactive — no hangs, correct
  quoting, `stdin=null` default; all spawns route through
  `platform::shell_command_configured`.
- Process kill/liveness via Win32 (no `taskkill`/`tasklist` spawn).
- Git operations (libgit2 — never shells out, so immune to the spawn hazard).
- LSP binary resolution probes `.cmd`/`.exe`/`.bat` on PATH.

## Open items / next steps

- **WIN-5** — LSP spawn-timeout under EDR: needs a `spawn_blocking` spec before
  implementation. *(the only remaining open/deferred item)*
- **Linux/CI compile of the cfg(unix) changes** — the WIN-3/WIN-6/WIN-11 + WIN-9
  work was authored & tested only on Windows, where the `cfg(unix)` paths are
  excluded from compilation. A static audit (2026-06-09) reviewed every
  unix-only surface the Windows compiler skipped — `unix.rs::shell_command_configured`
  (stdin=null add), peer `cfg(unix)` gating (no-op on Linux), `is_denied`
  `normalize` (reduces to `to_path_buf` off-Windows), `inner.rs` foreground
  `child_pgid`/`_child_pgid` branches, `build_windows_cmdline` (`pub`, no
  dead-code lint), and `Cargo.toml` target-gating — and found **no Linux-compile
  hazards** (no missing imports, dead-code lints, dangling refs to the removed
  `shell_command` builder, or unbalanced cfg). Remaining gate is mechanical:
  `cargo build` + `cargo clippy -- -D warnings` + `cargo test` on Linux must pass
  before any cherry-pick of these commits to `master`.

_WIN-9 and WIN-12 were fixed 2026-06-09 — see History._

**2026-06-12 — Linux review session:**
- ✅ **Linux compile-gate PASSED.** `cargo build` + `cargo clippy --all-targets -- -D warnings` + `cargo test` all green on Linux against the full vdi-windows stack (vs merge-base `0c84c1a4`). Sole test failure is the environmental live-reranker integration test (`reranker_returns_scores_in_input_order`), proven branch-independent (no `src/retrieval/` changes on this branch). The documented graduation blocker is cleared; the cfg(unix) surfaces (WIN-3/6/9/11/17) compile clean on Linux.
- WIN-19 / WIN-20 / WIN-21 found & fixed (`vdi-windows 9cba50cb`). WIN-19/20 are cfg(windows) — verified by reasoning; compiler-gated on the VDI build (no mingw cross-toolchain on the Linux host). WIN-21 is Linux-checked.
- **WIN-19 follow-up (`368aa9df`):** moved `lsp_binary_name_with` + its 5 tests from `cfg(windows) platform::windows` to `platform::mod` — now run on the Linux gate (lib 2685→2690), so WIN-19's `.exe`-first behavior is a running Linux test, not just reasoning. (The rust LSP mux that blocked this edit recovered after a rebuild + `/mcp` restart — the rebuilt binary carries the `cded34f0` orphaned-lock reap fix this branch predates.)
- **Deferred minor review findings** (noticed 2026-06-12, not fixed — low/info): (1) `process_alive` uses `GetExitCodeProcess == STILL_ACTIVE(259)` — ambiguous if a child genuinely exits with code 259; `WaitForSingleObject(h,0)` is more robust. (2) No Windows process-*tree* kill — `terminate_process` kills only the cmd.exe PID (matches old taskkill `/F` without `/T`; relates to deferred WIN-5). (3) `is_denied` comparison stays case-sensitive — a non-canonicalized input with case variation could slip past on a case-insensitive Windows FS (pre-existing, narrow window).
- **Branch divergence:** `vdi-windows` (30 ahead of merge-base) and `experiments` (10 ahead) have diverged; `experiments` carries the `src/lsp/manager.rs` / mux-single-owner refactor this branch predates. A graduation rebase will touch `src/lsp/manager.rs`. (Also: the rust LSP mux failed to spawn in this worktree — likely the orphaned-RocksDB-lock bug fixed on experiments by `cded34f0`, absent here.)
**2026-06-12 (cont.) — rebased on `experiments`; MinGW+wine local Windows loop established:**
- ✅ **Rebased `vdi-windows` onto `experiments`** (34 commits replayed, **zero conflicts**, linear). The feared semantic conflict in `src/lsp/manager.rs` (mux-single-owner refactor) did not materialize — `cargo build` + `clippy --all-targets -D warnings` + `cargo test` (2754 pass, 1 env reranker) all green post-rebase. Branch now 36 ahead of `experiments`; `origin/vdi-windows` divergent (force-push needed when sharing). The mux now spawns (rebase pulled in `cded34f0`).
- ✅ **Local off-VDI Windows verification loop — `scripts/build-windows.sh`** (commit `154abbef`). MinGW-w64 + `x86_64-pc-windows-gnu` cross-compiles a valid PE32+ binary on Linux with default features (`ring`, vendored-libgit2, bundled SQLite, sqlite-vec, 9 tree-sitter grammars all link); wine then *executes* the test binaries. This **retires the "WIN-19/20 verified by reasoning only" caveat**: `win32_terminate_and_liveness` + `win32_liveness_false_for_dead_pid` (real `OpenProcess`/`TerminateProcess`/`GetExitCodeProcess`) and the WIN-19 `.exe`-first probe tests now PASS under wine. Cross-compile knobs are `CARGO_TARGET_*` env overrides, kept out of the committed `.cargo/config.toml` so the VDI native-gnu build is unaffected.
- ✅ **First catch (`396bd62a`):** `is_test_runner_exe` (`src/lsp/manager.rs`, experiments-origin mux code) was dead in the windows non-test lib build (its sole caller is unix-only) — invisible to the Linux gate, surfaced by the gnu cross-compile. Fixed with `cfg_attr(windows, allow(dead_code))`.
- ⚠️ **Follow-up (candidate WIN-22) — test-code warning cluster:** the gnu *test* build shows ~7 platform-conditional warnings (unused `uri_to_path`, the `sneaky_link` symlink-test vars in `path_security`, etc.) — unix-only test helpers unused when test code compiles for windows. Harmless to the binary; would trip a gnu CI job running `cargo test`/clippy with `-D warnings`. Clean before adding such a gate.
- **Next step — automate the gnu ABI:** CI already runs `cargo test` on `windows-latest` (**MSVC**) × {default, local-embed, no-features}, but **nothing automated tests the gnu ABI shipped to the EDR/VDI**. Options: add an `ubuntu-latest` cross-compile job (mingw + `scripts/build-windows.sh build`, optionally `+ wine` for tests), or register a self-hosted/VDI gnu runner. wine validates logic, not EDR — the VDI stays the EDR-realism gate.
## Relationships

- Spec: `docs/superpowers/specs/2026-06-08-vdi-reliability-hardening-design.md`
- Plan: `docs/superpowers/plans/2026-06-08-vdi-reliability-hardening.md`
- Session log: `docs/trackers/vdi-reliability-session-log.md`
- Active bug files: `docs/issues/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md`,
  `docs/issues/2026-06-08-windows-run-command-child-process-hang.md`,
  `docs/issues/2026-06-09-windows-test-suite-preexisting-failures.md`
- Archived CI-Windows bugs: `docs/issues/archive/2026-05-24-ci-windows-*.md`

## How to append

When a Windows issue is found or its status changes:

1. `artifact(action="get", id="<id>", entry_filter={...})` — confirm it is not
   already tracked.
2. `artifact_augment(id="<id>", merge=true, params={issues:[...existing..., {new WIN-N}]})`
   — next free integer; never reuse or delete; flip status + cite the fixing
   commit (master-side SHA after cherry-pick) in `ref`.
3. Re-sync the "## Issue index" table above with the render_template columns.
4. For a brand-new incident, also open a `docs/issues/<date>-<slug>.md` and cite
   it in `ref`.

## History

### 2026-06-09 — tracker created
Seeded with 13 WIN-N entries from the VDI reliability work stream plus the
2026-05-24 CI-Windows archive. Created after a `librarian reindex` (649
artifacts) confirmed no existing Windows tracker among the 36 live ones.


### 2026-06-09 — WIN-9 + WIN-12 fixed; Windows lib suite green
Fixed the 5 in-repo WIN-9 failures — 2 real bugs (`detect_project_root`
marker-shadowing; `is_denied` `\\?\` verbatim deny-list bypass) + 3
test-portability fixes (open file handle blocking `metadata()`; `/tmp` seed
absent on Windows; payload built from the agent's canonical root) — in
`1d8cde48`. Fixed WIN-12 companion-hook tool-name drift in
`codescout-companion:71aceeb` (branch `fix/windows-tool-name-drift`). Full
`cargo test --lib` now green on Windows: 2598 passed, 0 failed, 13 ignored.


### 2026-06-09 — WIN-14/15/16 opened (VDI speed: remaining spawn + build levers)
After the AV/EDR exclusion lever was ruled out (cannot modify on this VDI),
opened the three in-our-control speed issues a spawn audit surfaced: WIN-14
(git-spawn on every activation → libgit2), WIN-15 (GPU-probe spawns at
onboarding), WIN-16 (Windows build linker + jobs tuning). Ranked WIN-14 first
(small, removes a spawn *and* an unbounded-hang risk from the activation path).


### 2026-06-09 — WIN-14 + WIN-17 fixed (VDI speed pass, part 1)
WIN-14: `resolve_head_sha` now uses libgit2 (`revparse_single("HEAD").short_id()`)
instead of spawning `git rev-parse` at every activation — removes an EDR-taxed
spawn and an unbounded-`.output()` hang risk (`bcc712ae`). WIN-17: discovered
while running the clippy gate for WIN-14 — `cargo clippy -- -D warnings` was red
on Windows because the cfg(unix) peer + mux gating left `is_codescout_kotlin_home`,
five `CodeScoutServer` peer-serve helpers, and `parse_env_kv` dead, plus a
`missing_const_for_thread_local` clippy-1.96 false positive; fixed with scoped
`cfg_attr(not(unix), allow(dead_code))` + one `allow` (`2f6e35c3`). Verified on
Windows: clippy green (lib + bins), `cargo test --lib` green (2598 passed).
WIN-15 (GPU-probe spawns) and WIN-16 (.cargo linker/jobs) remain open.


### 2026-06-09 — WIN-15 + WIN-16 + WIN-18 (VDI speed pass, part 2)
WIN-15: GPU subprocess probes (nvidia-smi/rocm-smi) skipped on Windows unless
`CODESCOUT_GPU_PROBE` is set — sync CreateProcessW in a 2s timeout can't preempt a
hung spawn, so a stalled probe blocked a tokio worker; pure `gpu_probe_enabled()`
+ test (`8ceb908f`). WIN-16: the toolchain is windows-gnu (MinGW), not MSVC —
framing corrected. The real win was removing `jobs = 64` from `.cargo/config.toml`:
on a 4-core VDI that was 16x oversubscription (64 EDR-taxed rustc on 4 cores);
cargo now auto-detects logical CPUs (`20aa7df3`). The lld fast-linker half is
deferred. WIN-18 (new): while probing the linker I compiled a 3-line hello-world
to a standalone exe — CrowdStrike quarantined it as malware within seconds. A
textbook EDR false positive on tiny unsigned PEs; large cargo outputs survive.
Lesson: do not produce throwaway standalone binaries on this VDI, and there is a
latent risk of spurious build/test failures if a real artifact is ever flagged.
AV exclusions are out of our control here.
