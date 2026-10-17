---
status: fixed
opened: 2026-10-17
closed: 2026-10-17
severity: high
owner: marius
related: [docs/issues/2026-10-17-a-write-directly-in-home-has-no-working-approve-write-remedy.md, docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md, docs/issues/2026-10-17-extra-write-roots-entry-is-not-validated-an-empty-one-admits-every-path.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: `approve_write` accepts a path with an unresolved `..` (and a file, and a name that does not exist yet) that ends up opening the home to writes

## Summary

The rule "approval refuses the home and every directory above it" could be walked around three ways,
all measured with the real binary (`codescout start --project <p>`, decoy `HOME`, scrubbed
environment) on branch `fix/write-allowlist-port-2026-10-17` before the follow-up commit; the first
also exists on `experiments`, where the approval has no home guard at all beyond equality:

1. `approve_write("zz/../../..")` while `zz` does not exist. `best_effort_canonicalize` keeps a
   literal `..` when a component is missing, and `too_broad` compares components, so the spelling
   never looked like an ancestor of the home. The stored session root was re-resolved on EVERY
   call, so after an ordinary `create_file("zz/keep.txt")` it collapsed onto the home itself and
   `edit_file(<home>/.bashrc, insert=append)` wrote it.
2. `approve_write(<home>/.bashrc)` (an existing FILE) and `approve_write(<home>/.newrc)` (a name
   that does not exist yet): the file became a session root, and `starts_with` admits the root
   itself, so `<home>/.bashrc` was writable and `<home>/.newrc` creatable.
3. An ABSOLUTE `extra_write_roots` entry spelled with `..`
   (`["<home>/work/../"]`) was accepted at load and made `<home>/.bashrc` writable.

All three falsified two statements written earlier on this branch: the denial text "so no approval
can make this write possible" and the sentence in
`2026-10-17-a-write-directly-in-home-has-no-working-approve-write-remedy.md` that no approval can
make a file directly in a home writable. Both are true now (measured below). Fixed and verified.

## Class fit

A containment check that compares the SPELLING of a path before it is fully resolved, and a stored
root that is resolved again later: the same shape as the temp-dir and parent-only canonicalization
issues this file is related to. It differs in that the unresolved part is a `..` through a directory
the caller can create afterwards, so the check and the use see different paths.

## Symptom (Effect)

Real binary, driver `port/mcp_r.py` in the session scratchpad, `home/work/proj` the project, `FX`
the fixture dir. Before (branch head `8822f616`):

```
E1 approve_write({"path": "zz/../../.."}) -> {"approved": "FX/E/home/work/proj/zz/../../..", "scope": "this session only"}
E2 create_file({"path": "zz/keep.txt", ...}) -> {"status": "ok", "rel_path": "zz/keep.txt"}
E3 edit_file({"path": "FX/E/home/.bashrc", "insert": "append", ...}) -> {"status": "ok", ...}
  decoy .bashrc: '# decoy bashrc\necho PWNED-E\n'
E4 approve_write({"path": "FX/E/home"}) -> {"ok": false, "error": "approve_write: ... is too broad ..."}
E5a approve_write(FX/E5/home/.bashrc) -> {"approved": "FX/E5/home/.bashrc", ...}
E5b edit_file(FX/E5/home/.bashrc, append) -> ok;  decoy .bashrc: '# decoy bashrc\necho PWNED-E5\n'
E5c approve_write(FX/E5/home/.newrc) -> approved;  E5d create_file(.newrc) -> ok;  .newrc exists: True
X1 edit_file(FX/X/home/.bashrc, append) with extra_write_roots = ["FX/X/home/work/../"] -> ok;  decoy .bashrc: '# decoy bashrc\necho PWNED-X\n'
```

After (the fix, same driver, `port/mcp-r1.txt`):

```
E1 approve_write({"path": "zz/../../.."}) -> {"ok": false, "error": "approve_write: 'zz/../../..' contains '..' that could not be resolved (a directory in it does not exist yet); pass a path with no unresolved '..' segments"}
E2 create_file(zz/keep.txt) -> ok
E3 edit_file(FX/E/home/.bashrc, append) -> {"ok": false, "error": "approve_write: 'FX/E/home' is too broad ..."}   decoy .bashrc: '# decoy bashrc\n'
E5a approve_write(FX/E5/home/.bashrc) -> {"ok": false, "error": "approve_write: '.../.bashrc' is not a directory; approve the directory to write into"}
E5b edit_file(.bashrc, append) -> refused;  decoy .bashrc: '# decoy bashrc\n'
E5c approve_write(.../.newrc) -> approved (it names a directory that does not exist yet);  E5d create_file(.newrc) -> refused;  .newrc exists: False
X: server up: False (the load refuses the entry);  decoy .bashrc: '# decoy bashrc\n'
```

(The E3/E5b/E5d text is the ack path's own denial at the time of that run; the follow-up commit in
this series routes it through the same wording as the other write path.)

## Reproduction

`python3 mcp_r.py <codescout binary> <workdir>`, scenarios E, E5 and X (session scratchpad `port/`).
In-process, the same flows are the tests `approve_with_an_unresolved_dotdot_cannot_open_the_home_to_writes`
and `approving_a_file_directly_in_the_home_cannot_make_it_writable` (`src/tools/edit_file/tests.rs`),
which drive the real `ApproveWrite`, `CreateFile` and `EditFile` tools in a child process whose
`HOME` is a decoy.

## Environment

Linux, rustc 1.97.1, branch `fix/write-allowlist-port-2026-10-17` cut from `experiments` 27efb11d.

## Root cause

- `validate_approve_path_in` took `best_effort_canonicalize(path)` as the resolved path. That keeps
  a `..` whenever any component is missing, and `too_broad` (component-wise `starts_with`) cannot see
  through it.
- `classify_write_path_in` re-resolved each stored session root on every call, so a root that was
  harmless when approved could resolve onto the home later (a `..` whose directory was then
  created, or a directory replaced by a link to the home). Nothing re-applied the approval's own
  rules at use time.
- A session root was treated as "a path under which writes are allowed", and `starts_with` includes
  the root itself, so a root that named a FILE directly in the home admitted that file.
- `validate_list_limits` refused relative and empty `extra_write_roots` entries but not an absolute
  one with a `..`.

Measured 2026-10-17: the runs above.

## Evidence

Red before the fix (in-process, child process with decoy `HOME`):

```
approve_with_an_unresolved_dotdot_cannot_open_the_home_to_writes:
  the decoy ~/.bashrc was written. approve_write -> Ok({"approved": ".../home/work/proj/zz/../../..", ...}); edit_file -> Ok("ok")
approving_a_file_directly_in_the_home_cannot_make_it_writable:
  ~/.bashrc was written. approve(file) -> Ok({"approved": ".../home/.bashrc", ...}); edit_file -> Ok("ok")
approve_refuses_an_unresolved_dotdot:  got: Ok(".../zz/../a")
approve_refuses_an_existing_file:      got: Ok(".../home/.bashrc")
a_session_root_spelled_with_dotdot_adds_no_root / a_session_root_that_became_a_link_to_the_home_adds_no_root: got: Allowed(".../home/.bashrc")
a_session_root_never_admits_a_file_that_is_the_root_itself:  got: Allowed(".../home/.newrc")
an_absolute_extra_write_root_with_dotdot_admits_nothing:     got: Allowed(".../elsewhere.txt")
validate_list_limits_rejects_an_absolute_extra_write_root_with_dotdot:  a `..` entry must be refused: ()
```

## Hypotheses tried

1. Refuse `..` at approval only. Not enough on its own: a link swapped in after approval resolves a
   previously fine root onto the home. The classifier therefore also drops a session root that
   resolves onto a filesystem root, a home or a directory above one at USE time.

## Fix

Commit `fix(path-security): approve_write cannot open the home through a .. or a file, and roots are
re-checked per call`:

- `validate_approve_path_in` refuses a resolved path that still has a `..` (naming it) and an
  existing non-directory.
- `classify_write_path_in`: a session root spelled with `..`, or that now resolves onto a filesystem
  root, a protected home or a directory above one (`too_broad`), is skipped; a session root admits
  only what is INSIDE it, never a file named like it; an `extra_write_roots` entry with `..` is
  skipped.
- `ProjectConfig::validate_list_limits` refuses an `extra_write_roots` entry with a `..`.

Only narrows: no path that was refused before is admitted now.

## Tests added

`src/util/path_security.rs`: `approve_refuses_an_unresolved_dotdot`, `approve_refuses_an_existing_file`,
`a_session_root_spelled_with_dotdot_adds_no_root`,
`a_session_root_spelled_with_dotdot_adds_no_root_even_when_it_is_narrow`,
`a_session_root_that_became_a_link_to_the_home_adds_no_root`,
`a_session_root_never_admits_a_file_that_is_the_root_itself`,
`an_absolute_extra_write_root_with_dotdot_admits_nothing`. `src/config/project.rs`:
`validate_list_limits_rejects_an_absolute_extra_write_root_with_dotdot`. `src/tools/edit_file/tests.rs`:
the two real-tool flows above.

### Mutation results (one guard per mutant, 2026-10-17)

Same method as in `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`, tests
`util::path_security`, `config::project`, `tools::edit_file` (and `tools::core`).

| mutant | killed by |
|---|---|
| mA1 approve allows an unresolved `..` | `approve_refuses_an_unresolved_dotdot`: KILLED |
| mA2 approve allows an existing file | `approve_refuses_an_existing_file`: KILLED |
| mA3 session root with `..` kept | first run SURVIVED (the broad-root filter caught the test's shape); added `a_session_root_spelled_with_dotdot_adds_no_root_even_when_it_is_narrow`; re-run: KILLED |
| mA4 session root that resolves onto a home kept | `a_session_root_that_became_a_link_to_the_home_adds_no_root`: KILLED |
| mA5 a session root admits the root itself | `a_session_root_never_admits_a_file_that_is_the_root_itself`, `approving_a_file_directly_in_the_home_cannot_make_it_writable` (real tools): KILLED |
| mA6 extra root with `..` kept | `an_absolute_extra_write_root_with_dotdot_admits_nothing`: KILLED |
| mA7 config accepts a `..` entry | `validate_list_limits_rejects_an_absolute_extra_write_root_with_dotdot`: KILLED |

## Workarounds

Do not approve paths you cannot resolve; keep the home out of every write root.

## Resume

Nothing open. The related issue stays open for a different question (file-scoped approval).

## References

- `docs/issues/2026-10-17-a-write-directly-in-home-has-no-working-approve-write-remedy.md`
- `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`
- `docs/issues/2026-10-17-extra-write-roots-entry-is-not-validated-an-empty-one-admits-every-path.md`
