---
kind: bug
status: fixed
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-10-16
opened: 2026-09-25
owner: marius
related:
- scripts/phase2-score-dp1.py
- docs/evals/phase1-local-classifier-preregistration.md
severity: low
---

# BUG: the judge-channel check does not see synced claude.ai skills, and only `--tools ""` keeps them out of the prompt

## Summary

`dirty_reasons` (`scripts/phase2-score-dp1.py`) certifies a `CLAUDE_CONFIG_DIR` as the rule-tell campaign's **clean channel** by checking for a `CLAUDE.md`, enabled plugins and hooks. It does not check **skills synced from claude.ai**. The campaign's channel, `judge-config-main`, acquired 8 of them (`anthropic-skills:docx`, `pdf`, `pptx`, `xlsx`, `docs`, `skill-creator`, `import-memory`, `morning`) at 2026-09-24 10:42, two minutes after it was created. They are registered in every session run through it, and the check passes.

**Measured harmless so far:** model input is identical to a fresh directory's, because every caller passes `--tools ""`. That is a property of the callers, not of the check.

## Symptom (Effect)

None observed in any result. The session init event run through `judge-config-main` lists 8 more skills and slash commands than a fresh directory, and `dirty_reasons` returns `[]` for it.

## Reproduction

Run `claude -p "Say OK." --model claude-sonnet-5 --tools "" --system-prompt "X." --strict-mcp-config --no-session-persistence --output-format stream-json --verbose` with `CLAUDE_CONFIG_DIR` set to `judge-config-main`, then to a fresh directory holding only the credentials link and `{"enabledPlugins":{},"hooks":{}}`, after one warm-up call. Compare the `system`/`init` events and the `result` usage.

## Environment

Claude Code CLI on the subscription, 2026-09-25; profile `~/.claude-sdd`.

## Root cause

The check enumerates three ways a config dir can add context. The synced-skill path is a fourth, and the check predates noticing it.

## Evidence

2026-09-25, from session `571eb3d6`:

| config dir | `input_tokens` | skills registered |
|---|---|---|
| `judge-config-main` | 457 | 18 bundled + 8 `anthropic-skills:*` |
| fresh directory (second call) | 457 | 18 bundled |

- **A first call on a fresh directory reads 304** in `--output-format json`, and every later call reads 478. That transient is not the synced skills: a fresh directory with `syncClaudeAiSkills: false` creates no `skills/` directory and shows the same 304 → 478 step. The stream-json runs above read 456 and 457 from the first call.
- **So synced skills add nothing to model input under `--tools ""`.** Every judged or generated run in the campaign passes that flag, so none of their inputs differ from a fresh channel's.

**2026-09-26, same session, on the phase-1b labeller's own model and flags** (Claude Code 2.1.283; `claude-opus-5-5`; `--tools ""`; run_labellers' system prompt), prompted by a Codex preflight review (`docs/research/2026-09-26-codex-phase1b-labelling-preflight-review.md`) that rightly rejected a first control in which the "fresh" directory synced the same skills during the measured run:

| config dir | input tokens | skills registered (stream-json `init`) |
|---|---|---|
| `judge-config-main` | 412 | 18 bundled + 8 `anthropic-skills:*` |
| `judge-config-blocked`: `skills` and `plugins` created as plain files, so syncing is impossible | 412 | 18 bundled; `plugin_errors`: "claude.ai plugin sync disabled this session ... a stray file is in the way" |

- The blocked directory still held `skills` and `plugins` as plain files after its run. With the mechanism removed and model, prompt and flags fixed, the token count is unchanged, so the 8 synced skills are registered but not in the labeller's context. Both runs show `tools: []` and `mcp_servers: []`.
- A second, independent removal mechanism from 2026-09-25's `syncClaudeAiSkills: false`, and on Opus 5.5, not Sonnet 5.
- No first-call transient under Opus 5.5: each fresh directory read 412 on its first call, in both output formats.
- Not attributed: about 390 tokens of the 412 beyond the roughly 20-token prompt, common to both configurations. The measurement bounds the synced skills, not the whole context. The same channel served phase 1's audits.

