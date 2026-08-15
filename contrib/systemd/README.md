# Blast-radius cap for codescout MCP servers (Linux / systemd)

Sample, **opt-in** deployment hardening. Nothing here is installed or activated by
codescout — copy it in yourself if you want it.

Tracks `docs/issues/archive/2026-07-10-oom-blast-radius-cgroup-cap.md`.

## What problem this solves

The 68 GB OOM root cause is fixed (`sync_project` streams, so peak indexing memory
is O(batch) — see `docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md`). This is
about the **blast radius of a future runaway**, which is currently unbounded:

A codescout or LSP process that grows without limit can exhaust host RAM + swap and
trigger the **kernel global** OOM killer. That kills across the whole machine — in
the 2026-06-19 event it took `systemd-journald` down with it — rather than isolating
the offending process.

A cgroup memory cap converts that into a **local** kill: the runaway hits its own
`MemoryMax`, the kernel cgroup-OOM-kills it alone, and the desktop survives.

## `oom_score_adj` — reviewed, no change needed (2026-07-13)

The kernel OOM logs show codescout running with `oom_score_adj=200`, which looked
suspicious enough to be filed as its own item. It is **not codescout's doing**, and it
is **not a bug**:

```
$ cat /proc/$(pgrep -x codescout | head -1)/oom_score_adj
200
$ cat /proc/$(pgrep -x konsole   | head -1)/oom_score_adj
200
$ cat /proc/$(pgrep -u "$UID" -x systemd | head -1)/oom_score_adj
100
```

`oom_score_adj` is **inherited across fork/exec**. codescout is spawned by the MCP
client, which is a child of the terminal — cgroup
`user.slice/…/app.slice/app-org.kde.konsole-*.scope/tab(*).scope`. The desktop session
sets `+200` on `app.slice` processes deliberately, so that under memory pressure the
kernel prefers killing a *user application* over the shell, compositor, or session
manager.

That bias is **correct and desirable**. Lowering codescout's `oom_score_adj` would bias
the kernel toward killing your desktop instead. Do not "fix" it. The 2026-06-19 host
outage was caused by the unbounded 68 GB allocation, not by the kill-preference —
codescout's own `anon-rss` was the largest on the machine by far.

## The cap

Because codescout inherits the terminal's scope, it has no unit of its own to cap. Two
ways to give it one:

### Option A — a dedicated slice (recommended)

`~/.config/systemd/user/codescout.slice`:

```ini
[Unit]
Description=Resource cap for codescout MCP servers

[Slice]
# Hard ceiling. Past this the cgroup OOM-killer kills processes IN THIS SLICE ONLY.
MemoryMax=8G
# Soft ceiling: reclaim pressure starts here, before the hard kill.
MemoryHigh=6G
# No swap. Swap-thrashing a runaway is what freezes the desktop; fail fast instead.
MemorySwapMax=0
```

Then launch the MCP server inside it. In your MCP client config, wrap the command:

```jsonc
{
  "command": "systemd-run",
  "args": [
    "--user", "--quiet", "--pipe", "--collect",
    "--slice=codescout.slice",
    "--", "codescout", "serve"
  ]
}
```

`--pipe` is essential: codescout's MCP transport is stdio, so stdin/stdout must pass
through untouched. `--collect` reaps the transient unit on exit.

Verify it took:

```bash
systemctl --user status codescout.slice
cat /proc/$(pgrep -x codescout | head -1)/cgroup   # should name codescout.slice
```

### Option B — cap the terminal's scope

Coarser, and it also caps every other process in that terminal — usually not what you
want, but it needs no client config change:

```bash
systemctl --user set-property app-org.kde.konsole@.service MemoryMax=8G
```

## Picking `MemoryMax`

Size it **above** legitimate peak and **below** "the desktop is in trouble". Normal
steady-state is well under 1 GB; indexing peaks are O(batch) since the streaming fix.
`8G` on a 32 GB+ host is a generous ceiling that still fires long before the host is
threatened. On a 16 GB host, `4G` is a better starting point.

The cap is a **backstop, not a tuning knob** — if you find yourself raising it to make
normal work fit, that is a leak to investigate, not a limit to relax.

## Not covered here

The JVM-based LSPs (kotlin-language-server) had their own uncapped-heap problem and
their own native-memory watchdog work — closed 2026-08-15, archived at
`docs/issues/archive/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md`. codescout now spawns
them with `-Xmx2g` and kills a process group that exceeds the RSS ceiling. A slice cap
contains them too, but `-Xmx` is the more precise instrument.
