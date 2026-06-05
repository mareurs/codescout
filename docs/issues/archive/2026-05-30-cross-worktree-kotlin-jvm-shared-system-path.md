---
status: fixed
opened: 2026-05-30
closed: 2026-05-30
severity: medium
owner: marius
related: [2026-05-30-shared-server-global-active-project-race]
tags: [concurrency, kotlin, lsp, gradle, memory]
kind: bug
---

# BUG: different worktrees of one repo spawn N kotlin-lsp JVMs sharing one unguarded IntelliJ system-path + Gradle home

## Summary
The kotlin LSP mux socket is keyed on the workspace **path** hash, so each git worktree of
the same repo gets its **own** mux → its **own** kotlin-lsp JVM. The mux's whole purpose
(one JVM per project) is defeated across worktrees. Worse, every JVM is launched with the
same **fixed** `--system-path=/tmp/codescout-mux-kotlin-lsp` and
`GRADLE_USER_HOME=/tmp/codescout-mux-gradle`, so N JVMs read/write **one** IntelliJ
`system/{index,log}` dir and **one** Gradle home with no lock guarding the shared state.
No crash, no error logged — it's silent shared state. The first symptom that actually bites
is **RAM**: N × ~2 GiB JVMs.

## Symptom (Effect)
3 separate `codescout start --project <worktree> --debug` instances (plus 3 pre-existing
muxes) → **6 kotlin-lsp JVMs alive simultaneously**:
```
pid=1163480 rss=433MB   pid=1189478 rss=472MB   pid=1189684 rss=2101MB
pid=1189920 rss=1228MB  pid=1190478 rss=1355MB  pid=1190550 rss=1400MB   # ≈ 7 GiB
```
Six distinct kotlin mux sockets, one per worktree path, all spawning kotlin-lsp with the
**same** system-path and GRADLE_USER_HOME:
```
mux --cwd .../codescout                 sock=...7e868829...  GRADLE_USER_HOME=/tmp/codescout-mux-gradle -- kotlin-lsp --system-path=/tmp/codescout-mux-kotlin-lsp
mux --cwd .../backend-kotlin            sock=...26a9e85d...  (same gradle home + system-path)
mux --cwd .../weekly-pattern            sock=...c85ec91b...  (same)
mux --cwd .../cc-exp-1                  sock=...2a70f388...  (same)
mux --cwd .../cc-exp-2                  sock=...573bfc45...  (same)
mux --cwd .../cc-exp-3                  sock=...bdfd622e...  (same)
```
`lsof +D /tmp/codescout-mux-kotlin-lsp/system` → **5 distinct PIDs** hold the shared system
dir open at once. The shared `.app.lock` is 0 bytes and **not** flock-held (lsof shows no
holder) — so the JVMs never detect each other. `intellij-server.log` logs no conflict.
Only **1** Gradle daemon serves all six. RAM available dropped 13 GiB → ~1 GiB.

## Reproduction
1. `git worktree add` ≥2 worktrees of a Kotlin repo.
2. Launch one codescout instance per worktree:
   `codescout start --project <worktree> --debug` (pre-warm spawns kotlin-lsp on startup).
3. `pgrep -af 'kotlin-lsp --stdio'` → one JVM **per worktree** (no dedup).
4. `lsof +D /tmp/codescout-mux-kotlin-lsp/system` → multiple PIDs sharing one system dir.
5. `free -h` → RSS scales linearly with worktree count (~2 GiB each).

Commit: `5436d06e` (experiments).

## Environment
Linux, 30 GiB RAM, codescout 0.14.0, kotlin-lsp (JetBrains intellij-server).
Probe: `/home/marius/work/mirela/backend-kotlin` + 4 worktrees.

## Root cause
`src/lsp/mux/mod.rs:14` `workspace_hash` hashes the workspace **root path**; `:20`
`socket_path_for_workspace` builds the socket name from it. Two worktrees of one repo have
different paths → different hashes → different sockets → separate muxes → separate JVMs.
The dedup is per-path, not per-repo.

`src/lsp/servers/mod.rs:62-63` hard-codes the contention surface:
```rust
let system_dir  = std::env::temp_dir().join("codescout-mux-kotlin-lsp");  // fixed, shared
let gradle_home = std::env::temp_dir().join("codescout-mux-gradle");      // fixed, shared
```
These are not parameterized by workspace, so every kotlin-lsp JVM — regardless of worktree —
points at the **same** IntelliJ system dir and Gradle home. The two keying granularities
disagree: socket per-path, system-state global. Result: N JVMs, one unguarded shared
system/index dir, one Gradle home.

