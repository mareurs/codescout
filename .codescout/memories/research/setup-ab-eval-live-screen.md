# Setup A/B study — Stage 4b: Live Rust bare-arm screen (finished 2026-11-02)

Branch `research/setup-ab-eval-20261027` was MERGED into `experiments` (merge commit be464f836, pushed to origin/experiments by the operator on 2026-11-02); the branch and its worktree `.worktrees/setup-ab-eval` were removed on 2026-11-02. The tools now live at `tools/setup-ab-eval/` in the main checkout: run `freeze_screen.py check` and any restart from there (screen-registration.json is tracked there; its command line in GO.template.json still names the old worktree path, so substitute the main-checkout path). Range e30119f88..4cabbac8a; registration commit 4cabbac8a. Evidence: `/home/marius/work/claude/setup-ab-data/live-screen/` (rows.jsonl, batch.jsonl, runs/, GO.json, screen-plan.json), builds under `.../live-tasks/<id>/`, SDD ledger + reports archived at `.../sdd-ledger-2026-11-01-live-screen/`. Spec + amendments: `docs/superpowers/specs/2026-11-01-setup-ab-live-screen-design.md` (read the pool v2/v3/v4 amendments and the operator runbook first).

## What ran
- Operator go: "write GO.json yourself and start the screen" (2026-11-01). Controller wrote GO.json (plan bdf4c33ec80d, registration 466a57feea7b, cap 50 / per-call 5 / reserve 5), token file `/home/marius/.config/s5/token` (the ~/.claude account).
- 24 bare runs (sonnet-5-5 and opus-5-5, 2 reps, 6 tasks), all scored, stop "plan complete", spent $10.95. Screen rows are CALIBRATION only: a comparison needs a fresh registration and fresh runs (cs / full arms were not built for live tasks).
- Pool v4 (final): obscura-412, -477, -414, -447, -390 (h4ckf0r0day__) + rust-lang__rust-analyzer-22789. Dropped before any model run: polars-28484 (grades OOM-killed at the frozen 8 GB limit), gitbutler-14920 (3 expected tests fail as uid 1000, control ran as root; one cause not isolated), rust-sdk-1021 (all 9 F2P exist only in the gold patch).

## Results (calibration; pass counts per 2 reps)
| task | sonnet | opus |
|---|---|---|
| rust-analyzer-22789 | 1/2 | 2/2 |
| obscura-412 | 2/2 | 2/2 |
| obscura-477 | 0/2 | 0/2 |
| obscura-414 | 2/2 | 2/2 |
| obscura-447 | 2/2 | 2/2 |
| obscura-390 | 2/2 | 2/2 |
19 of 24 pass. Cost per run: sonnet $0.04–0.44, opus $0.18–1.99 (total $10.95). Ceiling again on 5 of 6 tasks; obscura-477 fails for all 4 runs on the single F2P `dynamic_script_onload_fires$dynamic_external_scripts_execute_and_fire_load` (gold passes it in the same grader). Not investigated whether 477 is truly hard or underspecified; opus 477 rows show numTurns 1 with 2.5–3M input tokens and 542 s / 1460 s wall (odd; check how numTurns is computed before quoting).

## Methodology facts to disclose in any write-up
- Relaxed gate and pools were chosen AFTER seeing the controls; pool reductions (polars, gitbutler, sdk) and the F2P presence rule (F2P kept iff in the control base log or named as a whole word in the test patch's added lines; obscura-447 kept 2 of 3) were decided after build/grade attempts and BEFORE any model run.
- Leak gate: needles (shingles only) baselined against the tool layer `/opt/sab` (6 shingles dropped for obscura-390); Dockerfile scrubs ignored/untracked non-target files (rust-analyzer: test-output.log) and unreadable /tmp debris; FIFOs skipped and recorded.
- rust-analyzer-22789 keeps `* -text` for 3 CRLF blobs; run.py's patch capture strips CRs, so an agent edit to crates/hir-def/src/nameres/tests/imports.rs, crates/proc-macro-srv-cli/README.md or crates/proc-macro-srv/src/bridge.rs grades "patch does not apply" = grader error = infra-other = batch STOP + latch (not a single lost run). Did not occur.
- run.py does NOT re-verify a screen registration: run `python3 -B freeze_screen.py check` (tools/setup-ab-eval, same SAB_DATA) right before GO.json and before every restart.
- Grading as uid 1000, 8 GB, --cap-drop ALL; control had run as root.

## Open items
- Operator: merge decision for the branch; rotate both token files (`/home/marius/.config/s5/token`, `token-claude-stef`) and the other exposed credentials listed in memory `research/setup-ab-eval`.
- Deferred minors: see the archived ledger (progress.md).
- Disk: `sab-*` images of the six pool tasks (run + arm ~20–45 GB each) stay for possible re-runs; images of dropped tasks (polars, gitbutler, sdk) and the sab-test-export-* fixtures were removed; `sab-armpre-cs-p18` and Stage 3 images kept. btrfs + containerd: `df` lags image removal by up to hours.
- Next steps if the study continues: investigate obscura-477 (hard vs underspecified); a comparison stage would need cs/full arms for live tasks, a Qdrant template per task and fresh registration.
