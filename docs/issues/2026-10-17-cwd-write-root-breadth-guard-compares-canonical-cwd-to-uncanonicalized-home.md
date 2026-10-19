---
status: fixed
opened: 2026-10-17
closed: 2026-10-17
severity: low
owner: marius
related: [docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md]
tags:
- cluster/unclassified
kind: bug
unverified: "Reproduced and fixed at unit level with injected homes and cwd (util::path_security; clippy clean). Not run end to end with a server launched from a real symlinked $HOME; full gate not run. The reviewer's probe of a copy of the logic (not the real function) showed the neighbouring cases."
---

# BUG: the cwd write-root breadth guard compares a canonical cwd to an un-canonicalized `$HOME`, so a symlinked `$HOME` is not seen as broad

## Summary

In `classify_write_path_with_temp_dir` (since renamed `classify_write_path_with_env`, `src/util/path_security.rs`) the cwd is added to the write
allowlist unless it is too broad:

```rust
let cwd_canon = best_effort_canonicalize(&cwd);
let is_broad = cwd_canon == Path::new("/") || home_dir().is_some_and(|h| cwd_canon == h);
```

`cwd_canon` is canonical but `home_dir()` is read raw from `$HOME`. When `$HOME` is, or sits under,
a symlink (`/home/u -> /data/u`), a server launched from `$HOME` has `cwd_canon == /data/u` while
`h == /home/u`, so the guard does not fire and the whole home directory becomes a write root
without a prompt.

## Class fit

No existing class names a comparison of a canonical path against a value held in its literal form;
this is the same shape as the temp-dir bug it was found beside. Filed unclassified, and a second
sighting of the shape (the first is the related issue) is the reason to revisit that.

## Symptom (Effect)

With `$HOME` behind a symlink and the MCP server launched from `$HOME`, a write to `~/.bashrc`,
`~/.profile`, `~/.claude/settings.json` or `~/.config/codescout/.env` is allowed without an
`approve_write`. None of those is on the deny-list. Fails open, but only for that host layout.

## Reproduction

Reproduced 2026-10-17 as a unit test, with the home passed in (the process-wide `$HOME` cannot be
varied from a parallel test). The cwd guard was first extracted, behaviour unchanged, into
`cwd_allow_root(cwd, home)`; with `cwd = <canonical dir>` and `home = <symlink to it>` it returned
`Some("/home/marius/.cache/cc-tmp/.tmpYpRVHR/data/u")` where `None` is correct. Not run end to end
with a server launched from a real symlinked `$HOME`.

## Environment

Linux, codescout `experiments` at 299395e5 plus the temp-dir fix.

## Root cause

The comparison above. `validate_approve_path` does it correctly
(`best_effort_canonicalize(&h) == resolved`) and so does `temp_allow_root`, which the temp-dir fix
added.

## Evidence

Read at the bytes on 2026-10-17 (`src/util/path_security.rs`, the `cwd_canon` lines above). Found by
the security review of the temp-dir fix, which asked for the canonical-home form to be copied from
`validate_approve_path` and not from this line.

## Hypotheses tried

None.

## Fix

**Where it landed (recorded 2026-10-19).** This text was written on branch `worktree-agent-a40d6fa7287de0e17`, which was never merged. Its commits were superseded by a different implementation on `experiments`: `classify_write_path_in` filters the cwd and temp roots through `too_broad(root, &env.homes)`. The names below (`cwd_allow_root`, `is_too_broad_root`, `WriteEnv.homes` as `protected_homes()`) belong to the branch. The tests on `experiments` that pin the same cases are `cwd_that_is_an_ancestor_of_home_adds_no_root`, `a_cwd_of_the_filesystem_root_adds_no_root`, `a_cwd_that_is_home_behind_a_symlink_adds_no_root` and `a_cwd_above_any_protected_home_adds_no_root`. They were matched by name and not run for this note.

Applied 2026-10-17 on the branch; reshaped the same day after a security review. The cwd root is
`cwd_allow_root(cwd, canonical_homes)`: it canonicalizes the cwd and refuses it when
`is_too_broad_root` says so, the one predicate shared with the temp root (a filesystem root, or a
protected home directory, or an ancestor of one; homes in canonical form). Homes come from
`WriteEnv.homes` = `protected_homes()`, built once per classification, so the guard compares the
canonical cwd with canonical homes instead of a raw `$HOME`.

Only narrows: the cwd stops being a write root when it is `$HOME` through a symlink, and (the
former open design question, decided by the review) when it is an ANCESTOR of `$HOME` (`/home`
made every user's home writable). A project under home, by either spelling, and a directory outside
home keep resolving to the same canonical root.

Callers: the only cwd guard is the one in `classify_write_path_with_env`; its callers are
`classify_write_path` / `validate_write_path` in `src/fs/mod.rs`, `src/tools/core/write_ack.rs` and
`src/tools/symbol/edit_code.rs`. The broader hardening (missing or empty `$HOME`, the temp root) is
in `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`.

## Tests added

In `src/util/path_security.rs` (tests module; unix-only, symlink fixture), driven through
`classify_write_path_with_env` with a `WriteEnv` so no test reads the process cwd or `$HOME`:

- `a_cwd_that_is_home_or_above_it_admits_nothing` — cwd is the canonical home with home known as the
  symlink (the bug), cwd spelled as the symlink, and cwd an ancestor of home.
- `a_cwd_inside_a_symlinked_home_stays_a_write_root` — control: a project under home by either
  spelling stays a write root, in canonical form.
- `a_temp_dir_and_a_cwd_above_home_cannot_admit_it_together` — both implicit roots above home.
- `the_breadth_predicate_refuses_exactly_the_broad_roots` (portable) covers the predicate itself.

Mutations, one per guarded site, each observed red: cwd given no homes (the first version of the
test reached only the helper, and the review found that call site unpinned); homes not
canonicalized; cwd not canonicalized (killed by the control, not by the cwd-is-home cases, because
an un-canonicalized symlink cwd is merely a different, narrower root); ancestor rule weakened to
equality.

## Workarounds

Do not launch the server from a symlinked `$HOME` (only mattered before the fix).

## Resume

Archive once the gate is green on experiments (record the fix SHA and patch-id then).

## References

- `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`
