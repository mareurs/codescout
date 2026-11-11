# Setup A/B fresh tasks, batch 2 (2026-11-06): why candidates were dropped, and the builder gaps they exposed

Batch 2 builds: nushell 18704 and 19054 and obscura 1221/1259/1384 did not become pool tasks. Each has `dropped.json` or an infra note under `setup-ab-data/fresh-build/evidence/<id>/`.

- nushell-18704: dropped. 12 of 13 F2P tests are unit tests defined inside the gold patch's source files, not in the test patch (f2p_absent_at_base=12). The grader applies only the test patch, so no agent patch can pass them by name. The one reachable test asserts more than the issue states.
- nushell-19054: dropped. The gold patch changes Cargo.toml/Cargo.lock, so 20 of 74 test binary hashes differ between base and gold. Test ids embed the hash, so 8166 of 8194 F2P ids look absent at base, and an honest fix leaving Cargo files alone would never match the gold ids.
- obscura-1221: gate-failed marker cleared as an infrastructure fault (every test step failed with `failed to download ab_glyph ... --offline`; logs empty). Not retried; needs the builder to fetch all dependencies in the warm step.
- obscura-1259 and 1384: refused, the archive tree differs from the upstream tree (eol/export-ignore/export-subst). Not diagnosed.
- obscura-719: its own base tests do not compile.

Pool v1 check (done): for all nine controls, no test binary hash differs between base and gold, so id-based grading is sound there.

**Standing audit for every exit-3 task, before the readers:** (1) every F2P test is defined by the test patch or exists at base; (2) base and gold binary hashes are equal for the binaries holding kept tests; (3) no ENOSPC/Bus error/OOM/offline-download signature in the logs.

**SDD builder items (not yet built):** refuse F2P tests that only the gold patch defines; refuse a hash change between base and gold; classify ENOSPC/Bus error/OOM as infrastructure error, never `gate-failed`; fetch all dependencies in the warm step; diagnose the tree mismatch.
