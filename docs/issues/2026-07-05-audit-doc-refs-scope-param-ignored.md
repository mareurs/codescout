---
status: open
opened: 2026-07-05
closed:
severity: low
owner: marius
related: []
tags: [librarian, audit_doc_refs, schema-drift]
kind: bug
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
Either implement scope widening (reuse `tools::scope::apply_scope`) or error recoverably on
non-default scope ("audit_doc_refs is project-scoped in v1") — silence is the bug.

## Tests added
N/A — not yet fixed.

## Workarounds
Activate the project you want scanned before calling.

## Resume
Decide implement-vs-reject; if reject, add a `RecoverableError` guard at the top of
`audit_doc_refs::call` when `args.scope` is present and != "project", plus a router test.

## References
`src/librarian/tools/audit_doc_refs/mod.rs:129-178`, `src/librarian/tools/librarian.rs:42-92`.
