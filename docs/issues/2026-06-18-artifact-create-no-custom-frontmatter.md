---
id: '13164fb35d6f71ed'
kind: bug
status: open
title: 'BUG: artifact(action="create") cannot set time_scope or custom frontmatter — recognized field hardcoded to None, no passthrough param'
owners: []
tags:
- librarian
- artifact-create
- artifact-update
- frontmatter
- tracker
topic: null
time_scope: null
---


## Summary

`artifact(action="create")` hard-codes `time_scope: None` in both the
`Frontmatter` struct and the `ArtifactRow` it writes, despite `time_scope`
being a first-class recognized catalog field (`Frontmatter` struct,
`ArtifactRow`, and the `write_field_to_frontmatter` WRITABLE allowlist in
`src/librarian/tools/update.rs`). There is no `time_scope` param on the
create API, and no `extra`/`frontmatter` passthrough for arbitrary custom
keys. The same gap exists in `artifact(action="update")`'s `patch`: the
`UpdatePatch` struct (`src/librarian/tools/update.rs:10-36`) accepts only
`status`, `title`, `owners`, `tags`, `topic`, `body`, `body_edits`, `params`
— `time_scope` is not patchable either.

## Repro

```
artifact(action="create",
  kind="tracker",
  title="Session Passover – 2026-06-18",
  rel_path="docs/trackers/session-passover-2026-06-18.md",
  # FAILS silently — no time_scope param exists:
  # time_scope="2026-06-18",
  body="..."
)
# time_scope is not written; field is None in catalog and absent from frontmatter
```

Concrete case: a session-passover tracker also needs correlation keys
`origin_session_id` and `branch` in frontmatter so downstream consumers can
filter by session without parsing the body. These are custom keys — not
recognized by the `Frontmatter` struct at all — so they had to be stashed in
the body instead, losing the ability to query them as catalog fields.

Cross-ref: passover artifact `cada4e50e6b3cfba` (claude-plugins repo) surfaced
this limitation when building the session-passover tracker pattern.

## Expected vs Actual

**Expected:** `artifact(action="create", kind="tracker", ..., time_scope="2026-W25")`
writes `time_scope: 2026-W25` in the YAML frontmatter and populates
`ArtifactRow.time_scope` in the catalog. Similarly, `artifact(action="update",
patch={time_scope: "..."})` should work.

**Actual:**

- `create` — `time_scope` is not a recognized param (`Args` struct,
  `src/librarian/tools/create.rs:34-46`); the field is hardcoded to `None`
  (`src/librarian/tools/create.rs` inside `call`, `Frontmatter { ...,
  time_scope: None }` and `ArtifactRow { ..., time_scope: None }`).
- `update` — `UpdatePatch` is `#[serde(deny_unknown_fields)]`
  (`src/librarian/tools/update.rs:10-36`); passing `patch={time_scope: "..."}` 
  returns a deserialization error. `time_scope` is not in the struct.
- Custom keys (e.g. `origin_session_id`, `branch`) — not modeled anywhere in
  the create/update surface; no passthrough path exists.

## Bug vs Enhancement Determination

**Mixed — one is a bug, one is an enhancement:**

1. **Bug** (`time_scope`): `time_scope` is a recognized first-class field in
   `Frontmatter` (`src/librarian/frontmatter.rs:19`), `ArtifactRow`, and the
   `write_field_to_frontmatter` WRITABLE allowlist
   (`src/librarian/tools/update.rs:350`, `WRITABLE = &["status", "title",
   "topic", "time_scope"]`). The API surface simply never wired up a param for
   it on create or update. This is an omission — the field is intentional and
   recognized; it just can't be set at creation time or patched afterward.

2. **Enhancement** (custom frontmatter passthrough): Arbitrary keys like
   `origin_session_id` or `branch` are not part of the `Frontmatter` struct at
   all. Supporting them requires a deliberate design decision (pass-through
   serialization, schema validation, catalog column vs. opaque YAML-only field,
   etc.). This is a net-new capability.

## Impact

- Trackers that need temporal scoping cannot set `time_scope` at create time
  without a follow-up body edit hack.
- Correlation/provenance metadata (session id, branch, origin) cannot live in
  frontmatter — it must be stuffed into the body, losing queryability via
  `artifact(find)` field filters.
- Session-passover tracker pattern (claude-plugins, passover artifact
  `cada4e50e6b3cfba`) is the concrete case: needed `origin_session_id` +
  `branch` + `time_scope` as frontmatter; ended up embedding them in the body
  because the create API has no path for them.

## Fix Sketch

**Part 1 — `time_scope` (bug fix, low-risk):**

1. Add `time_scope: Option<String>` to `Args` in
   `src/librarian/tools/create.rs`.
2. Thread it into `Frontmatter { ..., time_scope: a.time_scope.clone() }` and
   `ArtifactRow { ..., time_scope: a.time_scope }` in `create::call`.
