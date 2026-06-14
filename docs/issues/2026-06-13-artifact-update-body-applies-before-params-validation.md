---
id: '3a95d897e0677b0d'
kind: bug
status: fixed
title: 'artifact(update): body edits persist when params validation fails — non-atomic patch'
owners: []
tags:
- librarian
- artifact-update
- atomicity
- augmentation
topic: null
time_scope: null
closed: 2026-06-13
---

# BUG: artifact(update) applies body edits before params validation — failed patch leaves artifact half-updated

## Summary

`artifact(action="update")` with a patch carrying both `body_edits` and `params` applies the body mutation (file write + catalog upsert) **before** `merge_params` schema-validates the merged params. When validation fails, the call returns a RecoverableError but the body change has already persisted — the tracker's render is ahead of its params cache, and a retry of the identical `body_edits` fails with `old_string not found`, compounding the confusion. A secondary doc/behavior mismatch invites the trigger: the tool schema says params are "RFC 7396 merge-patched", but the merge is intentionally shallow, so partial nested objects wipe sibling keys and trip `params_schema` validation.

## Symptom (Effect)

Observed live 2026-06-13, refreshing the claude-plugins `version-bump-checklist` tracker (`cc8cb9e23ab5cc67`).

Call 1 — `artifact(update, id, commit_refresh=true, patch={params: {plugins: {buddy: {canonical, readme, profiles: {".claude": {installed}, …}}}}, body_edits: [<State edit>, <History insert>]})` returned:

```
merge_params: patch violates params_schema: /plugins/buddy/profiles/.claude: "cache_dir_exists" is a required property; /plugins/buddy/profiles/.claude: "install_path_matches_profile" is a required property; /plugins/buddy/profiles/.claude-sdd: "cache_dir_exists" is a required property
```

Call 2 — same call with schema-complete params and the same `body_edits` returned:

```
body_edits[0]: old_string not found in section '## State'. The text must match exactly (whitespace-sensitive). (hint: Check heading name and old_string content.)
```

…because call 1's body edits had already landed. Subsequent `artifact(get, full=true)` confirmed the split state: body `## State` showed buddy `0.7.19` and the new `### 2026-06-13` History entry, while `augmentation.params` still held buddy `0.7.18` / `last_refresh_commit: a959619`, and `last_refreshed_at` was unchanged (the `commit_refresh` from the failed call correctly did not record).

## Reproduction

codescout `26ae1c4b`, live MCP session, any augmented artifact whose `params_schema` marks nested properties required (e.g. the version-bump-checklist archetype):

1. `artifact(action="update", id=<tracker>, commit_refresh=true, patch={params: {<partial nested object missing a required sibling key>}, body_edits: [{heading: "## State", action: "edit", old_string: <current text>, new_string: <new text>}]})`
2. Call returns `merge_params: patch violates params_schema: …`.
3. `artifact(get, id, heading="## State")` → the body edit from step 1 **persisted**.
4. Re-issue the same call with schema-valid params → `body_edits[0]: old_string not found`.

## Environment

Linux, codescout MCP (live session transport), project `claude-plugins`, codescout commit `26ae1c4b`. Artifact: `docs/trackers/version-bump-checklist.md` (`cc8cb9e23ab5cc67`), augmented with params_schema.

## Root cause

Ordering in `src/librarian/tools/update.rs:288-295`: the new body content is written and the catalog row upserted (`artifact::upsert(&cat, &updated_row)?` at `src/librarian/tools/update.rs:291`) **before** `merge_params` runs at `src/librarian/tools/update.rs:293-295`. `merge_params` (`src/librarian/catalog/augmentation.rs:91-101`) validates the merged params against `params_schema` and propagates a RecoverableError via `?` — at which point the body mutation is already durable. No rollback or validate-before-mutate phase exists.

