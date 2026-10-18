---
status: fixed
opened: 2026-10-17
closed: 2026-10-18
severity: high
owner: marius
related: [docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md, docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md, docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md]
tags:
- cluster/unclassified
kind: bug
unverified: Fix verified by running the fixing tests on experiments 1b9c4dab7 (12 passed), not by re-running this issue's own probe patch. The classifier in path_security.rs still answers allowed for a leaf link to a denied file; the fix refuses at the write resolvers. edit_code rename wiring is not integration-tested (survived a mutation in the fixing branch). Unix only. The librarian write sites were not re-measured by this session.
---

# BUG: `edit_file` on an in-root leaf symlink to a deny-listed file reads the file, and `insert` copies it into the project where `read_file` returns it

## Summary

The write classifier checks the deny list against "resolved parent plus the leaf as written", so a
symlink inside the project root whose LEAF points at a deny-listed file (`<root>/leaf ->
~/.ssh/id_rsa`) is classified `Allowed`. `edit_file` then reads the target with
`std::fs::read_to_string`, which follows the link. Measured against the real tools on 2026-10-17
(decoy key under a fixture `HOME`): the not-found error of `edit_file` echoed the decoy's lines, and
`edit_file` with `insert = "append"` replaced the link with a regular file in the project holding
the decoy's content, which `read_file` then returned. `read_file` on the link itself is refused. The
victim file is not modified. This is a READ of a deny-listed file; the write side of the same leaf
shape is in the related issue, which is owned by another session and is not touched here.

**Ownership and status.** Fixed on `experiments` by the peer session's commit `38a492b4`, merged in
`1b9c4dab7`. The peer filed its own record of the same bug:
`docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md`.
That file holds the fix, its tests and its mutation table. This file keeps the original measurements
(the probe output, the not-found echo, `std::fs::write` through a link, the librarian write sites)
and points at that record. Closed 2026-10-18 after the check under Tests added.

## Class fit

The deny list is enforced on the spelling of a path the caller supplied, and a leaf symlink
changes what that spelling names after the check. That is the same containment-on-one-spelling
shape as the related leaf-symlink and temp-dir issues, but the consequence differs: it discloses
a deny-listed file rather than writing outside the root. Filed unclassified.

## Symptom (Effect)

Verbatim output of the probe (fixture `HOME` = `<scratchpad>/port/leaf-home`, decoy `.ssh/id_rsa` =
`DECOY-KEY-LINE-ONE\nDECOY-KEY-LINE-TWO\n`, project root a fresh tempdir with `leaf` and `leaf2`
symlinks to the decoy):

```
1 read_file(leaf) -> Err(access denied: 'leaf' is in a protected location)
2 edit_file(leaf, old_string not present) -> Err(old_string not found in leaf — hint: No exact or whitespace-normalized match. Copy the actual bytes shown (or from read_file) and retry.)
  key file after step 2: "DECOY-KEY-LINE-ONE\nDECOY-KEY-LINE-TWO\n"; leaf is symlink: true
2b edit_file(leaf, 2-line old_string, first line right) -> Err(old_string not found in leaf. Nearest content at lines 1-2:
DECOY-KEY-LINE-ONE
DECOY-KEY-LINE-TWO — hint: No exact or whitespace-normalized match. Copy the actual bytes shown (or from read_file) and retry.)
3 edit_file(leaf2, insert=append) -> Ok(String("ok"))
  key file after step 3: "DECOY-KEY-LINE-ONE\nDECOY-KEY-LINE-TWO\n"; leaf2 is symlink: false; leaf2 content: Some("DECOY-KEY-LINE-ONE\nDECOY-KEY-LINE-TWO\nAPPENDED\n")
4 read_file(leaf2) after step 3 -> Ok(Object {"content": String("DECOY-KEY-LINE-ONE\nDECOY-KEY-LINE-TWO\nAPPENDED\n"), "total_lines": Number(3)})
```

Step 2 shows the echo is conditional (no line of `old_string` matched, so no window); step 2b shows
that one correct line, for a private key the well-known `-----BEGIN ... PRIVATE KEY-----`, is enough
to get a window of as many lines as `old_string` has.

## Reproduction

