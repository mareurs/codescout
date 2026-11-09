# librarian doctor latency — measured 2026-11-09

`librarian(action="doctor")` / `codescout doctor` takes 10–21 s per call (usage.db, 38 calls, avg 5.1 s). Measured here on a 7,294-row catalog, release build.

## What was measured

- First measurement (3 interleaved runs per binary): wall 11.5–13.6 s; user 5.3–5.5 s; sys 1.8–2.7 s.
- `readlink` calls: 1,241,564 → 422,753 after commit 2e8766861 (`managed_base` resolved a path's ancestors once per root; now once per call). Report content (violations) identical before and after; only live catalog counters differ.

## Wall time — settled (second measurement, same day)

5 rounds, each round runs old / new / pathspec back to back, release builds, 64-core machine, peer sessions running.

| binary | content | median wall | wall range | median user+sys |
|---|---|---|---|---|
| old | before 2e8766861 | 8.7 s | 6.6–10.3 s | 7.0 s |
| new | 2e8766861 (`managed_base`) | 6.7 s | 6.0–9.7 s | 6.1 s |
| pathspec | + e9b4bb7b2 (history walk asks only about citing files) | 6.0 s | 4.8–8.3 s | 5.2 s |

- new beat old in 5 of 5 rounds; pathspec beat new in 5 of 5 rounds.
- Both changes together: about 2.7 s (31%) less wall at the median. Load drift (a peer started work mid-run) widened the ranges: read the gain as roughly 2–3 s.
- Correction: the first note called the `managed_base` gain "inside the noise". Paired by round, it is not.
- Correction: the "about 4 s of wall that is neither user nor sys" is **not a fixed cost of doctor**. Wall minus (user+sys) ran 0.0–2.6 s across the 15 runs, largest in rounds where the load average rose. A later single run: 7.4 s wall, 4.5 user, 1.8 sys (gap 1.1 s) at load 2.3. The gap tracks machine load (correlation over 15 runs, not a proven cause). No sleep, fsync or flock was found with strace.

## Where the user CPU goes (callgrind, 52.2 G instructions, release + line tables, before e9b4bb7b2)

| share | what |
|---|---|
| 24.3% | libgit2 `git_diff_tree_to_tree`, 424 calls, from `paths_touched_since` (one tree diff per commit of the last 7 days: `SETTLE_DAYS = 7`, `max_commits` ceiling) — about 30 M instructions per diff |
| 15.5% | libgit2 `git_odb__hashobj` (object hash verification on read), inside the same walk |
| 27% | link-scan `extract` over every artifact body, once per artifact (memo works: 7,119 misses / 7,257 calls) |
| 14.9% | `scan_artifact_paths` (per-row checks, file reads, hashing) |
| rest | each other scan ≤ 6% |

`scan_open_bug_cited_from_source_walking` is the caller of `paths_touched_since`. e9b4bb7b2 narrows its diff to the citing files with a pathspec; user CPU fell about 15% (4 of 4 pairs, then 5 of 5). Whether libgit2 prunes subtrees by pathspec is inferred from that CPU drop, not read from libgit2.

## Not established

- Seconds per share: callgrind counts instructions, not time.
- The cause of the 0–2.6 s wall-vs-CPU gap beyond its link to load. `strace -w` gave nothing usable (futex/epoll_wait totals are idle runtime threads).
- `readlink` count of the pathspec binary under `strace -c`: 671,336 in one run, against 422,753 recorded earlier. Different binary and run; not reconciled.
- Whether turning off libgit2 strict hash verification would cut the walk without changing its result. Not tried. The option is process-global, so it also reaches every other git2 use in the server.
- Not examined: link-scan `extract` (27%) and `scan_artifact_paths` (15%).

## How to reproduce

- Wall: build release binaries of each commit, copy them aside, run them interleaved (`for i in 1..5; for v in old new pathspec`), `TIMEFORMAT='%R %U %S'`, and compare medians AND per-round pairs. A single run proves nothing: the range is 6.6–10.3 s for one binary.
- Syscalls: `strace -c -f codescout doctor`.
- CPU: `valgrind --tool=callgrind` on a release build with `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_STRIP=false`, then `callgrind_annotate --inclusive=yes` and `--tree=calling`.
- Build into a leased slot (`scripts/with-slot.sh`), never over `~/.cargo/bin/codescout`.
