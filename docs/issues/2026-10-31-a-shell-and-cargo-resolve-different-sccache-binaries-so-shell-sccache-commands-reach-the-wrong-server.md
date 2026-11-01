---
id: '8a695dda7dfdf476'
kind: bug
status: fixed
title: 'BUG: a shell and cargo resolve different sccache binaries, so shell sccache commands reach the wrong server'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- sccache
- gate
- machine-env
topic: gate build pool and sccache
closed: 2026-11-01
opened: 2026-10-31
severity: medium
unverified: 'Which sccache version pairs are wire-compatible is not established: only 0.19 server with 0.15 client (and the reverse, for --show-stats) was observed to fail. The effect on the shared default port 4226 is inferred, not measured.'
---

# BUG: a shell and cargo resolve different sccache binaries, so shell sccache commands reach the wrong server

## Summary

On this machine, the word `sccache` names two binaries of different versions. A plain shell runs `/usr/bin/sccache` (0.19.0). Cargo runs `~/.cargo/bin/sccache` (0.15.0) as its `rustc-wrapper`. The two versions cannot talk to each other's server. So every `sccache` command that a session types to inspect or control the gate's cache reaches the build's server with the wrong client, and it fails or misleads.

## Class fit

This shows the IC-5 trait: the environment that a session uses to diagnose the build is not the environment that builds. The shell's `PATH` has no `~/.cargo/bin`, while the rustup proxy gives cargo's children a `PATH` where `~/.cargo/bin` wins. It differs from the other IC-5 members because no test is involved: the divergence is in which executable a bare command name resolves to, and the failure lands on the inspection tool, not on a test result.

## Symptom (Effect)

From a shell, `sccache --show-stats` against the shared server on port 4226:

```
sccache: error: failed to get stats from server
sccache: caused by: failed to get effective cache mode from server
sccache: caused by: Failed to read response header
sccache: caused by: failed to fill whole buffer
```

A server started from a shell (`SCCACHE_SERVER_PORT=4227 sccache --start-server`, so 0.19.0) made every cargo compile that used it fail in 1–2 seconds, exit 101, with:

```
sccache: error: failed to execute compile
sccache: caused by: error reading compile response from server
sccache: caused by: invalid value: integer `13`, expected variant index 0 <= i < 6
```

## Reproduction

At HEAD `8c47651c0` on `experiments`:

1. `which -a sccache` → only `/usr/bin/sccache`. `sccache --version` → `sccache 0.19.0`.
2. `~/.cargo/bin/sccache --version` → `sccache 0.15.0`.
3. During any cargo build, `for p in $(pgrep -x sccache); do readlink /proc/$p/exe; done` → `/home/marius/.cargo/bin/sccache` for the wrapper processes and for the server (PID 3315615 on 2026-10-31).
4. `sccache --show-stats` from the shell → the error above.
5. With `SCCACHE_SERVER_PORT=<free port>`, run `sccache --start-server` from the shell, then any `cargo build` with the same port → every compile fails with `invalid value: integer 13`.

## Environment

Arch Linux, kernel 7.2.8-zen1. `cargo` is `/usr/bin/cargo` → `/usr/bin/rustup` (the rustup proxy). Toolchain 1.97.1. The repo's `.cargo/config.toml` sets `rustc-wrapper = "sccache"` (a bare name, resolved through `PATH`). The shell `PATH` (as `run_command` sees it) contains no `~/.cargo/bin` entry.

## Root cause

Two installs that drift apart:

- `/usr/bin/sccache` belongs to the pacman package `sccache 0.19.0-1`. pacman upgraded it 0.16.0 → 0.17.0 on 2026-09-07, → 0.18.0 on 2026-10-02 and → 0.19.0 on 2026-10-25 (`/var/log/pacman.log`).
- `~/.cargo/bin/sccache` is `cargo install sccache` 0.15.0 (`~/.cargo/.crates.toml`), and nothing upgrades it.

`rustc-wrapper = "sccache"` is resolved through `PATH`. The shell finds only `/usr/bin/sccache`. Cargo's wrapper processes run `~/.cargo/bin/sccache`, so the shared server is 0.15.0 too.

measured 2026-10-31: `readlink /proc/<pid>/exe` on running wrapper and server processes → `~/.cargo/bin/sccache`. `which -a sccache` in the shell → `/usr/bin/sccache` only.

