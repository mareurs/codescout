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

**Correction (2026-06-21, live-verify).** The claim "No `-Xmx` … anywhere on the launch path" / "the cap is fictional" is imprecise about the *distribution*. The kotlin-lsp launcher ships `/usr/share/kotlin/kotlin-lsp/bin/intellij-server.vmoptions` containing **`-Xmx2048m`** (2 GiB). What's true is that **codescout's own launch path** sets no `-Xmx`, *and* — critically — the distribution's vmoptions cap is **not reliably applied to codescout-spawned instances**: the §Evidence "Live growth curve" shows a **20+ GiB GC sawtooth** (23→27→23 GB) reaching ~35.7 GiB on 2026-06-20, and a multi-GB GC sawtooth can only be reclaimable *Java heap* — so that JVM genuinely ran a ~31 GiB heap *despite* the `-Xmx2048m` vmoptions being installed (since Jun 12). The likely mechanism: codescout's `--system-path` / `-Duser.home` redirect changes how the JetBrains launcher resolves its vmoptions file. **Net:** the explicit `-Xmx2g` we inject via `JAVA_TOOL_OPTIONS` (which the JVM *always* honors) is **load-bearing**, not redundant — it is the only reliable heap cap. Tracked in `docs/trackers/bug-fix-session-log.md` F-25.
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

### Live capture 2026-07-08 — heap cap holds, but the "small native residual" assumption is wrong

Three organic mux spawns against the same codescout-repo Kotlin fixture, captured
by a 5s-interval passive RSS monitor, during an unrelated live investigation
triggered by a user report of kotlin-lsp "going rogue" on the real `backend-kotlin`
project (which itself showed no anomaly — RSS stayed under 500 MB throughout, all
session).

**`jcmd` verification on two of the three** (independent of the 2026-06-21 check):
`VM.flags` confirmed `-XX:MaxHeapSize=2147483648` (exactly 2 GiB, matching `-Xmx2g`)
in effect on both. Fix 1 is not silently bypassed.

```
Capture A — pid 464910, spawned 03:36:09
  t+0s    rss=840MB
  t+5s    rss=4549MB
  t+10s   rss=9149MB
  t+15s   rss=11161MB
  ...     oscillates 13-17GB for ~40s
  t+~106s rss=29462MB avail=13926MB   <- GC.heap_info: heap used=566MB/2GiB (heap NOT the driver)
  ...     oscillates 24-27GB for ~2 min
  03:38:37 rss=31797MB avail=12998MB
  03:39:03 rss=35019MB avail=7900MB   <- avail minimum this capture
  03:39:24 rss=35582MB                <- peak, matches pre-fix 35.7GB ceiling to within 0.1GB
  ...     oscillates 24-35GB for ~2 min
  03:41:48 rss=3830MB  avail=40640MB  <- sudden full release (24GB->3.8GB in 11s)
  (stable ~3.8-4.4GB for 50 min, then idle-timeout exit at 04:37:07)

Capture B — pid 838177, spawned 04:37:12 (5s after Capture A exited)
  t+0s    rss=835MB
  ...     climbs to 23313MB by t+~56s  <- GC.heap_info: heap used=1593MB/2GiB (heap NEAR its cap this time)
  ...     oscillates 20-30GB for ~70s
  04:39:30 rss=36419MB avail=10765MB  <- new peak, exceeds Capture A's 35.6GB
  -> killed manually (SIGTERM to mux+JVM process group) at rss=37.3GB, avail=10.8GB;
     avail recovered to 45GB within 3s of the kill

Capture C — pid 955632, spawned 04:52:45 (13 min after Capture B was killed)
  t+0s    rss=3286MB
  ...     oscillates 8-30GB for ~2 min
  04:55:24 rss=36561MB avail=9816MB
  -> killed manually (same method) at rss=37.4GB, avail recovering to 31GB after
```

**Corrections to the 2026-06-20 evidence's conclusion:**

1. **The "small native residual" framing is wrong.** That entry assumed an `-Xmx2g`
   cap would collapse the ~20GB heap-driven sawtooth to "≤2GB, leaving only the
   small native residual." Live-verified counter-evidence: with the cap
   *confirmed in effect* on both jcmd-checked captures, native memory alone
   reached 35.6GB (capture A) and 37.4GB (capture B) — matching or *exceeding*
   the entire pre-fix ceiling. Fix 1 changed which component balloons (heap ->
   native); it did not lower the worst-case host-memory exposure for this
   workload.
