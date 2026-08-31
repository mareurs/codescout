---
id: '82468dd001929c61'
kind: bug
status: fixed
title: 'BUG: artifact(action="update", status=...) silently drops the top-level status param the schema documents; only patch={status} works'
tags:
- librarian
- artifact
- update
- silent-noop
- schema-drift
- cluster/accepted-parameter-silently-dropped
closed: 2026-07-20
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

Implemented option (1) — honor the param — with the Repair-and-Continue split.

`update::Args` gains `#[serde(default)] status: Option<String>`. At the top of `call()`, before the worktree rebind, a top-level `status` is folded into `patch.status`:

- **Top-level only** — lifted into the patch, and an advisory `corrections` array is attached to the response naming the canonical form. One correct reading, so repair instead of erroring: a `RecoverableError` here would cost a second LLM call for input whose intent is unambiguous.
- **Both, agreeing** — silently accepted, no note. Agreement is not ambiguity.
- **Both, conflicting** — `RecoverableError` naming both values, and nothing is written. Per the convention's "writes get a higher bar", a wrong guess on a write is unrecoverable, so this is the one case that must refuse rather than pick.

The lift happens before `resolve_write_target` rebinds `a`, so worktree-forked writes get the same treatment.

The schema text needed no change — it already promised this behaviour; the code now matches it.
## Tests added

`src/librarian/tools/update.rs`:
- `update_lifts_top_level_status_into_the_patch` — the regression proper: top-level `status` plus an unrelated `patch` must set both, and advertise the `corrections` note. Confirmed failing first (`left: "draft", right: "fixed"`).
- `update_top_level_status_reaches_the_frontmatter` — asserts the on-disk YAML agrees with the catalog row. The original bug was caught only because both read `draft`; a fix that updated only the row would be a subtler version of the same defect. Confirmed failing first.
- `update_conflicting_status_sources_are_refused` — conflicting values return a `RecoverableError` AND write nothing (asserts the row holds neither value). Confirmed failing first (the call previously returned `updated: true`).
- `update_top_level_status_agreeing_with_patch_is_not_flagged` — no spurious correction note on agreement. Passed before the fix (vacuously, via the working `patch.status` path) and still passes — kept as a guard against over-eager flagging.

Full suite: 3399 passed, 43 ignored; `cargo fmt` + `cargo clippy --all-targets -- -D warnings` clean.
## Workarounds
Always pass status inside the patch: `artifact(action="update", id=..., patch={"status": "fixed"})`. Never rely on the top-level param for `update`. After any status flip meant to close a bug, verify with `artifact(action="get")` or `head -5` on the file before treating it as closed.

## Resume

Fixed on `experiments`, not yet cherry-picked to `master`. Archive this file only after the fix ships to `master` (`git branch --contains <fix-sha>`).

Worth a follow-up sweep, not done here: other actions in the `artifact` dispatcher may have the same schema-vs-Args asymmetry, since the dispatcher tolerates unknown fields by design. `create` and `update` were checked; `move`, `link`, `graft`, and `append_entry` were not audited for top-level params their schema advertises but their `Args` omits.
## References
- Occurrence: closing `docs/issues/2026-07-20-append-entry-id-drift-params-vs-body.md`, 2026-07-20.
- Convention this fix should follow: `memory("conventions")` § Repair-and-Continue Input Handling; ADR `docs/adrs/2026-07-10-repair-and-continue-input-handling.md`.
- Same silent-partial-success family as `docs/issues/2026-07-02-artifact-augment-params-path-bare-array-silent-noop.md` (a librarian write that reports success while discarding the payload).
