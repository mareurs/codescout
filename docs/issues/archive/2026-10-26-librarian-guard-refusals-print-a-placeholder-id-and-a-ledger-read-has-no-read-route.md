---
id: 56dcc467826e17ae
kind: bug
status: fixed
title: librarian-guard refusals print a literal id placeholder and the ledger refusal for a read names no read route
tags:
- cluster/hint-composed-without-the-request
closed: 2026-10-27
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

Fixed in `src/util/librarian_guard.rs`. A new `frontmatter_id` reads the file's own `id:` (`is_librarian_artifact` now calls it, same first-`id:`-line rule). `guard_with_oracle` builds `id_arg` from it, so all three hint arms print `id="<the real id>"`. When the file has no `id:`, which is the case for a ledger or an augmented tracker, the hint adds a step `doc(action="find", rel_path="<path>")` and names the id as `<id from the find above>`. For an absolute path the step uses the file name, because `rel_path` is repo-relative and matched by substring. A refused READ of a ledger now also names `doc(action="get", ...)`; write refusals keep their old shape.

Limit: a stamped file whose `id:` line is stale (well-formed but no longer the catalog id) would be named as is. The guard validates the id's shape and never its value, as `is_librarian_id` documents.

## Fix provenance

- **SHA:** `aac42dba8` (`experiments`)
- **patch-id:** `934af73a2517fdaa157bee50f2fb368f43e68c8e`

## Tests added

In `src/util/librarian_guard.rs`:

- `a_ledger_read_refusal_names_the_read_route` (seen failing before the fix)
- `a_stamped_refusal_names_the_files_own_id_not_a_placeholder` (seen failing)
- `a_refusal_with_no_id_in_the_file_says_how_to_find_it` (seen failing)
- `the_find_lookup_for_an_absolute_path_uses_the_file_name` (seen failing)
- `a_ledger_write_refusal_does_not_grow_a_read_route`, a narrowness pin. It passed before the fix and after it, by design.

Not verified end to end: the new `find` step was not run against a live server, so that `rel_path` with a file name returns the artifact rests on the tool schema text and not on a call.

## Workarounds

`doc(action="get", id=…)` after `doc(action="find")`.

## Resume

N/A — fixed. One open check: run the new hint's `doc(action="find", rel_path="<file name>")` once on a rebuilt server and confirm it returns the ledger.

## References

- `src/util/librarian_guard.rs`
