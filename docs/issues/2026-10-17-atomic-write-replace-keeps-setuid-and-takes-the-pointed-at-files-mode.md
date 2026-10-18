---
status: open
opened: 2026-10-17
closed:
severity: low
owner: marius
related: [docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md, docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: `atomic_write` re-applies the target's setuid/setgid bits after the write, and a replaced symlink takes the pointed-at file's mode, so a leaf link to a setuid file yields a setuid file owned by the writer

## Summary

`atomic_write` (`src/util/fs.rs`) copies the existing target's full mode, setuid and setgid included, onto the new
file after writing it (`fchmod`). Two consequences were measured: it undoes the kernel's clearing of setuid/setgid
on a write, and, because the mode is read with `metadata` (which follows a symlink), replacing an in-root symlink to
a setuid file produces a regular in-root file that is setuid and owned by whoever ran the write. Both are
pre-existing and not a regression of the staging fix; this records them. **Needs a deliberate decision: strip
setuid/setgid on replace?**

## Class fit

Not the containment class of the allowlist issue and not the staging-link defect: the path is allowed and the write
lands where it should; the question is which permission bits a replacement file inherits. Filed unclassified, as a
low-severity decision record.

## Symptom (Effect)

Measured 2026-10-17 on branch `worktree-agent-a26646b8b970ff11a` (`util::fs` code of `133ac224`), as a non-root
user (uid 1000), in a throwaway unit test (removed afterwards) under a `tempdir()`:

```
PROBE plain in-place write mode=755                       # a normal write clears 6755 -> 755 (kernel)
PROBE link replaced: is_file=true mode=6755 uid=1000; pointed-at mode=6755
PROBE direct atomic_write over setuid file mode=6755      # the kernel's clearing is undone
```

The first line is the control: an ordinary `OpenOptions::write` over a `6755` file leaves `755`. `atomic_write`
leaves `6755`.

## Reproduction

`mode 06755` file `target`; `link -> target`; `atomic_write(link, "new")`: `link` becomes a regular file, mode
`6755`, owned by the caller; `target` is untouched. Then `atomic_write(target, "new2")`: mode stays `6755`.

## Environment

Linux, non-root, ext4/tmpfs-class filesystem under `$TMPDIR`. Not run as root, on other filesystems, or on a
`nosuid` mount (where the bits are inert).

## Root cause

`src/util/fs.rs` `atomic_write_with`: `existing_mode` comes from `std::fs::metadata(path)` (follows the link) and is
applied with `File::set_permissions(from_mode(mode))` after the write, whole, with no masking of `0o6000`. Measured
2026-10-17 (above). `the_final_mode_is_unchanged_for_every_kind_of_target` pins both "setuid is kept" (`0o4755`) and
"a symlink target takes the pointed-at file's mode", so changing either needs a decision, not a bug fix.

## Evidence

Output above. The practical risk is small: creating a setuid file needs the writer to be the file's owner already
or the pointed-at file to be setuid, and the result is owned by the writer, so it grants nothing beyond what that
user could do. It is recorded because a regular file that silently keeps setuid is surprising in a project tree.

## Hypotheses tried

None needed.

## Fix

Not applied. Options: mask `0o6000` off the mode on replace (matches what a plain write does); or keep it for an
existing regular target and drop it for a replaced symlink; or leave as is. Needs a deliberate decision.

## Tests added

N/A: a decision record, not a fix. If the decision is to strip, `the_final_mode_is_unchanged_for_every_kind_of_target`
(the `0o4755` row) changes with it.

## Workarounds

None needed.

## Resume

Decide; then change `atomic_write_with` and the final-mode test together.

## References

- `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`
- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
