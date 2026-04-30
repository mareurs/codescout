# Heartbeat memory fields

> ⚠ Experimental — may change without notice.

When codescout runs with `--debug`, the periodic heartbeat tracing event now
includes per-instance memory snapshots taken from `/proc/self/status`:

```text
heartbeat instance=6719 uptime_secs=30 active_projects=1 lsp_servers=[]
  vm_size_kb=4911904 vm_rss_kb=47868 vm_data_kb=462344 vm_peak_kb=4912028
```

| Field | Source | What it tells you |
|-------|--------|--------------------|
| `vm_size_kb` | `VmSize` | Total virtual address space (includes jemalloc reservations). |
| `vm_rss_kb` | `VmRSS` | Resident pages — the truthful "how much physical memory is this using right now". |
| `vm_data_kb` | `VmData` | Data + heap + stack pages. Grows with live allocations. |
| `vm_peak_kb` | `VmPeak` | High-water mark for `VmSize` over the process lifetime. |

## When to look

Useful when correlating long-running instance behaviour against per-project
tool call history (`~/.codescout/usage.db` and `<project>/.codescout/usage.db`).
The use case driving this addition is the open OOM investigation in
`docs/issues/memory-leak-x-session-freeze.md`: the kernel's OOM dump only
captures the moment of death; the heartbeat captures the trajectory.

## Platform notes

Linux only. On macOS/Windows the underlying `/proc/self/status` read fails
silently and all four fields log as `0`; heartbeat continues regardless.

## Cost

A `read_to_string("/proc/self/status")` plus a line-by-line parse. Runs once
every 30 seconds. Negligible.