A throwaway test, not committed (patch in the session scratchpad `port/leaf-probe.patch`, output in
`port/leaf-probe-out.txt`): in a child process with `HOME` set to the fixture (so the real deny list
applies to a decoy under it), build the `ToolContext` the `edit_file` tests use over a fresh project,
`symlink(<fixture>/.ssh/id_rsa, <root>/leaf)`, and call the real `ReadFile` and `EditFile` tools as
in the output above. Run on branch `fix/write-allowlist-port-2026-10-17` at the item-7 tree (now
41cc27d7, rebased afterwards over unrelated fix-ups); the code on this
path (`classify_write_path_in`'s leaf handling, `read_edit_target`) is not touched by that branch.

## Environment

Linux, rustc 1.97.1, codescout branch `fix/write-allowlist-port-2026-10-17` cut from `experiments`
27efb11d.

## Root cause

- `classify_write_path_in` resolves the target with `canonicalize_write_target`: the parent through
  its deepest existing ancestor, the leaf appended as written (`src/util/path_security.rs`). The
  deny list is then compared with that, so a leaf link to a denied file is not seen. `read_file`
  uses `validate_read_path`, which canonicalizes the whole path and so does see it (step 1).
- `read_edit_target` (`src/tools/edit_file/mod.rs`) reads the classified path with
  `std::fs::read_to_string`, which follows the leaf link; `not_found_msg` echoes the nearest window of
  the content it read (`nearest_window_hint`).
- With `insert`, the edit is written back and the link is replaced by a regular file holding the
  content that was read (step 3, `leaf2 is symlink: false`; the related issue attributes the
  replace-not-follow to the tool's atomic write, which was not re-read here), so a later
  `read_file` of an ordinary in-root file succeeds.

Measured 2026-10-17: the output above.

## Evidence

The output above is the evidence. Separately measured the same day, because the librarian came up in
an Opus review of the sibling branch: `std::fs::write` on a path whose leaf is a symlink writes
through the link (scratch fixture, rustc 1.97.1: after `fs::write(leaf-link)` the victim held the
new content and `leaf-link` was still a symlink; source `port/fswrite_probe.rs`).

The librarian writes with plain `std::fs::write` at these non-test sites (read, grep of
`src/librarian`): `tools/update.rs:775` and `:1000`, `tools/create.rs:481`, `tools/mv.rs:638`,
`tools/doctor.rs:2042`, `catalog/rekey.rs:470`, `catalog/augmentation.rs:1023`, `:1953`, `:2222`
(and the restore at `:1091`). Whether a leaf symlink can reach any of them through a librarian tool
was NOT measured: those tools take catalog paths, and which of them consult the write classifier
first was not traced.

## Hypotheses tried

1. The write also escapes. Rejected for `edit_file`: the victim kept its content in steps 2 and 3,
   consistent with the related issue's note that its atomic write does not follow a leaf link.
   Other write paths (`create_file` with `overwrite`, `edit_code`, the librarian) were not run.

## Fix

Fixed by the peer session, commit `38a492b4` (see its issue file named in the Summary for the
design: the link target is run through the existing validators at the two shared write resolvers,
`write_ack::resolve_write_or_capture` and `fs::resolve_write_path_for`). It does not change
`src/util/path_security.rs`: `classify_write_path_in` still answers allowed for
`<root>/leaf -> ~/.ssh/id_rsa`, and the refusal comes from the resolvers. Nothing was applied by
the session that filed this issue.

## Tests added

None added by this issue's session. Checked 2026-10-18 on `experiments` `1b9c4dab7` (worktree, own
target slot): `scripts/with-slot.sh cargo test --lib -- leaf_link leaf_link_probe` gave
`test result: ok. 12 passed; 0 failed; 2 ignored`. The ignored two are subprocess helpers that the
parent tests run with a fake `$HOME`. The tests that cover this issue's shapes:
`edit_file_on_a_leaf_link_to_a_readable_deny_listed_file_leaks_and_copies_nothing` (edit_file single,
batch and insert, and `create_file`, absolute and relative link) and
`a_leaf_link_to_a_deny_listed_file_is_refused_under_both_policies_in_both_spellings`. This issue's own
probe (`port/leaf-probe.patch`) was NOT re-run against the fix. Mutation results are in the peer's
issue file; this session did not repeat them.

## Workarounds

Do not leave symlinks to files outside the project, least of all to credential files, inside a
project root.

## Resume

Closed. Open residuals, all recorded in the peer's issue file: the classifier itself still answers
allowed for a leaf link to a denied file (the fix is at the resolvers), and the `edit_code` rename
wiring is not integration-tested. The librarian write sites listed under Evidence were not
re-measured by this session; the librarian symlink confinement branch that landed with the fix
carries its own tests, which ran green in the same filter above.

## References

- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
- `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`
- `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`
