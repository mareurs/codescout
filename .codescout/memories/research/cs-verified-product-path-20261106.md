# Slice 4: `[cs-verified]` product-path measurement — stopped at a floor (2026-11-06)

Pre-registration with amendments AM-1..AM-4: `docs/evals/cs-verified-product-path-preregistration.md`. Data: `docs/evals/data/2026-11-06-cs-verified-product-path/` (rows, census, analysis). Transcripts (outside git): `/home/marius/work/claude/hce-data/runs5/` and `runs5-pilot/`. Instrument: `tools/hint-channel-eval/stage5.py`, tests `tests/test_hint_channel_stage5.py` (43). Follows [[hint-channel-slices-20261105]] and [[hint-channel-authority]].

## Result
- 2 arms (A: declaration in the testbed's `CLAUDE.local.md`; B: none), the SHIPPED companion 1.21.0 hook, Sonnet 5.5, CLI 2.1.309, 5 Rust tasks, round 1 = 10 runs, USD 3.514 of 25 in total (pilot included). The operator stopped after round 1.
- 7 of 10 runs informative (A 3, B 4). In ALL 7 the agent had looked for the callers (references, or a name grep) BEFORE the edit. Lookups after the hint: 0 of 3 (A), 0 of 4 (B). The stratum "no prior lookup" is empty. p = 1.0 means nothing.
- So the registered question is NOT answerable on this task type: the `verify callers` advice arrives after the information the agent already has. This is a finding about the hint's timing, not about the declaration. Consistent with the census (937 deliveries, followed once).
- Not tested: the declaration when the agent has not scouted first; other hint classes; wrong-hint risk.

## Facts worth keeping
- The host release binary needs glibc 2.43 and does NOT start in the eval image (older glibc). Build the server inside the image: `docker run ... sab-agent-cs-cs-p16:f9b0fc9f8 cargo build --release --locked --no-default-features --features remote-embed,http,librarian` with `RUSTUP_TOOLCHAIN=1.97.1-x86_64-unknown-linux-gnu`, a writable CARGO_HOME with network. Took 2m58s of cargo time. The image's rustc is 1.97.1; the repo pinned 1.99.1 after `4098f07bb`, so the build used `6bd3745e4`.
- `advice.verified` is NOT stable per task: `callers_check.rs` has a 500 ms wall-clock budget (`CALLERS_CHECK_BUDGET`); a slow scan gives `CannotTell` = `verified:false`. Tasks 04 and 08 flipped between runs at machine load 16-33 (the timeout as the cause is consistent with the code but not shown). Stable firers: hce-01, 02, 03, 05, 07. hce-10 (visibility-only change) gives no advice. hce-03 needs the symbol `impl DoctorScope/cross_root_cites_edges` (a field has the same name).
- The hook strips `advice` from the result, so "verified:true" is proven by relocation (the `[cs-verified]` attachment keyed to the `tool_use_id`, no `"advice"` left). `verified:false` advice stays in the result by design.
- A run leaves about 1.6 GB (`testbed/`, cargo target); scoring needs only transcript, stream, run-record, outcome (about 600 KB). Prune `testbed/` after scoring.
- Agents often could not compile in this setup: they call `run_command` without the MCP prefix, or the companion `PreToolUse:Bash` hook blocks `cargo check`. Same in both arms.
- Do not set a HOME or RUSTUP_TOOLCHAIN for the server on the host without the real RUSTUP_HOME: rustup then downloads a toolchain into the fake HOME.

## Open
- The declaration stays active in the codescout checkout (`CLAUDE.local.md`, unmeasured effect). A design that tests it needs tasks where the agent does not scout callers first; new pre-registration.
- Uncommitted at the time of writing: the pre-registration, stage5.py, its test, the data directory.
