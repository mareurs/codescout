---
id: '5697348ca3d72700'
kind: bug
status: fixed
title: 'BUG: tests/mcp-smoke-kotlin.sh targets a project layout that no longer exists, so it cannot run'
tags:
- cluster/doc-contradicted-by-code
opened: 2026-10-16
closed: 2026-10-17
unverified: 'CLEARED 2026-10-17. Was: the repointed script had not been run live (only the static path gate and a path mutation); a live run with kotlin-lsp on 2026-10-17 passed 15/15, see Evidence.'
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

## Evidence

Live run of `tests/mcp-smoke-kotlin.sh`, 2026-10-17, after the fixes in this file (`TMPDIR=<scratch> ./tests/mcp-smoke-kotlin.sh`):

```
=== Tool Access Control ===
  PASS  [1]  read_file allows .properties files
  PASS  [1]  read_file allows .md files
  PASS  [1]  read_file allows .yml files
=== Symbol Navigation ===
  PASS  [1]  symbols overview finds Catalog members in Catalog.kt
  PASS  [1]  symbols locates the Catalog/search method
  PASS  [1]  symbols with include_body returns the add source
  PASS  [1]  symbols finds SearchResult sealed class with subclasses
  PASS  [1]  symbols returns Book members
=== Search ===
  PASS  [1]  grep finds suspend fun declarations
  PASS  [1]  tree glob locates model files
  PASS  [1]  semantic_search answers with a results envelope
=== Non-code Files ===
  PASS  [1]  read_file returns gradle.properties content
  PASS  [1]  read_file returns README.md with project name
=== Multi-step Exploration ===
  PASS  [2]  explore search: discover search flow (find class -> read method)
  PASS  [2]  explore models: discover model layer (find files -> symbols)
15/15 passed (0 failed) in 17 steps
```

The first live run, before the assertions were corrected, was 11/15: the overview step expected `createDefaultCatalog`, the body step expected `filter`/`searchText`, and the name lookup right after the overview said `0 matches`. Those are the sibling issues named under Fix.

## Fix

Decision taken (2026-10-17): the script targets a throwaway copy of the checked-in `tests/fixtures/kotlin-library`, which also resolves the `.codescout/` write concern.

- The copy is of the tracked files only (`git archive HEAD`, with a plain `cp -R` fallback outside a git tree and a warning), so untracked local state in the fixture directory (`.codescout/`, `.code-explorer/`) cannot change a result. It lives in a `mktemp -d` directory, with `TMPDIR` pointed inside it so kotlin-lsp's `temp_dir()` state goes with it.
- The `trap cleanup EXIT` is armed straight after the directory is made, so a failed build, copy or alias add cannot leak it. `cleanup` removes the per-run alias, stops the run's mux and `kotlin-lsp` (selected by the unique directory on their command lines) and only then removes the directory, because the mux re-creates its directories as it shuts down.
- The fixture carries no README, `.properties` or `.yml`, so the copy gets one of each written by the script to keep the non-code `read_file` steps.
- The `AuthService`/`CalendarService` assertions were re-pointed at `Catalog.kt`, `Book.kt`, `Genre.kt` and `Results.kt` (`SearchResult` is the sealed class), and the service-file glob at `**/models/*.kt`. Each assertion is on text a wrong answer cannot supply (`Catalog/search` is a name-path that appears nowhere in the request).
- `semantic_search` asserts the `results`/`total` envelope, as `tests/mcp-smoke-rust.sh` does: the copy is never indexed, so the reply is an empty envelope, which none of the earlier strings were in.
- `call()` retries a `0 matches` reply up to five times, because of `docs/issues/2026-10-17-symbols-name-lookup-on-kotlin-lsp-says-0-matches-for-a-symbol-that-exists.md`.
- The overview and body steps assert only what survives a fix of `docs/issues/2026-10-17-symbols-overview-of-a-kotlin-file-omits-members-and-top-level-functions.md` and `docs/issues/2026-10-17-symbols-include-body-on-kotlin-lsp-results-is-shifted-up-one-line.md`: the live output omits `createDefaultCatalog` from the overview and cuts the last line off a member's body, so the first assertions (written from the source) could not pass.
- Header and notes name `kotlin-lsp`, the server codescout starts (`src/lsp/servers/mod.rs:111`), not `kotlin-language-server`; the installer and manual drift is `docs/issues/2026-10-17-install-lsp-and-manual-name-kotlin-language-server-but-codescout-starts-kotlin-lsp.md`.

Changed: `tests/mcp-smoke-kotlin.sh`.

## Tests added

`the_kotlin_smoke_script_targets_a_checked_in_fixture_and_every_path_it_reads_exists` in `tests/mcp_smoke_scripts_reference_real_tools.rs`. It reads the script statically, with comment lines never counting as evidence, and requires: no `/home/<user>/` path anywhere; a `KOTLIN_FIXTURE` that exists; `KOTLIN_WORK` from `mktemp -d` and every `KOTLIN_PROJECT` assignment under it; `TMPDIR` exported inside it; `trap cleanup EXIT` armed after the directory exists and before `cargo build` and `mcp alias add`; a `cleanup()` that removes the alias, stops the processes and then removes the directory; the `git archive` copy (strip depth equal to the fixture's depth) and the `cp -R` fallback; and every `"path"` literal inside the fixture (no `..`, no leading `/`) or written by the script with `> "$KOTLIN_PROJECT/<path>"` on a code line. The same test runs the checker over mutants of the script, each of which must be reported, and a mutant whose anchor text has vanished panics.

Red against the old script (the two `/home/marius/work/mirela` lines). Green against the new one (5/5 in the target). Mutants run against the real file, each observed RED with the offending requirement: target the fixture itself; `cp`/`git archive` lines deleted; README write commented out; target under `$HOME`; `rm -rf` dropped; `pkill` dropped; `rm -rf` before `pkill`; alias removal dropped; `"path": "../../../Cargo.toml"`; trap removed; trap armed late; `TMPDIR` export removed; the README write present only in a comment. Ceiling: `"glob"` and `"pattern"` values and assertion strings are not resolved against the fixture, and a path that exists only as an untracked file passes the static check; only a live run reads those.

## Workarounds

Run `tests/mcp-smoke-rust.sh` only.

## Resume

Done. Live run 2026-10-17 (release build of this branch, `kotlin-lsp` at `/usr/bin/kotlin-lsp`, `TMPDIR` pointed at a scratch directory): 15/15 passed in 17 steps, exit 0; the alias was removed, the scratch `TMPDIR` was empty afterwards, and no process naming the run survived. An earlier run of the same script with the first assertions failed 11/15, which is what found the `semantic_search`, `include_body` and overview assertions that could not pass. Residual outside the script: the mux leaves a small `.lock` file per workspace in `/run/user/1000`, which the script cannot name. Archive with the fix SHA (labelled experiments) and its patch-id once this reaches experiments.

## References

- `docs/issues/archive/2026-09-24-residual-mcp-smoke-scripts-run.md`
- `tests/mcp-smoke-kotlin.sh`
