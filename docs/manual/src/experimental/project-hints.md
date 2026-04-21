# Project Hints

> ⚠ Experimental — may change without notice.

`activate_project` now returns a `project_hints` field with manifest-derived
context so agents have useful information even when `onboarding` has never been
run.

## Why

Previously, an agent hitting a codescout project in a client that never calls
`onboarding` saw only the `languages` list. No build commands, no entry points,
no manifest info. The agent had to probe the filesystem itself — or guess.

`project_hints` fills that gap with a cheap manifest probe that runs on every
`activate_project` call.

## What you get

```json
{
  "status": "ok",
  "project": "codescout",
  "project_root": "/home/you/work/codescout",
  "languages": ["rust"],
  "project_hints": {
    "primary_language": "rust",
    "manifest": "Cargo.toml",
    "entry_points": ["src/main.rs", "src/lib.rs"],
    "build_commands": ["cargo build", "cargo test", "cargo run"],
    "onboarded": false
  },
  "memories": [],
  "hint": "CWD: /home/you/work/codescout. Run project_status() for health checks and memory staleness."
}
```

### Fields

| Field | Meaning |
|-------|---------|
| `primary_language` | Language inferred from the detected manifest. `null` when no manifest recognised. |
| `manifest` | Filename that drove detection (`Cargo.toml`, `package.json`, etc.). |
| `entry_points` | Canonical entry-point files that exist on disk. Capped at 3. |
| `build_commands` | Build / test / run commands for the detected manifest. |
| `onboarded` | `true` when an `onboarding` memory exists (indicates a full onboarding has been performed at some point). |

## Supported manifests

| Manifest | Language | Notes |
|----------|----------|-------|
| `package.json` | `typescript` if `tsconfig.json` also exists, else `javascript` | Checked first — Node projects sometimes ship a `pyproject.toml` for tooling. |
| `Cargo.toml` | `rust` | |
| `pyproject.toml` / `setup.py` | `python` | |
| `go.mod` | `go` | |
| `pom.xml` | `java` | |
| `build.gradle.kts` / `build.gradle` | `kotlin` | |

## Relationship to `onboarding`

`project_hints` is **not** a replacement for the `onboarding` tool. It's a
cheap fallback for when `onboarding` has never been called:

| Signal | Source |
|--------|--------|
| Primary language, entry points, build commands | `project_hints` — always populated when a manifest exists |
| README summary, architecture notes, memory writes | `onboarding` — only after full scan |
| System prompt draft | `onboarding` — only after full scan |

When `onboarded: true`, the real memories (`project-overview`, `architecture`,
`language-patterns`) are the authoritative source. `project_hints` stays
populated for consistency but agents should prefer memory content when
available.

## Behaviour

- No probe runs if the project root has no recognised manifest — fields are
  `null` / empty arrays.
- All probes are read-only file-existence checks. No file parsing beyond
  checking `tsconfig.json` presence for TS vs JS disambiguation.
- First manifest match wins. `package.json` takes priority; otherwise the
  order is: Cargo.toml → pyproject.toml → setup.py → go.mod → pom.xml →
  build.gradle.kts → build.gradle.
