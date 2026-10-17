---
id: '5697348ca3d72700'
kind: bug
status: fixed
title: 'BUG: tests/mcp-smoke-kotlin.sh targets a project layout that no longer exists, so it cannot run'
tags:
- cluster/doc-contradicted-by-code
opened: 2026-10-16
closed: 2026-10-17
unverified: 'STANDING the repointed script has not been run live: no kotlin-language-server on this machine and a release build is out of scope for the fixing session. Only the static gate (every path literal exists in the fixture or is written by the script) and a path mutation were run; assertion strings, globs and the LSP-dependent symbols steps are unproven until someone runs the Kotlin smoke script with the server installed.'
owner: marius
related:
- docs/issues/archive/2026-09-24-residual-mcp-smoke-scripts-run.md
severity: low
---

# BUG: tests/mcp-smoke-kotlin.sh targets a project layout that no longer exists, so it cannot run

## Summary

`tests/mcp-smoke-kotlin.sh` hardcodes `KOTLIN_PROJECT=/home/marius/work/mirela/backend-kotlin` and asserts on `src/main/kotlin/edu/planner/service/AuthService.kt`, `CalendarService.kt`, `NotificationService.kt`, `build.gradle.kts` and `gradle.properties` at that project's root. Found while running the smoke scripts for `docs/issues/archive/2026-09-24-residual-mcp-smoke-scripts-run.md`, 2026-10-16.

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

The script's target was a project on one developer's disk (`/home/marius/work/mirela/backend-kotlin`), outside this repository, so nothing in the repo could keep its layout true. It also would have written `.codescout/` state into that live project. The checked-in `tests/fixtures/kotlin-library` was the available project the repo owns.

## Fix

Decision taken (2026-10-17): the script targets a throwaway copy of the checked-in `tests/fixtures/kotlin-library` (`KOTLIN_FIXTURE`, copied into `mktemp -d`, removed by the `cleanup` trap), which also resolves the `.codescout/` write concern. The fixture carries no README, `.properties` or `.yml`, so the copy gets one of each written by the script (`gradle.properties`, `README.md`, `docker-compose.yml`) to keep the non-code `read_file` steps. The AuthService/CalendarService assertions were re-pointed at `Catalog.kt`, `Book.kt`, `Genre.kt` and `Results.kt` (`SearchResult` is the sealed class), and the service-file glob at `**/models/*.kt`. Changed: `tests/mcp-smoke-kotlin.sh`.

## Tests added

`the_kotlin_smoke_script_targets_a_checked_in_fixture_and_every_path_it_reads_exists` in `tests/mcp_smoke_scripts_reference_real_tools.rs`: the script names no `/home/<user>/` path, declares a `KOTLIN_FIXTURE` that exists, and every `"path"` literal exists in the fixture or is written by the script into its copy; non-vacuity asserted. Red against the old script (names the `/home/marius/work/mirela` lines), green against the new one (5/5 in the target). Mutation: renaming `Genre.kt` to `Genr.kt` in the script reds it with the offending line; restored. Ceiling: `"glob"` and `"pattern"` values and assertion strings are not resolved against the fixture; only a live run reads those (see `unverified`).

## Workarounds

Run `tests/mcp-smoke-rust.sh` only.

## Resume

Run `./tests/mcp-smoke-kotlin.sh` once with `kotlin-language-server` installed and record the pass count here; if an assertion string disagrees with the fixture, fix the script. Then archive with the fix SHA (labelled experiments) and its patch-id.

## References

- `docs/issues/archive/2026-09-24-residual-mcp-smoke-scripts-run.md`
- `tests/mcp-smoke-kotlin.sh`
