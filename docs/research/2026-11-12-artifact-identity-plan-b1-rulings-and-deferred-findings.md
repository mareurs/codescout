---
id: '8721253ab393c824'
kind: research
status: done
title: Plan B1 — rulings, deferred findings and gate history
owners:
- marius
tags:
- identity
- plan-b1
- sdd-ledger
topic: artifact identity refactor
---

# Plan B1 — rulings, deferred findings and gate history

This document keeps what the Plan B1 subagent-driven run recorded in its scratch ledger. The scratch ledger lived in a git-ignored worktree directory and was deleted after the merge. Nothing else holds the deferred findings.

- **Plan:** `docs/superpowers/plans/2026-11-11-artifact-identity-plan-b1-key-space-and-name-checks.md`
- **Spec:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`
- **Session log:** `docs/trackers/artifact-identity-session-log.md`, entries F-12 to F-14 and W-8.
- **Merged:** fast-forward to `a79985050` on `experiments`, on 2026-11-12. Branch head before the last merge of `experiments`: `ec51112f0`. Branch base: `1c96dd8f3`.
- **Not in this document:** the final review's seven Minor findings. The scratch ledger recorded only the ones that the triage promoted into the fix wave (groups 4 and 5 below).

## Open findings

Each row was a deferred Minor finding that no later commit had closed when the run ended. On 2026-11-12 each row was checked against the code; read § Triage of 2026-11-12 for what that found, because several rows below are now fixed. "Task N" is the Plan B1 task that raised it. All items are small unless the note says otherwise.

| Task | Finding |
|---|---|
| 1 | The `is_outside` doc lacks the no-dotdot caller contract that `key_of` states. |
| 1 | A blank pattern and a trailing slash have no tests. |
| 1 | The glob semantics (`*` crosses `/`, root-anchored, case-sensitive) are documented in the `.codescout/key-space.toml` header but still have no test. |
| 1 | The read-door table row cannot tell an error from an absent file. No test covers a `.codescout` directory linked outward (model: `repo_name.rs:484-497`). |
| 1 | The load error names the file twice. |
| 1 | A backslash in a pattern is read as a separator, because the globset escape is lost. No test or doc pins this. |
| 2 | `render` of `InvalidKey(NotMarkdown)` has no remedy text. The case is unreachable for callers that pass only `.md` files. |
| 2 | `folder_of` is duplicated between `gate.rs` and `index.rs:141-144`. Hoist it into `key.rs` next to `components`. |
| 2 | The `check_tree` doc should say that paths must already be normalised and unique. Two spellings of one path collide. |
| 2 | Two readmes in one folder, with one excluded, still report `PartialFolderExclusion`. This is an accepted warning. |
| 2 | No test covers two excluded indexes (`README.md` and `readme.md`) in one folder. The whole-subtree-clean test does not assert `r.outside`. |
| 3 | `head_exists` runs twice for the HEAD side. |
| 3 | An inherited absolute `GIT_INDEX_FILE`, with `--project` pointing at another repo, queries the wrong index. Document it or unset it. |
| 3 | The end-to-end test and `evaluate` inherit the global git config. Set `GIT_CONFIG_GLOBAL` on the `key-gate` command. |
| 3 | The JSON exit-2 object has no `mode` field. |
| 3 | The HEAD-malformed message does not say what to do next. |
| 3 and 4 | Out of scope for B1. Full mode of `key-gate` lists an unmerged path once per stage (`ls-files -z`, no dedupe at `key_gate.rs:96-108` and `:164`). A merge conflict may report a file that collides with itself. The hook does not expose this. |
| 4 | The hook's exit-2 branch drops the `key-gate` stderr when stdout is empty (a clap usage error). |
| 4 | A dead `$CODESCOUT_BIN` falls through silently. |
| 4 | A failing `git diff --cached` or `rev-parse` passes silently in the hook. |
| 4 | The stub `PROBLEM` text uses a readme pair that `key-gate` cannot print. |
| 4 | `tests/pre-commit-key-space.sh` hard-codes `PATH=/usr/bin:/bin`. It would fail instead of skipping on NixOS or where git or bash live elsewhere. Build the `PATH` from the directories of `command -v git bash mktemp env`. |
| 4 | The `.claude` comment says "one skill". Say "one tracked SKILL.md", because a tracked symlink also sits there. |
| 4 | The probe order is untested, because no case has two live candidates. The heading of case 12 overclaims. |
| 4 | The hook is dormant on a fresh checkout until `target/release/codescout` is built. This is by design, and the commit hook prints "NOT CHECKED". |
| 5 | The module doc must state that a shared vendored root gives a false `same_repo`, and that a history rewrite gives a false collision. Task 6 must not call `same_repo` proof. |
| 5 | `a_same_named_project_without_git_is_unverified` returns early when the tempdir lies under a git work tree. In production, the walk-up past the scan root treats a git-less project under an ancestor repo as that ancestor. Bound the walk-up at the scan root. |
| 5 | The revwalk-timeout path and an unborn HEAD are untested, so a mutant that returns a partial list on timeout survives. |
| 5 | `inspect` opens the repo twice. An optional cheaper first pass is `find_commit` of this repo's root ids in the candidate object database. |
| 5 | `read_dir` and `lookup_git_root` run per directory without a deadline check. Skip `node_modules` and `target` by name. Linked worktrees outside `.worktrees` are not skipped either. |
| 5 | "1 directories visited" has a grammar fault. `render_scope` prints the caller's own paths, so shorten `$HOME` before any prompt-facing use. |
| 5 | The Depth bound fires on almost every broad root. Task 6 words it as the weakest bound. |
| 5 | The `render_scope` test checks only keywords, not the "not examined" wording. The "a repo counts once" half of the nested-project test cannot fail, because collisions are deduplicated at the end. |
| 6 | No test covers the live dialog wiring with a peer present (`name_edge(choosing && peer.is_some())` and `elicit_answer(ctx.elicit_with_timeout)`). An in-process duplex harness exists at `src/server/native_context_tests.rs:17-23`. |
| 6 | `scrub_home` replaces raw text. A home of `/home/ma` turns `/home/marius/x` into `~rius/x`. `remember_error` and `written` are not shortened. |
| 6 | A decline drops the comments in `project.toml` (a known limit of `patch_project_table`). No test checks that a malformed `project.toml` on decline stays byte-identical and reports `remembered:false`. |
| 6 | `machine_workspace` does not honour `LIBRARIAN_WORKSPACE` (`default_config_path`). Resolve it through `LibrarianEnv` at the edge. |
| 6 | The read of the existing name before `onboarding_branch` widens the stale-read window. `repo_name::write` overwrites without checking, and nested projects have different write guards. Re-read before the write, or make the write create-only. |
| 6 | The dialog is asked before the scan, so a collision is reported only after the name is written. |
| 7 | A worktree branched before `repo.toml` was committed gets `suggest()` from the main checkout's `workspace.toml` or folder name, not from its `repo.toml`. `project.toml` is per checkout, so a decline does not carry to worktrees. |
| 7 | `repo_name_note` recomputes `to_forward_slash(project_root)`. Its doc says two small reads, but `suggest` also reads `workspace.toml` and `.git`. |
| 7 | `docs/manual/src/concepts/activate-project-output-optimization.md` does not list the new `workspace(activate)` and `onboarding` output fields. |
| 7 | The compact banner shows `suggested` and `action`, but not that the call writes a committed file. |
| Final | R15 (a): a folder-index readme is counted under its file name (`README.md` or `readme.md`), not under the folder that gives its key. A `git mv` of `docs/Bad A/` to `docs/Bad B/`, or fixing one bad folder while adding another, is not reported as new. Count a folder index under its parent folder name. |
| Final | R15 (b): the refusal may name the old file when the new duplicate-name file sorts first. The verdict is correct and the named path is wrong. Name the added path. |
| Final | `ScanLimits::default()` (depth 3, 20000 directories, 2 s) is a struct-field bound that the cap scanner cannot see. `render_scope` discloses it. Whether it needs a `RESULT_CAP` row is undecided. |

## Questions for the migration (Plans D and E)

- Seventeen files in the key space have an invalid key. Nine of them cannot simply be renamed: 3 GitHub templates (GitHub dictates the names) and 6 prompt templates under `src/` and `scripts/measure`. Decide for each of the nine: rename or exclude. The other eight can be renamed: `docs/PROGRESSIVE_DISCOVERABILITY.md`, `docs/issues/_TEMPLATE.md`, four archived issues with underscores in the name, one issue with an uppercase `A`, and one file with a dot in the stem (`...phase1.5.md`). An earlier version of this section said nine files in all.
- 14 of the 26 duplicate-key groups contain a plan. Only 6 of them are a plan and a spec that share a stem (the 2026-10-27 group is a triple with a review). 7 are `ecc-integration` plans against reports, and 1 is a research note against a plan. All stay in scope. This departs from the spec's named example, and the spec does not yet record the decision. See session-log entry F-14.
- The counts above were taken on 2026-11-12 with a script that replicates the key rules over `git ls-files` (2,755 `.md` files, 2,578 in the key space), because the built `target/release/codescout` predates the `key-gate` command. The 26 groups match the earlier figure. The 17 have not been confirmed with `key-gate` itself.

## Triage of 2026-11-12

Every row of § Open findings was read against the code that day by four read-only reviewers, and the claims that mattered most were read again at the source. Row numbers below are the Task column of that table. Two commits closed most of it: `6292766cf` and `a6f8712dd`.

**Fixed in `6292766cf`, each behaviour change with a test that failed first:**
- Task 5: a project with no `.git` inside the scan is judged as itself, not as an ancestor repository above the scan root (`visit_project` now stops at the scan root). This was the only row that could give a wrong verdict in production.
- Task 6: the dialog no longer overwrites a name another session wrote while it waited. `scrub_home` shortens only a whole path component, and `remember_error` and `written` pass through it. The scan reads the workspace file that `LIBRARIAN_WORKSPACE` names.
- Task 4: a failing `git rev-parse` or `git diff --cached` says NOT CHECKED. Exit 2 with an empty stdout shows key-gate's stderr. A dead `$CODESCOUT_BIN` is named. The stub `PROBLEM` text no longer uses a readme pair. The tests build `PATH` from where `git`, `bash` and `sed` are. The `.claude` comment says one tracked SKILL.md.
- Task 3: `key_gate()` in `tests/cli_key_gate.rs` sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM`.
- Task 2: `folder_of` is one function in `key.rs`. `check_tree` documents that paths must be normalised, with a test. Tests cover two excluded readmes and `r.outside`.
- Task 1: tests for glob semantics and for a `.codescout` directory linked outward. Doc lines for the no-dotdot contract and for the backslash.
- Task 5: the module doc states that `same_repo` is not proof. `render_scope` says "1 directory visited" and its test asserts the "not exhaustive" wording.

