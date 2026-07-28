---
kind: bug
status: fixed
title: "LibrarianAdapter::is_write matches dead tool names — read-only write-guard bypass"
last_observed: 2026-06-01
tags: [librarian, security, read-only, write-guard]
closed: 2026-06-04
---

# LibrarianAdapter::is_write matches dead tool names

## Symptom
`LibrarianAdapter::is_write` (`src/librarian/adapter.rs:71-81`) classifies every
librarian tool as a **read** (`is_write` → false) because it matches
`self.inner.name()` against tool names that no longer exist:
`artifact_create`, `artifact_update`, `artifact_link`, `artifact_observe`,
`artifact_event_create`, `librarian_reindex`. The live tool names are
`artifact`, `artifact_event`, `artifact_augment`, `artifact_refresh`,
`librarian` (`src/librarian/tools/mod.rs:137`).

## Impact
`CodeScoutServer::is_write_call` → `Tool::is_write` is the oracle for
`acquire_write_guard_if_writing` in `call_tool_inner`. Because all librarian
tools report read, the **main server's write-guard never engages for librarian
mutations** — so a **read-only workspace** (pinned non-home, `read_only=true`)
can still mutate the librarian catalog: `artifact action=create/update/delete/
move/link`, `librarian action=reindex`, `artifact_event`, `artifact_augment`,
`artifact_refresh`. The read-only contract leaks for the entire librarian
surface.

Discovered via the peer-delegation adversarial review (2026-06-01). The
peer-delegation wall does **not** rely on this — it uses a deny-by-default
allow-list of read tools, so librarian tools are unreachable over the peer
socket regardless. But the **main server's** write-guard does rely on it.

## Reproduction
1. Activate a workspace read-only (pinned non-home, or a non-home project with `read_only=true`).
2. Call `artifact(action="create", ...)` or `librarian(action="reindex")`.
3. Observe the catalog mutates despite read-only mode (write-guard never fires).

## Root cause
The match arms in `LibrarianAdapter::is_write` are stale — leftover from the
pre-2026-05-02 "tools-collapse" surface, never updated when the librarian tools
were consolidated to `artifact`/`artifact_event`/`artifact_augment`/
`artifact_refresh`/`librarian` with an `action` discriminant.

## Fix

**Implemented + verified 2026-06-04 on `experiments`.** Rewrote `LibrarianAdapter::is_write` (`src/librarian/adapter.rs`) to match the **live** tool names and inspect `action`:
- `artifact` → write on `create | update | move | delete | link` (find/get/graph/state_at read)
- `artifact_event` → write on `create` (`list` reads)
- `artifact_augment` → always write (no read action)
- `artifact_refresh` → read-only (`gather`/`list_stale`; the write-back is `artifact(update, commit_refresh)`)
- `librarian` → write on `reindex` and `audit_doc_refs` (unless `emit_tracker=false`); context/tracker_design/workspace_state_at/doctor read

Refined from the original sketch: `artifact_event list` and *all* of `artifact_refresh` are reads (the sketch over-marked them write — safe but over-restrictive), and `librarian audit_doc_refs` also writes (it emits a tracker by default).
## Workarounds
None needed for peer-delegation (allow-list excludes librarian). For the main
server, do not rely on read-only mode to protect the librarian catalog until fixed.

## Resume

**Fixed 2026-06-04 on `experiments`** (see ## Fix). Regression test `server::guide_hint_tests::is_write_call_classifies_librarian_surface` green; clippy clean. Not yet on master — ship via Standard Ship Sequence + frog audit, then `git mv` to `docs/issues/archive/` citing the **master-side** SHA.
