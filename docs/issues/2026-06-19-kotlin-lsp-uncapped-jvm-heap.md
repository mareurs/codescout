---
status: investigating
opened: 2026-06-19
closed:
severity: high
owner: marius
related:
  - docs/issues/2026-06-19-mcp-server-oom-68gb.md
  - docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md
tags:
  - memory
  - oom
  - kotlin
  - lsp
  - jvm
  - stability
kind: bug
---

# BUG: codescout spawns kotlin-lsp with no `-Xmx`, so the JVM default-sizes its heap to ¼ of host RAM (~31 GiB on a 125 GiB box) and balloons toward a host OOM

## Summary
The Kotlin LSP server codescout launches via its mux is started **without a
`-Xmx` heap cap**. A JVM with no `-Xmx` defaults its maximum heap to **25 % of
physical RAM** — on host `ripper` (125 GiB) that is **~31 GiB**. Observed live: a
kotlin-lsp serving the *codescout repo itself* (a 6-file Kotlin test fixture)
held **27–35 GiB RSS** while the kotlin-lsp serving the real `work/mirela/
backend-kotlin` project used 0.55 GiB. This is a per-host-RAM-scaled
host-OOM hazard, **distinct from** the 2026-06-19 Rust-side 68 GiB OOM (that
victim was the codescout Rust process; this is its JVM child).

## Symptom (Effect)
Live `ps` snapshot during investigation (2026-06-19 ~17:30 EEST):

```
PID 3557650  RSS 27.9 GB (VSZ 38.6 GB)  kotlin-lsp --stdio   ← mux cwd = /home/marius/work/claude/codescout
PID 3433683  RSS 0.55 GB                 kotlin-lsp --stdio   ← mux cwd = /home/marius/work/mirela/backend-kotlin
```

- pid 3557650 peaked at **35.8 GB** at ~228 s of age, then GC'd down to ~27.9 GB
  and held there (3 samples over 6 s: flat at 27,923,2xx KB). ~31 GiB band.
- At the moment of the kill the host was at the OOM edge: `free -g` showed
  `Mem used 94 / free 2`, `Swap 3/3` (100 % full). Killing the JVM freed ~26 GB
  (`used` 94→68, `free` 2→28).
- The JVM **ignored SIGTERM** (still alive after 3 s, mid-GC); only `kill -9`
  reaped it.

## Reproduction
1. `git rev-parse HEAD` → `915867df` on `experiments`, host with large RAM
   (≥64 GiB so ¼-heap ≥16 GiB is conspicuous).
2. Run codescout as an MCP server with cwd at a workspace containing any `.kt`
   file (the in-repo `tests/fixtures/kotlin-library/` suffices).
3. Issue any Kotlin LSP-backed call (`symbols`/`symbol_at`/`references` on a
   `.kt`) to make the mux spawn `kotlin-lsp`.
4. Watch the spawned JVM's RSS: `ps -eo pid,rss,cmd | grep 'kotlin-lsp --stdio'`.
   It climbs toward ~¼ of host RAM during indexing rather than settling at the
   ~2 GiB the code assumes.

Confirmed at source level even without re-running (see Root cause); a passive
`Monitor` is armed to catch the next organic re-balloon (alerts when any
kotlin-lsp crosses 1 GiB RSS).

## Environment
- Host `ripper`, Arch Linux (kernel 7.0.x-zen), **125 GiB RAM**, 64 cores, 4 GiB swap.
- codescout `915867df` (`experiments`); binary `~/.cargo/bin/codescout` → `target/release/codescout`.
- kotlin-lsp launched by `codescout mux` with
  `JAVA_TOOL_OPTIONS=-Duser.home=/home/marius/.cache/codescout/kotlin-lsp-home/<hash>`
  — **no `-Xmx`**.

## Root cause
The Kotlin `LspServerConfig` env is built in
`src/lsp/servers/mod.rs:85-106`. `JAVA_TOOL_OPTIONS` is assembled as **only**
a `-Duser.home=<cache>` redirect:

```rust
let java_tool_options = match std::env::var("JAVA_TOOL_OPTIONS") {
    Ok(prev) if !prev.trim().is_empty() => format!("{prev} -Duser.home={}", analyzer_home.display()),
    _ => format!("-Duser.home={}", analyzer_home.display()),
};
```

No `-Xmx` is appended here or anywhere on the launch path. The JVM therefore
applies its default `MaxHeapSize` = 25 % of physical RAM → ~31 GiB on this host,
and grows to fill it during analysis.

