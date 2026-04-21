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
