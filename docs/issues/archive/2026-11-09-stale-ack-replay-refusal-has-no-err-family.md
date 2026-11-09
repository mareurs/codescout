---
id: 9abba2df81c08b7d
kind: bug
status: fixed
title: The create_file stale-replay refusal has no err_family, because its message starts with a path
tags:
- cluster/unclassified
closed: 2026-11-09
opened: 2026-11-09
---

## Summary
The refusal of an `ack` replay over a file that changed since the refusal (`<path> changed since create_file refused to overwrite it, so the kept write was not replayed`) leaves a `usage.db` row with `outcome=recoverable_error` and no `err_family`. The classifier decides the dispatcher's other `ack` refusals by their fixed prefix (`ack handle …` → `ack_handle_refused`), but this message starts with a path, which is data, and no rule matches its fixed sentence.

## Class fit
No roster class fits without forcing it. The refusal is correct and its text is clear; the defect is that the instrument cannot name it, so a count by family cannot tell a stale replay from the unclassified long tail. That is an observation gap in one message, and the roster has no class for it.

## Symptom (Effect)
```
sqlite3 .codescout/usage.db "SELECT id, outcome, err_family FROM tool_calls WHERE error_msg LIKE '%changed since create_file refused%'"
```
returns rows with `err_family` NULL.

## Reproduction
1. `create_file` on an existing path without `overwrite`: refused with `ack="@ack_…"`.
2. Change the file from outside.
3. `create_file(ack="@ack_…")`: refused with the `changed since` text.
4. Read the newest `create_file` row.

Measured 2026-11-09 on a rebuilt server at `6b0c6ba92`: three such rows (the existing-path and the shrink-guard flows), row `282378` among them.

## Environment
codescout on `experiments`, `usage.db` of this project.

## Root cause
`normalize_err_family` (`src/usage/db.rs`) has one early rule for the replay refusals: `msg.starts_with("ack handle ")`. The refusal built in `CodeScoutServer::replay_ack` (`src/server.rs`) for a changed file is `format!("{} changed since create_file refused to overwrite it, so the kept write was not replayed", guard.path.display())`, so its first words are the path. No rule in the chain matches it, so the function returns `None`. Read from `src/usage/db.rs` and `src/server.rs` on 2026-11-09.

## Evidence
Measured 2026-11-09 in this project's `usage.db`, rows from 2026-11-01:

| outcome | `err_family` NULL | count |
|---|---|---|
| `recoverable_error` | no | 1160 |
| `recoverable_error` | yes | 74 |
| `error` | no | 11 |
| `error` | yes | 3 |

The 74 are mostly other tools' validation refusals (`doc` class-fit and cluster-tag refusals, `edit_file` markdown-only, `doc` unknown id, `grep` multiline, `read_file` json_path). Three of the 74 are this message, and those three come from this session's own live probes (`target/ack-probe`, `target/shrink-probe`), not from organic use. So the observed cost today is nil: the gap is real, and nothing yet depends on it.

Not established: how often a stale replay happens in ordinary use, since the feature is a day old.

## Hypotheses tried
None needed: the classifier chain and the message are in the source.

## Fix

Fixed in `6043fb27a` on `experiments` (patch-id `7b0c9912cc5d`).

`normalize_err_family` has a new early rule that decides by the fixed sentence after the path (` changed since create_file refused to overwrite it, so the kept write was not replayed`) and returns `ack_replay_file_changed`. The family is in `ERR_FAMILIES`, so the backfill gate re-classified history once. The remedy differs from `ack_handle_refused`: the handle was valid and the file moved, so the caller sends a fresh write.

Verified live on a rebuilt server (binary `6043fb27a`) on 2026-11-09: a replay over a file changed after the refusal left row 285420 with `err_family` `ack_replay_file_changed`. Rows with this message and a NULL family: none left, so the backfill re-classified the earlier ones.

## Tests added

- `the_dispatcher_refusals_have_a_family_whatever_their_data_says` (`src/usage/db.rs`) gains a case with a hostile path (`/tmp/heading Fix not found/index is locked`).
- `an_ack_does_not_replace_a_file_that_changed_since_the_refusal` (`src/server/overwrite_ack_tests.rs`) classifies the real refusal text the server sent, not a copy of it.

With the rule's text broken, both fail. Gate green on 2026-11-09: LEAN 6303, DEFAULT 9191, Python 1764 passed, clippy clean.

## Workarounds
Query by message: `error_msg LIKE '%changed since create_file refused%'`.

## Resume

Nothing left to do for this message. The other NULL-family recoverable errors counted in Evidence (other tools' validation refusals) were not part of this issue and were not touched.

## Fix provenance

- **SHA:** `6043fb27a` (`experiments`)
- **patch-id:** `7b0c9912cc5d`

## References
- `src/usage/db.rs` (`normalize_err_family`, `ERR_FAMILIES`)
- `src/server.rs` (`replay_ack`)
- `docs/issues/archive/2026-11-08-access-gate-refusals-leave-no-usage-row.md`
- `docs/issues/archive/2026-11-08-create-file-existing-path-refusal-makes-the-agent-resend-the-whole-write.md`
