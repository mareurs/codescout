---
status: fixed
opened: 2026-10-17
closed: 2026-10-17
severity: medium
owner: marius
related: [docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'Fixed for the WRITE (augmentation_sidecar::write_through). Not examined: the READ side of the same declaration (doctor sidecar_shape_drift and sidecar_unparseable open the declared path), which can read an existing .yaml outside the repo and report on it. The fix refuses an absolute or ..-bearing declared path; it does not resolve a declared path that passes through a symlinked directory inside the repo.'
---

# BUG: `augmentation_sidecar::write_through` rewrites any existing `.yaml` file the document's own frontmatter names, inside or outside the repository

## Summary

`expects_augmentation:` in a document's frontmatter names its committed sidecar. `parse_declaration` accepts any string ending
in `.yaml`/`.yml` as a sidecar path and checks nothing else. `write_through` then did `root.join(&rel)` and, when
`path.is_file()`, rewrote it with the rendered sidecar. `Path::join` lets an ABSOLUTE `rel` replace the root outright and a
`..` segment climb out of it, so a document could name any existing `.yaml` file on the machine. Found while sweeping the
librarian's `fs::write` sites for the leaf-symlink issues; it is not a symlink defect.

## Class fit

A write through a path the document, not the caller, supplied, with no containment check on it: the same family as the
catalog-supplied paths in the `doc(update)` issue, reached through frontmatter text instead of a row. No named class
covers it; filed unclassified.

## Symptom (Effect)

Running `doc(action="augment")` on an artifact whose frontmatter says `expects_augmentation: ../outside.yaml` (or an absolute
path) overwrites that existing file with the rendered sidecar and returns `written: Some("../outside.yaml")`.

## Reproduction

Measured 2026-10-17 by a throwaway test (removed; it became the permanent test below): repo with `.git`, an existing
`base/outside.yaml` holding `prompt: OUTSIDE-ORIGINAL`, an artifact with `expects_augmentation: ../outside.yaml`, an
augmentation row, then `write_through(&cat, id, Authored::All)`:

```
PROBE result=Ok(WriteThrough { written: Some("../outside.yaml"), refused: [] })
PROBE victim_now="schema_version: 1\nprompt: Maintain the T-N table.\nentry_collection: observations\nparams_schema: ..."
```

The permanent test failed the same way against the unfixed code (red) before the fix.

## Environment

Linux, `cargo test --lib`.

## Root cause

`src/librarian/tools/doctor.rs` `parse_declaration` (extension-gated only) feeds `src/librarian/augmentation_sidecar.rs`
`write_through`, which joins the declared string onto the git root with no check that the result stays under it.

## Evidence

Measured, above. Reachability: the document must be an artifact the user then augments (`doc(action="augment")` calls
`write_through` after any shape change), so a hostile `.md` in a cloned repository is enough on its own only if someone
augments it; the sidecar must already exist as a file (`write_through` never creates one) and be writable by the user.

## Hypotheses tried

1. **Hypothesis:** `parse_declaration` rejects traversal. **Test:** read it. **Verdict:** rejected; only the extension is
   checked.

## Fix

Branch `worktree-agent-a66600865e063ffd2`, commit `d703c12b`, patch-id `f5e66cb1e077da8dd841797973a0519517ee586e`. In
`write_through`, before `root.join(&rel)`: refuse a `rel` with any `ParentDir`, `RootDir` or `Prefix` component, with a
recoverable error that names the declaring artifact and says to declare a repo-relative path. A loud error rather than a
silent skip, because `write_through`'s contract is that callers surface a failure.

## Tests added

`write_through_refuses_a_declared_sidecar_path_that_leaves_the_repo` (both `../outside.yaml` and the absolute spelling;
asserts the outside file is byte-identical), control `write_through_still_rewrites_a_declared_sidecar_inside_the_repo`.
Mutation probe: whole check off KILLED; `ParentDir` dropped KILLED (relative spelling); `RootDir` dropped KILLED (absolute
spelling).

## Workarounds

Do not run `doc(action="augment")` on an artifact from an untrusted repository whose frontmatter declares a sidecar you did
not write.

## Resume

N/A. Follow-up: check the read side (`sidecar_shape_drift`, `sidecar_unparseable`) for the same unconfined declared path.

## References

- `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md`
