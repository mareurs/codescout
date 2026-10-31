# Setup A/B study — which token file belongs to which Claude Code profile (no secrets here)

- `/home/marius/.config/s5/token` (mode 0600) = the long-lived token of the `~/.claude` (main profile) account. Used for Stage 1 (12 dev runs) and the Stage 3 pilot (48 runs). Operator allowed its reuse; it was pasted in a chat earlier, so it is exposed — rotate after the study.
- `/home/marius/.config/s5/token-claude-stef` (mode 0600) = the long-lived token of the `~/.claude-stef` account, supplied by the operator on 2026-10-31 (pasted in a chat — exposed, rotate after the study). Used for the Stage 3 extension.
- The runner reads the token from the file named by the env var `SAB_TOKEN_FILE` (no default path; mode 0600 regular file required) and puts it into each agent container's env file for the run's duration.
- `~/.claude-stef` itself authenticates through a short-lived `.credentials.json` (no long-lived token in settings env), so the runner cannot use that profile directly: it needs a long-lived token string from `claude setup-token` run with `CLAUDE_CONFIG_DIR=~/.claude-stef`.
- The study spans two accounts: disclose it (plan tier, rate limits and the cost basis may differ; model, prompts, arms and cost measurement are identical). See memory `research/setup-ab-eval`.