Contributing factor: `apply_merge_patch` (`src/librarian/catalog/augmentation.rs:117-127`) is a **shallow** merge — its docstring says "Nested objects are overwritten in full (not recursively merged). This is intentional — artifact params are expected to be flat key-value objects." But the `artifact` tool schema describes `params` as "RFC 7396 merge-patched into the augmentation params", and RFC 7396 merges nested objects recursively. Callers following the tool description send partial nested patches (preserving siblings is exactly what RFC 7396 promises), which the shallow merge turns into schema violations — arming the non-atomicity above.

## Evidence

### Live tool-call transcript (claude-plugins session 2026-06-13)

Both verbatim error strings in Symptom above; the split-state `artifact(get, full=true)` response showed `body` with `0.7.19` + the new History heading at preview line 37 while `augmentation.params.plugins.buddy.canonical == "0.7.18"`.

### Source

- `src/librarian/tools/update.rs:288-295` — upsert at 291 precedes `merge_params` at 294.
- `src/librarian/catalog/augmentation.rs:91-101` — validation inside `merge_params`, error after merge.
- `src/librarian/catalog/augmentation.rs:117-127` — shallow merge, "This is intentional" docstring.

## Hypotheses tried

1. **Hypothesis:** schema validation runs against the patch document, not the merged result. **Test:** read `merge_params` (`src/librarian/catalog/augmentation.rs:95-100`). **Verdict:** rejected — it merges into `current` first, then validates the merged value.
2. **Hypothesis:** merge is deep RFC 7396, so partial nested objects preserve siblings and the schema error must come from elsewhere. **Test:** read `apply_merge_patch` (`src/librarian/catalog/augmentation.rs:117-127`). **Verdict:** rejected — shallow by explicit design; nested objects replaced wholesale.
3. **Hypothesis:** body edits applied before params validation, no rollback. **Test:** read `update.rs:271-295` ordering. **Verdict:** confirmed — see Root cause.

## Fix

**Implemented and committed on `experiments` (2026-06-14, this change); pending cherry-pick to master — cite the master SHA here then, per CLAUDE.md § "After cherry-pick".**

1. **Validate-then-mutate.** Extracted `merge_params_dry` in `src/librarian/catalog/augmentation.rs` — it does the merge + `params_schema` validation WITHOUT writing — and added `pub fn validate_params_patch` over it. In `src/librarian/tools/update.rs::call`, a new guard calls `validate_params_patch(&cat, &a.id, params_patch)?` **immediately before `std::fs::write`**, so a schema violation aborts before the file write and the catalog upsert. `merge_params` was refactored to route through `merge_params_dry` (external behavior unchanged: same return, same UPDATE) and still runs after the upsert to persist.

2. The shallow-vs-RFC-7396 merge-semantics mismatch (the secondary contributing factor) is left as a separate follow-up — not required for atomicity. Either make `apply_merge_patch` recursive or correct the `artifact` tool-schema wording.

Result: a combined `{body, params}` update is now atomic — an invalid params patch leaves the body and catalog row untouched.
## Tests added

`params_schema_violation_leaves_body_unchanged` — `src/librarian/tools/update.rs` tests module. Seeds an augmented artifact with a `params_schema` requiring `count: integer`, then issues a combined update carrying a VALID body overwrite + a schema-violating params patch. Asserts the call errors (naming `params_schema`) AND the on-disk body is byte-identical to before. Without the fix the body write precedes the validation failure, so the file diverges and the test fails.

Full lib suite green: `cargo test --lib` → 2691 passed, 0 failed, 9 ignored.
## Workarounds

- Send **complete, schema-valid nested objects** in every `params` patch (treat the merge as shallow regardless of the RFC 7396 wording).
- After a `merge_params` failure on a combined patch, `artifact(get, full=true)` and diff body vs params before retrying — retry only the params half (the body half already landed).

## Resume

N/A — fix implemented + verified in the working tree; pending commit by the codescout maintainer session. Once on master, move this file to `docs/issues/archive/`.
## References

- claude-plugins `docs/trackers/skill-loading-session-log.md` (work stream where this fired)
- claude-plugins tracker artifact `cc8cb9e23ab5cc67` (the half-updated artifact, since reconciled)
- Tool schema text: `artifact` → `patch` description ("RFC 7396 merge-patched")
