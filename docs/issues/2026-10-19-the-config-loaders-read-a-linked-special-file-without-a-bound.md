---
status: open
opened: 2026-10-19
closed:
severity: low
owner: marius
related: [docs/issues/2026-10-17-project-toml-under-the-writable-project-root-can-set-security-profile-root.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'By reading only. NOT run: a link to /dev/zero would read until memory is exhausted, so no probe was made on this machine. The claim that a special file reports length 0 is from the stat(2) contract, not measured here.'
---

# BUG: the config loaders check a file's reported length and then read it without a bound, so a project.toml that is a link to an endless special file is read until memory runs out

## Summary

`ProjectConfig::load_with_global_base`, `ProjectConfig::read_project_toml_table`,
`protected_files::security_on_disk` and `GlobalConfig::load_from_dir` each call `metadata().len()`,
refuse above 1 MiB, then `std::fs::read_to_string(path)` with no bound on the read itself. A path
whose reported length is 0 but which never reaches end of file (a link to `/dev/zero`, or a FIFO
nobody writes to) passes the length check and is then read until memory is exhausted, or forever.
`project.toml` is repository content and can be such a link.

## Class fit

A size limit that is enforced on a number the file reports about itself rather than on the bytes
actually read, so a file that lies about its size defeats it. Filed unclassified: it is a bound on
the read, not a path-containment fault, and no existing class names a limit checked before and not
during the operation it protects.

## Symptom (Effect)

By reading, not measured: activating (or reloading) a project whose `.codescout/project.toml` links
to `/dev/zero` never returns a config, and the process grows until the OS stops it. The same read
runs on every reload (`ActiveProject::reload_config_from_disk`) and in the write guard
(`security_on_disk`) on the next `atomic_write` of that path.

## Reproduction

Not run, deliberately. Shape: `ln -s /dev/zero <project>/.codescout/project.toml`, then start
`codescout` on the project. Do not do this on a machine you need.

## Environment

Linux, branch `fix/config-security-2026-10-18` (base `experiments` 7a09623bd). The sites are
older than that branch (`load_with_global_base` and `load_from_dir` predate it); the two helper sites
were added by it and copy the loader's pattern.

## Root cause

The 1 MiB cap is applied to `std::fs::metadata(path).len()`, then `std::fs::read_to_string(path)`
reads to EOF. For a regular file the two agree; for a character device, a FIFO or `/proc` entry the
reported length is 0 (or arbitrary) and says nothing about what a read returns. Sites, in the tree
measured: `src/config/project.rs` `load_with_global_base` and `read_project_toml_table`,
`src/config/protected_files.rs` `security_on_disk`, `src/config/global.rs` `load_from_dir`.
Inferred from the code, not measured.

## Evidence

The four call sequences above (`metadata` then `read_to_string`, no `take`).

## Hypotheses tried

None.

## Fix

Not applied; out of the delta of the review that found it. Direction: read through
`File::open(path)?.take(MAX + 1).read_to_string(..)` and refuse when more than `MAX` bytes came
back, so the bound is on the bytes read, and refuse a path that is not a regular file
(`metadata().is_file()` after following links) before opening it, which also avoids blocking on a
FIFO. One shared helper for the four sites would keep them from drifting.

## Tests added

N/A. Not fixed.

## Workarounds

Do not point `project.toml` at a special file; a project from an untrusted source should not be
activated without looking at `.codescout/`.

## Resume

Add the bounded, regular-file-only read helper, use it at the four sites, and test with a FIFO
(which blocks, so run it with a timeout) and a file larger than the cap, never `/dev/zero`.

## References

- `docs/issues/2026-10-17-project-toml-under-the-writable-project-root-can-set-security-profile-root.md`
