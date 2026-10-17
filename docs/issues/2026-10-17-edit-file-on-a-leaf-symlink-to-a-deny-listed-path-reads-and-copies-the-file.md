---
status: open
opened: 2026-10-17
closed:
severity: high
owner: marius
related: [docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: `edit_file` on an in-root leaf symlink to a deny-listed path reads the file the read tools refuse, echoes it in its not-found error, and can copy it into the project

## Summary

The write classifier checks the deny list against "resolved parent + leaf as written"
(`src/util/path_security.rs:528-555`), so `<root>/leaf -> ~/.ssh/id_rsa` classifies as an allowed in-root write.
`edit_file` then reads the file through the link (`src/tools/edit_file/mod.rs:789`) although `read_file` refuses the
same link. Measured: a near-miss multi-line `old_string` returned the file's lines inside the error, and a unique
match succeeded, which replaces the link with a regular in-project copy of the secret.

## Class fit

This is the leaf-symlink shape (shape 1) of the write-allowlist issue, but with the READ side as the escape: the
classifier answers correctly for the spelling it is given and the deny list is applied to that spelling, not to
what the OS resolves. Filed unclassified for the same reason as that issue: no class names a containment check
that resolves the parent and trusts the leaf. It is separate because the consequence is disclosure of a
deny-listed file, not an out-of-root write (which `atomic_write` prevents by renaming over the link).

## Symptom (Effect)

With `leaf.txt` a symlink inside the project to a deny-listed file, `edit_file` on `leaf.txt`:

- with an `old_string` that nearly matches over several lines, fails with a message that contains the file's lines
  (the not-found "nearest content" hint);
- with a unique `old_string`, succeeds, the victim is untouched, and `leaf.txt` becomes a regular file holding the
  victim's content with the edit applied. The copy is an ordinary in-project file, so `read_file` then reads it.

## Reproduction

Measured 2026-10-17 on branch `worktree-agent-a26646b8b970ff11a`, with a throwaway test in
`src/tools/edit_file/tests.rs` (removed afterwards). The deny list is built from `$HOME`, so the test ran as a
single filtered test with `HOME=<scratchpad>/fakehome` and `fakehome/.ssh/id_rsa` holding the text
`FAKE-SECRET-KEY-MARKER` (no real key was involved); project root a `tempdir()`, `leaf.txt` symlinked to that file.

```
read_file(leaf):                 ok=false leaks=false      # the read path denies it
edit_file(leaf, 1-line miss):    ok=false leaks=false      # "No exact or whitespace-normalized match ..."
edit_file(leaf, 2-line near miss): ok=false leaks=true     # error text contains the file's lines
edit_file(leaf, unique match, 1-line file): ok=true        # key unchanged; leaf now a REGULAR file with the content, edited
```

(The unique-match case ran against a one-line file; against the three-line file the same call was refused as
"found 3 times", which is the tool's own ambiguity check, not a containment check.)

## Environment

Linux, worktree branch cut from `experiments` at 65fc6d6a; fake `$HOME` under the scratchpad.

## Root cause

`path_security.rs:528-555` compares deny entries with "resolved parent + leaf as written"; for `<root>/leaf` that is
`<root>/leaf`, which is allowed. `edit_file/mod.rs:789` reads the path (following the link) before any further check.
Measured 2026-10-17 (above). `read_file` refuses because the read path resolves the full path.

## Evidence

Output above. The exact text of the leaking error was not printed (the test checked `contains` for the marker, to
avoid dumping file content into logs); that the leak comes from the not-found nearest-content hint is inferred
from the message shape, not shown line by line.

## Hypotheses tried

None needed.

## Fix

Not applied. Candidates: for a write target whose leaf is a symlink, classify the fully resolved path as well and
require both to be allowed; or refuse to edit through a leaf link at all (the writer already replaces the link, so
an edit through one is surprising anyway).

## Tests added

N/A: filed, not fixed. The probe above becomes the regression test (it needs a way to inject the home directory
instead of reading `$HOME`).

## Workarounds

Do not keep symlinks to credential files inside a project root.

## Resume

Fix the classifier or the read in `edit_file`, then add the probe as a red test. Also check `edit_code`,
`edit_markdown` and the other tools that read the target before writing.

## References

- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
