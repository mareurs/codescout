---
status: zombie
opened: 2026-04-18
severity: critical
owner: marius
related: []
tags: ["memory-leak", "oom", "embeddings", "ptmalloc2", "phase-2"]
last_observed: 2026-04-30
---

# BUG: codescout memory leak → OOM → X session freeze

**Severity:** Critical (causes full desktop freeze requiring reboot)
**Discovered:** 2026-04-18
**Recurrence:** Confirmed on Apr 14 (3×) and Apr 18; likely earlier

---

## Summary

codescout grows to ~120 GB virtual memory over hours/days of uptime via a memory leak
(likely in the embeddings/vector index component). When the system OOMs, the kernel kills
a critical X11 process (kwin_x11 or similar), leaving Xorg running with a frozen frame and
no window manager. Appears as a hard freeze; only a reboot recovers it.

---

## Update 2026-04-30 — Apr-18 fixes were INSUFFICIENT

The streaming pipeline refactor and jemalloc switch (both shipped after 2026-04-18)
did NOT eliminate the leak. Three further OOM events recorded:

| Date | PID | total-vm | anon-rss | Source |
|------|-----|----------|----------|--------|
| 2026-04-28 12:36:54 | 1325794 | 142,089,552 kB (~142 GB) | 59,899,252 kB (~60 GB) | kernel oom-killer |
| 2026-04-29 14:29:29 |  961626 | 142,009,676 kB (~142 GB) | 66,846,988 kB (~67 GB) | kernel oom-killer |
| 2026-04-30 09:30:55 | 3726624 | (systemd-level kill) | — | `user@1000.service: Killing process` |

Pattern is consistent: codescout grows to ~142 GB virtual / 60–67 GB RSS over
hours-to-days of uptime, then gets OOM-killed. The 142 GB ceiling is suspiciously
flat across both kernel kills — likely an `RLIMIT_AS` / cgroup cap, not the true
ceiling. Real ceiling probably higher.

**Implication:** the build_index peak-overlap fix and jemalloc allocator are not
sufficient. The remaining live retained memory (60+ GB anon-rss) is from
something else, likely:

- ONNX Runtime session caches (per shape variant, never freed)
- Long-lived sqlite-vec / catalog connections holding pages
- LSP client buffers / file content caches accumulating across reindex calls
- `did_open` document state in the LSP mux not being released

Need a fresh investigation: heaptrack against a long-running instance that has
hit ≥10 GB RSS, focused on what the live allocations point to.
## Phase 1 — Systematic evidence gathering (2026-04-30)

### Process accounting at the Apr 28 12:36 OOM

From the kernel oom-killer dump:

- **31 codescout instances** alive in the `app-org.kde.konsole-*` cgroups.
- **30 of them flat** at `total_vm ≈ 1,245,000 pages = 5.0 GB` and `rss ≈ 1,500–2,500 pages = ~8 MB`. This is the jemalloc steady-state reservation for an idle process.
- **Only PID 1325794** had grown:
  - `total_vm = 35,522,388 pages = 142 GB`
  - `anon-rss = 14,976,502 pages = 60 GB`
  - `pgtables_bytes = 121,688,064 = 116 MB`

`pgtables=116 MB` covers ~30 M page-table entries → 30 M × 4 KB pages mapped → fully-populated 120 GB of address space at 4 KB granularity. **The pattern is many small allocations, not a few huge mmaps.**

After `oom_reaper` ran: anon-rss dropped to 8,956 kB. So the live working set was 60 GB but most of it was anon memory the kernel could reap once SIGKILL'd.

### Snapshot of currently running instances (2026-04-30, after the latest fixes)

| Class | Uptime | VmSize | VmRSS | VmData |
|-------|--------|--------|-------|--------|
| Long-running idle | 3–4 h | ~5.0 GB | 50–100 MB | 500–800 MB |
| Active (debug bin) | 30 min | ~5.0 GB | ~80 MB | ~520 MB |
| Background mux | 10 min | ~4.7 GB | ~28 MB | ~210 MB |

