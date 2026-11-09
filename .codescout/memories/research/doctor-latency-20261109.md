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
- Correction: the "about 4 s of wall that is neither user nor sys" is **not a fixed cost of doctor**. Wall minus (user+sys) ran 0.0–2.6 s across 15 runs, largest in rounds where the load average rose. The gap tracks machine load (correlation, not a proven cause). No sleep, fsync or flock was found with strace.

## Where the user CPU goes — second callgrind profile (HEAD f34fbc8b3, 41.0 G instructions; the first, before the pathspec change, was 52.2 G)

| share | what |
|---|---|
| 37.7% | `scan_undefined_entries` → `extract_memoized` → `extract` (36.3%). Inside: `scan_tokens` 19.6% (regex 17.7%, mostly the bounded backtracker), the rest is `pulldown-cmark` parsing |
| 19.2% | `scan_artifact_paths` — but 14.5% of the total is `sha2::compress256` running the SOFTWARE path (see the instrument note below), so the real share is far lower |
| rest | each other scan smaller |

- The git history walk (24.3% of the first profile) is no longer in the top rows after e9b4bb7b2.
- `extract` runs once per unique artifact body. The memo (`ExtractMemoScope`, per call) hit rate is about 2% (7,119 misses of 7,257 calls), so it cannot remove the cost. Only a cache across calls could, and that adds a staleness risk to a diagnostic tool; not tried, not recommended.

## Instrument note — callgrind and SHA-256

Valgrind does not expose the CPU's SHA instructions, so `sha2` falls back to its software `soft::compress`. This machine has `sha_ni` (AMD Ryzen Threadripper PRO 3975WX), so the real run uses the hardware path. Every share of `sha2` / hashing in a callgrind profile here is inflated. The first note's "`scan_artifact_paths` 14.9%" was this artifact and is withdrawn. Regex and parser costs are native code and are not affected.

## Regex guard in `scan_tokens`

`double_qualified_re` and `cross_repo_re` both end in `:<TOKEN>`, so they cannot match a chunk where `entry_re` finds nothing; `scan_tokens` now runs them only when `entry_re` matches. Soundness is pinned by the test `a_chunk_without_an_entry_token_matches_neither_qualified_pattern`.
- Instructions: 41,018,926,495 → 40,032,188,669 (−2.4%, deterministic).
- Wall/user: median user 4.60 s → 4.45 s, median wall 5.57 s → 5.41 s over 5 paired rounds; one round worse by 0.40 s. Inside the noise; the instruction count is the reliable number.
- Report byte-identical in all 5 rounds.

## Not established

- Seconds per share: callgrind counts instructions, not time.
- The cause of the 0–2.6 s wall-vs-CPU gap beyond its link to load. `strace -w` gave nothing usable (futex/epoll_wait totals are idle runtime threads).
- `readlink` count of the pathspec binary under `strace -c`: 671,336 in one run, against 422,753 recorded earlier. Different binary and run; not reconciled.
- Whether turning off libgit2 strict hash verification would cut the (now smaller) walk. Not tried. The option is process-global.
- The real (hardware-SHA) cost of `scan_artifact_paths`: no profile here can show it. Time it natively before touching it.
- The `pulldown-cmark` parse share inside `extract`: not split out from the regex share beyond the numbers above.

## How to reproduce

- Wall: build release binaries of each commit, copy them aside, run them interleaved (`for i in 1..5; for v in old new`), `TIMEFORMAT='%R %U %S'`, and compare medians AND per-round pairs. A single run proves nothing: the range is 6.6–10.3 s for one binary.
- Small changes (a few %): compare callgrind instruction totals instead; wall is too noisy.
- Syscalls: `strace -c -f codescout doctor`.
- CPU: `valgrind --tool=callgrind` on a release build with `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_STRIP=false`, then `callgrind_annotate --inclusive=yes` and `--tree=calling`. Never trust a SHA/hash share from it (see the instrument note).
- Build in a clean worktree at HEAD (the shared tree may hold a peer's broken edits), into a leased slot (`scripts/with-slot.sh`), never over `~/.cargo/bin/codescout`.
