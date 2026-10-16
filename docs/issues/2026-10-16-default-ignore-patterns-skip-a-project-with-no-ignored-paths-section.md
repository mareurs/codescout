---
id: '90157cd66c15921c'
kind: bug
status: fixed
title: 'BUG: a project with no [ignored_paths] section gets no default ignore patterns in the code index'
owners:
- marius
tags:
- cluster/declared-not-wired
- retrieval
- config
- indexing
opened: 2026-10-16
closed: 2026-10-16
owner: marius
related:
- docs/superpowers/specs/2026-06-30-default-ignore-globs-design.md
severity: medium
---

# BUG: a project with no [ignored_paths] section gets no default ignore patterns in the code index

## Summary
`default_ignored_patterns()` (`.git`, `.codescout`, `node_modules`, `target`, `__pycache__`, `.venv`, `dist`, `build`, …) reaches a project only when its `project.toml` has an `[ignored_paths]` section without a `patterns` key. When the section is absent, the derived `Default` gives `patterns = []`, and every indexer caller passes that list through with no fallback. The 2026-06-30 design's goal, "dependency/artifact trees are excluded by default", does not hold for such projects.

## Symptom (Effect)
The code index walk for such a project is filtered only by `.gitignore` and the `lang_for_ext` allowlist, so un-gitignored dependency or build trees are embedded. Observed on this machine (2026-10-16): 5 of 7 real `.codescout/project.toml` files carry an explicit `patterns = []` (claude-plugins, and four under `~/work/mirela/`), so they ignore nothing either; one of them, backend-kotlin, is the deliberate case the design names.

## Reproduction
At experiments `a16f7d42`:
1. Take a project whose `.codescout/project.toml` has no `[ignored_paths]` section (or no `project.toml`), with no `~/.config/codescout/config.toml`.
2. `ProjectConfig::load_or_default(root)` → `config.ignored_paths.patterns` is `[]` (confirmed by a scratch probe during the 2026-10-16 triage; I re-read the code path, I did not re-run the probe).
3. `index` / `codescout index` pass that `[]` as `SyncOpts.ignore_patterns`.

## Environment
codescout, Linux, branch experiments, HEAD `a16f7d42`.

## Root cause
- `src/config/project.rs:174-177`: `#[derive(Debug, Clone, Serialize, Deserialize, Default)] pub struct IgnoredPathsSection { #[serde(default = "default_ignored_patterns")] pub patterns: Vec<String>, … }`. The serde field default applies only when the table is present; the derived `Default` ignores it and yields an empty `Vec`.
- `src/config/project.rs:29-30`: `#[serde(default)] pub ignored_paths: IgnoredPathsSection` — an absent section takes the derived `Default`.
- `src/config/project.rs:646` (`default_for`) and `src/tools/onboarding.rs:1061` (the onboarding-built config) both use the derived `Default`.

## Evidence
- Callers take `.patterns` as is: `src/tools/semantic/index.rs:230`, `:339`, `:158-160` (library branch); `src/agent/mod.rs:1979`; `src/main.rs:312-314`; `src/bin/sync_project.rs:93-95`. `src/retrieval/sync.rs:30-32` documents "an empty vec ignores nothing".
- The design `docs/superpowers/specs/2026-06-30-default-ignore-globs-design.md` § 3 states "key absent → defaults apply".
- Provenance: found 2026-10-16 by the effort-steering Stage A critic experiment (a grounded critic reading the design against its snapshot `b9f63ee2`), verified at that snapshot by an independent verifier, and re-checked live at HEAD (`docs/research/2026-10-14-effort-steering-when-to-switch.md`).

## Hypotheses tried

N/A — the cause was read from the code. Verification at `f5f1c6f7`: a new test loading a `project.toml` with no `[ignored_paths]` section, one with no `project.toml`, and `ProjectConfig::default_for` all returned `patterns == []` (3 red) while the present-section-without-key and explicit-`patterns = []` cases passed — exactly the split the issue describes.

The `unverified:` frontmatter question, by reading only (not measured): `GlobalIgnoredPathsSection` has one `skip_serializing_if` field, so a present global config should re-serialise to an empty `[ignored_paths]` table and the serde default would apply there, meaning a machine with a global config got defaults and one without did not. Moot after the fix, since both paths now agree. Onboarding is where the explicit `patterns = []` came from, again by reading: `src/tools/onboarding.rs` builds the config with `Default::default()` and writes it with `toml::to_string_pretty`, which serialised the derived-empty list.

## Fix

Implemented 2026-10-16 on branch `worktree-agent-a5de56cff4863b268`, commit `80d4cb49` (not yet on `experiments`/master; re-cite the SHA after the merge). `src/config/project.rs`:

- `IgnoredPathsSection` implements `Default` by hand: `patterns` is `default_ignored_patterns()`. Absent section, absent file, `default_for` and onboarding's `Default::default()` now agree with the present-table serde default. An explicit `patterns = []` still means "ignore nothing".
- `patterns` carries `skip_serializing_if = is_default_ignored_patterns`. Without it, the whole-config write-backs (`onboarding.rs` creation and the two version-stamp rewrites) would write the 28 defaults into every `project.toml`, freezing them so a later default could never reach those projects. With it, an unmodified list stays implicit; a custom list or an explicit `[]` differs from the defaults and is still written, so a rewrite cannot flip an opt-out.

Decision taken on the issue's open question: onboarding writes no `patterns` key for a default config (so new projects follow future defaults). Already-written explicit `patterns = []` files (5 on this machine) are untouched; migrating them is still the operator's call.

## Tests added

All in `src/config/project.rs` `tests`:

- `absent_ignored_paths_section_gets_default_patterns` (red before the fix)
- `missing_project_toml_gets_default_patterns` (red before)
- `default_for_and_default_agree_on_default_patterns` (red before)
- `ignored_paths_section_without_patterns_key_gets_default_patterns` (guard, green before and after)
- `explicit_empty_patterns_still_means_ignore_nothing` (guard)
- `default_patterns_are_not_frozen_into_a_written_project_toml`, `explicit_empty_patterns_survive_a_serialise_round_trip` (guard the write-back half of the fix)

`cargo test --lib` over the whole lib: 6876 passed, 2 failed (`util::path_security::tests::write_to_tmp_allowed`, `tools::read_file::buffer_edge_tests::siblings_go_before_breadcrumb_and_the_pair_is_judged_together`); neither file mentions ignored paths, both fail deterministically on a rerun, and they were not run on a clean baseline.

## Workarounds
Add `[ignored_paths]` without `patterns`, or with an explicit list, to the project's `project.toml`.

## Resume

N/A — fixed. Archive to `docs/issues/archive/` once `80d4cb49` (or its cherry-pick) is on master (`git branch --contains <sha>`).

## References
- `docs/superpowers/specs/2026-06-30-default-ignore-globs-design.md`
- `docs/research/2026-10-14-effort-steering-when-to-switch.md` (Stage A)