**No process is currently growing.** The leak is not time-based — the offending instance has to be doing specific work.

### Activity around the Apr 28 OOM (per-project usage.db)

The `southpole` project (largest workspace) shows 83 tool calls in the hour before the kill — predominantly `read_markdown`, `grep`, `run_command`, `list_dir`. No `index_project`, no `semantic_search`, no `rename_symbol` in that exact window.

7-day aggregate of memory-relevant tools (Apr 21–28):

| project | tool | count | max latency |
|---|---|---|---|
| code-explorer | `find_symbol` | 478 | 10.3 s |
| backend-kotlin | `find_symbol` | 207 | 8.8 s |
| code-explorer | `list_symbols` | 189 | 3.1 s |
| code-explorer | `semantic_search` | 83 | 0.1 s |
| backend-kotlin | `list_symbols` | 74 | 5.7 s |
| backend-kotlin | `index_project` | 2 | 21 ms |

Heavy LSP usage (`find_symbol` / `list_symbols` cross-project), plus `semantic_search`. `index_project` calls were no-op staleness checks (≤21 ms — full reindex would take seconds-to-minutes).

### Hypothesis ranking after Phase 1

The 60 GB live anon allocations + 30M PTEs pattern argues against:
- **OutputBuffer** — bounded LRU (max 20 entries, ≪ GB).
- **Few-large-mmaps** like a single Vec — would show as a small number of 1-GB+ mappings, not 30M small ones.

It argues *for*:
- **ONNX Runtime / fastembed model arenas** — the embed crate creates a fresh embedder inside each `index_project` call (`src/embed/index.rs:1968`, `:2138`) without going through `Agent::get_or_create_embedder`. ONNX Runtime sessions are well-known to retain per-shape inference arenas across drops; the C++ side does not free them when the Rust `Drop` runs.
- **Tree-sitter / LSP document caches** that grow per `did_open` and never free across project switches.
- **Per-project state retained on `activate_project`** when switching workspaces.

## Phase 2 plan — instrumentation before fixes

We are guessing the suspect. Need ground truth from the live process before we patch.

1. **Memory in the heartbeat log.** Extend `tracing::info!(\"heartbeat ...\")` in `src/server.rs` to include `vm_size_kb`, `vm_rss_kb`, `vm_data_kb` from `/proc/self/status`. 30s cadence already there. Cheap, always-on. Lets us see exactly when an instance starts growing and correlate against the next tool call recorded in usage.db.
2. **Heaptrack on one instance.** Run one debug instance under `heaptrack target/release/codescout start --debug` for normal usage. When it crosses 10 GB RSS, send SIGTERM and analyse the flamegraph for the culprit allocator chain.
3. **Drive-test the embedder hypothesis.** Synthetic loop: call `index_project` 50× in a row on a small project. Watch `vm_size_kb` between iterations. If it grows monotonically, the embedder/ONNX path is confirmed and the fix is to route the indexer through `Agent::get_or_create_embedder` (cache hit instead of fresh creation each time).
4. **Drive-test the LSP-document hypothesis.** Synthetic loop: open and close 1000 files via `find_symbol` / `goto_definition`. Watch `vm_size_kb`. If it grows, the LSP DocumentState fan-out is leaking `did_open` content.

Step 1 is the cheap, low-risk move. Until that ships, every OOM teaches us nothing new because we have no per-instance memory time-series.
## Phase 2 step 3 result — fastembed local is NOT the leaker (2026-04-30)

`examples/embed_leak_probe` runs `build_index(force=true)` 30× on a synthetic
10-file project with `local:AllMiniLML6V2Q`. Memory time-series:

