---
id: '4b9cb04dfdfea67e'
kind: report
status: done
title: doc(create) baseline — retained native response
tags:
- ecc
- evaluation
topic: ecc-integration
---

# Retained baseline response

Captured from a completed native default-profile run. This is an evaluation output, not an authoritative product contract. See the comparison report for adjudication and source stability.

## 1. Bounded observed-behavior spec draft

**Evidence state:** static inspection of the real `/home/marius/work/claude/codescout` checkout through codescout MCP, pinned to that workspace. Repository instructions were read. No ECC adapter skills, behavior tests, builds, gates, mutations, or persistent writes were performed. “Observed” below means source and assertions inspected, not runtime behavior reproduced.

### Temporary workspace admission

[create::call](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:341) parses arguments, rejects non-null `augment.params_path`, and resolves the base directory before invoking the temporary-workspace guard. An explicit `repo` selects a named workspace root; otherwise the current project’s `abs_path` supplies the root. Unknown repositories and missing active projects produce recoverable errors.

[guard_temp_workspace_write](/home/marius/work/claude/codescout/src/librarian/tools/temp_write_guard.rs:62) canonicalizes the workspace root and catalog filename where possible, then delegates to [should_refuse](/home/marius/work/claude/codescout/src/librarian/tools/temp_write_guard.rs:50).

| Inputs after resolution | Guard outcome |
|---|---|
| Root under the configured temporary directory; file-backed catalog outside it; no opt-in | Refuse |
| Catalog also under the temporary directory | Admit |
| Catalog path is `None`, including an in-memory catalog | Admit |
| Root outside the temporary directory | Admit |
| Explicit opt-in | Admit |

“Under” is Rust path-prefix comparison, `Path::starts_with`, including equality. The guard classifies **any reported file-backed catalog outside the temporary directory** as “real”; it does not identify a particular shared catalog.

[TempGuardEnv::from_env](/home/marius/work/claude/codescout/src/librarian/tools/temp_write_guard.rs:34) snapshots environment inputs. Only `1` or case-insensitive `true` enables `CODESCOUT_ALLOW_TEMP_WORKSPACE`; other values, missing values, and environment-read errors do not enable it. Temporary-directory canonicalization falls back to the raw temporary path.

Two further admission fallbacks matter:

- Root or catalog canonicalization failure uses the corresponding raw path.
- [catalog_db_path](/home/marius/work/claude/codescout/src/librarian/catalog/mod.rs:729) maps both an empty database filename **and a query/conversion error** to `None`. The guard therefore cannot distinguish “in-memory” from “catalog location unavailable.”

Refusal returns a `LibrarianRecoverableError` explaining the pollution risk and suggesting an isolated catalog or explicit opt-in. It precedes create’s artifact-row, directory, and file writes. This does **not** establish whole-request side-effect freedom: the surrounding dispatcher attempts an audit stamp, and the adapter can perform catalog maintenance before invoking create.

### Create outcomes and ordering

The inspected [create::call](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:341) establishes this sequence:

1. Parse required `rel_path`, `kind`, `title`, and `body`; default owners, tags, and extra fields. Nested augmentation rejects unknown keys. Non-null `augment.params_path` is explicitly refused.
2. Resolve the base directory and apply temporary-workspace admission.
3. Normalize the relative path, reject absolute paths and `..`, refuse an existing target, and invoke leaf-symlink and repository-containment checks.
4. Check reserved extra keys, prefix ownership, leading body frontmatter, project requirements, status, and bug `opened` date before artifact persistence.
5. Render frontmatter/body and invoke protected-file validation.
6. Upsert the artifact and mint its slug; optionally upsert augmentation.
7. Create parent directories and perform a plain `fs::write`.
8. Register the current worktree when applicable.
9. Return `id` and forward-slash `abs_path`; add `tracker_hint` for a tracker created without augmentation.

Important bounds:

- Bug status defaults to `open`; other kinds default to `draft`. Requested bug statuses are vocabulary-checked; other kinds’ statuses deliberately remain free-form. See [resolve_status](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:289).
- Validator refusal occurs before artifact persistence and parent-directory creation. This is distinct from failures during persistence.
- Artifact/slug/augmentation errors propagate before file creation, but earlier catalog changes are not compensated by create.
- Directory/file-write failure invokes [undo_failed_create](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:330), which attempts artifact deletion and file removal. Cleanup is best effort; created parent directories are not removed.
- Worktree-registration failure propagates **after** the file and catalog writes, without create-level compensation.

### Caller-visible error conversion

The nearest path inspected was:

[Artifact::call](/home/marius/work/claude/codescout/src/librarian/tools/artifact.rs:529) → `create::call` → [LibrarianAdapter::call](/home/marius/work/claude/codescout/src/librarian/adapter.rs:335) → [bridge_recoverable_error](/home/marius/work/claude/codescout/src/librarian/adapter.rs:1190) → [select_error_render](/home/marius/work/claude/codescout/src/server.rs:3013) / [route_tool_error](/home/marius/work/claude/codescout/src/server.rs:3172).

- Librarian recoverable errors become host recoverable errors, retaining message and optional hint.
- Host recoverable errors render a JSON body with `ok:false`, `error`, and optional guidance through the successful MCP-result constructor. Transport success therefore does not mean create succeeded.
- Ordinary filesystem/database failures reach the fatal route: an error MCP result containing the outermost error message. The complete context chain is logged server-side.
- The shared router also recognizes particular transient-LSP error text; that is not evidence that ordinary create failures receive transient treatment.