**Fixed in `a6f8712dd`:**
- Final R15 (a) and (b): `new_since` counts a folder index under its folder's name, and a refusal names the file that was added, not an older one that sorts later. Both tests failed first.
- Task 5: a scan does not enter `node_modules` or `target`, and `inspect` opens the repository once. The unborn-HEAD case is tested through a scan. The skip test failed first.
- Task 1: a parse error from `load` names the file once. The test failed first.
- Task 7: a linked worktree offers its main checkout's `repo.toml` name ahead of a guess. The test failed first. The compact `REPO NAME UNCONFIRMED` banner says the call writes the committed file, and the manual page lists the repo-name fields.
- Task 3: the JSON NOT CHECKED object has a `mode`; a malformed `key-space.toml` at HEAD says how to go on; `head_exists` is asked once; the doc names the `GIT_INDEX_FILE` caveat for `--project`. The `mode` and HEAD-message tests were written together with the fix and were not run red first.
- Task 4: a case with two live candidates asserts the probe order.
- Task 6: a doc line says the dialog is asked before the scan, which is why a collision is reported after the name is written.

**Found already fixed, no change needed:** the blank-pattern and trailing-slash tests (Task 1); the revwalk-timeout path and the per-directory deadline check (Task 5); duplicated unmerged paths (Task 3 and 4), which `check_tree` collapses in a `BTreeSet`. `repo_name_note` no longer recomputes `to_forward_slash` (Task 7).

