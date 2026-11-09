---
id: '6d4870275a3e1af6'
kind: spec
status: draft
title: doc(create) — bounded observed-behavior draft
tags:
- ecc
- evaluation
topic: ecc-integration
---

# doc(create) — bounded observed-behavior draft

Extracted from the completed ECC run and retained as a draft. This is observed implementation, not intended product contract. Requirements use the run's inspected source; consulted enforcement files were stable across the pair, while other source files changed. Tests in this draft were inspected, not run by the native evaluator. The controller's separate gate result is recorded in the comparison.

## Scope and evidence

Applied both installed skills: **ecc-spec-miner** and **ecc-silent-failure-review**, after reading `CLAUDE.md`.

- **Workspace:** `/home/marius/work/claude/codescout`
- **Observed HEAD:** `6043fb27aeb21eb03c52238f2af3c189a97c16e9`
- **Dirty state:** the checkout contains existing changes; targeted `git status --short` reported no changes for the inspected source files.
- **Method:** static source navigation through codescout MCP, with absolute workspace pins. Test assertions were **inspected, not run**.
- **Boundary:** `create::call`, its temporary-write guard and directly relevant helpers, nearest dispatcher/error conversion, and selected assertions. No files, artifacts, memory, bug records, mutations, builds, gates, or behavior tests were created or run.

## 1. Bounded observed-behavior spec draft

**Contract status throughout:** observed implementation, not a declaration of the desired product contract. Error-routing intent is additionally documented in the inspected error-handling guide and error-type documentation.

### Requirement: resolve the destination before admission

- **Behavior:** WHEN `repo` is supplied, THEN create selects the matching workspace root; an unknown name produces a recoverable error listing valid names. Otherwise it uses `current_project.abs_path`; absence produces a recoverable error with activation/repo guidance.
- **Enforcement:** [`create::call`](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:363).
- **Evidence state:** code-inspected. These resolution-test bodies were not inspected.

### Requirement: refuse temporary roots against an outside-temp catalog unless opted in

- **Behavior:** WHEN the resolved root starts with the configured temp directory, the catalog has a backing path outside that directory, and `opted_in` is false, THEN create returns a `LibrarianRecoverableError` naming the temp-directory restriction and suggesting an isolated catalog or opt-in.
- **Admission:** this guard permits a non-temp root, a catalog under temp, a catalog with no reported backing path, or an opted-in caller. Admission here does not establish that later create checks will succeed.
- **Enforcement:** [`create::call`](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:400), [`guard_temp_workspace_write`](/home/marius/work/claude/codescout/src/librarian/tools/temp_write_guard.rs:62), [`should_refuse`](/home/marius/work/claude/codescout/src/librarian/tools/temp_write_guard.rs:50).
- **Evidence state:** code-inspected; all guard-test assertions inspected/not-run. The [create-level refusal test](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:2084) checks that the error contains `"temp dir"`; it does **not** assert file or catalog absence.

### Requirement: use captured environment settings and best-effort path resolution

- **Behavior:** WHEN `TempGuardEnv::from_env` constructs the settings, THEN only `CODESCOUT_ALLOW_TEMP_WORKSPACE=1` or case-insensitive `true` enables opt-in. The temp directory is canonicalized if possible. The guard canonicalizes the root and reported catalog path, falling back to their raw paths on failure.
- **Enforcement:** [`TempGuardEnv::from_env`](/home/marius/work/claude/codescout/src/librarian/tools/temp_write_guard.rs:32), [`guard_temp_workspace_write`](/home/marius/work/claude/codescout/src/librarian/tools/temp_write_guard.rs:62).
- **Evidence state:** code-inspected. Environment parsing and canonicalization-failure assertions were not inspected.
- **Qualification:** [`catalog_db_path`](/home/marius/work/claude/codescout/src/librarian/catalog/mod.rs:729) returns `None` both for an empty SQLite filename and for a failed query/conversion; the guard cannot distinguish those outcomes.

