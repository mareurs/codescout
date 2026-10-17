---
status: fixed
opened: 2026-10-17
closed: 2026-10-17
severity: high
owner: marius
related: [docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md, docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md, docs/issues/2026-10-17-a-write-directly-in-home-has-no-working-approve-write-remedy.md, docs/issues/2026-10-17-extra-write-roots-entry-is-not-validated-an-empty-one-admits-every-path.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'STANDING a TMPDIR that is or sits under a symlink is still followed to its target (F1a/b/c/f/g), by design: see Fix. On Windows only an unset or empty USERPROFILE is recovered, and that path was type-checked (cargo check --target x86_64-pc-windows-gnu --lib) but never run.'
---

# BUG: the write allowlist reads `$HOME` and `$TMPDIR` as given, so an unset, empty, relative or wrong `$HOME` and a relative or `..` `$TMPDIR` switch its guards off or point them at the wrong directory

## Summary

`src/util/path_security.rs` took two environment values on trust: `$HOME` (deny list, breadth
guards) and `$TMPDIR` (an implicit write root). A `$TMPDIR` of `.` or `a/../b` became a write root
at a directory the operator never named; an unset, empty or relative `$HOME` emptied the `~`
entries of the deny list and switched the `$HOME` breadth guard off; a `$HOME` that names some
other directory left the account's real home unprotected. Each was measured against the real
library (`classify_write_path`, `validate_approve_path`, `validate_read_path`) on `experiments`
27efb11d. The fixes are ported, one commit each, onto branch `fix/write-allowlist-port-2026-10-17`
from branch `worktree-agent-a40d6fa7287de0e17` (head 1f5443fd), rebuilt on `experiments`' own seams
(`WriteEnv`, `canonical_temp_write_root`, `temp_root_admits`, `is_overly_broad_write_root`,
`denied_paths_with`) with deny-wins-over-every-root kept.

## Class fit

Environment input trusted as if it were a validated path: the same trait as the temp-dir and
parent-only canonicalization bugs this file is related to, where a value compared against a
canonical target was never itself checked. It differs in that nothing here is a symlink: the
inputs are missing, empty or spelled in a way `fs::canonicalize` hides.

## Symptom (Effect)

Measured 2026-10-17 with `pathsec_probe`, a throwaway example linking the REAL library of the tree
under test (classification only, nothing written; driver and outputs in the session scratchpad
`port/`). `before` is `experiments` 27efb11d, `after` is the branch head with items 1 to 6.
`~` is the real home `/home/marius`; `FX` is a fixture dir. `[ALLOWED]` is
`classify_write_path` returning `Allowed`; `outside` is `OutsideRoot`; `denied` is `Denied`.

| scenario (env, cwd) | probe | before | after |
|---|---|---|---|
| F1d `TMPDIR=~/.cache/../.claude` | write `~/.claude/settings.json` | ALLOWED | outside |
| F5a `TMPDIR=.` cwd `/home` | write `/home/otheruser/x` | ALLOWED | outside |
| F5c `TMPDIR=.claude` cwd `~` | write `~/.claude/settings.json` | ALLOWED | outside |
| F2a `HOME` unset, cwd `~` | `approve_write(~)`, read `~/.ssh/id_probe`, write `~/.bashrc`, write `~/.ssh/authorized_keys` | Ok, Ok, ALLOWED, ALLOWED | Err, Err, outside, denied |
| F2b `HOME` unset, `TMPDIR=~` | same four | Ok, Ok, ALLOWED, ALLOWED | Err, Err, outside, denied |
| F2c `HOME=` (empty), cwd `~` | same four | Ok, Err, ALLOWED, denied | Err, Err, outside, denied |
| F2d `HOME=` (empty), `TMPDIR=~` | same four | Ok, Ok, ALLOWED, ALLOWED | Err, Err, outside, denied |
| F2e `HOME=/nonexistent/u`, `TMPDIR=~` | same four | Ok, Ok, ALLOWED, ALLOWED | Err, Err, outside, denied |
| F2f `HOME=/nonexistent/u`, cwd `~` | same four | Ok, Ok, ALLOWED, ALLOWED | Err, Err, outside, denied |
| F2g `HOME=marius` (relative), cwd `/home` | `approve_write(~)`, read, write `~/.bashrc` | Err, Err, outside | Err, Err, outside (unchanged: the relative value happened to resolve to `~` from cwd `/home`) |
| F2h `HOME` unset, cwd `/` | `approve_write(~)`, read `~/.ssh/id_probe`, write `~/.ssh/authorized_keys` | Ok, Ok, outside | Err, Err, denied |
| F1a `TMPDIR=link->/etc` | write `/etc/probe-x` | ALLOWED | ALLOWED (documented residual) |
| F1b/F1c `TMPDIR=link->~/.claude` / `~/.config` | write under the target | ALLOWED | ALLOWED (documented residual) |
| F1f/F1g `TMPDIR=link->/home` / `/home` | write `/home/otheruser/x` | ALLOWED | ALLOWED (documented residual) |

No row of THIS table moved towards ALLOWED or `Ok`. That is not the same as "nothing refuses less":
the review found a shape this table did not probe. With `HOME` a symlink to the real home,
`validate_read_path("<link>/.aws/credentials")` (the entry missing, so the target cannot be
resolved and keeps its spelling) was refused on `experiments` and allowed on the first version of
this branch, because the homes had been canonicalized and the literal spelling dropped. Fixed in a
follow-up commit: the deny list expands against the home as given AND its canonical form
(`protected_homes_of`); `a_missing_deny_entry_is_matched_under_the_symlink_spelling_of_the_home`
and `a_read_through_a_symlinked_home_of_a_missing_deny_entry_is_refused` were red first
(`Ok(".../link/.aws/credentials")`). Re-measured with the real binary after the fix (scenario C of
`port/mcp_r.py`).

## Reproduction

`PROBE_BIN=<probe built from the tree> FX_DIR=<scratch> run.sh` (session scratchpad `port/`),
scenarios as named above. The unit tests below reproduce the same shapes without the real home.

## Environment

Linux, rustc 1.97.1, codescout `experiments` 27efb11d, branch `fix/write-allowlist-port-2026-10-17`.

## Root cause

- `canonical_temp_write_root` handed `$TMPDIR` straight to `fs::canonicalize`, which resolves a
  relative path against the process cwd and collapses `..`.
- `platform::home_dir` returned `$HOME` as read, so an unset value was `None`, an empty one `""`
  and a relative one a path resolved against the cwd. `Path::starts_with("")` is true for every
  path, and `~/.ssh` expanded against `""` is the relative `.ssh`.
- `WriteEnv.home` and `validate_approve_path` knew exactly one home, so a `$HOME` naming another
  directory left the account's real home unprotected.

Measured 2026-10-17: the probe table above. `std::env::home_dir` was checked as an alternative and
is not enough: its 1.88.0 docs (this crate's MSRV) say `HOME` set to an empty string is returned
as `Some("")`; the 1.97.1 source (`library/std/src/sys/paths/unix.rs`) filters an empty `HOME` and
falls back to the passwd entry; a relative `HOME` is returned as given by both.

## Evidence

Before each change the red tests failed with these lines (full runs in the commit sequence):

```
a_relative_temp_dir_is_not_a_write_root:   left: Some("<worktree>")  right: None
a_temp_dir_spelled_with_dotdot_is_not_a_write_root:  left: Some("<tmp>/.tmpXXXX/sibling")  right: None
a_relative_temp_dir_admits_no_write_through_classification:  `TMPDIR=.` must add no root, got: Allowed(".../ce_relative_tmpdir_probe.txt")
a_dotdot_temp_dir_admits_no_write_through_classification:    a `..` TMPDIR must add no root, got: Allowed(".../sibling/x.txt")
an_{unset,empty}_home_still_protects_the_accounts_home / a_relative_home_...:
    reading "/home/marius/.ssh/id_probe" must be refused, got: Ok("/home/marius/.ssh/id_probe")
a_wrong_home_still_protects_the_accounts_home:
    reading "/home/marius/.ssh/id_probe" must be refused, got: Ok("/home/marius/.ssh/id_probe")
```

## Hypotheses tried

1. `std::env::home_dir` would do. Rejected, see Root cause.
2. Caching the whole resolved home (not just the account lookup) would be cheaper. Rejected: it
   serves a stale `$HOME` after the environment changes; `resolved_home_dir_is_not_stale_after_home_changes`
   fails against that mutant (m2i).

## Fix

Four commits are recorded here (patch-ids from `git show <sha> | git patch-id --stable`):

| item | commit | patch-id |
|---|---|---|
| 1 F5 + F1-`..` | 1347918e | 89f2eb8a5b1a44f76dd8c8704dada30a8b56e850 |
| 2 F2 unset/empty/relative `$HOME` | 999982c0 | 3b07988365379e51b2e633442b068587f2184163 |
| 3 F2 wrong `$HOME` | c955bb40 | 494899f1c07412d22b5dfbc53c9c3d03b87bebc6 |
| 7 serial cwd tests | 41cc27d7 | 8f69b74cd994e52308918adfc7be39f5df7900b4 |

(Items 4 and 5 are recorded in `2026-10-17-a-write-directly-in-home-has-no-working-approve-write-remedy.md`,
item 6 in `2026-10-17-extra-write-roots-entry-is-not-validated-an-empty-one-admits-every-path.md`.)

1. `canonical_temp_write_root` returns `None` for a path that is not absolute or that has a `..`
   component. Empty `$TMPDIR` was already refused (not absolute, and `canonicalize("")` fails);
   a test pins it.
2. `platform::{account_home_dir, resolve_home, resolved_home_dir}`; `path_security::home_dir`
   now reads `resolved_home_dir`. Unix `account_home_dir` is `getpwuid_r(geteuid())` with the
   `unsafe` narrowed to the `geteuid` call, the `getpwuid_r` call, the `zeroed` struct and the
   `CStr` read (a SAFETY comment on each). `lookup_account_home_with`, which takes the
   `getpwuid_r`-shaped closure and does the `CStr` read, is an `unsafe fn` with a `# Safety`
   section (review follow-up: as a safe fn it was sound only if every closure honoured the
   contract, a promise the type system did not carry). An EINTR retry bounded at 8, an ERANGE-grown buffer
   bounded at 1 MiB, and a `OnceLock` cache of the ACCOUNT lookup only: a definitive answer
   (found, or no entry) is kept, a failed call is not (a transient error must not switch the guard
   off for the life of the process). `$HOME` is read on every call, never cached.
   `geteuid` rather than `getuid`: file access is authorised by the effective uid, so that user's
   home is the one worth guarding. `std::env::home_dir` and `dirs-sys` use `getuid`; the two are
   equal for a process that is not setuid, so the choice never shows (mutant m2m survives, see
   below). Windows `account_home_dir` is `std::env::home_dir()` filtered to absolute: not
   deprecated on 1.88 and `USERPROFILE` then `GetUserProfileDirectoryW` since 1.85. It is NOT
   independent of the environment (a usable `USERPROFILE` wins), so a wrong `USERPROFILE` is not
   detected there; only an unset or empty one. Type-checked with
   `cargo check --target x86_64-pc-windows-gnu --lib --no-default-features`; not run.
   `PASSWD_BUF_MAX` carries `cap-class: NOT_A_CAP` (`tests/result_caps.rs` classifies every `_MAX`
   constant; it failed on this one until annotated). `tests/env_mutation_isolation.rs` bans
   `set_var` outside an allowlist; `resolved_home_dir_is_not_stale_after_home_changes` needs one
   (its subject is a `$HOME` that changes), so `src/platform/mod.rs` was ADDED to that allowlist
   with the reason (the mutation runs only in a child process). That is a repo-policy decision for
   the owner to accept or refuse; the alternative is dropping that one test.
3. `WriteEnv.home: Option<PathBuf>` became `WriteEnv.homes: Vec<PathBuf>` (`$HOME` resolved plus the
   account home, the home as given AND its canonical form (through the existing ancestor), deduplicated: `protected_homes_of`).
   `denied_paths_with` expands a `~` entry against every home (`expand_home_all`);
   `is_overly_broad_write_root`, `temp_root_admits` and `validate_approve_path` check every home;
   `validate_read_path` reads the same list (the read deny list is `denied_read_paths`).
   The field was renamed `home` -> `homes`; the test helper `env_of(temp, Some(home), cwd)` keeps its
   signature and `env_with_homes` injects several. No other reader of the old field exists.
7. `new_with_relative_path_canonicalizes_home_root` runs under `#[serial_test::serial]` with a
   `CwdGuard` whose `Drop` restores the cwd, panic or not. The 25 `util::path_security` tests that
   read the cwd through the ambient classifier or `std::env::current_dir()` are `#[serial]` too.

Documented residual, NOT closed, items 1: a `$TMPDIR` that is or sits under a SYMLINK is followed
to its target (F1a `TMPDIR=link -> /etc` admits `/etc`; F1b/F1c link to `~/.claude`/`~/.config`;
F1f/F1g `TMPDIR=/home` admits `/home/otheruser`). A macOS `/var -> /private/var` link is
indistinguishable from an attacker's link, and whoever controls `$TMPDIR` can already name the
directory literally; `temp_root_admits` also keeps a temp root above `$HOME` on purpose (sandboxes
whose `$HOME` is under `$TMPDIR`). A design decision; none proposed here.

Trade-offs recorded for the owner:
- `#[serial]` excludes only tests that opt in (`docs/conventions/test-env-isolation.md`). Tests
  elsewhere that reach `WriteEnv::ambient()` through a tool are not covered. The cwd cannot be
  injected for the agent test (resolving a relative `--project` against it is the subject).
- The race was NOT reproduced before the change: the filtered pair passed 30 of 30 runs on the
  unmodified tree as well, so the serial attribute is protection by construction, not a measured fix.
- `cwd=/etc` (or any non-broad cwd) is still an implicit write root (F9b2: `/etc/probe-x` ALLOWED
  both before and after). By design, unchanged.

## Tests added

`src/util/path_security.rs`: `a_relative_temp_dir_is_not_a_write_root`,
`a_temp_dir_spelled_with_dotdot_is_not_a_write_root`, `an_empty_temp_dir_is_not_a_write_root` and
`an_empty_temp_dir_admits_no_write_through_classification` (both green before: pinned),
`a_dotdot_temp_dir_admits_no_write_through_classification`,
`a_relative_temp_dir_admits_no_write_through_classification`,
`an_unset_home_still_protects_the_accounts_home`, `an_empty_home_...`, `a_relative_home_...`,
`a_wrong_home_still_protects_the_accounts_home` (each runs in a child process with its own `HOME`,
`platform::test_child`), `protected_homes_are_the_resolved_home_and_the_accounts_deduplicated`,
`two_spellings_of_one_home_are_both_kept_but_collapse_to_one_canonical_home`, `tilde_patterns_expand_against_every_home`,
`the_write_deny_list_covers_every_protected_home`,
`the_write_deny_list_covers_a_symlinked_entry_of_every_protected_home`,
`a_temp_root_admits_nothing_inside_any_protected_home`, `a_cwd_above_any_protected_home_adds_no_root`.
`src/platform/mod.rs`: `resolve_home_falls_back_to_the_account_for_an_unusable_home`,
`resolved_home_dir_is_not_stale_after_home_changes` (child process: changes its own `HOME` after
the account cache is filled). `src/platform/unix.rs`:
`account_home_dir_matches_the_passwd_entry_for_this_user`, the EINTR, ERANGE, no-entry and cache
tests (`a_lookup_interrupted_by_a_signal_is_retried` and 7 more in `platform::unix::tests`).
`src/agent/mod.rs`: `the_cwd_guard_restores_the_cwd_when_the_guarded_code_panics`.

### Mutation results (one guard per mutant, 2026-10-17)

Each mutant was applied to a clean checkout of the commit that introduced the guard, the stated
test set run (`cargo test --lib -- util::path_security platform::` plus `config::project`,
`agent::` where named), and the file restored byte for byte. A script under the session scratchpad,
not `scripts/mutation-probe.sh`: the branch worktree is private, so that script's isolation buys
nothing and its cold isolated build does.

| item | mutant | killed by |
|---|---|---|
| 1 | m1a `is_absolute` rule off | `a_relative_temp_dir_is_not_a_write_root`, `..admits_no_write..` (2 tests): KILLED |
| 1 | m1b `..` rule off | `a_temp_dir_spelled_with_dotdot_is_not_a_write_root`, `a_dotdot_temp_dir_admits_...`: KILLED |
| 2 | m2a `home_dir()` back to `$HOME` only | the 3 `*_home_still_protects_the_accounts_home` tests: KILLED |
| 2 | m2b no absolute filter on `$HOME` | `resolve_home_falls_back_...`, `resolved_home_dir_is_not_stale_...`, 2 child tests: KILLED |
| 2 | m2c no absolute filter on the result | `resolve_home_falls_back_...`: KILLED |
| 2 | m2d cache never read | `a_found_answer_is_looked_up_once_and_kept`, `a_definitive_no_entry_is_kept_too`: KILLED |
| 2 | m2e a failed lookup is cached | `a_failed_lookup_is_not_kept_so_the_next_call_asks_again`: KILLED |
| 2 | m2n no-entry is not cached | `a_definitive_no_entry_is_kept_too`: KILLED |
| 2 | m2f EINTR not retried | `a_lookup_interrupted_by_a_signal_is_retried`, `..gives_up_after_the_retry_bound`: KILLED |
| 2 | m2g EINTR bound `<` -> `<=` | `a_lookup_that_is_always_interrupted_gives_up_after_the_retry_bound`: KILLED |
| 2 | m2h ERANGE bound `<` -> `<=` | `a_buffer_that_never_suffices_stops_growing_at_the_bound`: KILLED |
| 2 | m2p ERANGE growth step | `a_too_small_buffer_is_grown_and_the_lookup_retried`: KILLED |
| 2 | m2i the resolved home is cached whole | `resolved_home_dir_is_not_stale_after_home_changes`: KILLED |
| 2 | m2j `pw_dir` trusted without `result` | `every_shape_of_no_usable_entry_is_reported_as_none`: KILLED |
| 2 | m2k ENOENT/ESRCH arm removed | `every_shape_of_no_usable_entry_is_reported_as_none`: KILLED |
| 2 | m2l relative `pw_dir` accepted | `every_shape_of_no_usable_entry_is_reported_as_none`: KILLED |
| 2 | m2m `geteuid` -> `getuid` | none: SURVIVED. Equivalent in any process whose real and effective uid are equal; no test can tell them apart without a setuid binary. Recorded, not killed. |
| 3 | m3a account home left out | `protected_homes_are_the_resolved_home_and_the_accounts_deduplicated`: KILLED |
| 3 | m3b no dedupe | same test, `two_spellings_of_one_home_are_both_kept_but_collapse_to_one_canonical_home`: KILLED |
| 3 | m3c homes not canonicalized | `two_spellings_of_one_home_are_both_kept_but_collapse_to_one_canonical_home`: KILLED (mutant measured on the version before the follow-up) |
| 3 | m3d `~/` expanded for the first home only | `tilde_patterns_expand_against_every_home` and 2 more: KILLED |
| 3 | m3e bare `~` first home only | `tilde_patterns_expand_against_every_home`: KILLED |
| 3 | m3f1 resolved-form deny list built from the first home only | `the_write_deny_list_covers_a_symlinked_entry_of_every_protected_home`: KILLED |
| 3 | m3f2 leaf-form deny list built from the first home only | same test: KILLED |
| 3 | m3g temp root checks the first home only | `a_temp_root_admits_nothing_inside_any_protected_home`: KILLED |
| 3 | m3h cwd breadth checks the first home only | `a_cwd_above_any_protected_home_adds_no_root`: KILLED |
| 3 | m3i `approve_write` checks the first home only | `a_wrong_home_still_protects_the_accounts_home` (child test): KILLED |
| 3 | m3j ambient env has no homes | 6 tests: the 4 `*_home_still_protects_..` child tests, `write_to_ssh_denied_even_if_under_project`, `validate_write_path_session_root_still_respects_deny_list`: KILLED |
| 3 | m3k read deny list uses the resolved home only | `a_wrong_home_still_protects_the_accounts_home` (child test): KILLED |
| 3 | m3l classifier homes not canonicalized | 3 symlinked-home tests: KILLED |
| 7 | m7a `CwdGuard::drop` does not restore | `the_cwd_guard_restores_the_cwd_when_the_guarded_code_panics`: KILLED |
| 7 | m7b `CwdGuard::change_to` does not move | same test (fixture assertion): KILLED |
| 7 | `#[serial]` attributes removed | not mutation-testable: a scheduling property with no deterministic observable |

Two shapes were predicted to survive before they were run and got their test first, so they were
never observed surviving: m3f1/m3f2 (the two deny-list forms are unioned, so a plain home is
covered by either; only a symlinked directory entry needs the resolved form and only a symlinked
file entry the leaf form) and m4c in the other issue file.

## Workarounds

Do not export a relative or `..` `$TMPDIR`; keep `$HOME` an absolute path.

## Resume

Nothing open here except the documented symlinked-`$TMPDIR` residual and the Windows caveats in
`unverified`. A reviewer who wants the symlink case closed needs a rule that tells an attacker's
`TMPDIR` link from macOS's own `/var` link; that is a design decision.

## References

- `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`
- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
- `docs/issues/2026-10-17-a-write-directly-in-home-has-no-working-approve-write-remedy.md`
- `docs/issues/2026-10-17-extra-write-roots-entry-is-not-validated-an-empty-one-admits-every-path.md`
