---
id: '3dff5caea3feebce'
kind: bug
status: open
title: 'BUG: the Windows lanes stay red on four further causes after the doctor cluster — POSIX-absolute fixtures, verbatim-prefix worktree detection, config-dir resolution, and parent pid'
tags:
- windows
- ci
- path-form
- worktree
- cross-platform
closed: ''
opened: 2026-08-26
owner: marius
severity: high
unverified: 'Grouped from ONE wine run plus one MSVC log. The groups are inferred from panic messages, not from fixing any of them, and the wine/MSVC split is only partly established: wine cannot run the Git Bash path at all, so the 8 shell-related failures below are wine-only noise and the 9th in that module may or may not be real. Nothing here has been reproduced twice.'
---

## Summary

`af5d0dab` cleared the 31-test `doctor` cluster. The Windows lanes stay red. On CI run
`32740102144` (`047dd433`) the `windows-latest / default` lane failed **46**; 31 were the
doctor cluster and 1 was the ambient-embedder test (`d81064f7`), leaving **14**.

Reproduced locally under wine — `scripts/build-windows.sh test --lib -- config::global
util::path_security tools::config:: retrieval::index_lock usage::content_tests
tools::rendezvous tools::grep tools::core server::guide_hint` → **467 passed / 20 failed**.
The count differs from 14 in both directions, and the differences are the useful part.

## Symptom (Effect)

Three Windows lanes red. Unlike the doctor cluster this is **not one cause**: the panics
group into at least four, two of which look like production defects rather than fixtures.

## Evidence — the groups

### A. Not real: no POSIX shell under wine (8 of the 20)

Eight `server::guide_hint_tests` failures are all the same error:

> `no POSIX shell available to run commands — Install Git for Windows (which provides Git
> Bash), or point codescout at an existing bash.exe with CODESCOUT_BASH=…`

MSVC has Git Bash; wine here does not. **MSVC failed only ONE test in that module**
(`an_artifact_call_naming_a_tracker_path_delivers_the_tracker_guide`), so eight of these
nine are wine-environment noise — the same reason `.github/workflows/ci.yml` carries a
skip list on the wine job. Whether the ninth is real cannot be separated from this run.

### B. Wine passes what MSVC fails (2)

`retrieval::index_lock::tests` ×2 failed on MSVC and **pass under wine**. So they are
MSVC-specific — file-locking or pid semantics that wine emulates differently. They cannot
be worked with the local loop, which is worth knowing before someone tries.

### C. POSIX-absolute paths in fixtures (3) — same class as the doctor cluster

- `tools::grep::tests::unsatisfiable_absolute_glob_flags_only_absolute_paths_outside_the_root`
  — `left: None, right: Some("/home/u/other/x.rs")`. `/home/u/...` is not an absolute path
  on Windows, so the "absolute and outside the root" branch is never taken.
- `util::path_security::tests::an_option_glob_does_not_force_the_in_project_verdict`
  — *"a glob filter beside an out-of-project search root is not a project read"*.
- `util::path_security::tests::hard_denials_say_that_approve_write_will_not_help`
  — *"an unresolved '..' must be a hard Denied, not approvable"*.

These are **security-layer** tests, so the fixtures must be fixed in a way that keeps
testing the same predicate rather than one that merely goes green — the boundary check in
`containing_root` is the guard `delete` and `move` rely on.

**FIXED 2026-08-26 — `cd0bfcaa`, patch-id `65de6c9d9f660dc657915fe7d7f9c5a19546e0a6`.**
Two were the spelling bug and are drive-prefixed. The third was not: it needs `..` to
survive canonicalization, and Windows normalizes `..` lexically, so the arm is unreachable
there — measured, not assumed, once the test's `let … else` was made to name what it got
(`Allowed("\\?\C:\…\.tmpC5EDa4\escape.rs")`). Split into a `cfg(not(windows))` test plus a
`cfg(windows)` sibling that pins the lexical resolution, so the difference fails loudly if
it ever changes rather than sitting behind a skip.

**The vacuous half was the worse half.** In `unsatisfiable_absolute_glob`'s test every case
degraded to `None` on Windows: that failed the two `Some` assertions loudly, and made the
three `None` assertions pass while asserting nothing. A fixture that cannot reach the
branch it names is a green that reads the same in a broken world.

### D. Verbatim `\\?\` prefix vs plain, in worktree detection (4) — looks like production

- `tools::config::tests::activating_a_linked_worktree_reports_the_divergence_it_creates`
  and `…a_worktree_whose_main_has_workspace_toml_reports_inherited_topology` — activation
  returns no `worktree` block at all, and `project_root` comes back verbatim:
  `//?/C:/users/…/main/.worktrees/feat`.
- `tools::core::tests::a_read_says_which_tree_it_answered_from_when_worktrees_are_unchosen`
  — the notice names `\\?\C:\users\…\main` while the worktree list beside it holds a plain
  `C:\users\…\wt-feat`. **The same response carries both spellings**, which is the tell.
- `usage::content_tests::record_content_pinned_into_a_worktree_writes_to_the_main_checkouts_db`
  — not yet read, grouped here on subject.