`watch_memory` in `src/lsp/mux/process.rs:751-786` documents a cap that does not
exist:

```
/// Emits warn at 4 GiB and error at 8 GiB — both well above the 2 GiB JVM heap cap,
/// so any trigger indicates native memory growth (RocksDB JNI, direct buffers, etc.).
```

The "2 GiB JVM heap cap" is fictional on the production path — the only `-Xmx2g`
literal in the repo is a **test fixture string** at `src/lsp/mux/process.rs:837`
(`kotlin_home_from_env_takes_last_user_home`), used to exercise the env parser,
never passed to a real JVM. Compounding it, `watch_memory` is **log-only**: at
8 GiB it emits `error!` and keeps going — it never caps, kills, or throttles. So
the JVM runs from the 8 GiB "CRITICAL" line up to ~31 GiB unbounded, and (per the
sibling OOM bug) the SIGKILL'd log tail never flushes anyway.

## Evidence

### Source: no `-Xmx` on launch path
`grep -E "Xmx|JAVA_TOOL_OPTIONS|user\.home"` over `src/` → 12 matches. Only
occurrence of `-Xmx` is the fixture string at `src/lsp/mux/process.rs:837`.
`src/lsp/servers/mod.rs:85-106` is the sole `JAVA_TOOL_OPTIONS` builder; it sets
only `-Duser.home`.

### Live: ¼-RAM heap band, 50× the real project's LSP
See Symptom. pid 3557650 (codescout repo, 6-file fixture) = 27–35 GiB; pid
3433683 (real backend-kotlin project) = 0.55 GiB. Memory scales with host RAM,
not workload — the fingerprint of an uncapped JVM default heap.

### Kernel: this is NOT the same process as the 68 GiB OOM
The 2026-06-19 16:23 OOM victim (pid 2621226) was `task=codescout` with
`rss_anon` = 65.5 GiB — anonymous **Rust** heap. The kotlin-lsp is a separate
JVM child. Both are unbounded-memory paths in the codescout process tree; this
file tracks only the JVM-heap one.

### Live growth curve (2026-06-20, organic respawn, pid 4043528, codescout-repo workspace)
Captured by a passive monitor (emits on each >0.5 GiB RSS step). The mux
respawned on its own from an old-binary server (no deliberate trigger), then:

```
etime ~17s   rss=10.9 GB
      ~25s   rss=12.9 GB
      ~29s   rss=13.0 GB
   00:02:10  rss=22.5 GB  avail=33.6 GB
   00:02:18  rss=27.0 GB  avail=28.8 GB   <- local plateau
   00:02:50  rss=23.2 GB  avail=32.2 GB   <- GC reclaim (sawtooth)
   00:02:58  rss=24.9 GB
   00:03:22  rss=26.5 GB
   00:03:30  rss=35.0 GB  avail=21.3 GB   <- breakout past heap-only ceiling
   00:03:46  rss=35.7 GB  avail=20.5 GB   <- true plateau
```

Two conclusions, both load-bearing for the fix:
1. **Reproducible ceiling ~35.7 GB** — matches the first instance's 35.8 GB peak
   (Symptom section) to within 0.1 GB across two independent JVMs. Deterministic,
   not noise.
2. **Heap-driven with native stacked on top.** The 23→27 GB GC sawtooth proves the
   bulk is *reclaimable JVM heap* (RocksDB/JNI native memory is not GC-reclaimed).
   The breakout to 35.7 GB (> the 25%-RAM ≈31 GiB default heap ceiling) shows
   native/direct-buffer memory stacks on top of max heap — so an `-Xmx2g` cap
   collapses the heap component (the 20+ GB sawtooth band) to ≤2 GB, leaving only
   the small native residual. Control case: the real `backend-kotlin` LSP held
   0.66 GB throughout.

The balloon is harmless while the host has headroom (GC stays ahead of
allocation); it OOMs only when the host is already pressured (as at the first
kill, `free`=2 GB / swap 100%) — then GC cannot outrun allocation and the JVM
rides to the ceiling, dragging the host down (the sibling Rust-OOM scenario, but
JVM-driven).
## Hypotheses tried
1. **Hypothesis:** The 27 GB is workload-driven (large project to index).
   **Test:** compare against the kotlin-lsp serving the real Kotlin backend
   project. **Verdict:** rejected — the 6-file fixture's LSP used 27–35 GB while
   the real project's used 0.55 GB. Memory tracks host RAM, not workload.
   **Evidence:** Live snapshot.
