# Setup A/B: fresh-task builder pilot (rust-analyzer PR #23096, 2026-11-04)

Context: the daily-setup arm design (docs/superpowers/specs/2026-11-04-setup-ab-daily-setup-arm-design.md, operator validated D1-D5 on 2026-11-04, "build and control" approved, cost cap soft: the run takes priority over the cap) needs fresh, multi-file Rust tasks. This pilot proves a fresh PR can become a controlled task with NO prebuilt image. No model spend; docker, git, gh, disk, time only. Evidence: `/home/marius/work/claude/setup-ab-data/fresh-pilot-2026-11-04/` (control-main.json, control-rerun8g.json, derived-*.json, gold.patch, test.patch, issue-23090.json, logs/). Scripts were throwaway in the job tmp dir (not in git). Image kept: `sab-fresh-rust-lang__rust-analyzer-23096:pilot`.

## Why a builder is needed (verified)
- `SWE-bench-Live/MultiLang` Rust split still has 171 records, newest 2026-07-29; the 68 post-cutoff ones were all controlled in Stage 4. No prebuilt image exists for Aug-Nov 2026.
- Of the 50 strict controls, 10 pass (gitbutler-14920, rust-sdk-1021, polars-28484, rust-analyzer 22789/22804/22827/22849/22855, rust-bindgen 3390/3391). The four other rust-analyzer ones are tiny (1 source file, 1-14 added lines): no use against the ceiling.
- The existing rust-analyzer image has no git remote and one synthetic commit, so upstream commits cannot be fetched into it.

## What worked (verified from the evidence)
Host partial clone 5 s; `git archive` of the base 3 s; tree swap 1 s; warm build with network 94 s; `docker commit` 202 s; control 448-483 s per state at 4 CPUs under `--network none`. Image 56.4 GB (+10.2 GB over the 43.7 GB base). Free disk 218 -> 207 GB. Derived sets: F2P 3, P2P 8,146 (expected 8,149), identical in both control runs. Freshness canary passed (new test exists and fails at base+test patch; 44 workspace crates recompiled). Strict gate: PASSED at the frozen 8 GB limit including the gold-repeat check; FAILED once at 24 GB only because the `--nocapture` output of two tests interleaved (`... 326ok.`) and the parser lost one line of `method_resolution_trait_autoderef` (it passed).

## Obstacles and fixes a general builder must carry
1. A merge commit has two parents (RA): base = first parent; base-to-merge diff matched the PR (11 files, +229 -120).
2. Leftover directories of crates absent from the new tree broke the `crates/*` glob: delete empty dirs and verify with `cargo metadata` before building.
3. The image bakes `CARGO_NET_OFFLINE=true`; override it for the build step only, grading stays offline.
4. Old mtimes make cargo run STALE binaries: touch every tracked file after the tree swap, then verify `Compiling` lines.
5. A background wrapper lost the exit code ("exit=0 seconds=0" for a failed build): capture the real code; never trust a 0-second cargo build. The controller caught this by reading the logs, not by trusting the subagent summary.
6. `--nocapture` interleaving makes the strict gold-repeat check flaky: use a tolerant parse or rerun only the mismatched tests.
7. Test names embed the compiled-binary hash, so they are tied to the image.
8. RA keeps tests inline in source files, so a path-based test/patch split is approximate (one 1-line inline test edit ended up in the gold patch; harmless here).
9. Subagents are NOT woken by background jobs they start: a subagent must poll with bounded waits, never end its turn on a job.

## Specification verdict (the important finding)
Under-specified for 2 of 3 F2P tests. Issue #23090 reports a false-positive type error for a pattern shadowing an invisible tuple-struct constructor; the behavioural F2P test fits it. `item_tree::tests::adts` and `::pub_self` assert a pretty-printer change (`struct Unit;` -> `struct Unit { ... }`) that follows from the gold patch removing `shape` from the item-tree struct and is not in the issue; a different correct fix fails them. Same class as obscura-477. RA's inline snapshot tests make this likely for many RA tasks. Needed before registration: a pre-registered F2P rule that drops F2P tests whose asserted behaviour the issue does not state (like the obscura-447 rule), with the task kept only if at least one F2P remains, decided BEFORE any model run; an LLM or manual read per task, cached on the hash of (issue, test patch, gold patch).

## Cost per task (rust-analyzer, base image reused)
About 10 GB disk and about 5 minutes build; control about 8 minutes per state, three states plus a repeat. Other repos (nushell, zellij, uv) need their own base image (20-45 GB each), so disk (207 GB free, 89 % used) is the binding limit, not money.

## Open
Builder tool not written (this was a throwaway probe). Next: decide the F2P specification rule, pick the repos and tasks, write the builder with the steps above, run controls for the validated candidates (nushell 16, rust-analyzer 6, zellij 7, uv 6 clean PRs; list in memory research/harder-task-benchmarks-20261104).
