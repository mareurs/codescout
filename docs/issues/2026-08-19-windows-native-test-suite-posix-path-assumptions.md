---
status: open
opened: 2026-08-19
closed:
severity: low
owner: marius
related: []
tags: [windows, test-design, path-handling]
kind: bug
---

# BUG: 9 tests across 4 files fail when run natively on Windows because they hardcode POSIX-only path literals/assumptions, with no production impact

## Summary
Running the full `cargo test` suite natively on Windows for the first time
(this VDI, gnu toolchain, after the experiments fast-forward) surfaced 9
previously-uncaught test failures across `src/config/global.rs`,
`src/util/path_security.rs`, `src/tools/config/tests.rs`, and
`src/tools/core/tests.rs`. All 9 are test-design gaps that assume POSIX
path semantics — none indicate a production defect; in each case the
underlying implementation is either confirmed correct or the test fixture
itself constructs an input real production code paths never produce.
Bundled into one bug file since each individual test is a small, related
instance of "test never had Windows coverage," rather than filing 4+
separate near-duplicate reports.

## Symptom (Effect)
```
# src/config/global.rs (3 tests — same cause)
config::global::tests::config_dir_prefers_xdg_config_home ... FAILED (Option::unwrap() on None)
config::global::tests::config_dir_xdg_wins_over_home ... FAILED
config::global::tests::env_path_derives_from_config_dir ... FAILED

# src/util/path_security.rs (2 tests — 2 distinct causes)
util::path_security::tests::hard_denials_say_that_approve_write_will_not_help ... FAILED
util::path_security::tests::an_option_glob_does_not_force_the_in_project_verdict ... FAILED

# src/tools/config/tests.rs + src/tools/core/tests.rs (3 tests — 2 distinct causes)
tools::config::tests::a_worktree_whose_main_has_workspace_toml_reports_inherited_topology ... FAILED
tools::config::tests::activating_a_linked_worktree_reports_the_divergence_it_creates ... FAILED
tools::core::tests::a_read_says_which_tree_it_answered_from_when_worktrees_are_unchosen ... FAILED

# src/memory/filter.rs (1 test — different cause: --release strips debug_assert!)
memory::filter::tests::filter_sections_empty_sections_is_caller_error - should panic ... FAILED
note: test did not panic as expected
```

## Reproduction
1. `git checkout experiments` at `5b54848fd2a4e7fe5da6bf277dc85de39958ff27`
2. `cargo +1.97.1-x86_64-pc-windows-gnu test --release --features server-stack --lib`
3. Observe the 9 failures above (out of 14 total that session; the other 5
   are filed as separate genuine-bug reports — see References).

## Environment
Windows 11 Enterprise 10.0.26200 (VDI), `1.97.1-x86_64-pc-windows-gnu`
toolchain (host toolchain forced to gnu — this VDI has no MSVC C++ Build
Tools; see `docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`).
`codescout` repo, `experiments` branch.

## Root cause
Four distinct mechanisms, one per cluster:

**1. `config::global` ×3** (`src/config/global.rs:245-303`): all three tests
call `global_config_dir_from(Some(OsStr::new("/tmp/...")), ...)`.
`global_config_dir_from` (`:49-61`) filters on `PathBuf::is_absolute()` —
but `PathBuf::from("/tmp/...")` is **not** absolute on Windows (Windows
requires a drive letter or UNC prefix; a bare `/`-rooted path is
"has_root" but not "absolute"). The filter rejects it, falls through to
`home` (also `None` in these tests), and `.unwrap()` panics on the
resulting `None`.

**2. `path_security` ×2** (`src/util/path_security.rs`):
- `hard_denials_say_that_approve_write_will_not_help` (exercises
  `classify_write_path`, `:273-373`) relies on the comment-stated invariant
  "`..` only survives canonicalization when an intermediate directory does
  not exist" — true on POSIX (component-by-component `readdir`), **false
  on Windows**, where `CreateFileW`'s NT path translation lexically
  collapses `dir\..` pairs as pure string manipulation before any
  filesystem lookup. The hard-denial branch (`:308-320`) never fires.
- `an_option_glob_does_not_force_the_in_project_verdict` (exercises
  `check_source_file_access` → `segment_reads_project_source`,
  `:1398-1422`) relies on `Path::is_absolute()` being true for a
  driveless POSIX-style path (`/home/u/work/otherrepo`) — false on
  Windows, same underlying std/Win32 semantics as cluster 1.