2. **Heap pressure is not a precondition.** Capture A's heap sat at 566MB/2GiB
   (28% full) while RSS hit 35.6GB; capture B's heap was genuinely near its cap
   (1.59GB/2GiB, 80% full) while RSS hit 37.4GB. The native growth happens
   independent of whether the heap itself is under pressure — ruling out
   "heap cap forces overflow into native" as the mechanism.
3. **Not monotonic — volatile climb/partial-release, not always self-resolving.**
   Capture A self-released dramatically (24GB->3.8GB in 11s, consistent with a
   completed indexing burst freeing mmap'd/off-heap buffers) without
   intervention. Captures B and C did NOT self-release before crossing the
   avail<15GiB danger band and were killed manually — the same intervention
   Fix 2 (below) proposes to automate.
4. **`NativeMemoryTracking` was not enabled** (`jcmd VM.native_memory summary`
   → "Native memory tracking is not enabled"), so the exact native category
   (RocksDB block cache / JNI direct buffers / JIT code cache / mmap'd analyzer
   index) could not be isolated this session. Recommend adding
   `-XX:NativeMemoryTracking=summary` to the mux's `JAVA_TOOL_OPTIONS` so the
   next occurrence can be diagnosed precisely without a restart.
5. **Reproducibility held**: 3/3 organic spawns against the same codescout-repo
   fixture project within ~90 minutes; 2/3 required manual intervention. The
   real `backend-kotlin` project's own concurrent kotlin-lsp instances (checked
   throughout) stayed under 500MB the entire session — consistent with the
   2026-06-20 entry's "50x the real project's LSP" observation, now confirmed
   to persist with Fix 1 applied.
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
2. **DONE (2026-07-10) — `watch_memory` now actuates.** On a kill verdict it
   `killpg`s the LSP process group (SIGTERM → 500ms → SIGKILL, via the shared
   `kill_process_group` helper; PGID == PID from `process_group(0)`). The verdict
   comes from a pure, unit-tested `classify_memory` (`src/lsp/mux/process.rs`)
   with two kill arms: an absolute rss+swap ceiling
   (`CODESCOUT_LSP_KILL_RSS_CEIL_MB`, default 24 GiB) that fires regardless of
   host RAM, and a host-`MemAvailable` floor (`CODESCOUT_LSP_KILL_AVAIL_FLOOR_MB`,
   default 15 GiB) gated on the process being large (≥ 8 GiB) so an innocent small
   LSP is spared under unrelated host pressure. `CODESCOUT_LSP_MEM_KILL_DISABLE=1`
   reverts to log-only. Added a `read_mem_available_kb` (`/proc/meminfo`) reader.
   On `experiments`; **not yet on master**.
3. **DONE — the `watch_memory` doc comment** was rewritten to describe the new
   kill actuation and its env knobs (it is no longer "log-only").
