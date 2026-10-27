---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: 2026-10-27
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md
- docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md
severity: low
---

# BUG: a write to a file directly in `$HOME` (or `/`) has no working `approve_write` remedy, and `approve_write` accepted a directory that contains the home

## Summary

When a write is `OutsideRoot`, both the denial text and the pending-ack flow told the caller to
`approve_write` the directory the file sits in. `validate_approve_path` refused `/` and `$HOME`
by equality, so for a file directly in either the named remedy was certain to be refused, and for
a file directly in a directory ABOVE the home (`/home`) the call was accepted and then admitted
`~/.bashrc` and every other file in the home that the deny list does not name.

Status, narrowed after two independent reviews of branch `fix/write-allowlist-port-2026-10-17`
(follow-up commits on the same branch):

- The breadth rule (item 4) is the security half. It held on the ack path from the start: base
  minted an `@ack` for a directory containing the home, the port refuses.
- The denial text (item 5) reached only `validate_write_path` (so `edit_code`), not the ack path
  that `create_file`, `edit_file` and `edit_markdown` use (`resolve_write_or_capture`), which kept
  returning `approve_write`'s own refusal (`approve_write: '<home>' is too broad ... Approve a
  subdirectory that does not contain it.`). Both paths now route through one function
  (`too_broad_directory_denial`); measured with the real binary.
- "No approval can make a file directly in a home, in a directory above one, or in `/` writable"
  was NOT true when first written. Three bypasses of it existed and are closed on the branch, see
  `2026-10-17-approve-write-accepts-an-unresolved-dotdot-that-collapses-onto-the-home.md`:
  a `..` through a directory that does not exist yet, approving the FILE itself (or a name that
  does not exist yet), and an absolute `extra_write_roots` entry spelled with `..`. The statement
  holds for `approve_write` now; `extra_write_roots` remains the operator's own explicit choice.

Still open, a design question: whether a file-scoped approval for a file directly in a home should
exist.

## Class fit

No existing class names a remedy that the next step of the same message refuses; this is closest
to the write-denial wording problems already archived, but there the named call could be run. The
second half (a breadth guard that compared by equality) is the same shape as the cwd-root guard
fixed in the related issue. Filed unclassified; a second guard whose message names a refused
call would be the reason to revisit.

## Symptom (Effect)

Before, for a file directly in `/home/marius` or in `/`:

```
write denied: '/home/marius/.bashrc' is outside the project root. Call approve_write(path="/home/marius") first to grant write access for this session.
approve_write: '/home/marius' is too broad — specify a subdirectory
```

and `approve_write(path="/home")` returned `Ok("/home")`.

## Reproduction

Measured 2026-10-17 with `pathsec_probe` against the real library (`validate_approve_path`,
`validate_write_path`), real home `/home/marius`, cwd under a fixture dir. `before` is `experiments`
27efb11d, `after` is the branch head. `FX` is a fixture dir.

| call | before | after |
|---|---|---|
| F8a `approve_write("/home")` | `Ok("/home")` | `Err("... '/home' is too broad — it is, or contains, the home directory '/home/marius' ...")` |
| F8a `approve_write(FX/homeparent)` (symlink to `/home`) | `Ok("/home")` | `Err` (same text, `/home`) |
| F8a `approve_write("/home/marius")` | `Err("... too broad — specify a subdirectory")` | `Err("... is, or contains, the home directory ...")` |
| F8a `approve_write("/")` | `Err(too broad)` | `Err(too broad)` |
| F8c `HOME=FX/base/a/b/home`, `approve_write(FX/base/a)` | `Ok` | `Err` (contains the home) |
| F8c same, `approve_write(FX/base/a/other)` | `Ok` | `Ok` (control: does not contain the home) |
| F8d `HOME=/tmp/h`, `approve_write("/tmp")` | `Ok("/tmp")` | `Err` (trade-off, below) |
| F8a denial for write `/home/marius/.bashrc` | `... Call approve_write(path="/home/marius") first ...` | `... is directly in the home directory '/home/marius', outside the project root. approve_write refuses a home directory as too broad, so no approval can make this write possible; only a file inside a subdirectory of the home can be approved.` |
| F8a denial for write `/probe-x` | `... Call approve_write(path="/") first ...` | `... is directly in the filesystem root, outside the project root. approve_write refuses a filesystem root as too broad, so no approval can make this write possible; only a file inside a subdirectory of it can be approved.` |
| F8c denial for write `/home/marius/.bashrc` with `HOME=FX/base/a/b/home` | `... Call approve_write(path="/home/marius") ...` | `... is directly in the home directory '/home/marius' ...` (the account home is named, not the wrong one it contains) |

No row moved towards `Ok`/ALLOWED.

## Environment

Linux, rustc 1.97.1, codescout `experiments` 27efb11d, branch `fix/write-allowlist-port-2026-10-17`.

## Root cause

`validate_approve_path` tested `resolved == "/"` and `canonical($HOME) == resolved`, so only the home
itself was refused, never a directory containing it. `validate_write_path` derived the directory to
approve from `resolved.parent()` without asking whether that directory was approvable.
Measured 2026-10-17: the table above.

## Evidence

The red tests before the change:

```
validate_approve_path_rejects_every_ancestor_of_the_home_directory:
    "/home" contains the home directory "/home/marius" and must be refused, got: Ok("/home")
