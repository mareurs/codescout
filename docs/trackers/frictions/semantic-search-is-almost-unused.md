---
kind: tracker
status: active
title: "Evidence — semantic_search is almost unused"
tags:
  - frictions
  - semantic_search
---

# Evidence — `semantic_search` is almost unused

Ledger entry: FR-16 in [index.md](index.md). Window: 2026-10-13 to 2026-11-08 UTC. Corpus: `.codescout/usage.db`. Source: the `search` miner.

## Scale

14 `semantic_search` calls against 9,189 `grep`, 10,469 `symbols` and 50,904 `run_command` calls. The 14 calls have latency from 22 ms to 6,032 ms (13 with an input; row 184749 has an empty input and output).

## The four worktree rows

Rows 199006, 201497, 206247 and 207525 return `{"results": [], "total": 0, "hint": "No index for worktree project `codescout@agent-…`. … Run index(action=\"build\") here …"}`. All four ran in worktree sessions (for example row 199006, under `.claude/worktrees/agent-a26646b8b…`). The query in row 199006 was "which files the semantic indexer walks and embeds, extension filter".

None of the four was followed by `index(action="build")` in the rows checked. The `index` tool has 21 calls in all, and the miner did not check them against these sessions.

## Avoidance (hypothesis)

About 4,700 of the 9,189 `grep` patterns are plain multi-word text, by a crude glob filter whose precision is not verified. So concept-shaped queries probably go to `grep`. The data cannot show why. Candidates: the tool is not on the agent's mind, worktree sessions have no index, or cost and trust (the reasons FR-4 gives for the LSP tools).

## Known

- `docs/issues/2026-11-04-semantic-bootstrap-disagrees-with-queryable-index.md` (fixed).
- `docs/issues/2026-11-10-semantic-ready-banner-unreachable-embedder.md` (open): the ready banner, not the worktree case.

## Not verified

- Whether a worktree session ever built an index after the hint.
- The precision of the multi-word `grep` filter.
- Why agents avoid the tool. Treat every cause above as a hypothesis.