How cargo's children get `~/.cargo/bin` ahead of `/usr/bin` is inferred, not measured: the rustup proxy is expected to put `$CARGO_HOME/bin` on the `PATH` of the toolchain that it runs.

## Evidence

### Versions and owners (2026-10-31)

```
/usr/bin/sccache: sccache 0.19.0
/usr/bin/sccache is owned by sccache 0.19.0-1
~/.cargo/bin/sccache: sccache 0.15.0
"sccache 0.15.0 (registry+https://github.com/rust-lang/crates.io-index)" = ["sccache"]
shell PATH has ~/.cargo/bin: 0
```

### Running processes during a gate build

```
1692004 /home/marius/.cargo/bin/sccache sccache /home/marius/.rustup/toolchains/1.97.1-x86_64-unknow
1692388 /home/marius/.cargo/bin/sccache sccache /home/marius/.rustup/toolchains/1.97.1-x86_64-unknow
3315615 /home/marius/.cargo/bin/sccache /home/marius/.cargo/bin/sccache
```

### Isolated-server measurement that failed

A scratch measurement for the gate tmpfs/zram design started a 0.19.0 server on port 4227. All three gate lanes (clippy, lean, default) failed in 1–2 s with exit 101 and the `integer 13` error. After the script was changed to start the server with `~/.cargo/bin/sccache`, the same lanes ran.

## Hypotheses tried

1. **Hypothesis:** the shared server is busy or wedged. **Test:** `ss -ltnp` showed it listening on 4226, and gate builds through it succeeded. **Verdict:** rejected. **Evidence link:** § Running processes during a gate build.
2. **Hypothesis:** there are two binaries with incompatible protocols. **Test:** `which -a`, `--version` on both, `readlink /proc/<pid>/exe` on the live processes. **Verdict:** confirmed. **Evidence link:** § Versions and owners.

## Fix

Fixed 2026-11-01 with option 1 (see § Resume). This was a machine-local defect, so no repo code is required. Options:

1. Remove the stale copy (`cargo uninstall sccache`), so `sccache` names one binary everywhere. Then restart the shared server with that binary, at a moment when no gate or `with-slot.sh` run holds a slot. If the 0.15.0 server keeps running after its binary changes, new 0.19.0 clients fail against it exactly as above, so the restart is required.
2. Keep both and always call the cargo one by its full path in diagnostic commands. This is fragile, because a bare `sccache` stays wrong.
3. Optional repo guard: `scripts/slot-pool.sh` could warn when the shell's `sccache --version` differs from the wrapper that cargo runs. This would catch the drift for every session, but it costs a check on every lease.

Risk to state: a session that starts a server from a shell on the DEFAULT port 4226, when the shared server is down, would fail every build on the machine until someone stops it. That is inferred from the port-4227 measurement, not measured on 4226.

## Tests added

N/A: the defect is in this machine's installed binaries, not in repo code. If option 3 is taken, it needs a test in `tests/gate-slot.sh`.

## Workarounds

Use the binary that cargo runs: `~/.cargo/bin/sccache --show-stats`, `~/.cargo/bin/sccache --stop-server`, and so on.

## Resume

Fixed 2026-11-01 on this machine by Fix option 1. No repo commit exists, so there is no SHA or patch-id.

Steps run, with all 13 `~/.cache/codescout-gate/slot-*.lock` held by one script (every `flock -n` succeeded):

1. `~/.cargo/bin/sccache --stop-server` (exit 0; the 0.15.0 server was the only sccache process).
2. `cargo uninstall sccache` (exit 0; `~/.cargo/bin/sccache` is gone).

Measured afterwards:

- `which -a sccache` → `/usr/bin/sccache` only; `sccache --show-stats` from the shell works and reports `Version (client) 0.19.0`.
- `readlink /proc/<pid>/exe` of the new server → `/usr/bin/sccache`.
- A scratch crate built with `RUSTC_WRAPPER=sccache cargo build` finished with exit 0.

Not established: the stats of the new server showed 705 cache misses and 0 hits. The cause is inferred to be a cold cache key space after the version change, not measured. The first gate runs may be slow.

## References

- `.cargo/config.toml` — `rustc-wrapper = "sccache"`.
- `scripts/slot-pool.sh` — the gate's leased pool, which the cache serves.
- `docs/issues/2026-10-30-the-gate-wall-time-is-recorded-nowhere-and-dev-debug-info-makes-huge-test-binaries.md` — related gate cost work.
- `/var/log/pacman.log` — the upgrade history of `/usr/bin/sccache`.
