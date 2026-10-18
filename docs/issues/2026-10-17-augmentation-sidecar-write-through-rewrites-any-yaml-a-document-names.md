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
unverified: 'The check and the write are separate syscalls, so a link planted between them still wins. Containment needs a repository root: with no non-link ancestor holding .git only the lexical and leaf rules apply. The append_entry.rs sidecar reads are a #[cfg(test)] doc gate and were left unchanged.'
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

## Round 2 (Opus review of `8ff717a8`), all measured red first

The first fix was LEXICAL, and the review found three more ways through the same declaration:

1. **A symlinked directory component.** `docs/augmentations -> <outside dir>` with `expects_augmentation:
   docs/augmentations/t.yaml`: `write_through` returned success and overwrote the outside YAML (through `doc(augment)` the
   call returned `"ok"`). A sidecar that does not parse is overwritten by design, so any existing YAML there was exposed.
   `doctor fix=export_augmentations` created files through the same link (`exported: 1`, a file in the outside directory).
2. **The READ side.** `sidecar_shape_drift`, `sidecar_unparseable` and `reindex`'s restore opened any declared absolute or
   `..` path, and `serde_yml`'s error echoed what it could not parse (`invalid type: string "ghp_…"`) into the finding the agent
   reads. Reindex's restore also turned an outside file's `prompt:` into a catalog row.
3. **`doc(augment)` surfaced the refusal as FATAL**, after the catalog had moved, with the wrong remedy: it wrapped every
   `write_through` error in a plain `anyhow!` (`isError: true`), persisted the new prompt first, and said "re-run once the path
   is writable".

Fix (`033bf24b`, patch-id `b5b500849f73cad3f85234d916eadb5b1f144839`): `augmentation_sidecar::confined_path` (lexical AND
resolved, `util::refuse_path_outside_root`) is the one gate; `read_in`, `declared_target` (`write_through`), the export and
`preflight_write_through` all go through it. A refused declared path is never opened, so no parser error can echo content. A
sidecar directory link whose target stays inside the repo, and a plain one, still work (controls). `doc(augment)` runs the
preflight BEFORE any row moves and keeps a refusal's recoverable type and its own remedy; the old message stays for a genuine
IO failure. Tests: `link_dir_tests.rs` (`doc_augment_*`, `doctor_export_does_not_create_sidecars_through_a_directory_link_out_of_the_repo`),
`doctor.rs` (`a_sidecar_declared_outside_the_repo_is_refused_and_its_content_never_echoed` with a control showing the parser echoes
the fake token when asked directly, `a_sidecar_behind_a_directory_link_out_of_the_repo_is_refused_and_never_echoed`), `reindex.rs`
(`a_declared_sidecar_outside_the_repo_is_neither_attached_nor_echoed`), `augment.rs`
(`a_refusal_from_write_through_stays_recoverable_and_keeps_its_own_remedy`). Mutations of each new guard KILLED; the export's own
`confined_path` call survives because the write door refuses the same path first (defence in depth). The three read sites in
`append_entry.rs` sit in `#[cfg(test)] mod taxonomy_recipes` (a doc-to-code gate over this repository), so they are not an agent-reachable
read and were left alone.

## Resume

N/A. The read side, the directory-link form and the agent-visible error shape are closed (round 2).

## References

- `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md`
