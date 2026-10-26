---
id: '030b05def6336fe4'
kind: bug
status: open
title: librarian-guard refusals print a literal id placeholder and the ledger refusal for a read names no read route
tags:
- cluster/hint-composed-without-the-request
opened: 2026-10-26
owner: marius
severity: low
---

# BUG: librarian-guard refusals print a literal `id="<id>"` and the ledger refusal for a read names no read route

## Summary

`guard_with_oracle` (`src/util/librarian_guard.rs:192-228`, read 2026-10-26) refuses a direct read or edit of a managed file and tells the agent to call `doc(action="get"|"update", id="<id>", …)`. The text prints the placeholder `<id>` and not the file's id, although the refused call named the path. For a ledger the refusal of a READ lists only "Add an entry" and "Edit anything else"; it has no read route.

## Class fit

The refusal is composed from the guard's verdict without the request it refuses: the path is known and the id is not offered. That is the hint-composed-without-the-request trait. It differs from the class's other members because the guard has already read the file text for its verdict.

## Symptom (Effect)

The agent that wanted to read a ledger gets instructions for appending or editing. The controller hit this directly on 2026-10-26: `read_file("docs/templates/session-log.md")` returned `This file is a ledger — it owns a PREFIX-N id namespace. • Add an entry: doc(action="append_entry", id="<id>", …) • Edit anything else: doc(action="update", …)`, with no `doc(action="get")`.

## Reproduction

1. `read_file(path="docs/templates/session-log.md")` (a ledger).
2. The error carries `id="<id>"` and no read route. (Reproduced 2026-10-26.)

## Environment

codescout `experiments` at `8df90f1b9`.

## Root cause

The hint text is a fixed string per guard arm (stamped, ledger, augmented). The arm text for a ledger covers writes only, whatever the access kind.

## Evidence

- `usage.db` 2026-09-27..2026-10-26: 173 `librarian_managed_artifact` errors in 58 sessions: `edit_file` 101, `read_file` 72. Subagent breakdown, unverified by the controller: `read_file` refusals were ledger 52 and augmented 20; 14 of the refusals were followed by a shell `sed`/`grep` of the same file.
- `src/util/librarian_guard.rs` lines 193, 196, 216, 226, 228 each contain the literal `id=\"<id>\"`.
- Whether the id is available to the ledger arm (a ledger need not carry a stamped `id:`) is not established.

## Hypotheses tried

N/A.

## Fix

Open. Name the id (or a `doc(action="find", …)` by path) where it is known, and give a read route in the ledger arm for a read.

## Tests added

None. A test would refuse a read of a ledger and assert the text names `doc(action="get"`.

## Workarounds

`doc(action="get", id=…)` after `doc(action="find")`.

## Resume

Found by the 2026-10 friction deep dive (`docs/trackers/friction-deep-dive-2026-10.md`, group G4). Fix not started.

## References

- `src/util/librarian_guard.rs`