2. **Hypothesis:** A `-Xmx2g` cap is set (per the `watch_memory` comment) and the
   growth is native (RocksDB JNI / direct buffers) above the heap.
   **Test:** grep the launch path for `-Xmx`. **Verdict:** rejected — no `-Xmx`
   on the production path; the comment's cap is fictional. **Evidence:** Source.
3. **Hypothesis:** Same root cause as the 68 GiB OOM. **Test:** read the kernel
   victim line. **Verdict:** rejected — that victim was the Rust `codescout`
   process (anon Rust heap), a different process. **Evidence:** Kernel.

## Fix

1. **DONE — explicit `-Xmx2g` appended to `java_tool_options`** in
   `src/lsp/servers/mod.rs` (Kotlin branch of `default_config`, ~line 85). Both
   match arms now end the string with ` -Xmx2g`, appended LAST so codescout's cap
   wins over any `-Xmx` inherited from the ambient `JAVA_TOOL_OPTIONS` (the JVM
   honors the final `-Xmx`). 2 GiB matches the invariant `watch_memory` already
   documents (heap ≤ 2 GiB → total RSS > 4 GiB means a genuine *native* leak).
   Implemented on `experiments`; **not yet cherry-picked to master**, **not yet
   live-verified** via `/mcp` restart.
2. **TODO (defense-in-depth) — make `watch_memory` actuate, not just log**: on the
   ERROR threshold, kill the LSP process group (the mux already holds `child_pgid`
   + `killpg` plumbing in `run`) so a future *native* leak self-bounds instead of
   riding to a host OOM. Deferred to a follow-up commit.
3. **TODO — the `watch_memory` doc comment** at `src/lsp/mux/process.rs:752` is now
   *true* (the 2 GiB cap exists), so no edit is strictly required; revisit only if
   the cap value changes.
4. Cross-ref the cgroup blast-radius cap from the sibling OOM bug (Fix 4 there).
## Tests added

`kotlin_caps_jvm_heap` in `src/lsp/servers/mod.rs` (tests module, inserted after
`kotlin_redirects_user_home_off_real_config`) — asserts the Kotlin
`LspServerConfig`'s `JAVA_TOOL_OPTIONS` contains an `-Xmx` token. Mirrors the
existing `kotlin_redirects_user_home_off_real_config` style. Full lib suite green
(2796 passed, 6 ignored); clippy `-D warnings` clean.
## Workarounds
- Export a heap cap into the environment codescout inherits, so the builder's
  `prev` branch carries it: `export JAVA_TOOL_OPTIONS="-Xmx2g"` before launching
  the MCP server (the builder appends `-Duser.home=…` after it; the JVM honors
  the explicit `-Xmx`).
- Or cap the whole server tree under a cgroup (sibling-bug Fix 4):
  `systemd-run --user --scope -p MemoryMax=20G -p MemorySwapMax=0 codescout start --debug`.
- Acute relief: `kill -9 <kotlin-lsp pid>`; the mux respawns it on next demand.

## Resume

Fix 1 is implemented on `experiments` (release binary rebuilt via `cargo rb`,
symlink intact). Remaining:
1. **Live-verify**: restart the MCP server (`/mcp`), trigger a Kotlin LSP call on a
   `.kt` (e.g. `symbols` on `tests/fixtures/kotlin-library/.../Book.kt`), and
   confirm via `ps -eo pid,rss,cmd | grep 'kotlin-lsp --stdio'` that the spawned
   JVM tops out near 2 GiB instead of ~31 GiB. The passive `Monitor` armed this
   session (alerts on any kotlin-lsp crossing 1 GiB) provides a backstop.
2. **Ship**: cherry-pick to master, rebase experiments, then flip status to
   `fixed` and set `closed:` (per template, only after live-verify + master).
3. Optionally implement Fix 2 (actuating kill in `watch_memory`).
## References
- Launch env builder: `src/lsp/servers/mod.rs:85-106`
- Memory watcher (log-only) + fictional-cap comment: `src/lsp/mux/process.rs:751-786`, comment at `:752`
- `-Xmx2g` fixture string (not production): `src/lsp/mux/process.rs:837`
- Sibling OOM (Rust-side): `docs/issues/2026-06-19-mcp-server-oom-68gb.md`
- Prior kotlin-lsp unbounded-disk bug (fixed): `docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md`
- Investigated from host `ripper`, 2026-06-19 ~17:10–17:35 EEST.
