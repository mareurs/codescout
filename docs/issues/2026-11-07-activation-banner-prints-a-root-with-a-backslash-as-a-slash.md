---
kind: bug
status: fixed
title: the activation banner prints a project root whose name holds a backslash as a different path
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-11-07
opened: 2026-11-07
unverified: 'Fixed, gated and seen live after a rebuild, with a failing test first and a killed mutant. One gap remains: the other root-valued fields listed under Fix were traced by reading only.'
---

# BUG: the activation banner prints a project root whose name holds a backslash as a different path

## Summary

The once-per-activation banner `[codescout] paths are relative to <root>` writes the root through `to_forward_slash`, which replaces each `\` with `/` on every platform. On Linux a backslash is an ordinary directory-name character, so a project rooted at `.../aw\proj` gets the banner `paths are relative to .../aw/proj`: a path that does not exist. Every relative path in the same response is relative to the real root, so a reader who follows the banner resolves them against the wrong directory.

## Class fit

`cluster/addressing-without-an-escape-hatch`, the same class as `2d8475ed5bc831e5`. The banner names a root the caller may feed back as an address, and the display form turns a legal directory-name character into the separator. This is the root-name half of the class; `2d8475ed5bc831e5` covers file names inside the project.

## Symptom (Effect)

After a rebuild on 2026-11-07, `grep(mode="files", pattern="needle", workspace=<scratchpad>/aw\proj)` answered:

```
1 matches in 1 files
      1  src/x.txt
[codescout] paths are relative to <scratchpad>/aw/proj
```

The directory on disk is `<scratchpad>/aw\proj` (`ls -b` shows `aw\\proj`).

## Reproduction

Run 2026-11-07, Linux, on the build after `fe7f4cfdf`:

```
mkdir -p "$SCRATCH/aw\\proj/src" "$SCRATCH/aw\\proj/.codescout"
printf '[project]\nname = "awproj"\n' > "$SCRATCH/aw\\proj/.codescout/project.toml"
printf 'needle\n' > "$SCRATCH/aw\\proj/src/x.txt"
grep(mode="files", pattern="needle", workspace="$SCRATCH/aw\\proj")
```

The first response after server start or after `workspace(activate)` carries the banner. `$SCRATCH` was the session scratchpad directory.

## Environment

Linux 7.2, branch `experiments`, build after `fe7f4cfdf`.

## Root cause

Read on 2026-11-07, `src/server.rs`, `CodeScoutServer::post_process`, line 1069: `let root = to_forward_slash(&root);`, then line 1071 formats the banner with it. `to_forward_slash` (`src/util/fs.rs`) is `p.to_string_lossy().replace('\\', "/")`. `display_path`, added in `a6babe715`, keeps a backslash on non-Windows hosts and is the fix pattern.

## Evidence

The live output quoted under Symptom, and the two code lines above.

## Hypotheses tried

None.

## Fix

Written and committed 2026-11-07 (`afdb4c3d3`): `src/server.rs`, `CodeScoutServer::post_process`, the banner root goes through `display_path` instead of `to_forward_slash`. The `to_forward_slash` import stays under `#[cfg(test)]`, because the test modules reach it through `use super::*`.

A test helper was added: `test_support::make_server_in_root(project_toml, root_name)`, with `make_server_with_project_toml` delegating to it, so a test can root the project under a chosen directory name.

Not changed: the other root-valued fields that use `to_forward_slash(&root)`: `project_root`, the CWD hint and `main_root` in `src/tools/config/mod.rs`, and the root in `src/agent/mod.rs`. They were traced by reading only. `relative_root` of a workspace member is normalized to forward slashes when the configuration is read (`src/workspace.rs`, `declared_relative_root`), so a member root with a backslash cannot be declared; that one is by design.

## Fix provenance

- **SHA:** `afdb4c3d3` (`experiments`)
- **patch-id:** `134975bbe211d7bc89f545f2589db9826db11fda`

Fix committed 2026-11-07, not pushed when this was written. Gate: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0; LEAN 6184 and DEFAULT 9064 passed, counts that include other sessions' commits.

## Tests added

`the_banner_names_a_root_whose_name_holds_a_backslash_as_it_is_on_disk` in `src/server.rs` (`cfg(unix)`): a project rooted at `aw\proj`; the first `post_process` response must carry `[codescout] paths are relative to <real root>`.

Red first: it failed on the old code with `want: .../aw\proj` and `got: .../aw/proj`. Mutant: `to_forward_slash` restored on the banner failed it; reverted. The existing `responses_emit_paths_relative_annotation_once_per_activation` still passes.

## Workarounds

Avoid backslashes in directory names. The relative paths in the response are correct; only the banner names the wrong root.

## Resume

Fixed in `afdb4c3d3` and seen live on 2026-11-07 after a rebuild and `/mcp` reconnect: `grep(mode="files")` on the project rooted at `aw\proj` answered `src/x.txt`, and the banner read `paths are relative to <scratchpad>/aw\proj`, with the backslash (before: `aw/proj`). See the `unverified` caveat in the frontmatter. Part of the class of `2d8475ed5bc831e5`.

## References

- `2d8475ed5bc831e5`, the file-name half of this class, fixed in `a6babe715`, `665a7758e` and `fe7f4cfdf`.
- `src/server.rs` (`post_process`), `src/util/fs.rs` (`to_forward_slash`, `display_path`).
