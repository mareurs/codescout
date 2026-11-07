---
kind: bug
status: open
title: the activation banner prints a project root whose name holds a backslash as a different path
tags:
- cluster/addressing-without-an-escape-hatch
opened: 2026-11-07
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

Not started. Direction: use `display_path(&root)` for the banner. The test `responses_emit_paths_relative_annotation_once_per_activation` in `src/server.rs` builds its expected root with `to_forward_slash`; add a case whose root name holds a backslash and requires the banner to name the real root. Related and not traced: other root-valued fields that use `to_forward_slash(&root)`, for example `project_root` in `src/tools/config/mod.rs` line 492 and the roots in `src/agent/mod.rs`, `src/tools/onboarding.rs` and `src/prompts/`.

## Tests added

N/A: bug only filed.

## Workarounds

Avoid backslashes in directory names. The relative paths in the response are correct; only the banner names the wrong root.

## Resume

Open. First step: write the test with a root named `aw\proj` and watch it fail with `aw/proj`.

## References

- `2d8475ed5bc831e5`, the file-name half of this class, fixed in `a6babe715`, `665a7758e` and `fe7f4cfdf`.
- `src/server.rs` (`post_process`), `src/util/fs.rs` (`to_forward_slash`, `display_path`).
