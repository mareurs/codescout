# Artifact Identity Plan B1 — Key Space, Uniqueness Gate and Repo-Name Checks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the identity model its operator-facing edges: a committed scope rule that says which trees have no key, a gate that refuses new key problems at commit time, and the repo-name checks (another checkout uses the name, an unconfirmed name noted at activation, a declined dialog remembered).

**Architecture:** Plan B1 of the derived-catalog plan B (B1 gates and name checks, B2 derived index builder). Plan A (merged on `experiments`) built `identity::{key,grammar,index,repo_name}`. Everything here is new code beside the old system. No catalog, indexer, `link_scan` or tool output changes. Two live surfaces gain a field or a note: the `onboarding` repo-name block and the `workspace(activate)` response.

**Tech Stack:** Rust 2021, `toml`, `serde`, `globset`, `git2` 0.20 (vendored; used only for first-commit lookup), `tempfile` for tests. Bash for the pre-commit script and its test.

**Spec:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`. Read § Identity: the key, § Index inputs, guard, machine state and entry ids, and the "Plan B owns" bullet. Evidence for every seam below: `docs/research/2026-11-11-artifact-identity-catalog-scout.md` and its `-review`.

## Global Constraints

- Key charset `^[a-z0-9]+(-[a-z0-9]+)*$`; the key is the lowercase filename stem; ASCII-only case folding (Plan A rules, unchanged).
- The scope rule lives in a NEW committed file `.codescout/key-space.toml`, never in `.codescout/librarian.toml` (`classify::RuleFile` and `Rule` are `deny_unknown_fields`; a new table there would break every older binary, see `src/librarian/requirements.rs:8-13`). Its serde struct uses `deny_unknown_fields` (it is new, no older binary reads it).
- A document outside the key space has no key and takes no part in uniqueness. Exclusion is by whole subtree.
- A gate refuses only problems that are in the staged index and were not at `HEAD`: it builds the report at both and compares. Editing a file that is already in a collision is therefore not refused, and a commit that only changes `.codescout/key-space.toml` is judged by the problems it creates. Each side reads the key-space file from its own tree (`git show HEAD:<path>`, `git show :<path>`), never from the working tree. The gate degrades open and says so when it cannot ask its question (dispatcher header, `scripts/pre-commit-run.sh`).
- A negative scan result names the scope it examined (`docs/adrs/2026-08-27-negative-results-name-their-scope.md`).
- No production code under `src/librarian/**` reads a file with `fs::read`, `File::open`, `OpenOptions` or `tokio::fs`; read through `read_door::for_project(root)` built **inside** the function (`ReadDoor` is `pub(crate)`; a `pub fn` signature must not name it). Gate: `no_librarian_production_read_bypasses_the_read_door` (`read_door.rs:1414`).
- Env-derived values (home dir, config paths) are resolved at the caller and passed in; no `std::env::set_var` anywhere (`tests/env_mutation_isolation.rs`).
- No new `onboarding` parameter (the `onboarding` input schema counts toward `TOOL_SURFACE_CHAR_BUDGET`). `ONBOARDING_VERSION` does not change (no prompt-slice or `builders.rs` change).
- Agent-facing text states the current situation only, never history.
- Before completing any task: `./scripts/gate.sh` green. Targeted: `scripts/with-slot.sh cargo test --lib <path>`.
- Commit with `scripts/commit-mine.sh -m "<message>"` after `git add <paths>`, following `docs/conventions/shared-checkout-commit-sequence.md`; never a bare `git commit`.

## Review Focus

1. A pattern that excludes a folder's `readme.md` but not its siblings (`docs/x/readme.md`) — expected: the gate reports `PartialFolderExclusion`, never silently re-owns the siblings. Test in Task 2.
2. A key collision between a staged file and an unchanged file — expected: refused, naming both paths; a collision between two unchanged files is NOT refused. Tests in Tasks 2 and 3.
3. A staged rename (old path deleted, new path added) — expected: judged on the new path only; the old key is free. Test in Task 3.
4. A missing or malformed `.codescout/key-space.toml` — expected: missing means "nothing excluded"; malformed is an error that names the file and makes the gate degrade open loudly, never a silent pass. Tests in Tasks 1 and 4.
5. A `.codescout/project.toml` that does not exist when a declined dialog must be remembered — expected: nothing is written, the dialog is allowed to ask again, and the response says so. Test in Task 6.
6. Editing a file that is already in a collision, or changing only `.codescout/key-space.toml` — expected: the first is not refused (the problem is not new), the second is judged by the problems it creates. Tests in Tasks 2 and 3.

---

### Task 1: The key-space scope rule

**Files:**
- Create: `src/librarian/identity/key_space.rs`
- Modify: `src/librarian/identity/mod.rs` (`pub mod key_space;`), `src/librarian/tools/read_door_tests.rs` (add the loader to the table in `repository_config_files_are_read_only_through_the_door`, ~:1161-1260: plain, in-repo link, outward link)

**Interfaces:**
- Consumes: `read_door::for_project`, `globset`.
- Produces:
  - `pub const KEY_SPACE_FILE_REL: &str = ".codescout/key-space.toml";`
  - `pub struct KeySpace` (private fields: compiled matchers, the pattern list)
  - `pub fn load(root: &Path) -> anyhow::Result<KeySpace>` — absent file → `KeySpace::empty()`; bad TOML, unknown key or bad glob → `Err` naming `KEY_SPACE_FILE_REL`.
  - `impl KeySpace { pub fn empty() -> KeySpace; pub fn from_text(text: &str) -> anyhow::Result<KeySpace>; pub fn from_patterns(patterns: &[&str]) -> anyhow::Result<KeySpace>; pub fn is_outside(&self, rel_path: &str) -> bool; pub fn patterns(&self) -> &[String]; }` — `from_text` parses the TOML text; `load` and every git-blob loader (Task 3, and Plan B2 Task 3) use it, so no caller reads the file twice by two routes.
  - File format: `exclude = ["contrib", "docs/evals/**"]`.

A pattern excludes a path when the glob matches the path or any ancestor directory of it, so `contrib` and `contrib/**` both exclude `contrib/pi/AGENTS.md`. Paths are git-root-relative with `/` separators; a backslash path is normalised first (as `key_of` does).

- [ ] **Step 1: Write the failing tests** in `key_space.rs`: `absent_file_excludes_nothing`; `a_bare_directory_pattern_excludes_its_whole_subtree` (`contrib` excludes `contrib/pi/AGENTS.md`, not `contribution.md`); `a_double_star_pattern_excludes_the_same_subtree`; `a_windows_separator_path_is_judged_like_the_forward_slash_form`; `malformed_toml_is_an_error_naming_the_file`; `an_unknown_key_is_an_error` (`deny_unknown_fields`); `from_text_parses_what_load_reads`; `a_dangling_link_is_an_error_not_absent` (the door's rule, mirrors Plan A's `repo_name::read`).
- [ ] **Step 2: Run them** — `scripts/with-slot.sh cargo test --lib identity::key_space` → FAIL (module missing).
- [ ] **Step 3: Implement** the signatures above. Compile each pattern twice (`p` and, when `p` does not already end in `/**`, `p/**`) so ancestor matching needs no path walk.
- [ ] **Step 4: Run the tests** → PASS; then the read-door table test → PASS with the new row.
- [ ] **Step 5: Commit** — `feat(librarian): a committed key-space scope rule (.codescout/key-space.toml)`.

---

### Task 2: The uniqueness gate, pure

**Files:**
- Create: `src/librarian/identity/gate.rs`
- Modify: `src/librarian/identity/mod.rs` (`pub mod gate;`)

**Interfaces:**
- Consumes: `key_space::KeySpace`, `index::{KeyIndex, DocInput, IndexProblem}`, `key::{key_of, is_folder_index}`, `repo_name::RepoName`.
- Produces:
  - `pub struct GateReport { pub in_space: usize, pub outside: Vec<String>, pub problems: Vec<GateProblem> }`
  - `pub enum GateProblem { InvalidKey(KeyError), DuplicateKey { key: Key, rel_paths: Vec<String> }, PartialFolderExclusion { folder: String } }`
  - `pub fn check_tree(this_repo: RepoName, space: &KeySpace, md_files: &[String]) -> GateReport` — `md_files` are git-root-relative, each once, sorted by the caller. Partitions by `space.is_outside`, builds a `KeyIndex` over the rest with `text: String::new()` (keys only), maps `IndexProblem` to `GateProblem`, and adds `PartialFolderExclusion` when an excluded path is a folder index (`is_folder_index`) while a sibling in the same directory is inside the space.
  - `impl GateReport { pub fn new_since(&self, before: &GateReport) -> Vec<&GateProblem> }` — the problems in `self` with no equal problem in `before` (`InvalidKey` equal by path; `DuplicateKey` equal by key and exact path set, so a collision that gains a member is new; `PartialFolderExclusion` equal by folder).
  - `impl GateProblem { pub fn render(&self) -> String }` — one line with the remedy (`KeyError` already carries remedy text).

- [ ] **Step 1: Write the failing tests:** `outside_paths_are_listed_and_take_no_part_in_uniqueness` (two files with one stem, one excluded → no problem, one `outside` entry); `a_collision_inside_the_space_is_reported_with_every_path`; `a_non_canonical_name_is_reported_not_skipped` (`Ünïcode Notes.md`, `v1.2-notes.md`); `an_excluded_folder_index_beside_included_siblings_is_a_partial_exclusion` (Review Focus 1); `new_since_ignores_a_problem_that_is_unchanged` (a collision present before and after is not returned, Review Focus 6); `new_since_reports_a_collision_that_gains_a_member`; `new_since_reports_a_staged_file_that_collides_with_an_unchanged_one` (Review Focus 2); `un_excluding_a_tree_in_the_key_space_file_reports_the_problems_it_creates`; `an_empty_tree_is_clean`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): the key-space uniqueness gate (pure)`.

---

### Task 3: `codescout key-gate`

**Files:**
- Create: `src/cli/key_gate.rs`
- Modify: `src/cli/mod.rs` (`pub mod key_gate;`), `src/main.rs` (a `Commands::KeyGate` variant with a doc comment, and its match arm; model the exit code on `AuditDocRefs`, `src/cli/audit_doc_refs.rs:76`)
- Test: `src/cli/key_gate.rs` (`#[cfg(test)]`, temp git repos built with `Command::new("git")`, `-c user.name=t -c user.email=t@t`)

**Interfaces:**
- Consumes: Task 1 and 2 items; `current_project::lookup_git_root`; `repo_name::{read, suggest}`.
- Produces:
  - `#[derive(Debug, Args)] pub struct KeyGateArgs { pub project: Option<PathBuf>, pub staged: bool, pub json: bool }`
  - `pub async fn run(args: KeyGateArgs) -> Result<()>` — resolves the git root without opening the catalog (no `open_ctx`); builds one `GateReport` over the index and, with `--staged`, a second over `HEAD`, and prints `after.new_since(&before)`; without `--staged` it prints every problem at the index plus the counts and the `outside` list. Exit `0` clean, `1` problems, `2` cannot ask (not a git repo, malformed key-space file): prints `NOT CHECKED — <reason>`.
  - Private helpers: `fn list_md_at_head(root: &Path) -> Result<Vec<String>>` (`git ls-tree -r -z --name-only HEAD -- '*.md'`; an unborn `HEAD` gives an empty list), `fn list_md_in_index(root: &Path) -> Result<Vec<String>>` (`git ls-files -z -- '*.md'`), `fn key_space_from(root: &Path, rev: &str) -> Result<KeySpace>` (`git show <rev>:.codescout/key-space.toml`, with `rev` = `HEAD` or empty for the index; absent file → `KeySpace::empty()`; text through `KeySpace::from_text`).

- [ ] **Step 1: Write the failing tests:** `a_staged_new_file_that_collides_with_an_unchanged_one_exits_1` (Review Focus 2); `a_collision_between_unchanged_files_exits_0_in_staged_mode_and_1_in_full_mode`; `editing_a_file_already_in_a_collision_exits_0` (Review Focus 6); `a_commit_that_only_changes_the_key_space_file_is_judged_by_what_it_creates` (Review Focus 6; reads the staged copy, not the working tree); `the_first_commit_with_no_head_works`; `a_staged_rename_is_judged_on_the_new_path_only` (Review Focus 3); `a_malformed_key_space_file_exits_2_and_says_not_checked` (Review Focus 4); `outside_files_are_listed_with_the_scope_that_excluded_them`; `json_output_has_counts_problems_and_outside`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(cli): codescout key-gate`.

---

### Task 4: The pre-commit hook, its test, and this repo's own scope file

**Files:**
- Create: `scripts/pre-commit-key-space.sh`, `tests/pre-commit-key-space.sh`, `.codescout/key-space.toml`
- Modify: `scripts/pre-commit-run.sh` (one `run "key-space" scripts/pre-commit-key-space.sh` line beside dead-artifact-ids, :211-212), `.github/workflows/ci.yml` (a `run:` line for the new bash test beside :238-255)

**Interfaces:**
- Consumes: `codescout key-gate --staged`.
- Produces: a refusing, read-only hook that runs when a `*.md` file or `.codescout/key-space.toml` is staged: no compile, `degrade open and loudly` when no live binary is found (`$CODESCOUT_BIN`, `$root/target/release/codescout`, `command -v codescout`, each probed with `--help`), exit `0` on `NOT CHECKED`, exit `1` with the problem lines on stderr when `key-gate` exits 1. Model: `scripts/pre-commit-dead-artifact-ids.sh` (header with scope reasoning; staged-bytes versus worktree-bytes check). No `/home/<user>` path in the script (`tests/committed_paths.rs:184`).

- [ ] **Step 1: Write `tests/pre-commit-key-space.sh`** (stub binary through `CODESCOUT_BIN`, throwaway git repos): cases `no binary → exit 0 with NOT CHECKED`; `key-gate exit 1 → hook exit 1 and the lines printed`; `key-gate exit 2 → hook exit 0 and NOT CHECKED printed`; `no staged markdown and no staged key-space file → exit 0 without calling the binary`; `a staged key-space file alone does call the binary`. Run it → FAIL (script missing).
- [ ] **Step 2: Write the script**, register it, add the CI line. Run the bash test → PASS.
- [ ] **Step 3: Author `.codescout/key-space.toml` for this repo.** Run `codescout key-gate` (full mode) from a release build, then exclude the trees the census names as fixture, evidence or per-project memory material: at least `contrib`, `docs/evals`, `tests/fixtures`, `.codescout/projects`, `.codescout/memories`, `.claude`, `.buddy`. Keep adding until the remaining problems are only real documents the migration renames. Record the final counts (in space, outside, problems) in the commit message and in `docs/trackers/artifact-identity-session-log.md` through `doc(action="append_entry", id="abff63149e3c24e8", …)`.
- [ ] **Step 4: Run** `tests/pre-commit-key-space.sh`, `scripts/with-slot.sh cargo test --test hook_config --test committed_paths`, then `./scripts/gate.sh`. Expected: green.
- [ ] **Step 5: Commit** — `feat(hooks): refuse new key problems at commit time; this repo's key-space scope`.

---

### Task 5: Other checkouts that use the name

**Files:**
- Create: `src/librarian/identity/checkouts.rs`
- Modify: `src/librarian/identity/mod.rs` (`pub mod checkouts;`)

**Interfaces:**
- Consumes: `repo_name::{read, RepoName}`, `current_project::lookup_git_root`, `git2`.
- Produces:
  - `pub struct ScanLimits { pub max_depth: usize, pub max_dirs: usize, pub deadline: std::time::Duration }` with `Default` (depth 3, 20000 directories, 2 s).
  - `pub struct ScanReport { pub roots: Vec<PathBuf>, pub dirs_visited: usize, pub bounded_by: Option<Bound>, pub same_repo: Vec<PathBuf>, pub collisions: Vec<PathBuf>, pub unverified: Vec<PathBuf>, pub unreadable: Vec<(PathBuf, String)> }` and `pub enum Bound { Depth, Dirs, Deadline }`.
  - `pub fn first_commits(git_root: &Path) -> anyhow::Result<Vec<String>>` — the sorted ids of every parentless commit reachable from HEAD (`git2` revwalk; a repo with several root commits returns several; an unborn HEAD returns an empty vec).
  - `pub fn scan(name: &RepoName, this_git_root: &Path, roots: &[PathBuf], limits: &ScanLimits) -> ScanReport` — for each directory under each root (hidden directories other than `.codescout` itself are skipped; depth and size bounded), a directory with `.codescout/project.toml` is a project; resolve its git root; read `repo.toml` there; when the name equals `name` and the git root differs from `this_git_root`, compare `first_commits`: a non-empty intersection → `same_repo` (a repo with several root commits shares at least one with its clones), disjoint sets → `collisions`, and a shallow repository on either side (`git2::Repository::is_shallow`) → `unverified`, because its boundary commit looks parentless. An unreadable project or repo file is listed in `unreadable`, never skipped silently.
  - `pub fn render_scope(report: &ScanReport) -> String` — one line naming the roots, the directories visited and any bound hit (the negative-results rule).

- [ ] **Step 1: Write the failing tests** with real git repos in tempdirs (`git init`, commit, `git clone`): `a_clone_of_this_repo_is_the_same_repo_not_a_collision`; `an_independent_repo_with_the_same_name_is_a_collision`; `a_project_with_another_name_is_ignored`; `a_directory_deeper_than_max_depth_sets_bounded_by_depth`; `an_unreadable_repo_file_is_listed_not_skipped`; `a_shallow_clone_is_unverified_not_a_collision` (clone with `git clone --depth 1 file://<src>`: a plain local path ignores `--depth`); `two_repos_sharing_one_root_commit_are_the_same_repo` (one has an extra unrelated root merged in); `first_commits_returns_every_root_commit`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** Tests build every path inside tempdirs and pass `roots` as arguments; no `$HOME` read.
- [ ] **Step 4: Run** → PASS. **Step 5: Commit** — `feat(librarian): find other checkouts that use a repo name`.

---

### Task 6: Onboarding — the three-state dialog, the remembered decline, the scan result

**Files:**
- Modify: `src/tools/onboarding.rs`, `src/tools/core/types.rs`, `src/config/project.rs`, `src/tools/run_command/tests.rs` (the `ProjectSection { … }` literals at :9512, :9580 and :9946 gain the new field, as do `config/project.rs:1000` and `onboarding.rs:1359`; grep `ProjectSection {` first, or the build fails with E0063)
- Test: `src/tools/onboarding.rs` (`mod tests`, ~:1760-2120)

**Interfaces:**
- Consumes: Task 5 (`scan`, `render_scope`), `patch_project_table` (`onboarding.rs:114-141`), `ProjectConfig::load_or_default`.
- Produces:
  - In `types.rs`: `pub(crate) const ELICIT_DECLINED_MSG: &str` and `pub(crate) const ELICIT_CANCELLED_MSG: &str`, used by `elicit_inner` where it builds those two errors today (`types.rs:986`), so producer and consumer share one string.
  - In `onboarding.rs`: `enum ElicitAnswer { Answer(String), Declined, Unavailable }`; `repo_name_outcome` takes `ElicitAnswer` instead of `Option<String>`; `Declined` and `Cancelled` map to `Declined`, every other `Err` and `Ok(None)` to `Unavailable`. The `.ok().flatten()` collapse at ~:826-831 goes.
  - `ProjectSection` (`config/project.rs:45`) gains `#[serde(default)] pub repo_name_declined: bool`.
  - Behaviour: a `Declined` answer sets `repo_name_declined = true` in `.codescout/project.toml` through `patch_project_table` (skipped when the file is absent; the response then says `"remembered": false`). While the marker is set and no name exists, `onboarding` does not open the dialog; it returns the Unconfirmed block plus `"declined": true`. A confirmed write (elicited or `onboarding(repo_name=…)`) removes the marker.
  - The scan runs only when a name is being chosen: the Unconfirmed and Invalid blocks and `set_repo_name`, never for a Confirmed read of an existing name. The block gains `"checkouts": { "scope": <render_scope>, "same_repo": [...], "collisions": [...], "unverified": [...] }`; `collisions` non-empty adds an `"instruction"` telling the caller to pick another name.
  - Roots are resolved at one edge, `Onboarding::call`, and passed into a pure `repo_name_block_for`: the parent directory of the repo root plus the `[[roots]]` and umbrella-member parents of `workspace::load(workspace::default_config_path())`, and only when `ctx.peer.is_some()` (an interactive client, where the name is chosen). Without a peer the block says `"checkouts": { "scope": "not scanned: no interactive client" }`. Existing tests have `peer: None`, so none of them reads the machine's workspace config or scans a real folder.
  - `fn repo_root_of(project_root: &Path) -> PathBuf` becomes `pub(crate)` (Task 7 uses it).

- [ ] **Step 1: Write the failing tests** through the pure functions (no mock elicitation peer exists; `core/tests.rs:2505-2535` builds the decline errors by hand): `declined_and_cancelled_and_timeout_are_three_different_answers`; `a_declined_dialog_writes_the_marker_and_stops_asking`; `a_confirmed_write_clears_the_marker`; `no_project_toml_means_the_decline_is_not_remembered_and_says_so` (Review Focus 5); `the_block_names_the_scope_it_scanned_even_when_nothing_was_found`; `a_collision_adds_an_instruction`; `an_existing_name_is_never_overwritten_by_a_decline` (Plan A's rule survives); `a_confirmed_name_read_does_not_scan`; `no_interactive_client_means_no_scan_and_the_scope_says_so`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** the onboarding tests, `tool_surface_under_budget`, and the repo-name block tests → PASS.
- [ ] **Step 5: Commit** — `feat(onboarding): remember a declined repo-name dialog; report other checkouts that use the name`.

---

### Task 7: The activation note

**Files:**
- Modify: `src/tools/config/mod.rs` (`build_activation_response` before `timer.finish()` at ~:1255; `format_activate_project` ~:1259-1356)
- Test: `src/tools/config/tests.rs` (model pair: `activation_response_emits_legacy_index_when_db_present` :2805 and `..._omits_..._when_db_absent` :2843; compact-banner model :2951)

**Interfaces:**
- Consumes: `onboarding::repo_root_of` (Task 6), `repo_name::{read, suggest}`, `ProjectConfig.project.repo_name_declined` (the field is on `ProjectSection`).
- Produces: a named block `repo_name_unconfirmed: { suggested, action }` in the activation JSON, modelled on `system_prompt_stale`, present only when `repo_name::read(repo_root)` is `Ok(None)` and `repo_name_declined` is false. `action` is `onboarding(repo_name="<suggested>")`; when the activation is `read_only` the text says to activate read-write first (the call is refused otherwise: `Onboarding::is_write` is always true). The compact form gets one banner line. No second `hint` field (Pattern 5a).

- [ ] **Step 1: Write the failing tests:** `activation_notes_an_unconfirmed_repo_name`; `activation_omits_the_note_when_a_name_exists`; `activation_omits_the_note_after_a_decline`; `a_read_only_activation_does_not_offer_a_refused_call`; `the_compact_form_carries_the_note`; `a_malformed_repo_toml_is_reported_not_treated_as_unconfirmed` (an `Err` from `read` must not suggest a write).
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** the new tests and the existing activation tests (list in the scout report § 2) → PASS.
- [ ] **Step 5: Run `./scripts/gate.sh`** → green. **Step 6: Commit** — `feat(workspace): activation says when the repo name is unconfirmed`.

---

## After the plan

- Run B1 first, then B2, in sequence: both edit `src/main.rs` and `src/cli/mod.rs`, B2 reads `KeySpace` (B1 Task 1), and B2 Task 7's numbers depend on this repo's `.codescout/key-space.toml` (B1 Task 4) being committed.
- The "Plan B owns" items in the spec are all built here: the other-checkout check (Task 5, 6), the activation nudge (Task 7), the remembered decline (Task 6), the key-space scope rule (Task 1) and its uniqueness gate (Tasks 2 to 4).
- Not built here, on purpose: a doctor check for key problems. `doctor::call` holds the catalog lock for its whole run, has no time bound, and a new check owes four edits and two exhaustiveness tests. `codescout key-gate` covers the need without them.
