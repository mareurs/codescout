---
status: open
opened: 2026-06-03
closed:
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
Not implemented — logged at the user's request (they chose to file the issue rather
than patch in-session). Selected direction: *"languages reflect actual file mix;
primary_language by dominance."*

1. Add a bounded file-content language scan (lift/reuse `detect_languages` from
   `src/dashboard/api/project.rs:28-42`) returning languages ordered by file-count
   dominance. Keep it bounded (depth + file cap, `ignore::WalkBuilder` gitignore-aware)
   — `discover_projects` runs on every `Agent::new`/activate, so it must stay cheap.
2. In `src/workspace.rs::discover_projects`, keep the manifest walk for project-root
   detection and the nesting/domination logic, but in a post-pass set each
   `DiscoveredProject.languages = merge(file_dominance_scan(root.join(relative_root)),
   manifest_langs)` — file-dominance order first, manifest langs unioned as fallback so
   manifest-only dirs (and the existing fixture tests) still resolve.
3. Derive `primary_language` from the dominant (first) language rather than the
   manifest, in `src/mcp_resources/project_hints.rs` / `project_summary.rs` (or have
   them consume the discovered project's ordered `languages`).
4. Update tests (several assert manifest-derived languages on fixtures that have a
   manifest but no/few source files — the manifest-fallback keeps those green; fixtures
   with mismatched source files will correctly change):
   `src/workspace.rs` tests (675-987), `src/mcp_resources/project_hints.rs` tests
   (166-289), `src/mcp_resources/project_summary.rs` tests (206-235).

## Tests added
N/A — not yet fixed. Regression test to add: a fixture dir with `package.json`
(+scripts) AND `pyproject.toml` AND mostly `.py` files → assert
`primary_language == "python"` and that `languages` lists `python` first.

## Workarounds
- Trust the top-level `workspace(action="status").languages` (file-walk) over
  `project_hints.primary_language` and the per-project `languages` — the file-walk view
  is already accurate.
- After onboarding a polyglot repo, hand-correct the memories:
  `memory(action="write", topic="language-patterns", project_id=<id>, content=...)` and
  likewise for `onboarding`. Done this session for `hermes-agent` and `hermes_cli`.

## Resume
Implement the file-dominance scan + merge in `src/workspace.rs::discover_projects` as a
post-pass over `found` (before the root-first reorder near the end of the function).
Bound the walk like `src/dashboard/api/project.rs:28-42` (max_depth ~4–6, ~500-file cap,
gitignore-aware). Then repoint `primary_language` in
`src/mcp_resources/project_hints.rs:36-55` to the dominant language. Run
`cargo test workspace`, `cargo test project_hints`, `cargo test project_summary`; update
the manifest-only fixtures' expectations.

## References
- `src/workspace.rs:29-216` (`discover_projects`, manifest→language tables)
- `src/mcp_resources/project_hints.rs:36-55` (`probe_project_hints`, manifest-first `primary_language`)
- `src/mcp_resources/project_summary.rs:63-98` (`detect_primary_language`, manifest-first)
- `src/dashboard/api/project.rs:28-42` (`detect_languages`, the unused file-walk detector)
- `src/ast/mod.rs:61` (`detect_language`, per-file extension map)
- `src/tools/onboarding.rs:838-844`, `:1184` (per-project memory writers consuming `DiscoveredProject.languages`)
- Session 2026-06-03: hermes-agent / hermes_cli onboarding — memories corrected by hand
