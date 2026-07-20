---
id: '4fb67c0a804cba13'
kind: bug
status: open
title: 'BUG: artifact(action="update", status=...) silently drops the top-level status param the schema documents; only patch={status} works'
tags:
- librarian
- artifact
- update
- silent-noop
- schema-drift
closed: null
opened: 2026-07-20
owner: marius
related: []
severity: medium
---

## Summary
The `artifact` tool schema documents `status` as a top-level param for BOTH `create` and `update` ("find: shortcut eq-filter on status … **create/update: set status**"). For `update` it is silently discarded: `update::Args` has only `id`, `patch`, `commit_refresh`, `force`, and serde drops unknown fields by default. The call still returns `{"id": ..., "updated": true}`, so the caller has no signal the status change never happened. `patch={"status": "..."}` works correctly.

## Symptom (Effect)
```
artifact(action="update", id="e000f27a6dd6c0a0", status="fixed",
         patch={body_edits: [...]})
→ {"id": "e000f27a6dd6c0a0", "updated": true}
```
The `body_edits` land. The status does not. Both the catalog row and the on-disk frontmatter still read `status: draft`. The `updated: true` refers to the patch having applied — it says nothing about the ignored param.

This is worse than a plain no-op because the call *partially* succeeds: a caller batching a status flip with body edits sees the body change land and reasonably infers the whole call took.

## Reproduction
1. Take any artifact with `status: draft`.
2. `artifact(action="update", id="<id>", status="fixed", patch={"body_edits": [...]})`.
3. Observe `updated: true` and the body edits applied.
4. `artifact(action="get", id="<id>")` → `status` is still `draft`. `head -5 <file>` confirms the frontmatter agrees — this is a genuine dropped write, not catalog-vs-file drift.
5. `artifact(action="update", id="<id>", patch={"status": "fixed"})` → status flips correctly in both catalog and frontmatter.

Concrete occurrence (2026-07-20, this repo): closing `docs/issues/2026-07-20-append-entry-id-drift-params-vs-body.md`. The call passed `status="fixed"` top-level plus three `body_edits`; the body sections were rewritten but the file stayed `status: draft`. Caught only because a later `artifact(find, kind="bug")` sweep listed the supposedly-fixed bug as still open.

## Environment
codescout MCP `artifact` tool, `update` action. Catalog/librarian layer — not language- or project-specific.

## Root cause
`src/librarian/tools/update.rs` — `Args` (line ~49) declares exactly four fields: `id`, `patch: UpdatePatch`, `commit_refresh`, `force`. `UpdatePatch` carries `status`, but nothing lifts a top-level `status` into it. The dispatcher deliberately tolerates extra fields (see `update_action_passes_through_dispatcher_without_unknown_field_error` in `src/librarian/tools/artifact.rs`), which is what turns a would-be deserialization error into a silent drop.

The schema text in `src/librarian/tools/artifact.rs` (the `status` property description) promises `create/update: set status`. `create` honors it; `update` does not. So this is schema-vs-code drift, and the schema is the thing steering every agent's call shape.

## Evidence
- `symbols("src/librarian/tools/update.rs")` — `Args` has no `status` field; `UpdatePatch/status` exists at line 12.
- Schema string in `src/librarian/tools/artifact.rs`: `"find: shortcut eq-filter on status (disables archived-hide). create/update: set status."`
- Empirical: the two calls in Reproduction steps 2 and 5, with `head -5` on the file between them showing `status: draft` then `status: fixed`.

## Hypotheses tried
1. **Hypothesis**: the write landed in the catalog but the frontmatter writer skipped it (catalog-vs-file drift, the failure mode `librarian(doctor)` hunts).
   **Test**: compared `artifact(action="get")` against `head -5` of the file after the failed update.
   **Verdict**: rejected — both read `status: draft`. They agree; the write never happened at all.
   **Evidence link**: Reproduction step 4.

## Fix
Not implemented — filed for triage. Options:
1. **Honor it**: add `status: Option<String>` to `update::Args` and fold it into `patch.status` before applying (erroring if both are set and disagree). Matches the schema and `create`'s behavior; zero caller churn.
2. **Refuse it**: `deny_unknown_fields` on `update::Args`, or an explicit check, returning a `RecoverableError` pointing at `patch={status}`. Per the repo's Repair-and-Continue convention this is the *worse* option — the intent here has exactly one correct reading, so repairing it saves an LLM round-trip. Reserve the error for genuinely ambiguous input.
3. **Fix the schema instead**: drop `update` from the `status` property description. Cheapest, but leaves an asymmetry between `create` and `update` that agents will keep tripping over.

Option (1) plus a `corrections` advisory note is the closest fit to `memory("conventions")` § Repair-and-Continue.

## Tests added
None yet. When fixed: a regression asserting `update(status=..., patch={body_edits})` flips the status in BOTH the catalog row and the on-disk frontmatter — the existing `update_status_archived_persisted` only covers the `patch={status}` path, which is why this gap survived.

## Workarounds
Always pass status inside the patch: `artifact(action="update", id=..., patch={"status": "fixed"})`. Never rely on the top-level param for `update`. After any status flip meant to close a bug, verify with `artifact(action="get")` or `head -5` on the file before treating it as closed.

## Resume
Not started. Next action: add `status` to `update::Args` in `src/librarian/tools/update.rs`, fold into `patch.status` at the top of `call()`, and extend `update_status_archived_persisted` (or add a sibling) to cover the top-level-param path.

## References
- Occurrence: closing `docs/issues/2026-07-20-append-entry-id-drift-params-vs-body.md`, 2026-07-20.
- Convention this fix should follow: `memory("conventions")` § Repair-and-Continue Input Handling; ADR `docs/adrs/2026-07-10-repair-and-continue-input-handling.md`.
- Same silent-partial-success family as `docs/issues/2026-07-02-artifact-augment-params-path-bare-array-silent-noop.md` (a librarian write that reports success while discarding the payload).