## Evidence

### Process table — 6 JVMs, 6 sockets, shared paths
See Symptom. Captured via `pgrep -af 'codescout mux .* kotlin-lsp'` and
`ps -o pid,rss -C kotlin-lsp` during the 3-instance run.

### Shared system dir held by multiple PIDs
```
$ lsof +D /tmp/codescout-mux-kotlin-lsp/system | awk 'NR>1{print $2}' | sort -u | wc -l
5
$ ls /tmp/codescout-mux-kotlin-lsp/        # one .app.lock (0 bytes, not flock-held), one system/
.app.lock  system
$ pgrep -af GradleDaemon | wc -l
1
```

## Hypotheses tried
1. **Hypothesis:** worktrees of one repo share a single kotlin-lsp JVM (mux dedups them).
   **Test:** launch one instance per worktree, count `kotlin-lsp --stdio` procs.
   **Verdict:** rejected — one JVM per worktree path; no cross-worktree dedup.
   **Evidence:** process table.
2. **Hypothesis:** the shared `.app.lock` enforces single-instance (2nd JVM refuses/blocks).
   **Test:** `lsof` the `.app.lock`; check `intellij-server.log` for lock errors.
   **Verdict:** rejected — `.app.lock` is 0 bytes with no lsof holder; no conflict logged;
   JVMs coexist. It is unsynchronized shared access, not guarded contention.
   **Evidence:** shared-system-dir subsection.

## Fix

Implemented on `experiments` in **`d68ef72a`** (cherry-pick to master pending — update this to the master-side SHA after the Standard Ship Sequence, per CLAUDE.md § "After cherry-pick").

Chosen shape (user decision 2026-05-30): **system-dir per-worktree, Gradle home per-repo.**

`src/lsp/servers/mod.rs` kotlin branch:
- `--system-path` is now `codescout-mux-kotlin-lsp-<ws_hash>` where `ws_hash = crate::lsp::mux::workspace_hash(workspace_root)` — per worktree path, so distinct worktrees no longer alias one IntelliJ `system/index` dir.
- `GRADLE_USER_HOME` is now `codescout-mux-gradle-<repo_hash>` where `repo_hash = workspace_hash(detect_worktree_info(root).main_repo ∨ root)` — worktrees of one repo share a single dependency cache (no multi-GB re-download); different repos are isolated.

Keying granularity now matches the mux's own per-path socket key (`src/lsp/mux/mod.rs:20`). Same-worktree multi-instance still shares one system dir (correct — they share the JVM via the mux too).

Note on RAM: N concurrently-active worktrees still need N kotlin-lsp JVMs (different code states can't share one JVM correctly). This fix removes the *silent shared-index aliasing*, not the inherent per-worktree JVM cost.

Live end-to-end verification (rebuild + `/mcp` restart + two-worktree mux inspection showing distinct `--system-path`) pending a server reconnect.
## Tests added

`src/lsp/servers/mod.rs` `tests` module (verified red→green this session):
- `kotlin_system_path_is_per_workspace` — distinct workspace roots produce distinct `--system-path` (this is the regression for the aliasing bug; was red on the fixed-path code).
- `kotlin_system_path_is_stable_for_same_workspace` — same root → same system dir (deterministic; same-worktree multi-instance + per-path mux keep sharing one index).
- `kotlin_gradle_home_shared_across_worktrees_of_one_repo` — worktree + its main repo share `GRADLE_USER_HOME` but get distinct system dirs (linked-worktree fixture exercising `detect_worktree_info`).
## Workarounds
- For Kotlin, work in **one worktree per repo** at a time. Each extra concurrently-active
  worktree adds a ~2 GiB JVM against a shared, unguarded system dir.
- On low-RAM machines, treat concurrent Kotlin worktrees as an OOM risk — the multiplexer
  does not protect you across worktrees.

## Resume

Fixed. Remaining: (1) live verify after `/mcp` reconnect — activate two kotlin worktrees, confirm `pgrep -af 'kotlin-lsp'` shows distinct `--system-path` suffixes; (2) ship to master via Standard Ship Sequence and update the Fix-section SHA to master-side.
## References
- Related: `docs/issues/2026-05-30-shared-server-global-active-project-race.md`
- `src/lsp/mux/mod.rs:14,20` (per-path mux keying)
- `src/lsp/servers/mod.rs:62-63` (fixed shared system-path + GRADLE_USER_HOME)
- `docs/manual/src/concepts/kotlin-lsp-multiplexer.md` (§ Gradle Isolation — see R-11 gap)
- Recon: `docs/trackers/reconnaissance-patterns.md` R-11
