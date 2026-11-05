# Gate: TMPDIR on tmpfs vs btrfs for the nextest test phase (2026-11-05)

Question: does `TMPDIR` on tmpfs (/tmp) speed up the test phase compared with TMPDIR on btrfs (/home/marius/.cache/cc-tmp)? Follow-up to research/gate-speed-20261101 ("effect of tmpfs NOT measured") and research/gate-python-beside-rust-20261105.

## Method
Default-features lane only: `cargo nextest run --workspace --no-fail-fast` through `scripts/with-slot.sh --slot ram-3` (one named RAM slot, so the build stays warm; CODESCOUT_SKIP_ONNX_TESTS=1). The time compared is nextest's `Summary [..s]`, which excludes compilation. Three pairs, order alternated. The lean lane was NOT measured. Script: session scratchpad tmpdir-exp.sh (gone with the session).

## Results (Summary seconds; all runs 8880 run, 8880 passed, 62 skipped)
| run | TMPDIR | test phase | load avg 1 min, start -> end |
|---|---|---|---|
| warm | btrfs | 69.8 | ~36 -> 44 |
| btrfs1 | btrfs | 65.7 | 22.6 -> 37.6 |
| tmpfs1 | tmpfs | 61.3 | 30.1 -> 42.2 |
| tmpfs2 | tmpfs | 63.9 | 64.8 -> 59.6 |
| btrfs2 | btrfs | 90.7 | 50.4 -> 73.0 |
| btrfs3 | btrfs | 88.8 | 57.5 -> 84.7 |
| tmpfs3 | tmpfs | 57.5 | 67.4 -> 39.5 |
- Means over the 3 pairs: btrfs 81.7 s, tmpfs 60.9 s (difference 20.8 s). Per pair: -4.3, -26.9, -31.3 s. tmpfs was faster in 3 of 3 pairs.
- Spread: btrfs 65.7 to 90.7 s (25 s), tmpfs 57.5 to 63.9 s (6.4 s).
- tmpfs use by the tests: peak 33 to 38 MB; 2 MB left over after the last run (removed by the script). /tmp had 8.9G free.

## What this does and does not show
- The DIRECTION is consistent (3 of 3). The SIZE is not established: the quietest pair (btrfs1 vs tmpfs1, load 22 to 42) differs by 4.3 s (6.6%); the two large gaps were measured at load 50 to 85, and each run's end-of-run load includes the run itself. A shared disk under load (peer sessions and Docker write to the same btrfs) is a plausible cause of the wide btrfs spread, NOT proven.
- Not measured: the lean lane, the doctest steps, the Python lane, compilation with TMPDIR on tmpfs, a quiet machine.
- If the effect applied to both test lanes (about 52 s lean + 64 s default from research/gate-speed-20261101), the saving would be roughly 8 s (6.6%) to 35 s (30%), i.e. about 2 to 9% of a 389 s gate. That range is arithmetic on the data above, not a measurement of the gate.

## Done afterwards, and what is still not done

Done after this measurement (commit 40c24b9f5): scripts/gate.sh gives the two nextest commands a private TMPDIR under /tmp, only when `findmnt -no FSTYPE -T /tmp` is tmpfs and at least 1024 MiB is free (CODESCOUT_GATE_TMPFS_BASE, CODESCOUT_GATE_TMPFS_MIN_MB; `off` disables it). The directory is removed on every exit, and a stale one older than 12 h is swept. Tests: tests/test_python_runner.py. A repeat on a quieter machine is still not done: the 2026-11-05 gate A/B run could not separate this change from cache and load noise (see research/gate-python-beside-rust-20261105).
