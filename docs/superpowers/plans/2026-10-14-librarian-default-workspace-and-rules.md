---
id: b311929af4aa7b50
kind: plan
status: done
title: 'Librarian first-run defaults: create workspace.toml and ship the workflow classifier rules built in'
owners:
- marius
tags:
- librarian
- onboarding
- plan
---

# Librarian first-run defaults Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A new machine reaches a working librarian with no manual file: the server creates `~/.config/librarian/workspace.toml` when absent, and the built-in classifier rules cover the workflow directories.

**Architecture:** `DEFAULT_RULES_TOML` gains ten rules, so no file has to carry them. A new `workspace::load_or_init` creates an empty, commented file at the default path only. `LibrarianEnv` gets an explicit `init_default_workspace` flag that only `from_env()` sets, so tests never write into the real config directory.

**Tech Stack:** Rust, `toml`, `serde`, `dirs`, `tempfile` (tests).

**Spec:** `docs/issues/archive/2026-10-14-new-project-setup-does-not-configure-the-librarian.md` (id `ae511945b43ba0c8`; archived after the fix, formerly `docs/issues/2026-10-14-new-project-setup-does-not-configure-the-librarian.md`). The decisions below were taken with the user during brainstorming on 2026-10-14; there is no separate design document.

## Global Constraints

- Create the file only when `LibrarianEnv::workspace` is `None` (the default path). A missing file named by `LIBRARIAN_WORKSPACE` stays an error and creates nothing.
- Never overwrite an existing file. Creation uses `OpenOptions::create_new`.
- If creation fails (unwritable location), the server starts with `WorkspaceConfig::default()` and logs one `tracing::warn!`.
- A malformed existing file stays an error. It is never replaced.
- The created file is empty apart from comments. It carries no `[[rule]]` blocks and no `[[roots]]` (deprecated).
- `LibrarianEnv::default()` must leave `init_default_workspace` false. No test calls `std::env::set_var` (`docs/conventions/test-env-isolation.md`).
- User rules stay ahead of built-in rules. A frontmatter `kind:` stays ahead of both (`src/librarian/indexer.rs:454-458`).
- Out of scope: the "librarian started but cannot search" visibility problem, and the INFO-level log in `src/librarian/adapter.rs:34`. They get their own brainstorm.
- The checkout is shared. Stage files by explicit path, never `git add -A` (`docs/conventions/shared-checkout-commit-sequence.md`).
- Run `./scripts/gate.sh` before Task 4 closes the issue. Targeted runs use `scripts/with-slot.sh cargo test …`.

## Review Focus

- Existing user file with rules that overlap the new built-ins: the user rule must win. Test in Task 1 (`user_rules_override_defaults_when_concatenated` already pins it; keep it green).
- A file with a frontmatter `kind:` inside a directory that now has a built-in rule keeps its own kind. Test in Task 1.
- Read-only or impossible config directory: the server must still start. Test in Task 2.
- Malformed existing `workspace.toml` with creation enabled: error, file bytes unchanged. Test in Task 2.
- `LIBRARIAN_WORKSPACE` naming a missing file: still an error, nothing created. Tests in Task 2.
- Two servers starting at once on a clean machine: the one that loses `create_new` must load the file, not fail. The `AlreadyExists` branch cannot be forced from a unit test; the reviewer reads it.

---

### Task 1: Built-in classifier rules

**Files:**
- Modify: `src/librarian/classify.rs` — `DEFAULT_RULES_TOML` (lines 66-130); tests module (lines 182-378)
- Modify: `src/librarian/indexer.rs` — tests module (one new test)

**Interfaces:**
- Consumes: `load_rules`, `default_rules`, `classify` as they exist.
- Produces: `default_rules()` returns ten more rules. No signature changes.