**Still open:**
- Task 2: `render` of `InvalidKey(NotMarkdown)` has no remedy text (probably not worth fixing, the case is unreachable). The partial-exclusion warning for two readmes is accepted.
- Task 4: the hook is dormant until a binary is built, by design.
- Task 5: linked worktrees outside `.worktrees` are still entered; the Depth bound wording. (The "a repo counts once" assertion is closed: `two_projects_in_one_repo_are_judged_once` in `fb765398b` reads the vector before the final dedup, and failed red with the `judged` check removed.)
- Task 6: the dialog wiring is closed for the answer and decline paths: `a_name_typed_in_the_dialog_is_written_to_the_repo_file` and `a_declined_dialog_is_remembered_and_not_asked_again` (`a8ec6106e`) drive it over a duplex wire, and failed red with `dialog_wanted` forced to `false`. Still untested: the `peer.is_some()` condition that gates the scan.
- Final: whether `ScanLimits::default()` needs a `RESULT_CAP` row. That is a decision, not a fix.

The reviewers read the code; they did not run it. The fixes above were run: see the commit messages for the gate.

## Closed by a later commit

| Task | Finding | Closed by |
|---|---|---|
| 1 | Path normalisation differed from `key_of`, and `./` and `/` patterns were dead (Important). | Fix round 1, `5b9b2ce5a`. |
| 1 | The module doc said "The file is new". | Final group 5, `57d8f8663`. |
| 2 | `PartialFolderExclusion` covered only same-directory siblings. | R4, fix round 1 (`07dfd5bf9` to `9bf354153`). |
| 2 | No test made the `InvalidKey` path identity load-bearing. | Obsolete: R11 changed the identity to the file name. |
| 2 | Plan text for Task 2 was out of date against R3 and R4. | Plan commit `38b328d61`. |
| 3 | No test where a markdown path differs between the index and the working tree. | Final group 4, `f3075ec16`. |
| 4 | Test case 9 was not isolated from `PATH` (Important). | Fix round 1, `4671e2a4d` to `8f5371447`. |
| 4 | The dispatcher label was terse and the CI job name was stale. | Final group 5, `57d8f8663`. |
| 5 | The first bound hit hid stronger bounds (Important). | Fix round 1, `f4d0ddb06`. |
| 5 | The nested-project test could not fail for its headline claim. | R9, fix round 1. |
| 5 | The `note()` ranking was tested only for Depth replaced by a stronger bound. The `Bound` doc said "first bound". | Final group 4, `f3075ec16`. |
| 6 | The declined block kept telling the agent to ask. | R14, `d057b2bcc`. |
| 6 | The collisions instruction told the agent to pick a name. | R14, `d057b2bcc`. |
| 6 | A dismissed dialog was remembered as a decline (plan-level question). | R12, `d057b2bcc`. |
| 6 and 7 | `a_malformed_repo_toml_is_reported_by_onboarding_and_left_alone` failed when `TMPDIR` is inside a git checkout. | Final group 2, `d057b2bcc`. |
| 7 | The JSON form offered the write with no instruction to ask the user (Important). | R10, fix round 1, `2d1f9031d`. |
| 7 | The note fired on every read-only foreign activation. | R13, `aa66c739f`. |
| 7 | A doc-comment line was over 100 characters. | Final group 3, `aa66c739f`. |
| Final | A pure move of a file that already has a problem was refused as new (Important). | R11, `c846f1b23`. |
| Final | This repo had no committed `.codescout/repo.toml` (Important). | `ec51112f0`, with `name = "codescout"`. |
| Final | `CHECKOUTS_LISTED_MAX` had no cap-class annotation. | `6eef416c2`, see session-log entry F-12. |

