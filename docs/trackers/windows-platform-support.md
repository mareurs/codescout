---
id: '52451519052d207c'
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
expects_augmentation: docs/augmentations/docs-trackers-windows-platform-support.yaml
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
       artifact(action="append_entry", id="52451519052d207c", entry_collection="issues", id_prefix="WIN", entry={...})
      -- NEVER artifact_augment(merge=true, params={issues:[...]}): it replaces the array
     then re-sync this table. Filter rows live with:
       artifact(action="get", id="52451519052d207c", entry_filter={"status":{"eq":"open"}}) -->

| id | area | status | summary | ref | since |
|----|------|--------|---------|-----|-------|
| WIN-1 | process-spawn | fixed | run_command spawn hang + cmd.exe quote mangling (EDR grandchild holds pipe; .arg() MSVC-CRT quoting; inherited stdin REPL block) | docs/issues/archive/2026-06-08-windows-run-command-child-process-hang.md | 2026-06-08 |
| WIN-2 | process-spawn | fixed | process kill/liveness shelled out to taskkill/tasklist (spawn under EDR); now Win32 OpenProcess/TerminateProcess/GetExitCodeProcess | 9de846d4 | 2026-06-09 |
| WIN-3 | process-spawn | fixed | all 3 run_command spawn sites + BackgroundKillGuard routed through one platform::shell_command_configured; stdin=null default both platforms | 8c4c738f | 2026-06-09 |
| WIN-4 | lsp | fixed | LSP binary name hardcoded .cmd (npm shim only); now PATH-probes .cmd/.exe/.bat | docs/issues/archive/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md | 2026-06-06 |
| WIN-5 | lsp | deferred | bounded LSP spawn timeout under EDR — spawn() is sync CreateProcessW, needs spawn_blocking; init handshake already bounded | docs/trackers/archive/vdi-reliability-session-log.md (F-3) | 2026-06-09 |
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
| WIN-20 | process-spawn | fixed | run_command_inner Windows foreground arm leaked out/err temp files on the spawn-error path (TmpfileGuards built inside the future; an early `?` from spawn() orphaned the `.keep()`d files in %TEMP%); guards now created before spawn + moved in | docs/issues/archive/2026-06-12-windows-runcmd-tempfile-leak-spawn-error.md | 2026-06-12 |
| WIN-21 | build-install | fixed | local-embed + local-embed-dynamic are mutually-exclusive ONNX backends (static-link vs dlopen); enabling both handed `ort` conflicting features → cryptic link error. Added cfg(all(...)) compile_error! guard (Linux-verified) | vdi-windows 9cba50cb | 2026-06-12 |
| WIN-22 | build-install | mitigated | CrowdStrike quarantines the unsigned onnxruntime.dll that `local-embed*` downloads (file-level quarantine-on-write — same vector as WIN-18) → semantic search breaks on the VDI. Fix is config/build, no code: default build (drop local-embed*) + set [embeddings].url to the corporate OpenAI-compatible endpoint (RemoteEmbedder → zero ONNX on box) + reindex (dim change from 384-d local default). | docs/manual/src/configuration/embeddings-edr-windows.md | 2026-06-12 |
| WIN-23 | test-portability | fixed | 7 unix-only test helpers (uri_to_path / BUFFER_QUERY_INLINE_CAP / CountingSink / Ordering imports, project_ctx_with_progress, 2 symlink-test tempdir+link vars, agent_security_config) were live on unix but unused when test code compiles for windows-gnu (their consumers are all #[cfg(unix)] tests); gated #[cfg(unix)] / cfg(all(test,unix)) so the gnu `cargo test` compile is warning-clean — unblocks a -D-warnings gnu CI gate. Surfaced by scripts/build-windows.sh; Linux gate unchanged | vdi-windows 8632093b | 2026-06-12 |
| WIN-24 | process-spawn | fixed | legibility_scan `git_head` shelled out to `git -C <root> rev-parse HEAD` (unbounded `.output()`, no timeout → same EDR CreateProcessW hang class as WIN-14); the git-spawn-elimination law reached `resolve_head_sha` (WIN-14) but not this sibling. Now libgit2 `revparse_single("HEAD").id().to_string()`, mirroring `resolve_head_sha` + `probe_has_git_remote`. Found by an architecture review's whole-tree spawn-grep (Snow Lion). Host + windows-gnu check, clippy -D warnings, 19 legibility tests clean | vdi-windows f22ed192 | 2026-06-15 |
| WIN-25 | process-spawn | fixed | `collect_go_deps` shelled out to `go env GOMODCACHE` (blocking `.output()`, no timeout → EDR CreateProcessW hang, same class as WIN-14/WIN-24). `go` has no library binding, so GOMODCACHE is re-derived from the environment via a pure `go_mod_cache_from()` (GOMODCACHE → GOPATH/pkg/mod → `<home>/go/pkg/mod`, home via platform::home_dir()); a `go env -w`-only override degrades to source-not-found, never hangs. 5-case pure unit test on the Linux gate. Same Snow Lion review pass as WIN-24 | vdi-windows 13534cbd | 2026-06-15 |
| WIN-26 | retrieval-stack | fixed | VDI can't run Docker/Qdrant, so the WIN-22 remote-embeddings fix is necessary but NOT sufficient: code `semantic_search` is hard-wired to Qdrant (the in-process path was removed 2026-05-07). Needs a daemon-free "lite" stack = remote OpenAI dense + in-process sqlite-vec (statically-linked `vec0`, EDR-safe — already proven for librarian via `ArtifactBackend::SqliteVec`, see `migrate_v6.rs`). Plan: generalize that escape hatch to code search + memory. Phase 0 (dense always OpenAI-compatible + drop TEI + dense-only-leak fix) shipped 825c0c52; Phases 1-4 ALL SHIPPED to master (1: 0ff972f7 CodeVectorStore trait, 2a: b96c8ae4 SqliteVecCodeStore, 2b: 93ef0d43 sqlite memory store, 3: 9d40d36b dense-only + lite flag, 4: 5c1ecfa8 lean default build, server-stack feature-gated). Closed 2026-07-02 by verify-open pass. | docs/plans/archive/2026-06-16-two-stack-retrieval-lite.md | 2026-06-16 |
| WIN-27 | test-portability | open | first full wine suite (windows-gnu CI) showed 20 pre-existing failures; 8-test guide_hint cluster FIXED 2026-07-05 (make_server now seeds LIBRARIAN_WORKSPACE so build_tool_context does not depend on the absent ~/.config workspace under wine) and un-skipped in CI, verified 10 pass/1 ignored under wine; 12 remain skipped (symbols/glob-walk emulation quirks, preflight/gitignore, markdown compact, run_command quoting, head_sha) plus validate_prune_request_gates (the one real-Windows MSVC failure) | docs/issues/archive/2026-07-02-windows-gnu-wine-20-test-failures.md | 2026-07-02 |
| WIN-28 | test-portability | fixed | nine real-Windows (MSVC) lib failures on `experiments`: 7 in `librarian::tools::doctor` (catalog rehome + prune_missing), `librarian::util::like_escape_idiom_is_not_inlined_outside_helper`, `retrieval::index_lock::lock_path_is_not_sited_in_bare_temp_dir`. Three root causes, **zero product defects**: (a) POSIX-shaped absolute literals — `"/gone/old"` is not absolute on Windows, so `validate_rehome_request`'s gate rejected the fixture and `derive_dead_roots` skipped the row *by design*; (b) mixed-separator expectation vs OS-shaped `CARGO_MANIFEST_DIR`; (c) a Unix-only siting assertion — `per_user_runtime_dir()` returns bare `temp_dir()` on Windows deliberately, since `%LOCALAPPDATA%\Temp` is already per-user. Fixed by a `dead_root(tag)` fixture helper, `RepoPath` normalisation on both sides of two comparisons, `#[cfg(unix)]` scoping plus a new platform-independent replacement assertion, and a 4× Windows LSP-indexing budget. Verified CI run `31098286970`: windows/default **3283 passed 0 failed**, all three windows cells green. | docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md | 2026-08-06 |
| WIN-30 | ci | mitigated | Two Windows tests flake on timing/race assumptions and turned a 15-job run red with no code defect. `cold_start_over_budget_returns_none_but_keeps_warming` **fixed** — moved onto tokio's virtual clock (`start_paused` + `tokio::time::Instant`); `0.00s`, deterministic at 25/25, ceiling assertion now exact rather than a 100 ms tolerance. `background_command_with_quotes_captures_output` **mitigated** — its poll no longer discards the `Err` arm and the bound went 5 s → 15 s, so the next MSVC red names its own cause; the intermittency itself was never reproduced. The wine `--skip` is now *explained* rather than pending investigation: wine has no Python launcher. Technically fires WIN-29's reopen trigger; deliberately not reopened — see History. | docs/issues/2026-08-07-windows-ci-timing-flakes-block-the-gate.md | 2026-08-07 |
| WIN-29 | ci | fixed | `Windows-gnu cross (MinGW + wine)` red and undiagnosed — closed as a **duplicate of WIN-28**, confirmed twice: its failing-test set was byte-identical to `Test (windows-latest / default)`'s nine, and it went green from the same nine fixture fixes with no MinGW- or wine-specific change. No cross-target defect exists. Reopen only if the cross job ever fails a test `windows-latest` passes. | docs/issues/archive/2026-08-06-windows-gnu-cross-job-red-undiagnosed.md | 2026-08-06 |
| WIN-32 | process-spawn | fixed | run_command ran via `cmd /C`, so codescout's own documented buffer-query workflow (`grep`/`tail` on `@cmd_*` refs, Iron Law 3) was unrunnable on Windows. Now Git Bash `bash -c`: resolver skips %SystemRoot%\System32 (that `bash.exe` is the WSL launcher, which would silently move every command onto /mnt/c — measured 25-170x slower); one POSIX tokenizer so the security layer parses what the shell executes; `shell_path_str` forward-slashes interpolated paths at all 3 sites (`\` is a bash escape) — unquoted, since `is_buffer_only` matches on those strings and quoting would reclassify buffer-only commands past the dangerous-command gate; kill-on-close Job Object reaps the tree on cancel (`kill_on_drop` killed only the shell, and the regression test was vacuous under cmd because `sleep` did not exist). Also fixes `inject_tee`'s SF-4 allowlist rejecting `:`/`\`, which had made tee injection unreachable on Windows. NB: the 3 test targets that fail to launch with `Access is denied (os error 5)` during this work were **misattributed here** to WIN-18 quarantine; they are the **CyberArk EPM** application-control vector (see WIN-35). Distinct mechanism, distinct symptom: WIN-18 *deletes* a freshly written unsigned PE, CyberArk *denies execution* and leaves the file intact — `os error 5` on launch means the file is still there. Confirmed identical at HEAD by stash-and-compare, so still not caused by this change | b142a514 (experiments) | 2026-08-07 |
| WIN-33 | process-spawn | fixed | Regression introduced by WIN-32: `shell_command_configured` set `MSYS_NO_PATHCONV=1` + `MSYS2_ARG_CONV_EXCL=*`, which disable the argument rewriting that lets a **native** binary accept an MSYS-form path — so every `git -C /c/...` issued through `run_command` died with `cannot change to '/c/...': No such file or directory`. The justification given in d564c9bb (protecting `sed 's/a/b/'` and `find / -name x` from mangling) was impossible: `sed`/`find` in Git Bash are MSYS binaries, and MSYS only converts arguments crossing into a native program, so there was nothing to protect against — the cost was breaking the crossing that does happen, into `git.exe`. Now `env_remove`d rather than merely left unset, so a parent-exported value cannot change how commands resolve (same reasoning as pinning `GIT_PAGER=cat`). The regression test asserts on the **native** side of the boundary on purpose: a test driving only MSYS builtins (`ls /c/...`) passes either way, because MSYS programs resolve MSYS paths themselves and never see the conversion — that is precisely the green-check-that-cannot-fail which let this ship | e4b86447 (experiments); docs/issues/archive/2026-08-07-msys-pathconv-optout-breaks-native-exe-paths.md | 2026-08-07 |
| WIN-34 | path-handling | fixed | librarian `containing_root` could never match a catalog path on Windows, so `artifact(move)` and `artifact(delete)` failed with `no managed root contains <path>` for rows `create`/`find` handled fine. The catalog stores `abs_path` forward-slash-normalized and verbatim-prefixed (`//?/C:/Users/...`) — doctor's `check_backslash` enforces the forward slashes — while `current_project` keeps the native canonicalized spelling (`\\?\C:\Users\...`). Rust's Windows prefix parser matches literal backslashes only, so the first has no prefix component and the second parses as `VerbatimDisk('C')`; `Path::starts_with` compares components and can never bridge them. Now compares via `comparable_path()` (separators unified, verbatim prefix stripped; Windows only, since `\` is a legal filename byte on Unix) plus an explicit separator check preserving the security-relevant component boundary. Found by reading `catalog.db` directly, which falsified a symptom-consistent hypothesis in the bug file. doctor stayed green throughout because it polices the very convention that broke the comparison — a WIN-13-class spelling collision, and another unfalsifiable check | a8253b62 (experiments); docs/issues/archive/2026-08-07-artifact-move-cannot-resolve-source-in-subroot-workspace.md | 2026-08-07 |
| WIN-35 | tooling | open | Python LSP unusable on this host: `pyright`/`pyright-langserver` in `~/.local/bin` are uv trampoline `.exe` shims that fail with `uv trampoline failed to spawn Python child process / permission denied (os error 5)` after ~35s, so `edit_code` times out with `LSP request timed out after 30s: initialize` on every `.py` file and Iron Law 2 is unsatisfiable for Python here (fall back to `edit_file`). Cause is **CyberArk EPM** application control blocking the trampoline from spawning its Python child — **not** the WIN-18 CrowdStrike quarantine vector, which an earlier revision of this entry wrongly assumed from the shared `os error 5` signature. The two are distinct: WIN-18 *deletes* a freshly written unsigned PE; CyberArk *denies execution* and leaves the file intact. Not fixable from codescout — needs a CyberArk policy exception for the trampoline + venv interpreter; mitigation is a non-trampoline pyright (npm / bundled node). Separately actionable in codescout: surface the child's stderr on LSP init failure — the 35s failure exceeds the 30s init timeout, so a permanent failure reads to the agent as a retryable cold start | session 2026-08-07 observation | 2026-08-07 |
| WIN-36 | process-spawn | mitigated | Since WIN-32 every Windows spawn goes through Git Bash, so a host without Git for Windows cannot run commands **at all** — and the only diagnostic was the OS's bare `program not found`, which names neither the requirement nor the fix. Surfaced as 22 lib failures in the wine cross job on PR #10 (run `31220855460`); the three `windows-latest` cells pass because the GitHub runner ships Git. The fix is a mitigation **on purpose**: Git Bash stays a hard requirement, because falling back to `cmd /C` would re-open exactly what WIN-32 closed — the security layer tokenizing POSIX while the shell executes something else. The resolution chain is now a pure `resolve_git_bash(env, is_file)` returning `Option`, `shell_unavailable_hint()` turns `None` into a `RecoverableError` naming Git for Windows and `CODESCOUT_BASH`, and `run_command` preflights it before any spawn. The 22 wine failures are skipped as environmental, with the un-skip protocol in the workflow comment. Second-order finding: the not-installed branch was **untestable** — real env plus a real filesystem probe — which is why it shipped unexercised; three injected-probe tests now cover it, including the System32/WSL exclusion that was previously undecidable in a test. Root-cause close = install Git for Windows in the wine image and delete the skip block | docs/issues/archive/2026-08-08-run-command-unusable-without-git-bash.md | 2026-08-08 |

## Per-issue detail

One section per WIN-N, and its only job is to **define the token**: `link_scan` derives a
citable definition from a `## <ID> — <title>` heading and from nothing else, so before this
section existed all 129 external citations of WIN-N resolved to nothing (measured 2026-08-18,
`docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`).
Kept deliberately terse — the table above holds the full account; these are the landing spots a
citation needs. Written from the **table**, not from `params`, because the two had diverged: see
§ History 2026-08-18.

### WIN-1 — run_command spawn hang and cmd.exe quote mangling
`process-spawn` · **fixed** · 2026-06-08 · `docs/issues/archive/2026-06-08-windows-run-command-child-process-hang.md`
An EDR grandchild held the pipe open, `.arg()` applied MSVC-CRT quoting, and inherited stdin blocked on a REPL.

### WIN-2 — process kill and liveness shelled out to taskkill/tasklist
`process-spawn` · **fixed** · 2026-06-09 · `9de846d4`
Spawning under EDR to ask about a process; now Win32 `OpenProcess`/`TerminateProcess`/`GetExitCodeProcess`.

### WIN-3 — three spawn sites unified behind one platform helper
`process-spawn` · **fixed** · 2026-06-09 · `8c4c738f`
All three `run_command` spawn sites plus `BackgroundKillGuard` routed through `platform::shell_command_configured`; `stdin=null` by default on both platforms.

### WIN-4 — LSP binary name hardcoded to .cmd
`lsp` · **fixed** · 2026-06-06 · `docs/issues/archive/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md`
The `.cmd` assumption held only for npm shims; the resolver now PATH-probes `.cmd`/`.exe`/`.bat`.

### WIN-5 — LSP spawn timeout unbounded under EDR
`lsp` · **deferred** · 2026-06-09 · `docs/trackers/archive/vdi-reliability-session-log.md` (F-3)
`spawn()` is a synchronous `CreateProcessW` and needs `spawn_blocking`; the init handshake is already bounded.

**Valid:** dated 2026-06-09

### WIN-6 — peer module does not compile on Windows
`platform-gated` · **fixed** · 2026-06-09 · `5f8911b2`
Unix domain sockets have no Windows equivalent here; the module is gated behind `cfg(unix)`.

### WIN-7 — jemalloc-sys fails to build on MSVC
`platform-gated` · **fixed** · 2026-05-24 · `docs/issues/archive/2026-05-24-ci-windows-jemalloc-build-fail.md`
`tikv-jemalloc-sys` is now a `cfg(unix)` target dependency.

### WIN-8 — umbrella members need the verbatim `\\?\` prefix
`path-handling` · **mitigated** · 2026-06-08 · `%APPDATA%\librarian\workspace.toml`
`canonicalize().starts_with` compares prefixed paths, so `workspace.toml` is written with verbatim *and* plain fallbacks.

### WIN-9 — five real Windows lib failures, two of them product bugs
`test-portability` · **fixed** · 2026-06-09 · `1d8cde48` (experiments)
`detect_project_root` marker-shadowing and an `is_denied` verbatim deny-bypass were genuine defects; the other three were test fixes (open file handle, `/tmp` seed, canonical root).

### WIN-10 — the running .exe is locked during rebuild
`build-install` · **mitigated** · 2026-06-08 · `CLAUDE.md`
No `~/.cargo` symlink on Windows either; the workflow is move the exe aside, rebuild in the background, `/mcp` reload.

### WIN-11 — tool-count tests hardcoded the Unix count
`test-portability` · **fixed** · 2026-06-09 · `5881ed09`
22 including `peer`; now `cfg(unix)`-aware, 21 on Windows.

### WIN-12 — companion hooks named consolidated-away tools
`companion` · **fixed** · 2026-06-09 · `codescout-companion:71aceeb`
Updated to `edit_code`/`edit_file`/`edit_markdown`/`create_file`.

### WIN-13 — mixed-slash paths from librarian and guide_hint
`path-handling` · **mitigated** · 2026-05-24 · `docs/issues/archive/2026-05-24-ci-windows-default-feature-failures.md`
18 historical test failures traced to one spelling inconsistency — the same class WIN-34 later hit in the catalog.

### WIN-14 — resolve_head_sha shelled out to git rev-parse
`process-spawn` · **fixed** · 2026-06-09 · `bcc712ae` (experiments)
An unbounded `.output()` with no timeout at every project activation — EDR hang risk; now libgit2 `revparse_single().short_id()`, like its sibling `probe_has_git_remote`.

### WIN-15 — GPU probe spawned nvidia-smi at onboarding
`process-spawn` · **fixed** · 2026-06-09 · `8ceb908f` (experiments)
A synchronous `CreateProcessW` inside a 2s timeout that cannot preempt a hung spawn, stalling a tokio worker; now skipped on Windows unless `CODESCOUT_GPU_PROBE` is set.

### WIN-16 — jobs=64 oversubscribed the 4-core VDI
`build-install` · **mitigated** · 2026-06-09 · `20aa7df3` (experiments)
64 EDR-taxed rustc processes on 4 cores; the hardcoded cap is gone so cargo auto-detects. The lld fast linker stays deferred — unverified on windows-gnu, plus WIN-18 quarantine risk.

### WIN-17 — clippy -D warnings red from cfg(unix)-gated dead code
`test-portability` · **fixed** · 2026-06-09 · `2f6e35c3` (experiments)
Gating `peer` and `mux` left `is_codescout_kotlin_home`, five peer-serve methods and `parse_env_kv` dead, plus a `missing_const_for_thread_local` false positive in clippy 1.96.

### WIN-18 — EDR quarantines freshly built unsigned binaries
`build-install` · **open** · 2026-06-09 · session 2026-06-09 observation
CrowdStrike deleted a 3-line hello-world exe as malware seconds after rustc wrote it. Large cargo outputs survive — the heuristic targets tiny isolated PEs. Avoid throwaway standalone exes. Distinct from WIN-35's CyberArk vector: this one *deletes* the file, CyberArk *denies execution*.

### WIN-19 — .cmd preferred over .exe forced a cmd.exe shim
`lsp` · **fixed** · 2026-06-12 · `vdi-windows 9cba50cb`, `368aa9df`
Which re-opened WIN-1's EDR grandchild hazard; `.exe` is probed first now, and the logic plus five tests moved to `platform::mod` so they run on the Linux gate rather than only on Windows.

### WIN-20 — temp files leaked on the spawn-error path
`process-spawn` · **fixed** · 2026-06-12 · `docs/issues/archive/2026-06-12-windows-runcmd-tempfile-leak-spawn-error.md`
`TmpfileGuard`s were built inside the future, so an early `?` from `spawn()` orphaned the `.keep()`d files in `%TEMP%`; guards are now created before the spawn and moved in.

### WIN-21 — two ONNX backends enabled together broke the link
`build-install` · **fixed** · 2026-06-12 · `vdi-windows 9cba50cb`
`local-embed` (static) and `local-embed-dynamic` (dlopen) are mutually exclusive; enabling both handed `ort` conflicting features. A `cfg(all(...))` `compile_error!` now says so.

### WIN-22 — EDR quarantines the downloaded onnxruntime.dll
`build-install` · **mitigated** · 2026-06-12 · `docs/manual/src/configuration/embeddings-edr-windows.md`
Same quarantine-on-write vector as WIN-18, breaking semantic search on the VDI. The fix is configuration, not code: default build, `[embeddings].url` pointed at the corporate endpoint, then reindex for the dimension change.

### WIN-23 — unix-only test helpers broke the gnu warning-clean build
`test-portability` · **fixed** · 2026-06-12 · `vdi-windows 8632093b`
Seven helpers were live on unix but unused when test code compiles for windows-gnu, because every consumer is a `#[cfg(unix)]` test. Gating them unblocks a `-D warnings` gnu gate.

### WIN-24 — legibility_scan's git_head shelled out to git rev-parse
`process-spawn` · **fixed** · 2026-06-15 · `vdi-windows f22ed192`
The same unbounded-`.output()` EDR hang class as WIN-14 — the git-spawn-elimination law reached `resolve_head_sha` and not this sibling. Found by an architecture review's whole-tree spawn grep, which is the lesson: a law applied by hand leaves the call sites nobody swept.

### WIN-25 — collect_go_deps shelled out to go env GOMODCACHE
`process-spawn` · **fixed** · 2026-06-15 · `vdi-windows 13534cbd`
Same class again, and `go` has no library binding — so GOMODCACHE is re-derived purely from the environment, degrading to source-not-found rather than hanging when only `go env -w` set it.

### WIN-26 — semantic_search hard-wired to Qdrant, which the VDI cannot run
`retrieval-stack` · **fixed** · 2026-06-16 · `docs/plans/archive/2026-06-16-two-stack-retrieval-lite.md`
WIN-22's remote-embeddings fix was necessary but not sufficient without a daemon-free stack. Phases 0-4 all shipped to master; closed 2026-07-02 by a verify-open pass.

**Valid:** dated 2026-07-02

### WIN-27 — first full wine suite showed 20 pre-existing failures
`test-portability` · **open** · 2026-07-02 · `docs/issues/archive/2026-07-02-windows-gnu-wine-20-test-failures.md`
The 8-test `guide_hint` cluster was fixed 2026-07-05 and un-skipped; 12 remain skipped as emulation quirks.

**Valid:** dated 2026-07-02

### WIN-28 — nine real-Windows failures, zero product defects
`test-portability` · **fixed** · 2026-08-06 · `docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md`
Three root causes, all test-side: POSIX-shaped absolute literals that are not absolute on Windows, a mixed-separator expectation against an OS-shaped `CARGO_MANIFEST_DIR`, and a Unix-only siting assertion. CI run `31098286970` — 3283 passed, 0 failed.

### WIN-29 — windows-gnu cross job red, a duplicate of WIN-28
`ci` · **fixed** · 2026-08-06 · `docs/issues/archive/2026-08-06-windows-gnu-cross-job-red-undiagnosed.md`
Confirmed twice: byte-identical failing set to the `windows-latest` nine, and green from the same fixture fixes with no MinGW- or wine-specific change. Reopen only if the cross job fails a test `windows-latest` passes.

### WIN-30 — two Windows tests flaked on timing and turned a run red
`ci` · **mitigated** · 2026-08-07 · `docs/issues/2026-08-07-windows-ci-timing-flakes-block-the-gate.md`
`cold_start_over_budget_returns_none_but_keeps_warming` is fixed onto tokio's virtual clock (deterministic 25/25); `background_command_with_quotes_captures_output` is only mitigated — its poll no longer swallows the `Err` arm and the bound went 5s to 15s, but the intermittency was never reproduced.

### WIN-32 — run_command ran via cmd /C, so the buffer-query workflow was unrunnable
`process-spawn` · **fixed** · 2026-08-07 · `b142a514` (experiments)
Iron Law 3's own `grep`/`tail` on `@cmd_*` refs could not run on Windows. Now Git Bash `bash -c`, with the System32 `bash.exe` skipped (that is the WSL launcher, 25-170x slower via `/mnt/c`), one POSIX tokenizer so the security layer parses what the shell executes, and a kill-on-close Job Object.

### WIN-33 — MSYS path-conversion opt-out broke native git -C paths
`process-spawn` · **fixed** · 2026-08-07 · `e4b86447` (experiments)
A WIN-32 regression: `MSYS_NO_PATHCONV=1` disabled the rewriting that lets a *native* binary accept an MSYS path, so every `git -C /c/...` died. The justification was impossible — `sed`/`find` are MSYS binaries and never see the conversion. Its regression test asserts on the native side on purpose; a test driving only MSYS builtins passes either way, which is the green-check-that-cannot-fail that let it ship.

### WIN-34 — containing_root could never match a catalog path on Windows
`path-handling` · **fixed** · 2026-08-07 · `a8253b62` (experiments)
`artifact(move)`/`artifact(delete)` failed with `no managed root contains <path>` for rows `create`/`find` handled fine: the catalog stores forward-slash verbatim paths while `current_project` keeps native canonical ones, and Rust's prefix parser matches literal backslashes only. `doctor` stayed green because it polices the very convention that broke the comparison — another unfalsifiable check.

### WIN-35 — Python LSP unusable: uv trampoline blocked by CyberArk EPM
`tooling` · **open** · 2026-08-07 · session 2026-08-07 observation
`pyright` shims fail to spawn their Python child after ~35s, so `edit_code` times out on every `.py` file and Iron Law 2 is unsatisfiable for Python here. Not the WIN-18 vector — an earlier revision of the entry assumed that from the shared `os error 5`. Actionable in codescout: surface the child's stderr on init failure, since a 35s permanent failure past a 30s timeout reads as a retryable cold start.

### WIN-36 — no Git for Windows means no commands at all
`process-spawn` · **mitigated** · 2026-08-08 · `docs/issues/archive/2026-08-08-run-command-unusable-without-git-bash.md`
Since WIN-32 every Windows spawn goes through Git Bash, and the only diagnostic was the OS's bare `program not found`. Mitigated on purpose — falling back to `cmd /C` would re-open what WIN-32 closed. Second-order finding: the not-installed branch was untestable, which is why it shipped unexercised.
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
- ⚠️ **Follow-up (WIN-23) — test-code warning cluster:** the gnu *test* build shows ~7 platform-conditional warnings (unused `uri_to_path`, the `sneaky_link` symlink-test vars in `path_security`, etc.) — unix-only test helpers unused when test code compiles for windows. Harmless to the binary; would trip a gnu CI job running `cargo test`/clippy with `-D warnings`. **Cleaned in `8632093b`** — each item gated `#[cfg(unix)]` to match its (cfg-unix) consumer; the windows-gnu `cargo test` compile is now warning-clean, Linux gate unchanged (clippy `-D warnings` + 2697 lib tests pass). A `-D warnings` gnu CI gate is now unblocked.
- **Next step — automate the gnu ABI:** CI already runs `cargo test` on `windows-latest` (**MSVC**) × {default, local-embed, no-features}, but **nothing automated tests the gnu ABI shipped to the EDR/VDI**. Options: add an `ubuntu-latest` cross-compile job (mingw + `scripts/build-windows.sh build`, optionally `+ wine` for tests), or register a self-hosted/VDI gnu runner. wine validates logic, not EDR — the VDI stays the EDR-realism gate.
**2026-06-12 (cont.) — WIN-22: CrowdStrike kills local ONNX → remote embeddings.** Building the VDI with `local-embed*` downloads an **unsigned** `onnxruntime.dll`; CrowdStrike *quarantine-on-write* deletes it (the file-level WIN-18 vector), so semantic search silently drops to the SQL-`LIKE` lexical fallback. Resolution needs **no ONNX on the box**: default build (drop `local-embed*`) + `[embeddings].url` → corporate OpenAI-compatible endpoint (`RemoteEmbedder`) + **reindex** (embedding dimension changes from the 384-d local default). Runbook: [`configuration/embeddings-edr-windows.md`](../manual/src/configuration/embeddings-edr-windows.md). Constraints confirmed with user: file-quarantine (not runtime-kill), corporate embedding API reachable, semantic essential. candle (pure-Rust, no foreign DLL) stays the air-gapped fallback.
## Relationships

- Spec: `docs/superpowers/specs/2026-06-08-vdi-reliability-hardening-design.md`
- Plan: `docs/superpowers/plans/2026-06-08-vdi-reliability-hardening.md`
- Session log: `docs/trackers/archive/vdi-reliability-session-log.md`
- Bug files, all now archived (fixes verified; the label read "Active" until
  2026-08-06, when a repo-wide archive-citation repoint made the contradiction
  visible): `docs/issues/archive/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md`,
  `docs/issues/archive/2026-06-08-windows-run-command-child-process-hang.md`,
  `docs/issues/archive/2026-06-09-windows-test-suite-preexisting-failures.md`
- Archived CI-Windows bugs: `docs/issues/archive/2026-05-24-ci-windows-*.md`

## How to append

When a Windows issue is found or its status changes:

1. `artifact(action="get", id="52451519052d207c", entry_filter={...})` — confirm it is not
   already tracked.
2. `artifact(action="append_entry", id="52451519052d207c", entry_collection="issues", id_prefix="WIN", entry={area, status, summary, ref, since})` — the server assigns the next WIN-N atomically. To flip a status use `artifact(action="update_entry", entry_collection="issues", entry_id="WIN-N", fields={status: "..."})`. **Never** `artifact_augment(merge=true, params={issues:[...]})`: RFC 7396 replaces the array wholesale, which took the sibling T-N queue from 19 entries to 1 on 2026-08-16.
   Never reuse or delete a WIN-N id. When a fix lands, flip `status` in place via
   `update_entry` and cite the fixing commit (master-side SHA after cherry-pick)
   in `ref`.
3. Re-sync the "## Issue index" table above with the render_template columns.
3b. **Add a `### WIN-N — <title>` section under "## Per-issue detail".** This is not optional and
   it is not the table: `link_scan` defines an entry token *only* from a
   `## <ID> — <title>` heading, so a WIN-N that exists only as a params row and a table row
   **cannot be cited by anything** — every reference to it resolves to nothing, silently.
   Measured 2026-08-18 before this ledger was backfilled: 129 citations of WIN-N from 27 other
   files, all dead. Keep the section short; the table carries the detail. Confirm with
   `librarian(action="doctor")` — `ledger_defines_nothing` / `entry_without_definition`.
4. For a brand-new incident, also open a `docs/issues/<date>-<slug>.md` and cite
   it in `ref`.

## History

### 2026-08-18 — every WIN-N given a defining heading; params found lagging the table

All 35 entries (WIN-1…WIN-36, no WIN-31) now have a `### WIN-N — <title>` section under
§ *Per-issue detail*. Before this the ledger defined **no** WIN token anywhere, so every one of
the **129 citations of WIN-N from 27 other files** resolved to nothing. `link_scan` now
materialises **22 artifact→artifact edges** into this tracker that did not exist, from
`release-promotion-session-log.md` (32 citations), `reconnaissance-patterns.md`,
`codescout-usage-frictions.md`, an ADR, two plans, two specs and ten archived bug files.
`librarian(action="doctor")` no longer reports this ledger at all.

**The sections were written from the table, not from `params`, because the two had diverged —
and params was the stale one.** Measured while doing the backfill: `params.issues` holds 29
rows ending at WIN-29, with WIN-28 and WIN-29 both `open`; the committed table holds 35 rows
ending at WIN-36, with WIN-28 and WIN-29 `fixed` and full post-mortems, plus WIN-30, WIN-32,
WIN-33, WIN-34, WIN-35 and WIN-36 that params has never carried. Generating the sections from
params would have published two wrong statuses and silently omitted six entries.

That direction of drift is worth naming, because the tooling watches the other one.
`update_entry`'s `snapshot_stale` and `doctor`'s `snapshot_drift` both ask *"has the body kept
up with params?"*. Here the body ran ahead and params fell behind, which only `append_entry`'s
`warning` field notices, and only at the moment of an append. **Params for WIN-28…WIN-36 are
still stale — this pass deliberately did not rewrite them**, because six long summaries plus two
status flips is its own task and folding it into a heading backfill would have hidden it.

**Why the damage was invisible until 2026-08-18, verified in `resolve.rs`.** `link_scan`'s
dangling verdict is *prefix-gated*: `DefinitionIndex.known_prefixes` collects only prefixes with
at least one definition **anywhere in the corpus**, and `prefix_is_known` suppresses the dangling
report for any prefix that has none. Because this ledger defined no WIN token at all, `WIN` was
not a known prefix, so all 129 broken citations were **not counted as dangling** — they were
suppressed as if `WIN-24` were an ordinary uppercase-hyphen-number string in prose. The project's
dangling total sat at 621 before this backfill and 621 after it, unchanged, because those
citations were never in it. The only surface that ever saw this was `doctor`'s
`ledger_defines_nothing`
(`docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`).

### 2026-08-07 — WIN-30 half fixed, half mitigated; one of WIN-27's twelve unknown wine failures explained

**The budget test is fixed, and is now a stronger test than before.**
`cold_start_over_budget_returns_none_but_keeps_warming` moved to `#[tokio::test(start_paused =
true)]`. Every wait it depends on is a tokio timer — `SlowStart`'s 200 ms `delay`,
`client_within_budget`'s `timeout`, and the warm-up `sleep` — so auto-advance makes the schedule
deterministic, jumping to the 50 ms budget rather than the 200 ms cold start. The second half of
the change carries as much weight as the first: `std::time::Instant` is **not** virtualised under
`start_paused`, so leaving it would have made the 150 ms ceiling trivially true at ~0 ms elapsed —
non-flaky but vacuous. Switching to `tokio::time::Instant` turned that ceiling into an exact
statement about the schedule. Verified by mutation: tightening it to 40 ms fails with elapsed
sitting at the 50 ms budget, which proves both that the clock is virtual and that the assertion
still discriminates. Result: `finished in 0.00s`, 25/25 consecutive passes, no wall-clock
dependency left to flake.

**The background-output test is mitigated, not fixed, and the bug file's prescribed fix was
wrong.** WIN-30 was filed from CI log lines and said to replace a fixed wait with a bounded poll.
The test already polled — 50 × 100 ms. Implementing the prescription would have been a no-op
committed as a fix. The real defect was that `if let Ok(v) = out` **discarded the `Err` arm**, so a
command that never ran and one still flushing both ended the loop with the same contentless
message. The poll now keeps last stdout, last error, and an error count, and the bound went to
15 s (proportionate to a suite taking 138 s on that runner).

**That diagnostic paid for itself on its first run, and it closes a five-week-old unknown.** Run
under wine locally, the test failed with:

```
background command output not captured within 15s (read errors: 0); last stdout: "Can't
recognize 'py -c \"print('bg-ok', 2+2)\"' as an internal or external command, or batch
script.\r\n"; last error: ""
```

Three readings. **wine has no Python launcher** — the failure is environmental, so the wine
`--skip` is correct and permanent. **The background plumbing works under wine** — the buffer
captured cmd's own error text, so spawn, capture, and the `type @bg_*` read path all function
there; only the interpreter is absent. And therefore **this test leaves WIN-27's twelve-test "root
cause unknown" wine cluster** (`docs/issues/archive/2026-07-02-windows-gnu-wine-20-test-failures.md`,
whose Root cause still reads *"Unknown — under investigation"*). It was never a wine
path-handling mystery.