| iter | VmSize (kB) | VmRSS (kB) | VmData (kB) | VmPeak (kB) |
|------|-------------|------------|-------------|-------------|
| baseline | 4,385,684 | 13,644 | 141,112 | 4,385,684 |
| 1 | 4,927,288 | 34,376 | 168,948 | 4,929,048 |
| 5 | 4,928,340 | 46,092 | 186,852 | 4,929,388 |
| 10 | 4,927,596 | 45,684 | 190,480 | 4,929,532 |
| 15 | 4,927,772 | 49,436 | 195,216 | 4,929,532 |
| 30 | 4,928,752 | 49,328 | 199,920 | 4,929,696 |

`VmData` grows ~31 MB across 30 iterations but the rate decays
(0.5 MB/iter early → 0.3 MB/iter late). Asymptotic settling — likely
fastembed's ONNX session reusing arenas + tree-sitter parser caches warming
up — **not the runaway leak that produces 60 GB RSS in production**.

So the embedder hypothesis is downgraded for the local path. **Open
question:** does the same probe with a remote (Ollama / OpenAI) embedder
behave differently? The remote path differs structurally (HTTP client +
JSON serialisation per batch); leak in that path could plausibly manifest
only under remote configs.

### Refocus: LSP document/state path

The Apr 28 OOM happened during heavy `find_symbol` / `list_symbols` work on
`southpole` (Kotlin) plus `code-explorer` (Rust) — totals from
`usage.db`: 478 + 207 = 685 LSP symbol calls in the week before OOM.

Each `find_symbol` may trigger `did_open` for files visited; the LSP mux
fans these out per client tag. If `DocumentState` retains content blobs
without bounded eviction, repeated visits across thousands of files would
accumulate linearly. 60 GB RSS at, say, 50 KB per stored file content =
~1.2M stored documents — plausible for a long-running session that
touches a Kotlin monorepo.

Next probe to write: `examples/lsp_document_leak_probe` — open and close
1000 files via `find_symbol`/`hover` against a real LSP, watch
`vm_data_kb`. If it grows linearly, the LSP DocumentState is the leak.
## Phase 2 — LSP-document hypothesis is also rejected by code review (2026-04-30)

Read of the LSP state structs:

- `DocumentState.files: HashMap<String, (HashSet<String>, i64)>`
  (`src/lsp/mux/protocol.rs:62`) — URI → (client tags, version). No content.
- `LspClient.open_files: StdMutex<HashMap<PathBuf, i32>>`
  (`src/lsp/client.rs:213`) — path → version int. No content.

Codescout does not store LSP file content; that lives in rust-analyzer /
kotlin-lsp's own process address space, which is invisible to codescout's
RSS. Even at 1M tracked URIs the codescout-side overhead is ~100 MB, not
60 GB. **LSP DocumentState ruled out.**

## Phase 2 step 5 result — activate_project switching does NOT leak (2026-04-30)

`examples/activate_leak_probe` creates 8 synthetic Cargo projects in
tempdirs, then round-robins `Agent::activate` across them 50 times
(400 total activations). Memory time-series:

| iter | VmSize (kB) | VmRSS (kB) | VmData (kB) |
|------|-------------|------------|-------------|
| baseline | 4,342,460 | 7,836 | 140,880 |
| 1 | 4,411,376 | 13,560 | 144,400 |
| 200 | 4,411,376 | 13,696 | 144,504 |
| 400 | 4,411,376 | 13,696 | 144,504 |

VmSize is byte-identical from iter 1 onwards. VmRSS grows 136 kB total
across 399 activations = noise floor. **Per-project state in `Agent::inner`
is correctly dropped on switch.** Hypothesis ruled out.
## What we know vs. don't know after this session

Confirmed:
- The leak is per-instance, not per-binary: 30 of 31 codescout processes
  stayed at the 5 GB jemalloc baseline at the Apr 28 OOM. Only 1 grew.
- The growth is small allocations (`pgtables_bytes=116 MB` covers ~30M PTEs
  over 120 GB of address space at 4 KB granularity), not few-large-mmaps.
- Local fastembed (`local:AllMiniLML6V2Q`) does not leak meaningfully across
  `index_project` calls (Phase 2 step 3 result above).
- LSP `DocumentState` and `LspClient.open_files` cannot account for 60 GB
  RSS — they hold IDs and counters, not content.

