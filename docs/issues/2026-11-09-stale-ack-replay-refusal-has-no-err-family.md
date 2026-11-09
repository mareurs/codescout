---
id: '76822a54a32f1490'
kind: bug
status: open
title: The create_file stale-replay refusal has no err_family, because its message starts with a path
tags:
- cluster/unclassified
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
Not started. Add one rule to the early dispatcher block of `normalize_err_family`, deciding by the fixed sentence after the path (`" changed since create_file refused to overwrite it, so the kept write was not replayed"`), with its own family, for example `ack_replay_file_changed`: the handle was valid and the file moved, which takes a different remedy than `ack_handle_refused` (send the write again). Add the family to `ERR_FAMILIES` and extend `the_dispatcher_refusals_have_a_family_whatever_their_data_says` with a hostile path. Adding a family changes the fingerprint the backfill gate derives, so the one-time backfill re-classifies history; that is the path the 2026-11-08 families took.

## Tests added
None. This file records the defect only.

## Workarounds
Query by message: `error_msg LIKE '%changed since create_file refused%'`.

## Resume
Start from `normalize_err_family` and `the_dispatcher_refusals_have_a_family_whatever_their_data_says` in `src/usage/db.rs`, and the archived `docs/issues/archive/2026-11-08-access-gate-refusals-leave-no-usage-row.md` for the earlier families.

## References
- `src/usage/db.rs` (`normalize_err_family`, `ERR_FAMILIES`)
- `src/server.rs` (`replay_ack`)
- `docs/issues/archive/2026-11-08-access-gate-refusals-leave-no-usage-row.md`
- `docs/issues/archive/2026-11-08-create-file-existing-path-refusal-makes-the-agent-resend-the-whole-write.md`