**3. worktree topology ×3** (`src/tools/config/tests.rs`,
`src/tools/core/tests.rs`):
- Two tests (`a_worktree_whose_main_has_workspace_toml_reports_inherited_topology`,
  `activating_a_linked_worktree_reports_the_divergence_it_creates`)
  pre-canonicalize their fixture's base dir to a `\\?\`-verbatim Windows
  path *before* writing a fake `.git` worktree pointer file. Windows
  verbatim (`\\?\`) paths only treat `\` as a separator, not `/`, so
  `is_linked_worktree`'s `Path::components()` walk
  (`src/util/path_security.rs:518-538`) never yields a `"worktrees"`
  component — the whole `/main/.git/worktrees/feat` suffix collapses into
  one opaque literal, and `is_linked_worktree` returns `false`. Real
  `git worktree add` never writes `\\?\`-prefixed pointers, so production
  is unaffected — this is purely a test-fixture bug.
- `a_read_says_which_tree_it_answered_from_when_worktrees_are_unchosen`
  compares JSON-serialized tool output (backslashes escaped as `\\`)
  against `Path::display()`'s raw single-backslash form
  (`.contains(&wt.display().to_string())`) — can never match once the
  path contains a backslash, i.e. always on Windows. A missing
  escape-awareness bug in the assertion itself, trivially passes on POSIX
  (no backslashes to escape).

**4. `memory::filter` ×1** (`src/memory/filter.rs:298-302`): a
`#[should_panic(expected = "precondition")]` test relying on
`debug_assert!` (`:91-94`), which compiles out entirely in `--release`
builds. The test's own comment and the function's doc comment both state
this caveat explicitly — this cluster is *not* a Windows issue at all, it
fails identically in any `--release` build on any OS; it just happened to
surface in this session because the build used `--release`.

*Measured 2026-08-19: all reproduced via
`cargo +1.97.1-x86_64-pc-windows-gnu test --release --features server-stack --lib
<test> -- --nocapture`; root causes 1–3 confirmed by direct source reading
(std/Win32 path semantics) and, for cluster 3, an independent standalone
repro compiled with the same toolchain. Cluster 4 confirmed by the test's
own inline documentation.*

## Evidence
### Subagent investigation (2026-08-19)
Four parallel investigations (path_security, worktree topology, config
global — diagnosed inline by the main session — and memory filter) each
independently confirmed no production-code path is affected; see the
per-cluster mechanism descriptions above, each citing the exact
file:line evidence.

## Hypotheses tried
1. **Hypothesis:** These are regressions introduced by the 550-commit
   experiments fast-forward.
   **Test:** `git log` on each affected file/test to find when the
   test and the code it exercises were introduced.
   **Verdict:** rejected for clusters 1, 2, and 4 — this is the first time
   these tests have run natively on Windows/gnu, regardless of when the
   code was written; the assumptions are POSIX-only from the start.
   Cluster 3's specific tests ARE new (from the merge), but the bug is in
   the new tests' fixtures, not in the worktree-topology feature they
   exercise.
2. **Hypothesis:** Cluster 2 (path_security) indicates an actual security
   hole — Windows path handling bypassing a write-denial check.
   **Test:** Read `classify_write_path` and `check_source_file_access` in
   full; compared what each returns on Windows vs. what the POSIX-authored
   test expects.
   **Verdict:** rejected (with moderate confidence — see Resume) — Windows'
   own `..` collapse and `Path::is_absolute()` semantics don't appear to
   create a bypass, just a different code path (`Allowed`/`OutsideRoot`
   instead of the specific hard-`Denied` variant) than the POSIX test
   expects. Worth a maintainer sanity pass given this is security code.

## Fix
Not yet implemented, four independent small fixes:
1. `config::global` tests: use a platform-appropriate absolute path helper
   in test fixtures instead of hardcoded `/tmp/...` literals (e.g.
   `std::env::temp_dir()` or a `#[cfg(windows)]`/`#[cfg(unix)]` literal
   pair).
2. `path_security` tests: give Windows-aware inputs/expectations — a real
   drive-relative or verbatim-prefix operand for the outside-root test;
   for the hard-denial test, either accept `Allowed`/`OutsideRoot` as
   valid on Windows or construct an unresolvable `..` that Windows also
   can't lexically collapse (e.g. one where the *parent* directory is
   itself missing).
3. worktree tests: don't pre-canonicalize the fixture base dir to a
   `\\?\`-verbatim path before writing the fake `.git` pointer (tests 1–2);
   compare against the JSON-escaped form, or use a path-agnostic
   containment check, in test 3.
4. `memory::filter` test: either accept `#[cfg(debug_assertions)]`-gating
   this test, or replace the `debug_assert!` under test with a check that
   also fires in `--release` if the precondition genuinely must hold
   there too (product decision, not just a test fix).

## Tests added
N/A — not yet fixed; this bug file covers pre-existing tests whose fixtures
need repair.

## Workarounds
Run the suite with `cargo +<toolchain> test` (debug, not `--release`) to
avoid cluster 4; the other 8 have no workaround short of skipping them —
they don't reflect a usable-vs-broken product distinction on Windows.

## Resume
Work the four fixes independently (see Fix). Prioritize cluster 2
(path_security) first given it's security-adjacent code, even though the
initial read suggests no real bypass — get a second pair of eyes on
`classify_write_path`'s Windows behavior before considering this closed.

## References
- `src/config/global.rs:49-61,245-303`
- `src/util/path_security.rs:273-373,1398-1422,2492-2508,3662-3680`
- `src/tools/config/tests.rs` (~918-923, ~985-990)
- `src/tools/core/tests.rs:584-594,615`
- `src/memory/filter.rs:91-94,298-302`
- `docs/issues/archive/2026-06-09-windows-test-suite-preexisting-failures.md` (prior, already-fixed batch of Windows path issues in the same `path_security.rs` file — same recurring class, different instances)
- Sibling reports from the same investigation session: rendezvous PPID stub, doctor `outside_roots` separator bug, `artifact(create)` missing `to_forward_slash`, memory-embedder test EnvGuard gap (all filed 2026-08-19)