## Rulings

Each ruling states what was decided and what it costs if wrong.

- **R1 (Task 3).** The `RepoName` for `check_tree` is `repo_name::read(root)` when it returns `Ok(Some)`, else `suggest(root)`. Cost: none, because keys do not depend on the repo name.
- **R2 (Task 4, revised).** The implementer authors `.codescout/key-space.toml` and reports the counts. The controller appends the session-log entry. Cost: none.
- **R3 (Task 2).** A collision whose path set shrinks is not new. R11 replaced its swap clause.
- **R4 (Task 2).** `PartialFolderExclusion` fires at any depth for an excluded non-root folder index. A root index keeps the same-directory rule. Cost: one test pair and one fix round.
- **R5 (Task 3).** List markdown files by listing all tracked paths with `-z` and filtering through `key::markdown_stem`. A git pathspec is case-sensitive, and `key.rs` accepts `.MD`. Cost: none.
- **R6 (Task 4).** Implementers run targeted checks only. The controller runs `./scripts/gate.sh` in the background. Cost: none. F-12 records a gap this left.
- **R7 (Task 4).** Narrow `docs/evals` to the runs folder, because an exclusion removes a document's key. Cost: one commit.
- **R8 (Task 4).** Restore `.claude`, `contrib` and `docs/trackers/ecc-integration/evidence` as whole-subtree exclusions. Exclude a subtree whose names come from outside, or that is recorded evidence. Narrow only where the names are ours. Cost: a few more documents lose their key.
- **R9 (Task 5).** Promote the nested-project test fix into the fix round. Cost: one test edit.
- **R10 (Task 7).** Keep `action` verbatim and add an `instruction` field that tells the agent to ask the user before the write. Cost: one field.
- **R11 (final).** `new_since` counts problems instead of comparing path sets. A duplicate key is new only when it is absent before or its member count grows. An invalid key is identified by file name and is new only when the count for that name grows. Cost: a commit that deletes one problem file and adds another with the same key or name passes.
- **R12 (final).** A cancelled or dismissed dialog maps to `Unavailable` and is asked again. Only an explicit decline is remembered. This reverses the plan text. Cost: a user who cancels is asked again.
- **R13 (final).** `repo_name_unconfirmed` is not emitted on read-only activations. `repo_name_error` stays in both modes. Cost: no nudge on a read-only activation of a project without a name.
- **R14 (final).** The declined block says that the user declined, says not to ask again, names the undo and skips the scan. The collision text says to suggest a name to the user. Cost: none.
- **R15 (final).** The two residuals listed under Final in the open findings. Cost: a bad-folder replacement passes the hook.

## Gate history

| Tree | Result |
|---|---|
| `4671e2a4d` (during Task 4 fix round) | Green, informational only: exit 0 in 438 s, Python 1833 passed and 7 skipped. |
| `38b328d61` | Red. The only failing test was `result_caps every_cap_constant_is_classified`. |
| `6eef416c2` | Green: exit 0 in 6 m 37 s. Rust 9445 passed (no-default-features) and 9539 passed (default). Python 1833 passed and 7 skipped. |
| `ec51112f0` (after merging `experiments` at `47a21610a`) | Green: Python 1836 passed. |
| Second merge (`experiments` at `c7b80749d`) | The lean lane failed two peer LSP tests. The default lane passed. |
| Third merge (`experiments` at `56b3eb525`) | The lean lane passed. The default lane failed one peer LSP test. |

The LSP failures are tracked in `docs/issues/2026-11-12-lsp-external-write-tests-fail-intermittently-in-the-full-suite.md` and in session-log entry F-13.