**The skip-list split is resolved with evidence, and left unchanged in effect.** WIN-30's Fix had
called the split — skipped on wine, gating on MSVC — "the worst option". It is in fact justified:
the two platforms genuinely differ in whether `py` exists. Skipped on wine, load-bearing on
`windows-latest`, with the reasoning recorded in `.github/workflows/ci.yml` beside the flag. That
comment also warns explicitly against the tempting next step of making the test self-skip when
`py` is missing: a probe-gated early return would pass **vacuously** on `windows-latest` if the
probe ever misfired, silently disarming the MSVC-CRT quote-mangling regression guard — the only
place the test has any value. A permanently-skipped test on a platform that cannot run it costs
nothing; a guard that quietly stops guarding costs everything it was protecting.

Gate at the time of the change: `cargo fmt --check` clean, `cargo clippy --all-targets -D
warnings` clean, **3515 tests / 0 failed**, and `scripts/build-windows.sh check --lib --tests`
green for `x86_64-pc-windows-gnu` (0 errors) so the `#[cfg(windows)]` edit is known to compile.

### 2026-08-07 — WIN-30 opened (two timing flakes turned a 15-job run red with no code defect)

Run `31151333288` on `2d824f15` came back 13/15 with two single-test failures, one per Windows
job, at the moment a 440-commit promotion was waiting on that gate. Re-running only the two
failed jobs, unchanged, gave **15/15**.