Open questions (need live data):
- Does the remote embedder path (Ollama / OpenAI HTTP) leak per call?
- Does repeated `activate_project` across many workspaces retain old
  per-project state in `Agent::inner`?
- Does the rmcp service hold message history?
- Some allocation pattern we haven't yet pictured.

The cheap next move is **wait for the next OOM with the heartbeat memory
instrumentation in place** (commit `ef45b6e`). When an instance grows past
its 5 GB baseline we will have a 30-second-resolution time-series of
`vm_size_kb` / `vm_rss_kb` / `vm_data_kb` and can correlate against
per-project `usage.db` to identify the offending tool sequence. Heap
profiling under heaptrack is the obvious follow-up once we know which
project / tool sequence to reproduce against.

Further speculative probes without that data will keep coming back
negative.
## Evidence

### Apr 14 — explicit kernel OOM records (ring buffer intact)

```
Out of memory: Killed process 3366897 (codescout)
  total-vm: 120,522,388 kB   anon-rss: 82,441,136 kB   oom_score_adj: 200
  cgroup: app-org.kde.konsole-6839.scope

Out of memory: Killed process 3894960 (codescout)
  total-vm: 120,522,372 kB   anon-rss: 90,554,872 kB   oom_score_adj: 200
  cgroup: app-org.kde.konsole-6839.scope

Out of memory: Killed process 4018178 (codescout)
  total-vm: 129,544,972 kB   anon-rss: 90,075,808 kB   oom_score_adj: 200
  cgroup: app-org.kde.konsole-6840.scope
```

Apr 14 kills hit codescout directly → X session survived.

### Apr 18 — freeze event (kernel ring buffer lost, reconstructed from systemd)

```
17:40:32  amdgpu: VM memory stats non-zero for ALL KDE processes simultaneously
          (kwin_x11, plasmashell, konsole ×4, dolphin ×5, kded6, ksmserver, ...)
          → entire X11 session terminated at once

17:40:34  systemd: user-1000.slice: A process killed by OOM killer
          systemd: user@1000.service: A process killed by OOM killer

17:40:37  app-org.kde.konsole-6837.scope stopped
          peak: 73.1 GB RAM, 23.7 GB swap  (uptime: 1w 1d 14h)

17:40–17:58  Xorg (root process) still running → screen frozen, no WM, no input
17:58        Manual reboot
```

### Memory growth rate

| Boot | Peak VmSize | Peak anon-RSS | Outcome |
|------|-------------|---------------|---------|
| Session start | ~5–6 GB | small | normal |
| Hours later | growing | growing | normal |
| Days later | ~120–130 GB | ~82–90 GB | OOM kill |

Fresh boot (2026-04-18 18:00): VmSize already 5.7 GB after 2h.

---

## Root Cause Hypothesis