- [ ] **Step 1: Write the failing classification test** in `src/librarian/classify.rs` tests: `default_rules_classify_the_workflow_directories`. It loops over this table, calls `classify(&default_rules().unwrap(), path)`, and asserts `kind`, `status` and `time_scope`:

  | path | kind | status | time_scope |
  |---|---|---|---|
  | `docs/superpowers/specs/a.md` | `spec` | `Some("active")` | `None` |
  | `docs/superpowers/plans/b.md` | `plan` | `None` | `None` |
  | `docs/research/2026-01-01-c.md` | `memory` | `None` | `Some("dated_snapshot")` |
  | `docs/audits/d.md` | `audit` | `None` | `None` |
  | `docs/handoffs/e.md` | `handoff` | `None` | `None` |
  | `docs/runbooks/f.md` | `runbook` | `None` | `None` |
  | `docs/adrs/g.md` | `adr` | `Some("active")` | `None` |
  | `ROADMAP.md` | `roadmap` | `None` | `None` |
  | `docs/manual/sub/h.md` | `doc` | `None` | `None` |
  | `docs/trackers/sub/t.md` | `tracker` | `Some("active")` | `None` |

  Add a control row: `docs/misc/notes.md` → `classify` returns `None`. Put the failing path in every assertion message.
- [ ] **Step 2: Run it and see it fail.** `scripts/with-slot.sh cargo test --lib classify::tests::default_rules_classify_the_workflow_directories` → FAIL on the first row (`unwrap` of `None`).
- [ ] **Step 3: Append the ten rules** to `DEFAULT_RULES_TOML`, same `[[rule]]` shape as its neighbors, with `glob`, `kind`, and `status` / `time_scope` only where the table above has a value. Globs, in table order: `**/docs/superpowers/specs/*.md`, `**/docs/superpowers/plans/*.md`, `**/docs/research/*.md`, `**/docs/audits/*.md`, `**/docs/handoffs/*.md`, `**/docs/runbooks/*.md`, `**/docs/adrs/*.md`, `**/ROADMAP.md`, `**/docs/manual/**/*.md`, `**/docs/trackers/**/*.md`. Extend the doc comment above the constant: it lists what the defaults cover.
- [ ] **Step 4: Run the test** from Step 2 → PASS. Run `scripts/with-slot.sh cargo test --lib classify::` → all PASS.
- [ ] **Step 5: Write the frontmatter-precedence test** in `src/librarian/indexer.rs` tests: `a_frontmatter_kind_beats_a_built_in_rule`. Follow the neighbor `indexes_fixture_repo_with_mixed_classifications`: temp repo, `docs/superpowers/specs/x.md` with frontmatter `kind: plan`, rules from `default_rules()`, then `index_repo_sync`. Assert the stored row's `kind` is `plan`, not `spec`.
- [ ] **Step 6: Run it:** `scripts/with-slot.sh cargo test --lib indexer::tests::a_frontmatter_kind_beats_a_built_in_rule` → PASS (it pins existing behavior; it must also pass on the old code).
- [ ] **Step 7: Commit.** `git add src/librarian/classify.rs src/librarian/indexer.rs && git commit -m "feat(librarian): classify the workflow directories by default"`

### Task 2: Create the default workspace file

**Files:**
- Modify: `src/librarian/workspace.rs` — new constant and function; tests module (lines 67-147)
- Modify: `src/librarian/mod.rs` — `LibrarianEnv` (lines 45-68), `LibrarianEnv::from_env` (90-110), `build_tool_context_with` (174-180), `librarian_env_tests` (114-157), and one error-message test

**Interfaces:**
- Consumes: `workspace::load(path: &Path) -> Result<WorkspaceConfig>` and `WorkspaceConfig::default()`.
- Produces, in `src/librarian/workspace.rs`:
  - `pub const DEFAULT_WORKSPACE_TOML: &str` — comments only. It says: this is the global librarian file, not the per-project `.codescout/workspace.toml`; it is created on first start; an empty file is valid; and shows commented examples of `[[rule]]` (`glob`, `kind`), `ignore = [...]` and `[[umbrella]]` (`name`, `members`).
  - `pub fn load_or_init(path: &Path, init: bool) -> Result<WorkspaceConfig>`.