- **wine:** `lsp::budget_tests::cold_start_over_budget_returns_none_but_keeps_warming` —
  `must not wait out the cold start`. The assertion allows **100 ms of real-time slack** (a
  150 ms ceiling against a 50 ms budget) in a `#[tokio::test]` with no `start_paused`, even
  though every sleep involved is a tokio timer and virtual time would make it deterministic.
  Not on any `--skip` list, so this failure is new relative to WIN-27's baseline.
- **windows-latest (MSVC):** `background_command_with_quotes_captures_output` —
  `background command output not captured`. This one *is* in the wine job's `--skip` list, as
  one of WIN-27's twenty, but `windows-latest` does not skip it. WIN-27 named
  `validate_prune_request_gates` as "the one exception, also red on real Windows"; this makes
  it the **second**. The split is the worst of both options — exempted where it was noticed,
  load-bearing where it was not.

**WIN-29's reopen trigger technically fired, and WIN-30 deliberately does not reopen it.**
WIN-29 closed as a duplicate of WIN-28 under the condition *"Reopen only if the cross job ever
fails a test `windows-latest` passes"* — which is literally what happened, since wine failed
the budget test that MSVC passed. It is still not a cross-target defect: the test passed on an
unchanged re-run, and a wall-clock assertion with 100 ms of margin failing under emulation is
expected slowness, not a MinGW- or wine-specific code path. The genuine wine-specific signal is
narrower and worth stating on its own terms — **emulation overhead makes tight real-time
assertions unsafe on that job**, which is a property of the runner rather than of the target.
So the trigger is refined rather than honoured as written: reopen WIN-29 only if wine fails a
test MSVC passes **for a reason that survives a re-run**.

