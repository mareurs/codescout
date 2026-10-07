# Effort steering: verified findings and instrument rules (as of 2026-10-07)

Work lives in the `claude-plugins` repo (`effort-steering/`); the live list is its tracker
`docs/trackers/effort-steering-second-phase.md` (id `af4ccc8b94a95c91`, ES-1 to ES-23) and the passover
`ec8f1f91d436b653`. Everything below was measured with Sonnet 5.5 and Claude Code 2.1.292 unless said otherwise.
Numbers and caveats: ES-17 (mods), ES-18 (baselines), ES-20 (exact-answer tasks), ES-21 (S4 result).
The quality test (S5-lite, ES-22): spec `claude-plugins/docs/superpowers/specs/2026-10-07-effort-steering-quality-test-design.md`,
plan `claude-plugins/docs/superpowers/plans/2026-10-07-effort-steering-quality-test.md`.

## What is established

- **Steering sentence works at xhigh.** The hook appends "Answer directly without deliberating." as
  `additionalContext`. Registered S4 run (second registration, 8 prompts that think, `claude -p --effort xhigh`):
  mechanism_pass, mean thinking -37 percent, output -27 percent, p ~ 1e-4; reproduced from the raw transcripts.
  The first registration (medium, 12 prompts) failed because the model thinks ~0 tokens at medium (floor effect).
  QUALITY IS NOT MEASURED: the verdict (G4) is held until a quality check on real coding tasks with tests (ES-22).
- **The sentence is Anthropic's documented per-message suppressor** (platform.claude.com "Steering thinking", read
  2026-10-07), with a warning that thinking less "may reduce quality on tasks that benefit from reasoning" and that
  lowering effort "is usually the better first lever". No vendor quality number exists for it.
- **A Claude Code mod can set effort per request.** A hook on `turn.step` (fires before every model request, main and
  subagents) can `next({ ...e, effort })`. Headless works with `--plugin-dir`. The rewrite reaches the wire as the
  request's `output_config.effort` plus an appended `role: system` pin message. Mid-turn and turn-boundary changes
  caused no cache miss in the tested setup. Classic hooks cannot set effort. Mods run only in Claude Code.
- **At medium the model rarely thinks and solves nearly all exact-answer tasks** (33 of 35 tasks 3 of 3), so such tasks
  cannot show a quality effect. At xhigh it thinks a lot on design and debugging prompts (3 of 12 prompts hold ~94 percent).
- `claude -p --effort <level>` works for a child run; a saved settings or /model default does not reach a
  `--setting-sources local` child. `--tools ""` disables all tools (a model that wants Bash then writes raw tool XML).

## Instrument rules (each cost real time)

- `claude -p` in a loop needs stdin from `/dev/null`; check every first user message is one line (S5: equals the prompt file).
- Count a message's thinking blocks over ALL transcript records of its `message.id` (the thinking block sits in an
  earlier record than the tool_use or text).
- Take a message's USAGE from the record with the largest `output_tokens`, never the first record. In tool-loop
  transcripts early records can carry partial usage (2026-10-07: 13 of 596 multi-record messages; first record 3
  output tokens and no thinking field, last 4,916 with 2,756 thinking). S4's `tokensFromTranscript` keeps the first
  record; the S4 transcripts had 0 such messages, so S4 stands, but do not reuse it for tool loops.
- Subagent transcripts live at `<config>/projects/<slug>/<sessionId>/subagents/agent-*.jsonl`; count them too.
- Measured runs go DIRECT. A local wire recorder (ANTHROPIC_BASE_URL set) doubled the prefix and changed thinking.
- A "no effect" or "no miss" result needs a positive control (a model switch breaks the cache; use it).
- Grade with scripts, never from a remembered number (a remembered 153 was wrong; the DP says 571). Run model-written
  code only in `python3 -I` in a temp dir with a timeout, or in a container; never run `git` on an agent's tree on
  the host (a `.gitattributes` filter driver would execute).
- A `--max-budget-usd` cap is checked after the fact (a runaway cost $1.29 against a $1 cap); use a call timeout.
  Claude Code 2.1.292 has no max-turns flag.
- `claude --debug-file` shows no request parameters.
- Do not extrapolate a power or cost number without stating its assumptions; measure a pilot first.

## Where things are

- Probe scripts, flat mod sources, wire recorder: `claude-plugins/effort-steering/spikes/probes/` (README); calibration
  runner, task sets, graders: `.../spikes/dose/`. Both are throwaway (delete before any plugin.json). The S5 harness
  will live in `.../spikes/s5/` (throwaway too).
- Design and first-phase records: codescout `docs/superpowers/specs/2026-10-04-effort-steering-design.md`,
  `docs/research/2026-10-04-effort-steering-*.md`.
