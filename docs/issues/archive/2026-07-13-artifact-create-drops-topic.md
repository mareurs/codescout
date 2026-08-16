---
id: '5fec6397997603cb'
kind: bug
status: fixed
title: 'BUG: artifact(create) silently drops `topic` — not a recognized create param, hardcoded to null'
owners:
- marius
tags:
- librarian
- artifact-tool
- create
opened: 2026-07-13
owner: marius
severity: low
---

## Summary

`artifact(action="create", topic="...")` silently ignores the `topic` value — the created artifact's `topic` field is always `null`, and there is no error or warning. `topic` must be set via a follow-up `artifact(action="update", patch={topic: "..."})` call.

Originally reported as "create silently drops `status` and `topic`" — investigation (see Hypotheses tried) showed `status` is NOT affected; only `topic` is.

## Symptom (Effect)

Calling create with `topic` set has no effect on the resulting artifact:

```
artifact(action="create", ..., status="active", topic="scratch-verification-topic")
→ { "id": "961fbfd8d295a110", "abs_path": "docs/issues/_scratch-verify-create-params.md" }

artifact(action="get", id="961fbfd8d295a110", full=true)
→ { ..., "status": "active", "topic": null, ... }
```

`status` round-trips correctly (`"active"` in → `"active"` out). `topic` does not (`"scratch-verification-topic"` in → `null` out). No error is raised; the call reports success.

## Reproduction

Commit `61a800af6570ec60b00c412404d4e98528983a9c`, branch `experiments`.

1. `artifact(action="create", kind="note", title="...", rel_path="docs/issues/_scratch.md", body="...", topic="my-topic")`
2. `artifact(action="get", id="<returned id>", full=true)`
3. Observe `"topic": null` in the response instead of `"my-topic"`.

## Environment

codescout MCP server, `librarian-mcp` crate, `artifact` tool (`create` action). Rust, in-process SQLite catalog.

## Root cause

`src/librarian/tools/create.rs`:

- `Args` (lines 45–60) has no `topic` field at all, so `topic` in the JSON args is silently dropped by serde's default "ignore unknown fields" behavior (no `#[serde(deny_unknown_fields)]` on `Args`).
- In `call()` (lines 61–188), both the `Frontmatter` literal and the `ArtifactRow` literal hardcode `topic: None` unconditionally — there is no code path that could set it even if `Args` carried the value.
- The top-level MCP schema (`src/librarian/tools/artifact.rs`, `input_schema()`, lines 26–187) has no `topic` property at all — unlike `status` and `time_scope`, which are both documented there as `"create/update: ..."`. So the gap is visible at the schema layer too, not just internally.

By contrast, `status` (`Args::status: Option<String>`, `create.rs:55`) IS read and applied: `let status = a.status.as_deref().unwrap_or("draft").to_string();` (create.rs, inside `call()`), used for both `Frontmatter.status` and `ArtifactRow.status`. Confirmed via test `create_with_explicit_status_active` (create.rs tests module) and via live verification below — `status` is not broken.

`topic` IS patchable post-creation: `artifact(action="update", patch={topic: "..."})` is a declared/accepted key (see `update.rs` patch handling / `get_guide("librarian")` § patch accepted keys). So the workaround (create, then update) already works — the gap is purely "can't set it at creation time."

## Evidence

Live verification this session (scratch artifact, deleted after): created with `status="active", topic="scratch-verification-topic"`, then `artifact(get, full=true)` returned:

```json
{
  "id": "961fbfd8d295a110",
  "status": "active",
  "topic": null,
  ...
}
```

`status` round-tripped; `topic` did not. Scratch artifact `961fbfd8d295a110` (`docs/issues/_scratch-verify-create-params.md`) was deleted via `artifact(action="delete")` immediately after.

Static evidence — `src/librarian/tools/create.rs` `Args` struct (lines 45–60) and `call()` body (lines 61–188): both `Frontmatter{ topic: None, .. }` and `ArtifactRow{ topic: None, .. }` are unconditional literals; no `a.topic` exists to feed them.

## Hypotheses tried

1. **Hypothesis:** Both `status` and `topic` are dropped by `create`, as originally reported.
   **Test:** Read `create.rs` `Args` struct and `call()` body; live-created a scratch artifact with both fields set and inspected it via `get`.
   **Verdict:** rejected (partially) — `status` is correctly applied (`Args::status`, create.rs:55, used unconditionally in both `Frontmatter` and `ArtifactRow`); only `topic` is dropped.
   **Evidence link:** see Evidence section above (live verification + static read).

2. **Hypothesis:** `topic` is dropped only from the DB row (`ArtifactRow`) but written to frontmatter YAML on disk.
   **Test:** Inspected the `Frontmatter` literal in `call()` — `topic: None` is hardcoded there too, not just on `ArtifactRow`.
   **Verdict:** rejected — both the on-disk frontmatter and the catalog row are affected identically; there's no split-brain state to reconcile.

## Fix

Implemented via TDD:

1. Added `pub topic: Option<String>` to `Args` in `src/librarian/tools/create.rs`.
2. In `call()`, replaced both hardcoded `topic: None` literals (`Frontmatter` and `ArtifactRow`) with `a.topic.clone()` / `a.topic`.
3. Added a `"topic"` property to the MCP tool schema in `src/librarian/tools/artifact.rs` (`input_schema()`), positioned after `status` (matching `Args` field order), documented consistently with `status`/`time_scope`.
4. `src/prompts/guides/librarian.md` § "artifact(action=\"create\") — Required Fields" already documented `topic` as a create param — this was doc/code drift (doc described intended behavior that the code didn't implement); no doc change needed, the fix makes reality match the doc.

Not yet committed — changes are in the working tree on branch `experiments`. SHA to be filled in once committed.
## Tests added

`create_with_topic_persists_to_row_and_frontmatter` — `src/librarian/tools/create.rs` (tests module, inserted after `create_with_time_scope_persists_to_row_and_frontmatter`). Modeled on the existing `time_scope` regression test; asserts `topic` round-trips through both the catalog row and the on-disk YAML frontmatter.

Verified RED (failed with `left: None, right: Some("auth middleware")` before the fix) then GREEN after. Full suite: `cargo test --all-targets` → 3185 passed, 0 failed. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` both clean. Live-MCP release binary rebuilt (`cargo rb`, release profile, 47.10s) — pending `/mcp` reconnect for an end-to-end MCP-level check.
## Workarounds

Set `topic` via a follow-up call after `create`:

```
artifact(action="update", id="<id>", patch={topic: "my-topic"})
```

## Resume

Fixed on branch `experiments`, commit `d3842c7c` ("fix(librarian): honor topic on artifact(create), was silently dropped"). Not yet cherry-picked to `master` — per CLAUDE.md § "After cherry-pick", once it lands on `master` re-run `git rev-parse HEAD` there and cite that SHA (not this experiments-side one, which orphans after rebase) before archiving to `docs/issues/archive/`.
## References

- Related but distinct (already fixed): `docs/issues/2026-06-18-artifact-create-no-custom-frontmatter.md` (id `13164fb35d6f71ed`) — same class of "create can't set field X" bug, but for `time_scope`/`extra`, already resolved.
- `src/librarian/tools/create.rs` — `Args` struct, `call()`.
- `src/librarian/tools/artifact.rs` — `input_schema()`.
- `src/librarian/tools/update.rs` — where `topic` IS an accepted `patch` key (the working workaround path).