Worth recording what these failures were *not*. Three other runs the same day reported
run-level `failure` or `cancelled` while containing zero executed jobs (`steps=0`), during a
GitHub Actions major outage. These two carried `steps=12`. The `steps` count is the only field
that separates a scheduling outcome from a test outcome, and reading it is what kept three
separate wrong conclusions off the record.

### 2026-08-06 — WIN-28 + WIN-29 opened and closed same day; Windows fully green

First CI run against a non-stale `experiments` in three weeks (the remote had been
21 commits behind, so every prior verdict described code that no longer existed)
surfaced nine real-Windows MSVC failures and a red `Windows-gnu cross`. Both are
now fixed and the whole Windows matrix is green — run `31098286970` on `cd643d58`,
14 of 15 jobs green overall.

Three things worth carrying forward:

- **None of the nine was a product defect.** Two assertions were failing against
  *correct* implementations. Relaxing `derive_dead_roots`' `is_absolute()` guard to
  accept a POSIX fixture would have made a prune `WHERE` match every absolute row —
  a data-loss bug shipped to green a test. See W-3 in
  `docs/trackers/release-promotion-session-log.md`.
- **Separator bugs are unobservable on Linux.** 3488 tests passed locally before each
  of three pushes. One failure needed two CI round-trips because normalising a seeded
  path moved the mismatch downstream into the test's own comparison.