### Requirement: reject invalid create inputs before the main create writes

- **Behavior:** malformed arguments produce a host recoverable error. `augment.params_path` is explicitly refused. After admission, create normalizes and validates `rel_path`, rejects existing destinations, and invokes symlink, repository-boundary, metadata, prefix, body, project-requirement, status, date, and protected-file checks before its artifact insertion and parent/file creation.
- **Enforcement:** [`parse_args`](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:306), [`validate_rel_path`](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:9), [`create::call`](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:341).
- **Evidence state:** code-inspected at the call boundary; not every validator implementation was expanded. Inspected/not-run assertions cover [`params_path` refusal](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:1525), [existing-path refusal](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:1229), and [absence of newly created parent directories for selected validation refusals](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:777).
- **Qualification:** this is not a whole-request “no side effects” guarantee: dispatcher audit enrichment precedes create.

### Requirement: write catalog state before the file; compensate for disk failure

- **Behavior:** create first calls artifact upsert/slug minting, then optional augmentation upsert, then creates parent directories and writes the file. An error propagating from either catalog operation prevents the subsequent file write.
- **Disk failure:** WHEN directory creation or file writing fails, THEN `undo_failed_create` attempts artifact deletion and file removal, logs cleanup failures, and returns the original I/O error with context.
- **Enforcement:** [`create::call`](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:506), [`undo_failed_create`](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:330).
- **Evidence state:** code-inspected; assertions inspected/not-run for [artifact-insert failure](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:1247), [disk failure with augmentation](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:1290), and [successful cleanup of a partial file](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:1333). The disk-failure test is Unix-only and skips under effective UID 0.
- **Qualification:** compensation is best-effort, not guaranteed rollback.

### Requirement: complete registration before returning the success object

- **Behavior:** after file creation, a present current project triggers `ensure_registration`; its error propagates without invoking create cleanup. Otherwise create returns `id` and `abs_path`, plus `tracker_hint` for a tracker without augmentation.
- **Enforcement:** [`create::call`](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:569), [`ensure_registration`](/home/marius/work/claude/codescout/src/librarian/tools/worktree.rs:95).
- **Evidence state:** code-inspected. [Basic success assertions](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:607) inspect file/frontmatter existence and artifact lookup; [augmentation assertions](/home/marius/work/claude/codescout/src/librarian/tools/create.rs:1399) inspect stored prompt/params. Neither was run.

### Requirement: preserve error classification through the main MCP boundary

- **Behavior:** the [`Artifact` dispatcher](/home/marius/work/claude/codescout/src/librarian/tools/artifact.rs:529) forwards create errors. The [adapter](/home/marius/work/claude/codescout/src/librarian/adapter.rs:335) bridges librarian recoverable errors to host recoverable errors, preserving message and optional hint. The host returns `isError:false` with `ok:false`, `error`, and optional guidance. Ordinary I/O/database errors remain fatal; the host sends the outermost message and logs the full context chain.
- **Enforcement:** [`bridge_recoverable_error`](/home/marius/work/claude/codescout/src/librarian/adapter.rs:1190), [`select_error_render`](/home/marius/work/claude/codescout/src/server.rs:3013), [`route_tool_error`](/home/marius/work/claude/codescout/src/server.rs:3172).
- **Evidence state:** bridge and routing assertions inspected/not-run: [type conversion](/home/marius/work/claude/codescout/src/librarian/adapter.rs:2523), [fatal preservation](/home/marius/work/claude/codescout/src/librarian/adapter.rs:2540), [`ok:false`](/home/marius/work/claude/codescout/src/server.rs:10087), and [fatal flag](/home/marius/work/claude/codescout/src/server.rs:10180).
- **Separate boundary:** the inspected [standalone librarian mapper](/home/marius/work/claude/codescout/src/librarian/server.rs:86) renders librarian recoverable errors with `error`/optional `hint`, without `ok:false`.
