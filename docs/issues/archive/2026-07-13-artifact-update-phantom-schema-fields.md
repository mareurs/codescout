---
id: d40ce730e603467a
kind: bug
status: fixed
title: 'BUG: artifact(update) schema documents `owner`, `activeForm`, `addBlocks`, `addBlockedBy` — none exist anywhere in the implementation'
owners:
- marius
tags:
- librarian
- artifact-tool
- update
- schema
opened: 2026-07-13
owner: marius
severity: low
---

## Summary

The MCP `artifact` tool's `input_schema()` documents four top-level parameters as valid for `action="update"` — `owner`, `activeForm`, `addBlocks`, `addBlockedBy` — but none of them have any backing implementation anywhere in the codebase. Calling `update` with any of these silently no-ops: no error, no effect, nothing persisted. Found during the friction sweep opened by `docs/issues/2026-07-13-artifact-create-drops-topic.md` (same bug class: a documented/exposed param silently dropped by serde's default "ignore unknown fields" behavior).

These four names exactly match the harness's own `TaskUpdate` tool schema (`activeForm`, `addBlockedBy`, `addBlocks`, `owner`) — almost certainly copy-paste contamination from that unrelated tool's schema into `artifact`'s.

## Symptom (Effect)

Live verification (scratch artifact, deleted after):

```
artifact(action="create", kind="bug", rel_path="docs/issues/_scratch-param-sweep-test.md", title="SCRATCH TEST...", status="open")
→ { "id": "d36818991f7d61e0", ... }

artifact(action="update", id="d36818991f7d61e0",
         patch={"status":"investigating"},
         owner="marius-test-owner",
         activeForm="Testing phantom spinner label field",
         addBlocks=["T-999"], addBlockedBy=["T-998"])
→ { "id": "d36818991f7d61e0", "updated": true }   -- no error

artifact(action="get", id="d36818991f7d61e0", full=true)
→ status: "investigating" (correct -- patch field worked)
  owners: []  (unchanged -- "owner" had no effect)
  -- no trace of activeForm / addBlocks / addBlockedBy anywhere in the response
```

`artifact(action="delete", id="d36818991f7d61e0")` cleaned up the scratch artifact.

## Reproduction

Commit `d3842c7c62de7cc7a15d555938175c522a02ecad`, branch `experiments`.

1. Create any artifact.
2. `artifact(action="update", id="<id>", owner="x", activeForm="y", addBlocks=["a"], addBlockedBy=["b"])`.
3. Observe success response with no error.
4. `artifact(action="get", id="<id>")` shows none of the four values landed anywhere.

## Environment

codescout MCP server, `librarian-mcp` crate, `artifact` tool (`update` action). Rust, in-process SQLite catalog.

## Root cause

`src/librarian/tools/artifact.rs`, `input_schema()`:
- Line ~135: `"addBlocks": { ..., "description": "update: task IDs this artifact blocks" }`
- Line ~140: `"addBlockedBy": { ..., "description": "update: task IDs that block this artifact" }`
- Line ~145: `"owner": { "type": "string", "description": "update: set owner field" }`
- Line ~150: `"activeForm": { ..., "description": "update: present-continuous label shown in spinner" }`

`src/librarian/tools/update.rs`'s `Args` struct (`id`, `patch`, `commit_refresh`, `force`) and `UpdatePatch` struct (`status, title, owners, tags, topic, time_scope, extra, body, body_edits, params` — this one correctly has `#[serde(deny_unknown_fields)]`, confirmed by existing test `unknown_patch_key_rejected`) have no field matching any of the four. `Args` itself has no `#[serde(deny_unknown_fields)]`, so top-level unknown keys (`owner`, `activeForm`, `addBlocks`, `addBlockedBy`) are silently dropped by serde on deserialize — same mechanism as the `topic`-on-create bug.