- **Per-job CI logs are reachable while a run is still in progress.**
  `gh run view --log-failed` refuses until the whole run finishes, and a stalled
  sibling blocks it indefinitely (`Windows-gnu cross` sat 41+ min on "Install MinGW +
  wine", then was cancelled by the next push). Use
  `gh api --allow-escape-sequences /repos/{o}/{r}/actions/jobs/{id}/logs`.

WIN-27's row still lists `validate_prune_request_gates` as "the one real-Windows MSVC
failure" — that predates WIN-28 and is now stale in the sense that the real-Windows
failure set was nine, not one. `validate_prune_request_gates` itself passes in run
`31098286970`.

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

### 2026-06-16 — WIN-26 opened (two-stack retrieval: daemon-free lite stack)
The VDI constraint sharpened: it can't run Docker or Qdrant, and needs remote
OpenAI embeddings only. WIN-22's "remote dense" is necessary but not
sufficient — code `semantic_search` is hard-wired to Qdrant (the in-process
sqlite-vec path was removed 2026-05-07). The fix is a daemon-free **lite**
stack: remote OpenAI dense + in-process sqlite-vec. The decisive find is that
the sqlite-vec escape hatch already ships for librarian artifacts
(`ArtifactBackend::SqliteVec`, statically-linked `vec0` → EDR-safe, no foreign
DLL — unlike the WIN-22 onnxruntime.dll), so this is generalizing one proven
pattern to code search + memory, not inventing a store. Phase 0 (dense always
OpenAI-compatible, drop `DenseProtocol::Tei`, fix the memory dense-only sparse
leak, delete the benchmark matrix scaffolding) shipped in `825c0c52`. Phases
1-3 (store trait for code, sqlite-vec impls, lite wiring) + a follow-on
server-stack feature gate are laid out in
`docs/plans/archive/2026-06-16-two-stack-retrieval-lite.md`.

### 2026-07-02 — WIN-26 closed (verify-open pass, perf-windows brainstorm)
Recon during the perf+Windows brainstorm found WIN-26 zombie-open: the plan
(`docs/plans/archive/2026-06-16-two-stack-retrieval-lite.md`) marks Phases 0-4 ALL DONE and
`git branch --contains 5c1ecfa8` includes **master** — the daemon-free lite stack
(`SqliteVecCodeStore` + sqlite memory store + dense-only embedding + lean default
build) has shipped. Row flipped open→fixed. Also noticed: this tracker carries NO
augmentation (`augmentation: null`) while the Issue-index comment + "How to append"
instruct maintenance via `artifact_augment(id="42dfdfc8b1522192")` — an id absent
from the catalog (this artifact is `52451519052d207c`). The documented
merge+entry_filter protocol is currently impossible; tracked as F-2 in
`docs/trackers/archive/perf-windows-session-log.md`. Remaining genuinely-open rows: WIN-5
(deferred), WIN-18 (open, AV out of our control).


### 2026-07-02 — WIN-27 opened (first wine full-suite baseline: 20 pre-existing failures)
The new `windows-gnu` CI job (MinGW + wine) ran its first full `cargo test --lib`
under wine and surfaced 20 failures out of 2807 — 6 in the `symbols` search-mode
cluster, 8 in `server::guide_hint_tests` (shared-setup unwrap at server.rs:2966),
and 6 assorted (`activate_populates_head_sha`, `check_index_scope_respects_gitignore`,
`validate_prune_request_gates`, `reindex_backfills_commits_table`,
`format_compact_live_renders_claude_md_as_map_shape`,
`background_command_with_quotes_captures_output`). A wine bisect at `8431a1d5`
(pre-Task-6) reproduces the symbols cluster identically, proving these pre-date
this branch's work — not regressions. `validate_prune_request_gates` is the one
exception, also red on real Windows (windows-latest MSVC). The CI wine step now
`--skip`s all 20 by name so the job stays a green gate against new regressions;
full inventory + un-skip protocol in
`docs/issues/archive/2026-07-02-windows-gnu-wine-20-test-failures.md`.