## Hypotheses tried

- **"The synced skills add about 174 tokens per call."** Refuted: the jump is a first-call transient, present with syncing off.

## Fix

Applied 2026-10-16 (the "cheap form" below). `dirty_reasons` in `scripts/phase2-score-dp1.py` now reports two more reasons:

- `claude.ai skills synced`: any `skills/synced/**/SKILL.md` under the config dir. The layout was read from a real profile on 2026-10-16: `skills/synced/<org>_<account>/<skill>/SKILL.md`, beside hidden `.bucket-*` / `.last-complete-round` markers and a `manifest.json`. Markers and a manifest with no `SKILL.md` register nothing and are not dirty; a user skill at `skills/<name>/SKILL.md` is not the sync and is not reported by this check.
- `claude.ai skill sync enabled`: `syncClaudeAiSkills` is anything but exactly `false`. The CLI's own settings schema (claude 2.1.301, read from the binary) describes it as "Set to false to turn off syncing of the skills you have enabled on claude.ai"; unset therefore means syncing is on.

The clean channel is now `settings.json` = `{"enabledPlugins":{},"hooks":{},"syncClaudeAiSkills":false}`, and both refusal messages (`phase2-score-dp1.py`, `phase1-span-selector.py`) say so. Every other caller (`run_labellers.py`, `generate_synthetic.py`, `audit_synthetic.py`, `check_reviews.py`) goes through the same function.

What this does and does not change:
- Model input is unchanged. The measurement above (412 vs 412 and 457 vs 457 input tokens) stands; the new reasons describe the channel's state, not an input difference, and stay inert only while callers pass `--tools ""`.
- Recorded rows are untouched. Their `judge_channel: clean` headers were written under the three-condition check, on a channel (`judge-config-main`) that held 8 synced skills, so the word means less there than on a row written after this change. `docs/PROBES.md` says so in the `dirty_reasons` row.
- A dir that already synced stays dirty after the flag is set, until its `skills/synced` is removed. The judge dirs are session scratch (`<scratchpad>/judge-config-main`, rebuilt each session), so a rebuild is the normal path. A new session that reuses an old dir, or the documented `{"enabledPlugins":{},"hooks":{}}` recipe in the rule-tell scoring doc, is now refused until the flag is added; `--allow-dirty-judge` still runs it and labels the row `dirty: ...`.

## Tests added

`tests/test_phase2_score_dp1.py::DirtyReasons`, red first (4 failed, 12 passed before the change):

- `test_a_synced_claude_ai_skill_is_dirty`: a config dir that passes the three older checks and holds `skills/synced/org_acct/docx/SKILL.md` (the case this bug asked for);
- `test_skill_sync_left_unset_is_dirty`: flag unset, and flag `true`;
- `test_both_sync_reasons_are_reported_together`;
- `test_a_missing_settings_file_is_dirty_not_clean`: now also lists the sync reason;
- guards against over-reporting, which passed before and after: `test_hidden_markers_with_no_synced_skill_are_not_dirty`, `test_a_user_skill_outside_synced_is_not_the_synced_check`.

`clean_profile` (that file) and the two clean-dir writes in `tests/test_phase1b_audit.py` now set `syncClaudeAiSkills: false`. The `~/.claude-kat` and `~/.claude-sdd` profiles on this machine report both new reasons, as expected. Python light lane green (1228 passed).

## Workarounds

Keep `--tools ""` on every channel call. That is what makes the extra registration inert.

## Resume

Done. If a future caller enables tools on this channel, the synced skills stop being inert: measure the model input again before trusting that channel.

## References

- `scripts/phase2-score-dp1.py` (`dirty_reasons`, `SubscriptionJudge`)
- the synthetic pairs amendment in `docs/evals/phase1-local-classifier-preregistration.md`
