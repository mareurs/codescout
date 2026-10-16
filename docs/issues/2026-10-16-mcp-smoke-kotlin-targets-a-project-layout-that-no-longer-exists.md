---
id: '5697348ca3d72700'
kind: bug
status: open
title: 'BUG: tests/mcp-smoke-kotlin.sh targets a project layout that no longer exists, so it cannot run'
tags:
- cluster/doc-contradicted-by-code
opened: 2026-10-16
owner: marius
related:
- docs/issues/2026-09-24-residual-mcp-smoke-scripts-run.md
severity: low
---

# BUG: tests/mcp-smoke-kotlin.sh targets a project layout that no longer exists, so it cannot run

## Summary

`tests/mcp-smoke-kotlin.sh` hardcodes `KOTLIN_PROJECT=/home/marius/work/mirela/backend-kotlin` and asserts on `src/main/kotlin/edu/planner/service/AuthService.kt`, `CalendarService.kt`, `NotificationService.kt`, `build.gradle.kts` and `gradle.properties` at that project's root. Found while running the smoke scripts for `docs/issues/2026-09-24-residual-mcp-smoke-scripts-run.md`, 2026-10-16.

## Class fit

The script is a document of a fixture project's layout, and the fixture moved: the script's account of the project is contradicted by the project, and nothing re-reads the script against it, the same way the static tool-name gate had to be added for the server side.

## Symptom (Effect)

Checked 2026-10-16 with `wc -l` on the five paths the script reads: `README.md` (346 lines) and `docker-compose.yml` (82) exist; `src/main/kotlin/edu/planner/service/AuthService.kt`, `build.gradle.kts` and `gradle.properties` do not exist at that root (`ls src` fails). `find -maxdepth 4` finds `build.gradle.kts` and `gradle.properties` only under `ktor-server/` (and under `.worktrees/innovaplan-adopt/ktor-server/`), and finds no `AuthService.kt` or `CalendarService.kt`. Every symbols and read_file step on those paths would answer "not found", so the script cannot go green.

## Reproduction

```
ls /home/marius/work/mirela/backend-kotlin/src
find /home/marius/work/mirela/backend-kotlin -maxdepth 4 -name AuthService.kt -not -path '*/.git/*'
```

## Environment

This machine, 2026-10-16. The script also would start `codescout` against the live mirela project, which writes `.codescout/` state there, so it was not run.

## Root cause

Not investigated beyond the layout check above. Whether the planner service moved, was renamed, or lives in another repository is a question about the mirela project, not about codescout.

## Fix

Decision needed: pick a Kotlin fixture the script may run against (a checked-in `tests/fixtures/` Kotlin project, or a scratch copy of a mirela module) and re-point the paths. The helper-level repairs (plain-text assertions, guide stripping, no read_file refusal steps) are already applied to this script by the same change that fixed `mcp-smoke-rust.sh`, but they are unrun here.

## Tests added

N/A: a fixture decision is required before any live run; the static gates in `tests/mcp_smoke_scripts_reference_real_tools.rs` already cover this script.

## Workarounds

Run `tests/mcp-smoke-rust.sh` only.

## Resume

1. Choose the fixture. 2. Re-point `KOTLIN_PROJECT` and the file paths. 3. Run once against a throwaway copy and record the output.

## References

- `docs/issues/2026-09-24-residual-mcp-smoke-scripts-run.md`
- `tests/mcp-smoke-kotlin.sh`
