# Gate: the light Python lane runs beside the Rust lanes (2026-11-05)

Follow-up to research/gate-speed-20261101 (libtest -> nextest, 2007 s -> ~500 s).

## Measured
- Before (2026-11-04 evening, serial, RAM slot warm): GATE SECONDS TOTAL=501 PYTHON=136 FMT=5 CLIPPY=75 LEAN=121 DEFAULT=164. The five lane times summed to the total, so the lanes were serial (scripts/gate.sh).
- Python light lane alone (pytest over tests/test_*.py minus the 3 heavy files, same as scripts/python-tests.py), 2026-11-04: 1586 passed, 529 subtests, 119.99 s, load average 37.7 at start. The distribution is flat: slowest test 4.78 s (test_build_git_dirty, runs cargo on a fixture crate in its own target dir), top 30 tests ~36 s of 120 s. No single culprit; pytest-xdist is not installed.
- After (commit 89b7e1512, Python in the background, joined after the default lane): TOTAL=389 PYTHON=163 FMT=4 CLIPPY=75 LEAN=120 DEFAULT=190, all exits 0, 1589 Python tests passed. TOTAL now equals the sum of the Rust lanes; Python (163 s) is hidden behind them (~385 s).
- Caveat: ONE run each, not controlled. Load average 6.9 at the start of the after-run (65.8 at the end), the before-run's load was not recorded. Python took 163 s vs 136 s and DEFAULT 190 s vs 164 s, so part of the saving went to contention or load. Repeat >= 3 times before quoting the 112 s as a figure.

## A/B attempt 2026-11-05 (old gate.sh from 7938836ec vs HEAD gate.sh), PARKED by the user
Runner: scratchpad gate-ab/ab.py (not in git), both variants via `with-slot.sh --slot ram-0 bash <gate> --check`, alternating order, foreign-CPU sampler, warmup run excluded. Results in gate-ab/results.jsonl.
- Warm runs (lane CLIPPY <= 10 s = no recompile), all exit 0: old TOTAL 265 / 263 / 263 s (= sum of lanes, PYTHON ~136); new TOTAL 144 s (n=1, PYTHON 144 = the longest lane, other lanes sum 119; foreign CPU 4.57 cores, 0.57 over the 4-core validity limit I set).
- Cold runs (CLIPPY 70-76 s, recompiled): new TOTAL 335-353 s. Cause for two of them: another session leased ram-0 for worktree hints-server-slice and with-slot.sh evicted 4 and 57 compiled units; the warmup lost 177 for the same reason. Cause of the third cold run NOT established.
- Reading: when warm, old TOTAL ~ sum of lanes and new TOTAL ~ max(PYTHON, rest), about 120 s less. Only ONE warm new run, so it is an order of magnitude, not a figure. The tmpfs TMPDIR part is inside the noise (LEAN 51 vs 52-54, DEFAULT 64 vs 67-72, n=1).
- Runner finished 13:54 (15 gate runs, all exit 0). Its SUMMARY line (old n=2: 263, 263; new n=1: 335) is misleading: the one "valid" new run was a COLD run. Warm AND valid (clippy <= 10 s, foreign CPU <= 4 cores): old 263 s x2, new none. The last new run (p5, 344 s, clippy 77 s) was cold too, with 4 more units evicted by the peer's lease. The runner did not filter on cache state; a rerun should.
- Pitfall found: a named slot (--slot ram-N) must already exist as a directory. The pool had emptied slots 1-3; the runner retried "ram-3" 21 times on "no slot slot-3" and I first mislabelled that as "held". Read the log, not the retry message.
- Pitfall: the shared checkout and slot-0 are shared with other sessions, so cache state, not the change, decided most of the times.

## To resume (needs the user's word)
Frozen snapshot: `git clone --local` of HEAD into the scratchpad (own .git; gate.sh needs a git root, so `git archive` alone would fail at gate.sh line 69 — NOT yet tested), a slot of its own (create slot-1 in the RAM pool), one warmup, then >= 3 alternating pairs. Expect ~263 s old and ~144 s new if the arithmetic holds; that is an expectation, not a result.

## Why it is safe (read, not guessed)
- Python light needs neither the leased CARGO_TARGET_DIR nor the built codescout binary: of the 5 light-lane files that mention cargo or target/, 3 only quote "cargo test", 1 drives a stub gate, 1 (test_build_git_dirty.py) builds a tiny crate with its own CARGO_TARGET_DIR. The search was for those patterns only, not exhaustive.
- The default Cargo lane still finishes last among the Cargo lanes (gate-ordering.md: the shared-binary trap is about the Cargo lanes).
- An exit before the join stops the Python process by parent pid (pkill -P), tested in tests/test_python_runner.py.

## Not done / next
- Lean in parallel with default needs two leased target dirs (cli_doc.rs reads target/debug/codescout by path), one more ~10G RAM slot, and measurement under contention (swap was full). Not started.
- Clippy shares cargo's build-directory lock with the test lanes, so it also needs its own target dir to overlap.
- Open risk: test_build_git_dirty.py waits at most 60 s for cargo; contention on the package-cache lock could trip it. It passed in the after-run.
