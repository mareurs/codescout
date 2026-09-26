---
kind: bug
status: open
tags:
- cluster/guard-narrower-than-its-name
closed: null
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

Not applied. The cheap form:

- `dirty_reasons` also reports a non-empty `skills/synced/` or `syncClaudeAiSkills` not set to `false`;
- the channel's settings set `syncClaudeAiSkills: false`.

Applying it now would mark `judge-config-main` dirty mid-campaign, for no change in model input.

## Tests added

N/A. No fix yet. A fix needs a case in `tests/test_phase2_score_dp1.py` whose config dir passes the three existing checks and holds a synced skill.

## Workarounds

Keep `--tools ""` on every channel call. That is what makes the extra registration inert.

## Resume

Apply the fix at the next channel rebuild, or earlier if a caller ever enables tools on this channel.

## References

- `scripts/phase2-score-dp1.py` (`dirty_reasons`, `SubscriptionJudge`)
- the synthetic pairs amendment in `docs/evals/phase1-local-classifier-preregistration.md`
