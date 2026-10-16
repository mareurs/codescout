---
id: '90157cd66c15921c'
kind: bug
status: open
title: 'BUG: a project with no [ignored_paths] section gets no default ignore patterns in the code index'
owners:
- marius
tags:
- cluster/declared-not-wired
- retrieval
- config
- indexing
opened: 2026-10-16
owner: marius
related:
- docs/superpowers/specs/2026-06-30-default-ignore-globs-design.md
severity: medium
unverified: With a global ~/.config/codescout/config.toml present, GlobalConfig::to_toml_value may emit an empty [ignored_paths] table, which would make the serde field default apply; not measured. Whether onboarding wrote the explicit `patterns = []` seen in local project.toml files is not established.
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
N/A — the cause is read from the code, not inferred.

## Fix
Not started. Likely shape: implement `Default` for `IgnoredPathsSection` by hand so `patterns` defaults to `default_ignored_patterns()`, keep an explicit `patterns = []` meaning "ignore nothing", and decide what onboarding writes. Already-written explicit `[]` lists are untouched by such a fix; whether to migrate them is an operator decision. The index prunes newly excluded files on the next sync (the design's self-healing step).

## Tests added
None yet — a regression test should load a config with no `[ignored_paths]` section and assert the defaults.

## Workarounds
Add `[ignored_paths]` without `patterns`, or with an explicit list, to the project's `project.toml`.

## Resume
Decide the onboarding behaviour, then make the absent-section case return the defaults, with the regression test above.

## References
- `docs/superpowers/specs/2026-06-30-default-ignore-globs-design.md`
- `docs/research/2026-10-14-effort-steering-when-to-switch.md` (Stage A)
