# Spike: ask a live session through a fork of it (2026-11-15)

Question: can session A get an answer from session B's context without disturbing B, and what does
it cost? Mechanism from `claude-plugins/session-bridge` (`ask_session`, mode `ephemeral`, built
2026-05-18, not installed in any of the four profiles on 2026-11-15).

## Call (Claude Code, profile stef)

```
cd <B's cwd> && CLAUDE_CONFIG_DIR=<B's profile> claude -p --resume <B session id> --fork-session \
  --no-session-persistence --tools "Read,Grep,Glob" --strict-mcp-config --setting-sources "" \
  --output-format json --model claude-haiku-5-5 "<question>"
```

The resume must run in B's cwd and under B's profile, because the transcript lives there.

## Results (target: claude-plugins-f3, session 000a4944, transcript 8.4 MB)

| Ask | secs | cost | context | cache |
|---|---|---|---|---|
| 1, cold | 9.0 | $0.355 | 354,220 tokens | all written (1 h TTL) |
| 2, same question, minutes later | 5.1 | $0.019 | 354,220 tokens | all read |

- Both answers were correct: B said it waits for nothing, spec v2 is approved, and Plan L-E1 is
  committed as `461b98816`. Git confirms `461b98816` "docs(lifecycle): Plan L-E1 … (12 tasks)",
  22:57:47 +0200. Ask 2 added true detail (execution method still to choose, `/mcp` needed).
- B was not disturbed: the question text occurs 0 times in B's transcript after both asks, and no
  transcript was written for either fork session id (`317d58d6…`, `604973db…`).
- The first ask pays the cache write for the whole context. Later asks within the TTL pay about
  5 % of it, if the flags (system prompt, tools) stay the same.

## Limits

- One target, one question, two runs, Haiku only. No run with B's own model (Opus): its cost is not
  measured. With other flags than B's own, the fork cannot reuse B's own cache.
- Not measured: cache reuse after B's conversation has grown; asks against a 60 MB transcript;
  a question that needs tool use.
- `--setting-sources ""` should load no plugins or hooks. Weak check only: no `.buddy/<fork id>` dir
  appeared.

## Readings for the channel design

- RO ask through a fork works today and leaves B untouched. It costs about one cold cache write per
  target per hour, then cents.
- `session-bridge` `bidirectional` mode is obsolete: `claude --help` says `--resume` of a running
  session "starts a copy". RW must be a fork that acts, plus a notice to B.
- `session-bridge` writes `~/.claude/sessions/active.json`, the folder that Claude Code now uses for
  its own registry (`<pid>.json`), and `sb_instance` does not know the `stef` profile.

Related: [[research/session-notice-relevance-spike-20261115]], [[research/cross-harness-session-messaging]].