This is the WIN-30 class again (`doctor.rs`'s own header: *"the catalog stores `//?/C:/...`
while `current_project` holds `\\?\C:\...`"*), but in the worktree-detection path rather
than the catalog comparison, and it appears to be **production** behaviour rather than a
fixture: a real Windows user with a linked worktree would get no worktree block.

**FIXED 2026-08-26 — `9b048e17`, patch-id `f42a4e9faa0aac83d8f92f1f94ee66852e1e952c`. The
heading's guess above was WRONG, and is left standing because the correction is the
lesson.** Worktree detection is not broken on Windows: `is_linked_worktree` and
`worktree_main_root` both pass there. Measuring the primitives before believing the
symptom is what redirected this.

Two fixture mechanisms, neither the one the panics suggested:

1. **Verbatim prefix + forward slashes.** Three fixtures build the pointer as
   `format!("gitdir: {}/main/.git/worktrees/feat", base.display())` where `base` is
   **canonicalized** — on Windows the verbatim `\\?\C:\…` form. Inside a verbatim path
   Rust does **not** treat `/` as a separator, so the whole tail parses as ONE component,
   no `worktrees` component is seen, and the detector answers `false` — silently, since
   every failure arm returns `false`. Git never writes that spelling; only string
   concatenation does. **17 of the 20 sites in this crate are fine**, because their base
   is not canonicalized — which is precisely why it stayed invisible.
2. **JSON escaping.** `tools::core` substring-matched `wt.display()` against a JSON
   *document*, where Windows separators are escaped to `\\`. It could never match. Now
   reads the notice out of the parsed JSON.

A permanent guard came out of it: `is_linked_worktree_survives_canonicalization`. It began
as the discriminator for a hypothesis it then **refuted** — canonicalization alone is fine
— and was kept anyway, because every real caller passes the canonicalized form and nothing
else covered that shape.

### E. Config-dir resolution on Windows (3)

- `config_dir_xdg_wins_over_home` — `left: "/tmp/fake-home\\.config\\codescout"`,
  `right: "/tmp/xdg/codescout"`. `XDG_CONFIG_HOME` loses to the home-based path on Windows.
- `config_dir_prefers_xdg_config_home` and `env_path_derives_from_config_dir` —
  `Option::unwrap()` on `None` at `src/config/global.rs:247` and `:298`.

Open question this group turns on, and it is a **decision**, not a bug to fix blind: is
`XDG_CONFIG_HOME` meant to be honoured on Windows at all? If not, the tests are wrong; if
so, the resolver is. Answer that before touching either.

**FIXED 2026-08-26 — `b1681d76`, patch-id `f35b49bee1eb754961ae06610b90fbc073839acb`. There
was no decision to make, and the paragraph above is left standing because that is the
lesson.** `global_config_dir_from` has **no platform branch**: XDG is honoured everywhere,
and the only gate is `.filter(|p| p.is_absolute())`, implementing the spec's rule that a
relative value is invalid. Reading the function took less time than drafting the question.

So E is C wearing a different hat. `/tmp/xdg` has no drive letter, so on Windows it is
relative, is dropped as spec-invalid, and falls through to `HOME` — which is precisely
what the panic showed. Fixed with an `abs()` helper that drive-prefixes on Windows,
applied to every literal meant to be absolute and deliberately **not** to the
`relative/state` ones, which must stay relative on both platforms or the spec gate they
pin goes untested.

The third effect is the one worth carrying forward:
`config_dir_ignores_relative_xdg_and_falls_back_to_home` kept **passing** on Windows while
discriminating nothing, because both its inputs were relative there — it could no longer
tell *"ignores a relative value"* from *"ignores everything"*. Third time in this file that
the vacuous pass outlasted the loud failure.

### F. Parent pid is 0 (1)

`tools::rendezvous::tests::publish_records_the_parent_pid_the_hook_matches_on` —
*"ppid must be recorded, left: 0, right: 0"*. Either the Windows parent-pid lookup returns
0, or wine does not provide one. **Not separable from this run** — check MSVC's log for the
same assertion before assuming it is real.

## Reproduction

```
scripts/build-windows.sh test --lib -- <module filters>
```

Requires `mingw-w64`, `wine`, and `rustup target add x86_64-pc-windows-gnu` — all present
on this box. ~7 s per iteration once the target is built, against one CI round-trip
otherwise. Note the header caveat in `scripts/build-windows.sh`: this is the **gnu** ABI,
so a green run mirrors the VDI artifact, not the MSVC `windows-latest` runner — group B is
exactly that gap made visible.

## Root cause

Not established for any group. C is almost certainly fixture-side; D looks production-side;
E is a policy question first; B and F cannot be settled with the local loop.

## Fix

Not started. Suggested order — cheapest and most certain first:

1. **C**, mirroring `af5d0dab`: fixtures must spell paths the way the platform does. Keep
   the predicate under test intact; these are security-boundary tests.
2. **E**, once the XDG-on-Windows question is answered.
3. **D**, the only group with a plausible user-visible defect behind it.
4. **B/F** last, and only with MSVC evidence — the local loop cannot see them.

## Tests added

None yet.

## References

- `docs/issues/archive/2026-08-26-doctor-entry-validity-tests-spell-paths-natively-on-windows.md`
  — the 31-test cluster, same family, already fixed
- `docs/issues/archive/2026-08-26-ci-test-lanes-red-because-one-test-reads-ambient-embedder-config.md`
  — the 46th failure
- `scripts/build-windows.sh` — the local loop, and its ABI caveat
- `.github/workflows/ci.yml` — the wine job's skip list, which is group A's precedent