### Inspected assertion evidence

These assertions exist; none were executed:

| Area | Assertions actually inspected |
|---|---|
| Guard decision and wrapper | [guard tests](/home/marius/work/claude/codescout/src/librarian/tools/temp_write_guard.rs:128) assert refusal and each admission branch, wrapper recoverable-error type, and opposite verdicts on the same fixture with opt-in flipped. |
| Fixture validity | [synthetic_temp](/home/marius/work/claude/codescout/src/librarian/tools/temp_write_guard.rs:113) injects a synthetic temporary root; both physical fixture locations remain under the OS temporary directory. |
| Create wiring | [temporary-workspace refusal test](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:2084) expects an error containing `temp dir`. It does not assert unchanged catalog/file state or final MCP shape. |
| Basic success | [creates_file_and_row](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:607) checks file presence, frontmatter content, and lookup of the returned artifact ID. |
| Early refusal | [validator-refusal test](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:777) checks no new parent directory for malformed date, leading frontmatter, and invalid bug status. |
| Path refusal | [dangling-link test](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:816) checks target absence and link preservation; [ancestor-link test](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:917) checks no outside directory creation. |
| Persistence failure | [artifact-insert failure test](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:1247) asserts an error and no orphan file. [disk-write failure test](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:1290) checks zero artifact/augmentation rows and no file; it is Unix-only and returns early as root. |
| Cleanup | [cleanup-helper test](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:1333) checks successful deletion of a prepared row/empty file and repeated invocation without panic. It does not inject cleanup failure. |
| Conversion | [bridge assertions](/home/marius/work/claude/codescout/src/librarian/adapter.rs:2523) check host type/message/guidance and fatal-error preservation. Router assertions inspect [recoverable](/home/marius/work/claude/codescout/src/server.rs:10280) and [fatal](/home/marius/work/claude/codescout/src/server.rs:10381) result shapes separately. |

## 2. Silent-failure review

### Confirmed reporting defect: cleanup success is asserted without being known

[undo_failed_create](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:330) logs artifact-deletion failure and returns no cleanup outcome. Nevertheless, the disk-write error constructed by [create::call](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:561) always states:

> the catalog row was taken back

If deletion fails, that statement is false and the caller receives no indication that the catalog may retain the row. File-removal failure is likewise only logged.

The original write failure remains an error, so this is **misleading recovery reporting**, not a false successful create. Additionally, the fatal router exposes only this outer context, withholding the underlying OS error from the caller.

**Evidence:** confirmed control flow and returned-message construction; no cleanup failure reproduced. The inspected cleanup tests cover successful cleanup, not this failure branch.

### Confirmed partial outcomes; atomicity needs contract adjudication

The boundary has no transaction spanning artifact creation, augmentation, disk write, and registration:

- [upsert_and_mint_slug](/home/marius/work/claude/codescout/src/librarian/catalog/artifact.rs:336) performs artifact upsert before slug minting.
- Create performs [augmentation::upsert](/home/marius/work/claude/codescout/src/librarian/catalog/augmentation.rs:35) afterward and propagates failure without invoking cleanup.
- [ensure_registration](/home/marius/work/claude/codescout/src/librarian/tools/worktree.rs:95) can return an error after the file has been written.

Consequently, in ordinary autocommit operation, an error can accompany a persisted artifact row without its file, or a completed artifact/file with failed registration. The error response does not describe these partial outcomes.

[AugmentSpec’s comment](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:49) uses “created atomically with its augmentation,” but the inspected sequence does not establish transactional atomicity.

**Classification:** confirmed partial-state/error behavior and a contract ambiguity. I would not declare every such outcome a defect without adjudicating the intended atomicity guarantee. No corresponding failure injection was performed.

### Permitted fallbacks and open questions

- **Canonicalization fallback:** explicitly documented as fail-open. It can weaken temporary-root classification, but is not an accidentally swallowed error in this implementation.
- **Catalog-location failure:** `.ok()?` silently conflates lookup failure with an in-memory catalog and therefore admits the write. This information loss is confirmed; whether refusal is required when location is unknown remains open. No reachable lookup failure was demonstrated.
- **Audit stamping:** the dispatcher deliberately logs stamping failure and continues. The local comment defines it as best-effort enrichment; no create correctness defect established.
- **Recoverable transport success:** carries `ok:false` and an error message. This is an explicit error protocol, not silent success.
- **Non-bug free-form statuses:** explicitly permitted by the local implementation contract.

### Coverage and limitations

The initial references request timed out; a later request returned a limited reference set, and a depth-one call graph returned caller edges tagged as LSP-derived. These are navigation results, **not proof of complete caller coverage or absence of dynamic callers**. Only the nearest doc dispatcher/error-conversion path was followed; CLI and other returned callers were not reviewed.

Uninspected scope includes unrelated doc actions, full delegated validator internals, catalog-wide transaction/trigger behavior, GC internals, upstream access/locking machinery, filesystem races, and platform runtime behavior. Evaluation results, pilot worktrees, and other run outputs were not read. No executed-pass or mutation-survival claim is made.
