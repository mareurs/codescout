---
kind: tracker
status: active
title: "Evidence — symbols answers 0 matches as success when the language server failed"
tags:
  - frictions
  - symbols
---

# Evidence — `symbols` answers "0 matches" as success when the language server failed

Ledger entry: FR-5 in [index.md](index.md). Window: 2026-10-16 to 2026-11-09 UTC. Corpus: `.codescout/usage.db`. Source: the `search` miner, re-counted by the lead.

## Scale

```
select count(*) from tool_calls
where tool_name='symbols' and outcome='success'
  and output_json like '%gave no symbols%' and id<298900;   -- 71
```

Split by the text after "gave no symbols" (the miner's counts; categories may overlap by one):

- 46: "answered textDocument/documentSymbol with null"
- 19: "another mux instance holds the lock"
- 5: "Failed to read file for didOpen"

Only about a third of `symbols` rows have an `output_json` (FR-3), so the true count is higher.

## What the agent sees

Row 192214 (2026-10-16), verbatim:

> 0 matches
>
> warning: the language server gave no symbols for src/agent/mod.rs: mux process failed to start: Error: another mux instance holds the lock

The call has `outcome='success'`.

## Recovery

Calls in the next three, same session and agent (of 70 rows): `run_command` 29, `grep` 25, `symbols` 24. The agents catch the false zero and route around it with `rg` or `read_file`. The cost is calls, not lost work.

## Known

- `docs/issues/2026-11-02-a-second-client-that-spawns-a-rust-mux-loses-the-lock-race-and-fails-the-start-instead-of-connecting-to-the-winner.md` (mitigated): the mux-lock cause.
- `docs/issues/archive/2026-06-11-mux-failure-masks-rocksdb-lock-collision.md` (fixed): the same masking class.
- The "answered with null" variant: not checked against the ledger. FR-12 covers the `edit_code` side of the same null.

## Idea (hypothesis, not tested)

Return a `RecoverableError`, or a flag in the result, when the language server failed. The warning already names the file and the cause, so it meets ADR 2026-08-27 clause 1. The open question is whether a success with "0 matches" at the top invites a premature stop.

## Not verified

- Whether agents that stop after the zero exist. The data shows only the next three calls.
- How the 71 rows split across sessions.