Repo-wide grep for `addBlocks|addBlockedBy|activeForm` (`src/`) finds matches ONLY in `artifact.rs`'s schema declaration — zero implementation, zero storage column, zero task-dependency concept anywhere in `src/librarian/`. `owner` (singular) similarly has no backing — `update.rs` only has plural `owners: Option<Vec<String>>` inside `UpdatePatch`, and the only other `owner` string hits in `src/librarian/` (`filter.rs` lines 687/723/731) are generic filter-AST-engine test fixtures using an arbitrary hypothetical field name, unrelated to the artifact model.

Unlike the `topic` bug, there is no legitimate feature here to finish wiring up — this looks like accidental schema copy-paste, not an incomplete implementation.

## Evidence

See Symptom section above for the full live round trip. Static grep evidence:

```
grep "addBlocks|addBlockedBy|blocked_by|blocks_ids|activeForm" src/  → 3 matches, all in src/librarian/tools/artifact.rs (schema only)
grep "\bowner\b" src/librarian/  → hits in tracker_design.rs (prose), filter.rs (generic test fixtures), current_project.rs, audit_doc_refs (unrelated), artifact.rs (schema only — 2 hits: "owners" array description + the phantom "owner" singular)
```

## Hypotheses tried

1. **Hypothesis:** These fields are a partially-implemented task-dependency feature for artifacts (mirroring the harness's Task tool), just not finished.
   **Test:** Grepped all of `src/` for any task-dependency storage (`blocks`, `blocked_by`, a join table, a catalog column) and any `owner`-singular write path.
   **Verdict:** rejected — zero hits anywhere outside the schema declaration itself. If this were a partial feature, some catalog/model scaffolding would exist even if the wiring were incomplete; there is none.
   **Evidence link:** grep results above.

2. **Hypothesis:** `owner` (singular) might be a legitimate shortcut alias for setting a single-element `owners` list.
   **Test:** Checked `update.rs`'s frontmatter-patch helper (`apply_patch_to_frontmatter` or equivalent, lines ~84-101) for any singular-to-plural coercion.
   **Verdict:** rejected — only `patch.owners` (plural, `Option<Vec<String>>`) is read; no singular alias logic exists.

## Fix

Implemented via TDD: removed the four phantom property declarations (`addBlocks`, `addBlockedBy`, `owner`, `activeForm`) from `input_schema()` in `src/librarian/tools/artifact.rs`. No behavior change for real callers — these fields never did anything, so nothing that worked before can now break. Chose deletion over implementation: there is no scaffolding anywhere (no catalog column, no task-dependency table) to suggest a genuine half-built feature worth finishing, and building real task-dependency tracking for artifacts would be speculative new-feature work outside the scope of a bug fix.

Not yet committed — changes are in the working tree on branch `experiments`. SHA to be filled in once committed.
## Tests added

`input_schema_has_no_phantom_update_fields` — `src/librarian/tools/artifact.rs` (tests module, inserted after `find_action_routes_correctly`). Asserts `Artifact.input_schema()`'s `properties` object contains none of `owner`, `activeForm`, `addBlocks`, `addBlockedBy` — guards against reintroducing them via a future copy-paste from an unrelated tool schema.

Verified RED (failed with `schema documents \`owner\` but update.rs has no field backing it` before the fix) then GREEN after. Full suite: `cargo test --all-targets` → 3187 passed, 0 failed. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` both clean.
## Workarounds

None needed — these fields never worked, so there's no existing caller relying on them (would need to grep this project's own call sites first if that assumption needs re-checking before removal).

## Resume

Fixed on branch `experiments`, commit `a5743870` ("fix(librarian): remove phantom addBlocks/addBlockedBy/owner/activeForm from artifact schema"). Not yet cherry-picked to `master` — per CLAUDE.md § "After cherry-pick", once it lands on `master` re-run `git rev-parse HEAD` there and cite that SHA before archiving to `docs/issues/archive/`.
## References

- Sibling finding from the same sweep, same root mechanism: `docs/issues/2026-07-13-artifact-create-drops-topic.md` (id `8dfa0da20703f46c`).
- `src/librarian/tools/artifact.rs` — `input_schema()`.
- `src/librarian/tools/update.rs` — `Args`, `UpdatePatch`, `call()`.
