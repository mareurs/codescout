---
status: fixed
opened: 2026-06-03
closed: 2026-06-04
severity: medium
owner: marius
related: []
tags: [onboarding, workspace, language-detection, project-hints, memory]
kind: bug
---

# BUG: per-project `languages` + `primary_language` derive from manifest type, not file content — polyglot roots mislabeled (Python repo tagged JS)

## Summary
codescout infers a project's `languages` and `primary_language` from *which build
manifest is present*, never from the actual files. In a polyglot repo whose root
manifest doesn't match the dominant language — e.g. a Python project that keeps a
`package.json` at the root for tooling — the project is labeled
`typescript/javascript` and Python disappears. This silently corrupts the
onboarding `onboarding` + `language-patterns` memories (and the stored system-prompt
draft) that steer every agent session on that project.

## Symptom (Effect)
Observed live on the `hermes-agent` client repo (Python-dominant: `run_agent.py`,
`cli.py`, `agent/`, `gateway/`, `tools/`, `hermes_cli/`, `pyproject.toml` at root;
TS/JS confined to `ui-tui/`, `web/`, `website/`). `workspace(action="status")`
returned:

```json
"project_hints": {
  "primary_language": "javascript",
  "manifest": "package.json",
  "build_commands": ["npm test", "npm run build"]
},
"workspace": [
  { "id": "hermes-agent", "root": ".", "languages": ["typescript", "javascript"] }
]
```

…while the SAME response's top-level `languages` (the file-walk view) was correct:

```json
["bash","css","html","javascript","markdown","python","ruby","tsx","typescript"]
```

Downstream effect: `onboarding()` wrote the `language-patterns` and `onboarding`
memories for `hermes-agent` (and the `hermes_cli` sub-project) with TypeScript/
JavaScript only — no Python section — which had to be corrected by hand this
session.

## Reproduction
At codescout `d059f70` (branch `experiments`, v0.14.0):

1. Take any repo whose root has BOTH `package.json` (with `scripts` or `main`, so it
   passes the conditional-manifest check) AND `pyproject.toml`, but is mostly Python
   by file count.
2. `workspace(action="activate", path=<repo>)` then `workspace(action="status")`.
3. Observe `project_hints.primary_language == "javascript"` and the root project
   `languages == ["typescript","javascript"]`; Python is absent.
4. `onboarding()` → the written `language-patterns` memory omits Python.

## Environment
codescout v0.14.0, commit `d059f70cad875c74e7be5e61e2407ab2fc34df41`, branch
`experiments`. Rust, MCP. Reproduced against the active client project
`hermes-agent` — not codescout-specific; any polyglot repo where the root manifest
≠ dominant language triggers it.

## Root cause
Per-project language attribution keys off the manifest *type*, never the files:

- `src/workspace.rs:29-216` `discover_projects()` maps each manifest filename to a
  HARDCODED language list near the top of the function —
  `("package.json", &["typescript","javascript"])`,
  `("pyproject.toml", &["python"])`, etc. (the `manifests` / `conditional_manifests`
  tables). It records at most ONE manifest per directory
  (`!manifest_dirs.contains_key(&dir)` + `break` in the conditional loop), so a root
  holding both `package.json` and `pyproject.toml` keeps only the first one the
  walker hits — here `package.json` → `["typescript","javascript"]`. The resulting
  `DiscoveredProject.languages` reflects the manifest, not the file mix, and Python
  is dropped entirely.
- `src/mcp_resources/project_hints.rs:36-55` `probe_project_hints()` sets
  `primary_language` from `detect_manifest_info()` — manifest-first.
- `src/mcp_resources/project_summary.rs:63-98` `detect_primary_language()` is also
  manifest-first (`package.json` → `typescript` if `tsconfig.json` present else
  `javascript`), with only a `configured.first()` fallback.

The accurate, file-content approach already exists but is NOT used by discovery or
hints: `src/dashboard/api/project.rs:28-42` `detect_languages()` walks files and maps
extensions via `crate::ast::detect_language()` (`src/ast/mod.rs:61`). Nothing in the
discovery/hints path consults file content or file-count dominance.

