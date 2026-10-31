# Setup A/B study, Stage 3 result (pilot and extension)

Date: 2026-10-31. Branch: `research/setup-ab-eval-20261027`.
Registration: `tools/setup-ab-eval/stage3-registration.json`, sha256 `b75ec48492a9743def87fbf57d327d6f0a221fe5b91635fd2b8ab537f020be49`.
Evidence: `/home/marius/work/claude/setup-ab-data/study/` (not in git). Final analysis: `study/analysis-final.json`.

## What ran

The study compares three arms on 12 registered tasks, for two models.

- Arms: `bare` (no codescout), `cs` (codescout only), `full` (codescout plus the companion setup).
- Models: `claude-sonnet-5-5` and `claude-opus-5-5`.
- Tasks: 6 foreign (public repositories) and 6 private (codescout's own fixes).
- Pilot: 6 tasks, 48 runs, all `scored`.
- Extension: 6 tasks, 48 runs, all `scored`.
- Total: 96 scored runs. No `invalid-arm`, `plan-limit`, `infra-oom`, `infra-other` or `unknown-spend` outcome. No cap hit. No OOM kill.

## Cost

- Journal total at the stop `plan complete` (`stop-224-00699db2`): $69.03 of the $200 cap. This includes Stage 1 ($4.70) and the $13.22 carried from aborted pilot attempt 1.
- Pilot: $27.62. Extension: $23.49.
- No admission stop. The registered instrument check found no failure at the study level or in any arm.
- Longest run: 2111 s (pilot). Highest memory peak: 18.9 GB of 24 GB (opus, `full`).

## Result, per stratum

The registered rule compares pass counts (non-inferiority within one task) and the cost ratio against a floor F. Each stratum has 6 valid tasks, so each stratum decides. Foreign and private results are reported separately. There is no pooled headline.
Every comparison reads "worse". The reason is cost in every case: the 90% interval of the cost ratio lies above F.

Pass counts are equal or within one task in every cell. The pass rate is at a ceiling, so cost is the only measure that moves.

| Model | Comparison | Stratum | Passes (arm vs bare) | Cost ratio (geometric mean) | 90% interval | Floor F |
|---|---|---|---|---|---|---|
| sonnet | cs / bare | foreign | 5 vs 5 | 3.80 | 2.93 to 4.86 | 1.26 |
| sonnet | cs / bare | private | 6 vs 6 | 2.42 | 2.06 to 2.82 | 1.21 |
| sonnet | full / bare | foreign | 4 vs 5 | 5.90 | 4.36 to 7.47 | 1.26 |
| sonnet | full / bare | private | 6 vs 6 | 3.11 | 2.79 to 3.41 | 1.21 |
| opus | cs / bare | foreign | 4 vs 5 | 2.40 | 1.92 to 3.06 | 1.31 |
| opus | cs / bare | private | 6 vs 6 | 2.14 | 1.87 to 2.46 | 1.20 |
| opus | full / bare | foreign | 4 vs 5 | 3.12 | 2.56 to 3.82 | 1.31 |
| opus | full / bare | private | 6 vs 6 | 2.97 | 2.66 to 3.39 | 1.20 |

The private stratum is functionally contaminated for `cs` and `full`: the frozen codescout binary post-dates every private fix. Read the foreign stratum for any claim about codescout. Both strata agree in direction.

## Secondary measures (medians over runs)

| Model | Arm | Input tokens | Turns | Codescout share of tool calls |
|---|---|---|---|---|
| sonnet | bare | 178,683 | 8 | 0 |
| sonnet | cs | 951,663 | 27.5 | 0.93 |
| sonnet | full | 1,418,722 | 33 | 0.99 |
| opus | bare | 354,103 | 16 | 0 |
| opus | cs | 1,377,420 | 30.5 | 0.94 |
| opus | full | 2,448,730 | 40.5 | 1.00 |

Every `cs` and `full` run called codescout at least once. The codescout arms read far more input tokens and took more turns, with no gain in passes at this task level.

## Incidents

- Pilot attempt 1 (registration 6ff9185c0) ran 10 of 48 runs, then stopped when `cs-p07/sonnet/full` reached the 1800 s run timeout. The attempt was archived to `study/aborted-pilot-1/` and never analysed. The $5 per-call cap was charged as unknown spend and carried. The timeout rose to 3600 s and the memory limit to 24 GB, with a re-freeze.
- The first restart after the re-freeze stopped at startup (`stop-28-57cae6ce`, no spend). The corporate VPN routes (172.16/12, 192.168/16, 10/8) blocked every Docker default subnet. The operator disconnected the VPN, acknowledged the stop and restarted.
- Two accounts: Stage 1 and the pilot used the `~/.claude` account token. The extension used the `claude-stef` token. Costs are per run, so the split does not change the analysis.
- Credentials found during the study: old values in `~/.claude/settings.json` `permissions.allow` and a key in an old plugin cache `.env`. A forked agent printed an OAuth token value from a profile `settings.json`. Both study tokens were pasted in chat. The operator must rotate all of them.

## Limitations

- Private fixes (2026-08 to 2026-10) post-date the model cutoff (June 2026). The 6 public tasks pre-date it, so memorisation is possible. It applies to all arms equally and may contribute to the pass-rate ceiling.
- Five private tasks were replaced because the binary held gold strings.
- `full` has Skill and Task but no Bash, Edit or Write.
- The `compiled all targets` secondary undercounts `cs` and `full`.
- The frozen CLI is 2.1.309 (interactive use was 2.1.312). `--verbose` may inflate cost. Default effort was `medium`.
- The reranker had no listener. The LD_PRELOAD and exec-only caveats apply. Rust-analyzer OOM risk exists only in `cs` and `full`.
- Dev-task outcomes (Stage 1) are never scored or quoted.
- Deferred minor findings are listed in the SDD ledger. One of them is that the runner's `docker network create --internal sab-ab-net` has no explicit subnet.

## Reading the result

The registered rule gives "worse" for both codescout arms, in both strata and for both models, on cost. Pass counts do not separate the arms. This study measures cost at a pass-rate ceiling on 12 tasks. It does not show that codescout lowers the pass rate, and it does not measure the harder tasks where the pass rate could differ.