a_denial_does_not_name_an_approve_write_call_that_is_certain_to_be_refused:
    that call is refused for `/`: write denied: '/ce_probe_x.rs' is outside the project root. Call approve_write(path="/") first to grant write access for this session.
```

## Hypotheses tried

1. Reuse `validate_approve_path` wholesale as the pre-check in `validate_write_path`, as the first
   attempt on branch `worktree-agent-a40d6fa7287de0e17` did. Rejected: it also covers the deny-list
   refusal, which cannot be reached from an `OutsideRoot` decision (a directory the approval
   refuses for the deny list lies under a deny entry, and the classifier checks the same entries,
   in two resolved forms, before it can answer `OutsideRoot`). A branch for it was written, and
   its mutant (m5b) survived, so it was removed. Read from the two code paths and exercised by
   the `deny_list_holds_*` tests, not proven for every filesystem shape.

## Fix

Two commits on the branch:

| item | commit | patch-id |
|---|---|---|
| 4 F8a | abbd3ec8 | 2377c2b1ab8c0357a41f98aa4dbe5bead9f83060 |
| 5 F8b | 1e6827b8 | f842f5fa40fd3ac046c5be6f88ae7f7815d8a5aa |

- Item 4: `validate_approve_path_in(raw, project_root, homes)` refuses a filesystem root
  (`parent().is_none()`, which also catches the empty path and a Windows drive root) and any
  directory that is, or contains, a protected home (`too_broad`, comparing canonical forms through
  the existing ancestor on both sides).
- Item 5: `validate_write_path_in(.., &WriteEnv)` asks `too_broad` for the directory of an
  `OutsideRoot` target and, when it says yes, returns a denial that names why and what is true of
  THIS file instead of an `approve_write` call. "Write into a subdirectory of it instead" is not
  said for a file directly in the home (`~/.bashrc` IS the file); a file in an approvable
  directory still gets the callable `approve_write(path=...)`.

Trade-off, for the owner: with `HOME=/tmp/h` (common in container images), `approve_write("/tmp")`
is now refused, and so is approving a project that contains the home. Such a host approves a
subdirectory that does not contain the home. Still open as a design question, not work: whether a
file-scoped approval for a file directly in a home should exist. `validate_approve_path` must not be
widened to allow it.

## Fix provenance

- **SHA:** `de43b4553` (`experiments`)
- **patch-id:** `b7f59042aa844d595b2e739a1f87b4cd19d05adb`

## Resolution 2026-10-27

The open design question (should a file-scoped `approve_write` exist for a file directly in a home?) is decided: **no**. The operator asked for `~/.bashrc`, `~/.vimrc` and folders in `$HOME`. The route for the files is the operator's own config, not an approval the agent can grant to itself: those files run in later shells, so consent for them must come from the operator. `validate_approve_path` stays as it is.

- **Files:** list the file's absolute path in `security.extra_write_roots` in `.codescout/project.toml`. Verified on the classifier with a fake home: the entry admits exactly that file, whether it exists or not, and the deny list still wins (an entry naming `~/.ssh/id_rsa` or `~/.ssh` does not unlock them). Only the operator can add the entry, because `config::protected_files` refuses a write that changes the security table.
- **Denial text:** the too-broad denial now ends with that route, so it is no longer a dead end.
- **Folders:** a file in a NEW folder directly in the home needs no entry. The folder is a subdirectory of the home, so `approve_write` accepts it even though it does not exist yet. Pinned by a test. An empty folder is not a write; `run_command` makes it.
- **Known, documented, not changed:** a root is a path prefix, so a path below a listed entry is admitted too. For an existing file that write fails; for a listed name that does not exist yet, a directory of that name could be created. The cost is that one dotfile.
- **Not verified end to end:** the classifier tests do not run the real server, and the entry's absolute path must be written out (a leading `~` is not expanded). The operator must add the entries to each project that needs them; the global config cannot set this key.

## Tests added

`src/util/path_security.rs`: `validate_approve_path_rejects_every_ancestor_of_the_home_directory`
(ambient; red before), `approve_refuses_a_directory_that_contains_a_home_and_accepts_one_that_does_not`,
`approve_refuses_a_directory_above_either_of_two_homes`,
`approve_refuses_the_filesystem_root_with_no_home_at_all`,
`approve_sees_a_missing_home_behind_a_symlinked_ancestor`,
`approve_sees_a_missing_directory_behind_a_symlink_that_contains_the_home`,
`approve_still_refuses_a_deny_listed_directory_under_an_injected_home`,
`a_denial_does_not_name_an_approve_write_call_that_is_certain_to_be_refused` (ambient; red before),
`a_denial_says_what_is_true_for_each_directory_approve_write_refuses`,
`a_denial_names_the_home_the_file_is_directly_in_when_there_are_two`.

### Mutation results (one guard per mutant, 2026-10-17)

Same method as in `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`.

| item | mutant | killed by |
|---|---|---|
| 4 | m4a filesystem-root guard off | `approve_refuses_the_filesystem_root_with_no_home_at_all`: KILLED |
| 4 | m4b equality with a home only (no ancestor test) | the contains-a-home tests: KILLED |
| 4 | m4c breadth form not resolved through the existing ancestor | `approve_sees_a_missing_directory_behind_a_symlink_that_contains_the_home`: KILLED |
| 4 | m4d homes not canonicalized in the approval | `approve_sees_a_missing_home_behind_a_symlinked_ancestor`: KILLED |
| 4 | m4e deny list ignores the injected homes | `approve_still_refuses_a_deny_listed_directory_under_an_injected_home`: KILLED |
| 4 | m4g the public function passes no homes | the ambient ancestor test: KILLED |
| 5 | m5a the `too_broad` pre-check off | `a_denial_says_what_is_true_for_each_directory_approve_write_refuses`: KILLED |
| 5 | m5b the generic `validate_approve_path_in` pre-check off | none: SURVIVED. Unreachable branch, see Hypotheses; removed. |
| 5 | m5c the "directly in the home" arm off | `a_denial_says_what_is_true_...`: KILLED |
| 5 | m5d an equal home not preferred over a contained one | `a_denial_names_the_home_the_file_is_directly_in_when_there_are_two`: KILLED |
| 5 | m5g1 root guard in `too_broad` off | `approve_refuses_the_filesystem_root_with_no_home_at_all`: KILLED |
| 5 | m5g2 contained-home test off in `too_broad` | the contains-a-home tests: KILLED |
| 5 | m5g3 `too_broad` breadth form not resolved | `approve_sees_a_missing_directory_behind_a_symlink_that_contains_the_home`: KILLED |

The m5 mutants other than m5b were re-run on the squashed commit after the unreachable code branch
was removed; the first run (before the removal) gave the same verdicts.

Review follow-up (the ack path, `resolve_write_or_capture`), 2026-10-17:

| mutant | killed by |
|---|---|
| mB1 the ack path skips `too_broad_directory_denial` | `the_ack_path_says_what_is_true_for_a_file_in_a_directory_approve_write_refuses` (real `create_file` / `edit_file`): KILLED |
| mB2 `too_broad_directory_denial_in` returns none | the same test and `a_denial_does_not_name_an_approve_write_call_that_is_certain_to_be_refused`: KILLED |

Before the follow-up the ack test was red: `create_file(<home>/.bashrc2)` returned `approve_write:
'<home>' is too broad — it is, or contains, the home directory '<home>'. Approve a subdirectory that
does not contain it.` and `create_file("/probe-review-x")` returned `approve_write: '/' is too broad
— specify a subdirectory`. m4c was predicted to survive the first
symlink test and got its own test before being run, so it was not observed surviving.

Added 2026-10-27 in `src/util/path_security.rs`:

- `the_too_broad_denial_names_the_operators_route` (seen failing before the fix)
- `an_extra_write_root_naming_one_file_in_the_home_admits_that_file_alone` (its first draft also asserted that a path below a listed file is refused; that failed and showed the prefix behaviour, now pinned as known)
- `a_file_entry_in_the_home_never_unlocks_the_deny_list` and `a_file_in_a_new_folder_directly_in_the_home_can_be_approved` (characterization tests, passed on first run)

## Workarounds

Write into a subdirectory (with `$TMPDIR` a directory that is not `$HOME` or an ancestor of it).

## Resume

N/A — resolved. See Resolution 2026-10-27.

## References

- `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`
- `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`