**Investigated 2026-04-18 via full code audit.** No `mmap` usage in codescout Rust code (doc's original hypothesis was wrong). No HNSW in-memory index — sqlite-vec is fully disk-backed.

### Confirmed: `build_index` peak-memory overlap (`src/embed/index.rs:1604-1675`)

During `index_project`, three large data structures are alive simultaneously:

| Variable | Content | Size |
|---|---|---|
| `works: Vec<FileWork>` | all file chunks (original) | ~X bytes |
| `flat_texts: Vec<String>` | **clone** of all chunk content | ~X bytes |
| `batch_results: Vec<Option<Vec<Embedding>>>` | embeddings accumulating | ~Y bytes |

`flat_texts` is created by cloning all content out of `works` (L1604–1611), then iterated to spawn batch tasks (L1623–1636). After the spawn loop, `flat_texts` is no longer needed — but it was **never dropped** until `build_index` returned, staying alive throughout the entire async `join_next` loop (which can run for minutes on large projects).

**Fix applied (2026-04-18):** Added `drop(flat_texts)` after the spawn loop and `drop(file_chunk_counts)` after it's consumed into `boundaries`. Reduces peak RAM by ~1/3 for the content data during the embedding phase.

### Secondary cause: ptmalloc2 arena retention

glibc `malloc` (ptmalloc2) never returns freed arenas to the OS via `munmap` for mid-size allocations. After each large `index_project`, the peak RSS becomes the new permanent floor. Successive reindexes of a growing codebase ratchet RSS upward.

ONNX Runtime (fastembed's C++ backend) has the same behavior — its internal allocators grow per inference-session shape variant and never shrink.

### Remaining fix needed: streaming pipeline

The deeper fix is restructuring `build_index` to process N files at a time (embed + write to DB + drop), keeping peak at O(batch_size) instead of O(all_files). Also consider switching to `jemalloc` or `mimalloc` as the global allocator — both return freed memory to the OS, neutralising the ptmalloc2 retention problem entirely.
## Workarounds (apply now)

### 1. Restart codescout before memory grows

```bash
# Add a daily restart cron or systemd timer
# Current PID:
pgrep -a codescout
```

### 2. Cap virtual memory with ulimit (before starting)

```bash
ulimit -v $((12 * 1024 * 1024))   # 12 GB virtual limit
uvx codescout ...
```

### 3. Enable systemd-oomd to protect the session

systemd-oomd kills the *biggest cgroup leaf* before the kernel OOM fires — so it
would target codescout's konsole scope, not kwin_x11.

```bash
sudo systemctl enable --now systemd-oomd

# Verify user-1000.slice has oomd wired (already set to auto):
systemctl show user-1000.slice --property=ManagedOOMSwap,ManagedOOMMemoryPressure
```

### 4. Add MemoryMax to codescout's cgroup (runtime)

```bash
# Find its scope:
systemctl --user status | grep codescout
# Then cap it:
systemctl set-property <scope-name> MemoryMax=10G MemorySwapMax=2G
```

---

## Fix (investigate in code)

1. **Profile VmSize growth over time** — add a `/metrics` or periodic log line reporting
   `VmRSS`, `VmSize`, open file descriptors.

2. **Audit mmap usage** — search for `memmap2::MmapMut`, `MmapOptions`, or any
   `libc::mmap` calls. Ensure all mapped regions are unmapped when indices are dropped.

3. **Eviction / index reload** — if the semantic index is rebuilt on every `index_project`
   call without dropping the old one, each reindex leaks the prior allocation.

4. **Test**: run `valgrind --tool=massif` or `heaptrack` against a long-running codescout
   session and watch for unbounded growth in the allocator flamegraph.

---

## Secondary issue (unrelated to freeze)

AMD RX 7800 XT (amdgpu) emits DPCD link training errors (`dpcd_set_link_settings` failed)
when a DP monitor reconnects. Cosmetic on X11 (brief flicker), does not cause the freeze.
Present on every boot with a DP monitor on the AMD GPU.

```
kernel: amdgpu 0000:03:00.0: [drm] *ERROR* dpcd_set_link_settings:1122:
        core_link_write_dpcd (DP_DOWNSPREAD_CTRL) failed
```

Known RDNA3 + in-tree amdgpu bug. Workaround: use HDMI on the 7800 XT instead of DP.



## Status: zombie (2026-05-18)

No longer observed since `last_observed: 2026-04-30`. Kept open as a
zombie rather than closed because:

- The Phase 2 investigation rejected the two top hypotheses (fastembed
  local; LSP document path) without identifying a confirmed root cause.
- A latent regression could resurface with future LSP/embedding work.
- Closing as `fixed` would be misleading (no fix); closing as `wontfix`
  would be misleading (we'd fix it if it recurred).

**Re-open trigger:** any subsequent OOM that matches the Phase 1
signature — `cargo build` or codescout binary at >2 GB RSS during an
indexing-heavy session, X session freeze, or recurring `ptmalloc2`
heap-fragmentation evidence. If observed, flip `status: investigating`,
set `last_observed:` to that date, and resume from the Phase 2 step 5
result.