4. **Blast-radius cap (moved out).** The cgroup `MemoryMax`/`MemorySwapMax=0` blast-radius cap is now tracked in `docs/issues/2026-07-10-oom-blast-radius-cgroup-cap.md`. The sibling 68 GiB OOM bug was fixed and archived to `docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md`.
**Update (2026-06-21).** Fix 1 is **committed** as `3adb66e7` `fix(lsp): cap kotlin-lsp JVM heap with -Xmx2g` on `experiments` (code + the `kotlin_caps_jvm_heap` regression test), and **live-verified**: after `cargo rb` + `/mcp`, the codescout-repo kotlin-lsp JVM (PID 4100626, carrying our `-Xmx2g`) reports `jcmd … VM.flags` → `-XX:MaxHeapSize=2147483648` (exactly 2 GiB). Per the §Root cause correction, this is the *reliable* cap (the distribution's vmoptions `-Xmx2048m` is not dependably applied to our instances). **Still TODO:** Fix 2 (`watch_memory` actuation) remains the real defense for *native* (off-heap) growth, which `-Xmx` does not bound — the capped JVM's RSS is 4.16 GiB = 2 GiB heap + ~2 GiB native.
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


## Upstream status (researched 2026-07-08)

Confirmed via GitHub API + release notes: **`Kotlin/kotlin-lsp`** is JetBrains' official
LSP (binary name `intellij-server`), versioned by IntelliJ platform build number, not
semver. Installed: `LS-262.7569.0` (2026-06-09). Latest: `262.8190.0` (2026-06-19) — one
release behind.

**Version history relevant to memory:**
- `v262.2310.0` — "various memory leaks were fixed" (changelog, no issue # given)
- `v262.4739.0` — migrated index storage to **RocksDB** ("more robust state management
  and better performance") — the same RocksDB this doc and the sibling disk-escape bug
  already implicate as the likely native-memory source
- `v262.7569.0` (our version) — fixed "the regression introduced by the RocksDB migration"
  affecting completion/auto-import speed (a partial RocksDB-regression fix, not memory-focused)
- `v262.8190.0` (latest, NOT yet installed) — fixes upstream **#213**: nested projects
  imported too eagerly, causing "huge workspace caches"

**Three upstream issues, none solved by the maintainers:**

| # | Title | Status | Relevance |
|---|---|---|---|
| [#441](https://github.com/Kotlin/kotlin-lsp/issues/441) | Persistent heap exhaustion on medium/large repos | open | Reporters say raising `-Xmx` and tuning GC does **not** help — matches our heap-capped-but-RSS-unbounded finding |
| [#205](https://github.com/Kotlin/kotlin-lsp/issues/205) | Failed init spawns new JVMs, old ones never die | open, no fix | 34+ orphaned JVMs @ 2.5-3.2GB RSS each observed by reporter. Different mechanism (many small zombies vs. one ballooning process) but same "unbounded JVM footprint" family. Only workaround: disable the extension entirely |
| [#203](https://github.com/Kotlin/kotlin-lsp/issues/203) | LSP always queries all files in workspace root, ignoring `contentRoots` scoping | **open, zero maintainer response** | Reporter's OOM originates in indexing/VFS internals (`RefreshSession`, `IndexingImplKt`) while scanning files explicitly excluded by their `contentRoots` config (including unrelated `.git/objects/pack` files). **Likely our exact mechanism**: codescout's own launch path (`src/lsp/servers/mod.rs::default_config`) passes `--system-path` (controls index *storage* location) but no content-root scoping — kotlin-lsp's workspace root for the codescout-repo instance is the full monorepo (93,873 files across 8 languages), not just `tests/fixtures/kotlin-library`. If #203 holds, even adding scoping config may not be honored upstream. |

**Why this project (vs. `backend-kotlin`) is the worst offender:** file counts are
actually comparable (codescout: 93,873 files; backend-kotlin: 90,356) — codescout's
kotlin-lsp instance isn't ballooning because its *fixture* is unusually large, it's
because its *workspace root* (the whole monorepo, passed as `--cwd`) is a
multi-language repo where Kotlin is a small minority, and (per #203) kotlin-lsp
indexes the whole root regardless of what's actually being queried. `backend-kotlin`
avoids this because its workspace root genuinely *is* the Kotlin project.

**No upstream fix or workaround exists for the core mechanism (#203).** The only
actionable upstream item is the version bump to `262.8190.0` for the #213 cache fix,
which is unrelated to #203 but cheap and low-risk to pick up regardless.

**Update 2026-07-08 (same session):** upgraded via `yay -S kotlin-lsp-bin --noconfirm`
(AUR package `kotlin-lsp-bin`, host `ripper`). `kotlin-lsp --version` now reports
`LS-262.8190.0`, confirmed via `pacman -Qi`. This is a system-wide binary shared by
every codescout instance on the host — existing running kotlin-lsp processes keep
their old on-disk binary (already-open inode), new spawns pick up 262.8190.0
going forward. Does not address #203; still watching whether the codescout-repo
fixture's ballooning recurs post-upgrade (the mux respawns kotlin-lsp on demand,
so the next organic spawn will be the first live test of this version).

**First live test, post-upgrade (2026-07-08 05:29-07:37):** an organic mux respawn
(triggered by a codescout server rebuild + `/mcp` reconnect) spawned pid 1260818
against the codescout-repo workspace, confirmed via `/proc/1260818/exe` to be running
the new `262.8190.0` binary. It ran for **2h07m** and exited at 505 MB RSS —
essentially unchanged from its 504 MB startup baseline, never ballooned. Contrast
with all three pre-upgrade captures (this doc's "Live capture 2026-07-08" evidence),
which started climbing within 5-20s of spawn and reached 20-37 GB. One clean sample
is not proof #213's fix resolves this doc's symptom (or that #203 doesn't still
apply under different indexing triggers), but it's the first non-ballooning
codescout-repo kotlin-lsp lifecycle observed all session. Continue monitoring
further organic spawns before downgrading this bug's severity or status.
## Resume

Fix 1 **committed** (`3adb66e7`, `experiments`) and **live-verified twice more** (2026-07-08: `jcmd` MaxHeapSize = 2 GiB confirmed in effect on two independent codescout-repo JVMs) — the heap cap itself is not bypassed. However, the 2026-07-08 live captures (see Evidence) prove Fix 1 alone does **not** bound worst-case host-memory exposure: native memory independently reached 35.6-37.4 GB on 2 of 3 captures, matching or exceeding the pre-fix ceiling. Remaining:

1. **Ship to master** — cherry-pick `3adb66e7` (+ this doc's corrections) to `master`, rebase `experiments`, then flip status to `fixed` / set `closed:`. Given the 2026-07-08 findings, holding `status: fixed` until Fix 2 lands is correct — Fix 1 alone does not resolve the host-OOM risk this bug is titled for. **Gated:** the full `cargo test` on `experiments` currently has one *orthogonal* failure (`replace_symbol_surfaces_stale_error_after_max_retries`, an F-18/F-23-class kotlin-lsp range issue unrelated to this fix — see session-log F-26); resolve or explicitly accept that before the protected-branch cherry-pick.
2. **Fix 2 — DONE on `experiments` (2026-07-10).** `watch_memory` now kills the LSP process group on a threshold cross (absolute rss+swap ceiling 24 GiB, or host `MemAvailable` < 15 GiB while the process is ≥ 8 GiB), via the shared `kill_process_group` helper. Env-tunable (`CODESCOUT_LSP_KILL_RSS_CEIL_MB`, `CODESCOUT_LSP_KILL_AVAIL_FLOOR_MB`, `CODESCOUT_LSP_MEM_KILL_DISABLE`). Tests: `classify_memory_*` threshold table + `read_mem_available_kb_smoke`; killpg mechanics stay covered by `process_group_reaping_tests`. **Known limitation:** a mem-kill is a *mid-life* kill and does **not** count toward the LSP circuit breaker (`src/lsp/manager.rs` counts only startup failures), so a kill→respawn→grow→kill *slow* loop is possible (period ≈ cold-start + minutes of native growth) — bounded (host survives) but unthrottled. Follow-up: a per-workspace last-mem-kill timestamp in `LspManager` applying backoff before respawn. **Still needs:** cherry-pick to master (gated on the orthogonal F-26 failure per item 1) + live `/mcp` verification.
3. **Add `-XX:NativeMemoryTracking=summary`** to the mux's `JAVA_TOOL_OPTIONS` so the next occurrence can be diagnosed with an exact native-memory category breakdown (RocksDB vs JNI direct buffers vs JIT code cache) instead of inferring from the heap/RSS gap.
4. Cross-ref the cgroup blast-radius cap — now tracked in `docs/issues/2026-07-10-oom-blast-radius-cgroup-cap.md` (the sibling 68 GiB OOM bug is fixed + archived).
5. **Bump kotlin-lsp to `262.8190.0`** (see Upstream status) — picks up upstream #213's workspace-cache fix. Cheap, low-risk, independent of Fix 2/3.
6. **Investigate content-root scoping** for the codescout-repo kotlin-lsp launch (restrict indexing to `tests/fixtures/kotlin-library` instead of the full monorepo `--cwd`) — per upstream #203 (open, unfixed, no maintainer response), scoping config may not be honored; verify empirically before relying on it as a fix.
## References
- Launch env builder: `src/lsp/servers/mod.rs:85-106`
- Memory watcher (log-only) + fictional-cap comment: `src/lsp/mux/process.rs:751-786`, comment at `:752`
- `-Xmx2g` fixture string (not production): `src/lsp/mux/process.rs:837`
- Sibling OOM (Rust-side): `docs/issues/2026-06-19-mcp-server-oom-68gb.md`
- Prior kotlin-lsp unbounded-disk bug (fixed): `docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md`
- Investigated from host `ripper`, 2026-06-19 ~17:10–17:35 EEST.
