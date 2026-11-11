---
kind: tracker
status: active
title: "Evidence — edit_code was retried ten times on a null documentSymbol"
tags:
  - frictions
  - edit_code
  - lsp
---

# Evidence — `edit_code` was retried ten times on a null `documentSymbol`

Ledger entry: FR-12 in [index.md](index.md). Window: 2026-10-24 to 2026-11-01 UTC. Corpus: `.codescout/usage.db`. Source: the `editcode_other` miner.

## Scale

`err_family='lsp_document_symbol_null'` on `edit_code`: 22 rows (21 insert, 1 replace), 8 sessions. One session made 10 of them between 12:41 and 13:12 UTC on 2026-10-31, on `src/tools/run_command/attribution.rs` (ids 240585 to 240698).

## What the agent sees

Row 240585, verbatim: "the language server answered textDocument/documentSymbol with null … on every attempt for 5000 ms — hint: … rust-analyzer sends it while it reloads its workspace. Retry shortly."

## What the agent did in the burst

- `edit_code`: the same `insert` ten times.
- `run_command`: `sleep 60`, `sleep 90`, `sleep 120` and `pgrep` calls between retries (9 in the family).
- `edit_file`: four routes around, each of which succeeded.

## Known (cause)

- `docs/issues/archive/2026-10-23-documentsymbol-null-window-outlasts-the-5s-cold-budget-because-rust-analyzer-scans-the-nested-worktrees.md` (fixed).
- `docs/issues/2026-10-31-rust-analyzer-watcher-walks-the-directory-symlinks-in-resident-private-without-end-so-documentsymbol-stays-null.md` (mitigated). Same date window.

The cause is therefore handled. What this entry records is that the advice does not change after repeats. It is FR-5's sibling on the write side.

## Idea (hypothesis, not tested)

After N identical nulls on one path, name a different primitive: `edit_file` for a plain text insert, or `symbols` to check whether the server is ready. ADR 2026-08-27 clause 3 applies: offer it as a thing to check.

## Not verified

- Whether the retry loop is a single agent or several agents in one session.
- Whether the burst coincides with the symlink bug above (the dates match; the paths were not compared).