Net: `DiscoveredProject.languages` — consumed at `src/tools/onboarding.rs:1184`
(`p.discovered.languages.clone()`) and written into per-project memories at
`src/tools/onboarding.rs:838-844` — and `primary_language` are manifest artifacts, so
any repo where the root manifest ≠ dominant language is mislabeled, and the bad value
propagates into persisted memories + the system-prompt draft.

## Evidence
- Live `workspace(action="status")` on `hermes-agent`: `primary_language: "javascript"`,
  root `languages: ["typescript","javascript"]`, vs. the file-walk top-level
  `languages` listing `python` (+8 others). Quoted under **Symptom**.
- Manifest→language tables and single-manifest-per-dir + `break`:
  `src/workspace.rs:29-216` (`discover_projects`).
- Manifest-first primary language: `src/mcp_resources/project_hints.rs:36-55`,
  `src/mcp_resources/project_summary.rs:63-98`.
- The bypassed file-walk detector: `src/dashboard/api/project.rs:28-42`.
- Memory writers that consume the bad value: `src/tools/onboarding.rs:838-844`, `:1184`.

## Hypotheses tried
N/A — direct code-inspection finding corroborated by a live `workspace(status)`
observation; not intermittent.

## Fix

**Implemented 2026-06-04 on `experiments`** — direction: *languages reflect actual file mix; primary_language by dominance.*
- `src/workspace.rs`: added `scan_languages_by_dominance` (bounded depth-6 / 1000-file, gitignore-aware, file-count-ordered) + `dominant_language` (skips markdown) + `merge_languages`. `discover_projects` now runs a **post-pass** (after the manifest-based domination logic, so project detection is unperturbed) setting each `DiscoveredProject.languages = merge(dominance_scan(root), manifest_langs)` — dominance first, manifest unioned as fallback. Manifest-only dirs (empty scan) keep their manifest langs.
- `src/mcp_resources/project_hints.rs` + `project_summary.rs`: `primary_language` / `detect_primary_language` consult `workspace::dominant_language` first, manifest/configured as fallback.

Result: a Python-dominant repo with a tooling `package.json` reports `languages` led by `python` and `primary_language=python`; `python` is no longer dropped. Manifest-only fixtures unchanged (empty-scan guard), so existing tests stayed green.

clippy clean; full lib suite green (2616 pass).
## Tests added

- `workspace::tests::polyglot_root_languages_reflect_file_dominance` — root with `package.json` as the only manifest + 4 `.py` + 1 `.ts`: `languages` leads with `python` (dropped pre-fix), `typescript` retained as fallback, `dominant_language` = `python`.
## Workarounds
- Trust the top-level `workspace(action="status").languages` (file-walk) over
  `project_hints.primary_language` and the per-project `languages` — the file-walk view
  is already accurate.
- After onboarding a polyglot repo, hand-correct the memories:
  `memory(action="write", topic="language-patterns", project_id=<id>, content=...)` and
  likewise for `onboarding`. Done this session for `hermes-agent` and `hermes_cli`.

## Resume

**Fixed 2026-06-04 on `experiments`** (see ## Fix). Not yet on master — ship via Standard Ship Sequence, then `git mv` to `docs/issues/archive/` citing the **master-side** SHA. Note: the dominance scan now runs in three places (`discover_projects` + the two `primary_language` sites); a future optimization could compute once and thread it through, but each walk is bounded and acceptable on activate/status.
## References
- `src/workspace.rs:29-216` (`discover_projects`, manifest→language tables)
- `src/mcp_resources/project_hints.rs:36-55` (`probe_project_hints`, manifest-first `primary_language`)
- `src/mcp_resources/project_summary.rs:63-98` (`detect_primary_language`, manifest-first)
- `src/dashboard/api/project.rs:28-42` (`detect_languages`, the unused file-walk detector)
- `src/ast/mod.rs:61` (`detect_language`, per-file extension map)
- `src/tools/onboarding.rs:838-844`, `:1184` (per-project memory writers consuming `DiscoveredProject.languages`)
- Session 2026-06-03: hermes-agent / hermes_cli onboarding — memories corrected by hand