- Produces, in `src/librarian/mod.rs`:
  - field `pub init_default_workspace: bool` on `LibrarianEnv`, documented as "true only from `from_env()`; tests leave it false so they never write to the real config directory";
  - private `fn should_init_default_workspace(env: &LibrarianEnv) -> bool` = `env.init_default_workspace && env.workspace.is_none()`.

- [ ] **Step 1: Write the failing `workspace.rs` tests** (use `tempfile`):
  1. `init_creates_the_file_when_absent` — path inside a not-yet-existing subdirectory; after `load_or_init(path, true)`, the file exists, its bytes equal `DEFAULT_WORKSPACE_TOML`, and the returned config has no rules, no ignore and no umbrellas.
  2. `the_default_text_parses_to_the_default_config` — `toml::from_str::<WorkspaceConfig>(DEFAULT_WORKSPACE_TOML)` is `Ok` and empty.
  3. `init_never_overwrites_an_existing_file` — existing file with one `[[rule]]`; after `load_or_init(path, true)` the bytes are unchanged and `rules.len() == 1`.
  4. `without_init_a_missing_file_is_an_error_and_stays_missing` — `load_or_init(path, false)` is `Err`; the path still does not exist.
  5. `an_unwritable_location_falls_back_to_the_in_memory_default` — make a regular file `blocker`, use `blocker/workspace.toml` as the path; `load_or_init(path, true)` returns `Ok` with an empty config, and no file exists at the path. This does not depend on permissions, so it also holds when tests run as root.
  6. `a_malformed_existing_file_is_still_an_error` — file containing `not = [valid`; `load_or_init(path, true)` is `Err`; bytes unchanged.
- [ ] **Step 2: Run them and see them fail** (compile error: `load_or_init` undefined). `scripts/with-slot.sh cargo test --lib workspace::tests`.
- [ ] **Step 3: Implement `DEFAULT_WORKSPACE_TOML` and `load_or_init`.** With `init` true and the path absent: `create_dir_all(parent)`, then `create_new` + write the constant. `ErrorKind::AlreadyExists` counts as success (another server won the race). Any other error: `tracing::warn!` naming the path and the error, then `return Ok(WorkspaceConfig::default())`. After that, and whenever `init` is false, call `load(path)` unchanged.
- [ ] **Step 4: Run the Step 2 command** → all PASS.
- [ ] **Step 5: Write the failing `mod.rs` tests:**
  - in `librarian_env_tests`: `init_is_requested_only_for_the_default_path` — over the three cases (flag true + `workspace: None` → true; flag true + `workspace: Some(..)` → false; `LibrarianEnv::default()` → false) call `should_init_default_workspace`; and `from_env_requests_the_default_file` — `LibrarianEnv::from_env().init_default_workspace` is true while `LibrarianEnv::default().init_default_workspace` is false (this reads the process environment, which is allowed; it sets nothing);
  - in `tests`: `a_missing_explicit_workspace_is_an_error_that_names_a_real_remedy` — `#[tokio::test]`; call `build_tool_context_with(crate::lsp::LspManager::new_arc(), &LibrarianEnv { workspace: Some(<tempdir>/missing.toml), init_default_workspace: true, ..Default::default() })`. Take the error with `.err()` (the success type is not `Debug`). Assert the message contains the path, does not contain `import-codescout`, and the file was not created.
