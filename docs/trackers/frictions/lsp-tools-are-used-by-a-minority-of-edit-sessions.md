---
kind: tracker
status: active
title: "Evidence — the LSP-backed tools are used in a minority of edit sessions"
tags:
  - frictions
  - references
  - call_graph
  - symbol_at
---

# Evidence — the LSP-backed tools are used in a minority of edit sessions

Ledger entry: FR-4 in [index.md](index.md). Window: 2026-10-12 to 2026-11-10 UTC. Corpus: `.codescout/usage.db`, 462 sessions. Method: the `usage.db` analyst (scripts `q1.py` to `q7.py` in the session scratchpad), using rows with `id <= 298319` so its own queries are excluded.

## Adoption

An edit is `edit_file` or `edit_code` replace, insert, remove or rename. Blast radius is `references` or `call_graph`.

| Unit | Sessions with an edit | Also called blast | Blast before the first edit |
|---|---|---|---|
| All, by `session_id` | 250 | 75 (30.0%) | 35 (14.0%) |
| Parent calls only | 238 | 63 (26.5%) | 32 (13.4%) |
| Subagents only, by (session, agent) | 224 | 28 (12.5%) | 24 (10.7%) |
| `edit_code` only | 166 | 63 (38.0%) | 38 (22.9%) |

- 107 of 462 sessions (23%) used `references`, `call_graph` or `symbol_at`. By tool: `references` 83 sessions, `call_graph` 39, `symbol_at` 4.
- The top 5 sessions hold 90 of 390 `references` and `call_graph` calls. 57 of the 107 sessions made two or fewer.
- 25 sessions used rename. 12 of them called `references` or `call_graph` before the first rename.
- Weekly share of all calls: 0.33%, 0.25%, 0.35%, 0.24%, 0.55% (the last week covers two days). Flat to slightly rising.

## Cost

| Tool | n | p50 / p90 / p99 (ms) |
|---|---|---|
| `references` | 260 | 1,719 / 15,695 / 40,196 |
| `call_graph` | 130 | 445 / 5,043 / 30,329 |
| `symbol_at` | 7 | 648 / 4,370 / 6,725 |
| `edit_code` rename | 57 | 988 / 2,453 / 4,022 |
| `grep` | 9,161 | 15 / 289 / 1,652 |

- 43 of 260 `references` calls took 10 s or more. On `.rs` files `references` has p50 2,181 ms and p90 18,789 ms.
- The first `references`, `call_graph` or `symbol_at` call per (session, root) has p50 2,419 ms. Later calls have p50 1,016 ms.

## Trust

- 5 of 7 `symbol_at` successes say "no definition resolvable". Small n; 5 of the 7 calls came from one file and line.
- 18 `references` rows carry a misleading "index may still be warming" warning: see FR-1.
- TypeScript `call_graph` is always tree-sitter today. `typescript-language-server` advertises `callHierarchy` only when the client declares it, and the codescout client does not. Verified by the code-audit agent with an `initialize` probe and by reading `src/lsp/client.rs:1839-1842`.

## Discoverability (partly verified)

- `references`, `call_graph` and `symbol_at` carry `Availability::RequiresLsp` (`src/tools/symbol/references.rs:941`, `call_graph/mod.rs:907`, `symbol_at.rs:482`). In a project with no LSP language they are absent from the tool list. Two sessions (7d58cbaf on 2026-10-10, subagent a83e7054 on 2026-10-18) got "No such tool available" for `references`.
- The `symbol-navigation` guide was injected into 233 sessions and the opener reached 444 of 462. So unawareness is not the likely main driver. This has no direct test.

## Emulation

- 637 of 41,606 `run_command` rows with a command are recursive grep. 221 have a plain-identifier pattern, across 57 sessions. 23 of those were followed within 20 calls by an edit on the same word. The base rate is high, so this is a weak upper bound.

## Not verified / limits

- 19% of rows lack `input_json` and 37% lack `output_json` (FR-3). Native Bash work is invisible.
- The "sessions with an edit" denominator includes markdown edits.
- Short eval or dispatch sessions may explain some of the low use. Session intent was not classified.
- `symbol_at` has seven calls, so it supports no conclusion.
- Frequency is not friction. This entry states adoption and cost; it does not show that agents lost work by skipping the tools.

## Idea (hypothesis, not tested)

Lower the cost of the first call (prewarm the index at activation, or answer from `call_edges` when it is fresh). Fix the misleading warning first (FR-1).
