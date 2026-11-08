# Usage Analysis — 2026-11-08 (window 2026-10-25 .. 2026-11-08)

Scope: 20 of 99 `usage.db` files have >=20 calls in the window. Raw queries used
`called_at >= '2026-10-25'`. No rename fold was needed (window starts after 2026-09-02).

**Method correction.** The skill's queries count only `outcome='error'`. The recorder also writes
`recoverable_error`. In the codescout DB: 28 `error`, 1,960 `recoverable_error`, 59,540 `success`.
Counting only `error` gives 0.0% and hides a 3.2% recoverable-error rate. Update skill queries A/B/C/E.

## Cross-project summary

| project | calls | error | recoverable | overflow | avg ms |
|---|---|---|---|---|---|
| claude/codescout | 61,539 | 28 | 1,960 | 3,504 | 3,591 |
| mirela/backend-kotlin | 24,273 | 4 | 855 | 1,364 | 2,587 |
| stefanini/invest-europe/ie-pal-engine | 6,815 | 0 | 251 | 382 | 4,296 |
| claude/claude-plugins | 6,468 | 2 | 242 | 330 | 4,153 |
| mirela/eduplanner-mobile | 6,248 | 0 | 171 | 165 | 2,636 |
| mirela/eduplanner-ui | 5,151 | 5 | 237 | 226 | 1,327 |
| personal/home/ha | 5,048 | 1 | 101 | 284 | 2,551 |
| (13 smaller projects) | <=3,483 each | | | | |

## codescout — per tool (calls, avg/p50/p95/max ms, error, recoverable, overflow)

| tool | calls | avg | p50 | p95 | max | err | rec | ovf |
|---|---|---|---|---|---|---|---|---|
| run_command | 27,951 | 7,636 | 87 | 20,030 | 1,890,710 | 1 | 564 | 2,619 |
| read_file | 10,455 | 3 | 0 | 10 | 7,364 | 2 | 552 | 0 |
| symbols | 5,331 | 551 | 16 | 2,004 | 45,381 | 3 | 33 | 366 |
| grep | 3,910 | 148 | 10 | 445 | 99,824 | 0 | 42 | 190 |
| edit_file | 3,800 | 26 | 9 | 97 | 2,675 | 1 | 305 | 0 |
| doc | 3,703 | 68 | 21 | 277 | 10,981 | 10 | 240 | 263 |
| create_file | 2,170 | 6 | 2 | 16 | 2,320 | 2 | 53 | 0 |
| edit_code | 2,117 | 227 | 33 | 582 | 45,461 | 6 | 121 | 1 |
| memory | 1,007 | 392 | 1 | 1,682 | 4,877 | 0 | 42 | 0 |
| workspace | 424 | 1,620 | 117 | 2,023 | 54,618 | 0 | 3 | 8 |
| librarian | 166 | 6,243 | 894 | 28,589 | 128,955 | 0 | 1 | 54 |
| references | 101 | 6,188 | 1,727 | 32,454 | 40,196 | 1 | 4 | 2 |
| index | 14 | 19,014 | 24,982 | 42,197 | 42,197 | 0 | 0 | 0 |

## Top recoverable-error families (codescout)

| n | tool | family |
|---|---|---|
| 220 | run_command | il3_shell_on_source (corrected from 67: the first table listed top distinct messages only) |
| 99 | edit_file | il2_structural_edit (45 `fn `, 39 `def `, 15 batch `def `) |
| 84 | create_file/edit_file/doc | worktree_activate_required (30+29+25), plus edit_code 14 |
| 21 | grep | param_not_applied (`output_mode` is not a grep param; use `mode`) |
| 19 | edit_code | lsp_document_symbol_null |
| 16 | read_file | librarian_managed_artifact (`docs/trackers/issue-clusters.md`) |
| 35 | doc | missing_required_param / invalid_param_shape / unknown_enum_value (`action="read"`) |
| 13 | doc | artifact_rooted_in_temp |
| 310 | run_command | il3_pipe_to_trimmer (corrected from 9; bounded pipe followed by `&&`/`;`) |

## Slow run_command (>10 s): 1,743 calls, 55.3 h total (53 have no `input_json`)

| class | n | hours | avg s |
|---|---|---|---|
| sleep/wait loops | 244 | 20.7 | 306 |
| cargo | 464 | 12.2 | 95 |
| other | 574 | 11.6 | 72 |
| python | 420 | 8.6 | 73 |
| gate.sh | 12 | 1.9 | 580 |
| docker | 29 | 0.3 | 41 |

Sleep/wait loops are deliberate (eval campaigns); exclude them from latency conclusions.

## LSP

- Slowest handshake: java ~2.7 s avg (max 10.9 s, 287 `new_session`), kotlin 1.25 s (max 7.7 s). rust 0.1 s.
- Failed starts: java `LSP server disconnected` x21; rust `another mux instance holds the lock` x19;
  kotlin `mux process failed to start` x~40 (30 Oct - 4 Nov, `mux::server_stderr` text).

## Daily volume

Peak 2026-11-04: 7,167 calls, 30 sessions. Recoverable-error rate stays 1.6-4.0% per day; no trend.