3. Add `time_scope: Option<String>` to `UpdatePatch` in
   `src/librarian/tools/update.rs` (remove or relax `deny_unknown_fields` for
   this field) and thread it through all three body-building paths in
   `update::call` (mirroring the existing `topic` handling).
4. Update the MCP tool schema descriptions for `artifact(action="create")` and
   `artifact(action="update")` in `src/librarian/tools/artifact.rs`.

**Part 2 — custom frontmatter passthrough (enhancement, needs design):**

Options: (a) an `extra: Map<String, Value>` param serialized as opaque YAML
keys not indexed in the catalog SQL; (b) a typed extension mechanism with
per-field catalog column registration; (c) YAML-only (readable by tools but
not filterable). Option (a) is simplest and covers the passover use-case —
body consumers can parse the YAML; `artifact(find)` filters on standard fields.
The tradeoff (no SQL filtering on custom keys) should be documented.


## Part 1 — Shipped & Verified (2026-06-19)

`time_scope` is now a first-class param on both `artifact(action="create")` and
`artifact(action="update", patch={...})`.

**Code:**
- `src/librarian/tools/create.rs` — `Args.time_scope: Option<String>`; threaded into
  both `Frontmatter` and `ArtifactRow` (replacing the hardcoded `None`s).
- `src/librarian/tools/update.rs` — `UpdatePatch.time_scope`; mirrored `topic` across
  **all five** mutation sites (the body-overwrite fm path, the `fm_changing` guard, the
  `body_edits` `update_in_place` closure, the no-body `update_in_place` closure, and
  `updated_row`). The `fm_changing` guard was the non-obvious one — without it, a
  `body_edits`-path `time_scope` patch would silently no-op.
- `src/librarian/tools/artifact.rs` — added the `time_scope` schema property + updated
  the `patch` Accepted-keys list and the rel_path-rejection hint.
- `src/prompts/guides/librarian.md` — Accepted-keys line.

**Tests:** `create_with_time_scope_persists_to_row_and_frontmatter` (create.rs),
`update_time_scope_persists_to_row_and_frontmatter` (update.rs). Both assert the catalog
row **and** the on-disk YAML frontmatter. Full `cargo test --lib` 2792 pass, clippy clean.

**Live-verified** through the restarted MCP server: `artifact(create, …, time_scope="2026-W25")`
→ catalog `time_scope: "2026-W25"`; `artifact(update, patch={time_scope:"2026-Q3"})`
→ `time_scope: "2026-Q3"`, body preserved. Probe artifact deleted.

Commit (experiments-side): *recorded post-commit; master-side SHA pending cherry-pick.*

## Part 2 — Custom-frontmatter passthrough (implemented 2026-06-19)

Scope chosen: **passthrough only, not filterable** (no schema migration). `ALLOWED_FIELDS`
in `src/librarian/filter.rs` is a strict allowlist (with `rejects_non_allowlisted_column`
guard), so arbitrary keys can't be `find`-filtered without a real catalog column — that was
the deferred-as-too-heavy option. Custom keys are YAML-only: round-trip-safe, surfaced by
`get`, not catalog-indexed.

**Code:**
- `src/librarian/frontmatter.rs` — `Frontmatter.extra: BTreeMap<String, serde_json::Value>`
  via `#[serde(flatten)]` (dropped the `Eq` derive — `serde_json::Value` isn't `Eq`).
  This **also fixes a latent round-trip bug**: previously `update_in_place` dropped unknown
  keys, so any `artifact(update)` on a file with custom frontmatter (e.g. these bug files'
  `opened`/`closed`/`severity`/`owner`/`related`) silently wiped them.
- `src/librarian/tools/create.rs` — `Args.extra` → `Frontmatter.extra`.
- `src/librarian/tools/update.rs` — `UpdatePatch.extra` + `merge_extra` helper (upsert key,
  `null` deletes, omitted preserved), threaded across all four fm-mutation sites.
- `src/librarian/tools/get.rs` — surfaces non-empty `extra` from the parsed frontmatter.
- `src/librarian/tools/artifact.rs` — `extra` schema property + Accepted-keys list.
- `src/prompts/guides/librarian.md` — Accepted-keys + tradeoff note.

**Tests:** `captures_unknown_fields_into_extra`, `round_trip_preserves_extra` (frontmatter.rs);
`create_with_extra_writes_custom_frontmatter` (create.rs);
`update_extra_merges_preserves_and_deletes` — incl. the round-trip-safety assertion that
changing an unrelated field does NOT wipe `extra` (update.rs). Full `cargo test --lib` 2795
pass, clippy clean.

Both parts now implemented. `status` stays `open` pending live MCP verification of Part 2
(create/get/update with `extra` through the restarted server); flips to `fixed` after.
