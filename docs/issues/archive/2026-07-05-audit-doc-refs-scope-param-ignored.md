---
kind: bug
status: fixed
tags:
- librarian
- audit_doc_refs
- schema-drift
- cluster/accepted-parameter-silently-dropped
closed: 2026-07-06
opened: 2026-07-05
owner: marius
related: []
severity: low
---

# BUG: audit_doc_refs declares `scope` in its schema but ignores it

## Summary
The `librarian` tool schema documents a `scope` param that applies to `audit_doc_refs`, but
`AuditArgs` never deserializes it — the implementation always scans
`ctx.current_project.abs_path`. Callers passing `scope="repo"|"umbrella"` get project-scoped
results with no warning.

## Symptom (Effect)
`librarian(action="audit_doc_refs", scope="umbrella")` silently behaves identically to the
default project scope.

## Reproduction
Call with any non-default `scope`; observe `scan_meta`/file counts unchanged.

## Environment
codescout `experiments`, observed 2026-07-05 during plan-mode scouting.

## Root cause
`AuditArgs` (`src/librarian/tools/audit_doc_refs/mod.rs:129-138`) has no `scope` field;
`call()` derives `repo_root` directly from `ctx.current_project` (`mod.rs:171-178`). The
schema prose in `src/librarian/tools/librarian.rs` advertises `scope` for this action.

## Evidence
Scout report (plan-mode, 2026-07-05): "`scope` from the schema is declared but not read by
`AuditArgs` — the impl always uses `ctx.current_project.abs_path` as `repo_root`."

## Hypotheses tried
N/A — mechanism read directly from source.

## Fix

Shipped the reject path, not the implement path. `AuditArgs` gained a
`#[serde(default)] pub scope: Option<String>` field; `call()` now returns a
`RecoverableError` ("audit_doc_refs is project-scoped in v1") when `scope` is present
and not `"project"`. The `scope` schema description in `src/librarian/tools/librarian.rs`
was updated to say audit_doc_refs is project-scoped-only in v1.

Real `repo`/`umbrella` widening is deferred — logged as a Low-priority roadmap item
in `docs/ROADMAP.md` § Future Improvements, since (unlike the SQL-filtered tools that
share this `scope` param) audit_doc_refs walks the filesystem directly, so widening
means scanning multiple project roots and aggregating findings/trackers across
repos — real feature work, not a bugfix-sized change.
## Tests added

`src/librarian/tools/audit_doc_refs/mod.rs` tests module: `scope_repo_is_rejected`,
`scope_umbrella_is_rejected`, `scope_project_is_accepted`, `scope_absent_is_accepted`.
## Workarounds
Activate the project you want scanned before calling.

## Resume
Decide implement-vs-reject; if reject, add a `RecoverableError` guard at the top of
`audit_doc_refs::call` when `args.scope` is present and != "project", plus a router test.

## References
`src/librarian/tools/audit_doc_refs/mod.rs:129-178`, `src/librarian/tools/librarian.rs:42-92`.
