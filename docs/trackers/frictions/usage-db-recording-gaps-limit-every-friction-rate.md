---
kind: tracker
status: active
title: "Evidence — usage.db recording gaps limit every friction rate"
tags:
  - frictions
  - instrument
  - usage-db
---

# Evidence — `usage.db` recording gaps limit every friction rate

Ledger entry: FR-3 in [index.md](index.md). Window: 2026-10-12 to 2026-11-10 UTC. Corpus: `.codescout/usage.db`, 117,422 rows at the time of the `lsp` miner's count.

## The gaps

| Gap | Size | Where |
|---|---|---|
| `input_json` empty | 22,463 of 117,422 rows (19%) | Parent-session rows only. 48% before 2026-11-05, 0% from 2026-11-05. |
| `output_json` empty | 43,074 rows (37%) | 0% for subagent rows; about 50% for parent rows before 2026-11-05 and 65% from then. Parent rows lose it because the recorder writes `output_json` only when it runs in debug mode (`src/usage/mod.rs:335`, `if self.debug`). Why subagent rows always carry it is not verified. |
| `call_graph` output | empty on 123 of 126 success rows | So `call_graph` quality and the `lsp`-vs-`ts` edge source cannot be judged. |
| `symbols` | `input_json` empty on 3,245 of 10,449 rows (31%); `output_json` empty on 6,887 (66%) | The `search` miner's count. |
| `lsp_events` | no `session_id`, no `project_root` | LSP failures cannot join to sessions. Joins by time window are loose. |
| Hook advisories | not recorded | Hooks deliver `cs-hint` text through `additionalContext`. `usage.db` stores only tool results. |

## The `cs-hint` count is an artifact

The brief for this pass said that about 360 success rows carry a `cs-hint`. That figure was wrong as a measure of advisories. A `LIKE '%cs-hint%'` match finds file content, command output and memory names that contain the string.

Checked on 2026-11-11 by reading the text around the match in `run_command` rows 298467, 295117, 295015, 294882 and 294204. Each is output that merely contains "cs-hint": an analysis query result, a plugin check, a commit log, a memory note and a design note.

Per tool, success rows with the string: `run_command` 151 (95 with the bracketed form), `workspace` 91 (memory names), `read_file` 86, `grep` 24, `edit_file` 10 (3 real advisories: "1-2 unread sections in this file"), `memory` 6.

Probe stand-in rows (`[cs-hint] stand-in`, from `claude-plugins/codescout-companion/scripts/probe-cs-verified/standin.py`) also sit in the corpus.

## Counts that were wrong in the brief

- Error rows with a `— hint:` text: about 3,065 of 3,924 (78%), counted by the `hints` miner. The brief said about 2,360. The lead's arithmetic was wrong.

## Server-side failures agents rarely see (inferred)

`lsp_events` has 1,034 failed starts: kotlin "intellij-server has expired" 423 rows (2026-09-07 to 2026-11-04), java "LSP server disconnected" 347 (last 2026-10-31), rust "another mux instance holds the lock" 107 (last 2026-11-04), kotlin mux lock 142 (last 2026-10-24). Only 2 `tool_calls` rows carry the mux-lock text. Failures sit within 2 s of a `workspace` call at 1.5% for both failures and successes, so they look like background spawns the agent does not see. That is an inference from a rate comparison.

Known bugs: `docs/issues/2026-11-08-expired-kotlin-lsp-build-retried-until-the-call-times-out.md`, `docs/issues/2026-11-02-a-second-client-that-spawns-a-rust-mux-loses-the-lock-race-and-fails-the-start-instead-of-connecting-to-the-winner.md`, `docs/issues/2026-10-24-java-lsp-prewarm-fails-on-every-codescout-activation.md`.

## Idea (hypothesis, not tested)

- Record delivered hook advisories in their own column or table.
- Tag or exclude probe rows.
- Add `session_id` and `project_root` to `lsp_events`.
- Count guidance-only errors (for example `il3_pipe_to_trimmer`, 719 rows, no `— hint:` marker) in the probe.

## Not verified

- Which processes run the recorder in debug mode. That setting explains why parent rows lose `output_json` (`src/usage/mod.rs:335`), but the data does not say why subagent rows always carry it.
- Whether the empty-input gap closed on 2026-11-05 because of a fix or because of a client change. A source comment (`src/usage/mod.rs:330`) records the decision of 2026-11-02 (bug 20d94ebc7deec094) to record `input_json` with or without debug. The last empty-input row is 2026-11-04 16:17. That is consistent with a build rollout, but it was not traced per build.