- [ ] **Step 6: Run them and see them fail** (field and function undefined): `scripts/with-slot.sh cargo test --lib librarian::`.
- [ ] **Step 7: Implement the field, `should_init_default_workspace`, the `from_env()` line (`init_default_workspace: true`), and the call.** In `build_tool_context_with` replace `workspace::load(&cfg_path).with_context(..)` with `workspace::load_or_init(&cfg_path, should_init_default_workspace(env)).with_context(..)`. The new context text: `Load workspace config from {path} (the global librarian file, not .codescout/workspace.toml). Set LIBRARIAN_WORKSPACE to another path, or create an empty file there.` Leave `reindex_cli` (`#[cfg(test)]`, line 473) as it is.
- [ ] **Step 8: Run `scripts/with-slot.sh cargo test --lib librarian::` and `scripts/with-slot.sh cargo test --lib guide_hint_tests`** (the six `db`-only server tests from W-151) → all PASS. If a struct literal elsewhere fails to compile for the new field, add `..Default::default()` there.
- [ ] **Step 9: Measure end to end** with the isolated environment used in the issue's Evidence section (`XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `HOME` pointed at a scratch directory; clean git repo). Build first: `cargo build --release` through `./scripts/rb.sh` if the release binary is stale. Run `target/release/codescout doc find`. Expected: exit 0, the file `<XDG_CONFIG_HOME>/librarian/workspace.toml` exists and equals `DEFAULT_WORKSPACE_TOML`. Then run an MCP `tools/list` against `codescout start` there. Expected: `doc` and `librarian` are listed. Record both results in the issue.
- [ ] **Step 10: Commit.** `git add src/librarian/workspace.rs src/librarian/mod.rs && git commit -m "feat(librarian): create the default workspace.toml on first start"`

### Task 3: Manual

**Files:**
- Modify: `docs/manual/src/concepts/librarian-embedded.md`

**Interfaces:** none.

- [ ] **Step 1: Fix the opening paragraph.** It says the librarian is an "opt-in subsystem" while a later section says "enabled by default". Make both say enabled by default, with the opt-out knobs already in the table.
- [ ] **Step 2: Update "Built-in defaults already cover common patterns"** (section "Per-project classifier overrides") so the list matches `DEFAULT_RULES_TOML` after Task 1, including `docs/trackers/**` and the nine workflow directories.
- [ ] **Step 3: Add a short "First run" paragraph.** It states that the server creates `~/.config/librarian/workspace.toml` when it is absent (default path only), that the file is empty and safe to edit, that it is distinct from `<project>/.codescout/workspace.toml`, and that `LIBRARIAN_WORKSPACE` pointing at a missing file is an error. Remove or correct any text that tells a user to run `librarian-mcp import-codescout`.
- [ ] **Step 4: Check the manual against the code.** `grep` each rule glob named in the page against `DEFAULT_RULES_TOML`, and run the doc-reference audit through `./scripts/gate.sh` at Task 4.
- [ ] **Step 5: Commit.** `git add docs/manual/src/concepts/librarian-embedded.md && git commit -m "docs(librarian): document first-run defaults"`

### Task 4: Verify and close the record

**Files:**
- Modify: `docs/issues/archive/2026-10-14-new-project-setup-does-not-configure-the-librarian.md` (through `doc`, not by hand)
- Modify: `docs/trackers/bug-fix-session-log.md` (through `doc`): F-195 status

**Interfaces:** consumes the three commits above.

- [ ] **Step 1: Run `./scripts/gate.sh`.** Expected: green. Report any red with its output; do not claim green otherwise.
- [ ] **Step 2: Run the mutation probe on the new decisions** (`./scripts/mutation-probe.sh`, `docs/PROBES.md`): flipping `should_init_default_workspace` to always true, and making a non-`AlreadyExists` creation error return `Err` instead of the in-memory default, must each fail a test from Task 2. Record what the probe reports.
- [ ] **Step 3: Update the issue** with `doc(action="update", id="ae511945b43ba0c8", ...)`: `Fix` (what changed, with `path:line`), `Tests added` (the test names above), the Step 9 measurements, status `fixed`, `closed: <date>`. Record the fix SHA labelled `experiments` and its patch-id (`git show <sha> | git patch-id --stable`) for each of the three commits.
- [ ] **Step 4: Set F-195 to `fixed-verified`** in the tracker's Index row through `doc(action="update", id="2dd9d90bc83f9f49", patch={body_edits: [...]})`.
- [ ] **Step 5: Archive the issue** with `doc(action="move", id=..., new_rel_path="docs/issues/archive/2026-10-14-new-project-setup-does-not-configure-the-librarian.md")`. Stage both halves, old and new path. Then commit the record.
