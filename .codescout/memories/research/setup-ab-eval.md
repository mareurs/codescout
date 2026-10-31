# Setup A/B eval (codescout vs bare vs full) — COMPLETE, awaiting merge decision (2026-10-31 ~12:00)

Branch `research/setup-ab-eval-20261027` in worktree `/home/marius/work/claude/codescout/.worktrees/setup-ab-eval` (from `experiments`), HEAD fafea2105, 522 Python tests OK, Rust gate green at 1cd85ae44 (no Rust change since). Spec: `docs/superpowers/specs/2026-10-27-codescout-setup-ab-design.md`; plan: `docs/superpowers/plans/2026-10-27-setup-ab-eval-stage0.md`. Code: `tools/setup-ab-eval/`. Final report: `docs/research/2026-10-31-setup-ab-stage3.md` (Stage 1 report: `docs/research/2026-10-27-setup-ab-stage1.md`). Raw evidence (not in git; keep): `/home/marius/work/claude/setup-ab-data/study/` (incl. `analysis-final.json`, `analysis-pilot.json`, `aborted-pilot-1/`). SDD ledger archived (SDD workspace deleted): `/home/marius/work/claude/setup-ab-data/sdd-ledger-2026-10-27.md` — holds every Ruling line and deferred minor. Related memory: `research/setup-ab-eval-token-files`.

## Result (registered analysis, scored runs may be quoted)
- 96 scored runs (pilot 48 + extension 48), 0 non-scored, 0 cap hits, 0 OOM, instrument check clean, no admission stop. Journal total $69.03 of the $200 cap (pilot $27.62, extension $23.49, Stage 1 $4.70, carried $13.22 incl. $5 unknown for the attempt-1 timeout).
- Every comparison (cs/bare, full/bare; sonnet and opus; foreign and private strata) reads "worse" under the registered rule, on cost: cost ratio 2.1 to 5.9, 90% interval above floor F (1.2 to 1.3). Pass counts equal or within 1 task (ceiling). Foreign and private agree; pooled is never a headline; private stratum is functionally contaminated for cs/full.
- Median input tokens bare/cs/full: sonnet 179k/952k/1.42M, opus 354k/1.38M/2.45M; codescout share of tool calls 93 to 100% in cs/full.

## Open items
1. Operator decision: merge/push of the branch (finishing-a-development-branch menu was presented; nothing merged or pushed).
2. Operator to-do: rotate BOTH exposed tokens (`~/.config/s5/token` = ~/.claude account; `token-claude-stef` = ~/.claude-stef account) and the old credentials (Google key, PGPASSWORD, login entries) in `~/.claude/settings.json` permissions.allow and old plugin caches; a profile OAuth token was also printed in a transcript. The VPN may be reconnected now.
3. Keep `sab-*` images until the operator no longer needs re-analysis; then they may be removed (never touch `codescout-*` or Codex `pbr27-*`/`rae-*`).
4. Deferred minors: see the archived ledger. Notable: the runner's `docker network create --internal sab-ab-net` has no explicit subnet (fragile to VPN routes; fix `--subnet 100.120.0.0/24` + re-freeze).

## Known limitations (also in the report)
Frozen binary post-dates private fixes; private fixes post-date the model cutoff while 6 public tasks pre-date it (possible memorization, same for all arms); 5 private tasks replaced (binary held gold strings); two accounts; Skill/Task added to full arm and full has no Bash/Edit/Write; `compiled all targets` undercounts cs/full; frozen CLI 2.1.309; `--verbose` may inflate cost; default effort `medium`; reranker had no listener.
