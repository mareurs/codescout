---
status: mitigated
opened: 2026-05-24
closed: 2026-05-24
severity: medium
owner: marius
related: [docs/issues/2026-05-24-ci-macos-tempdir-canonicalization.md, docs/issues/2026-05-24-ci-windows-jemalloc-build-fail.md]
tags: [ci, windows, cross-platform, test-portability]
kind: bug
---

# BUG: ~19+ tests fail on windows-latest due to pre-existing Windows portability issues

## Summary

CI run 26357841051 (commit `a4abca2a`) was the **first time the
codescout test suite compiled and executed on Windows** (after a
session-long stack of platform-gating fixes: build.rs CRLF, src/prompts
CRLF, lsp::mux cfg(unix), rusqlite bundled, jemalloc cfg(unix)). With
the build phase clean, the next layer of pre-existing rot surfaced:
16 distinct test failures across `agent::tests`, `lsp::manager::tests`,
`tools::config::tests`, `tools::edit_file::tests`, `server::tests`,
and others — under the `no-features` test config.

**Status update 2026-05-24 (commits `c8d88f47..bc05c0b3`):** all 16
no-features failures addressed via 9 distinct fix mechanisms. Windows
`no-features` and `local-embed` configs now pass (first Windows greens
of the session, ever). The `default` config still has 18 separate
failures — pre-existing librarian-feature rot that this scout missed
because it was no-features-only. Those failures are tracked at
[`docs/issues/2026-05-24-ci-windows-default-feature-failures.md`](2026-05-24-ci-windows-default-feature-failures.md).

This file's status flips to `mitigated` (not `fixed`) because:
- 8 of the original 16 tests were fixed via code changes (path normalization,
  Instant arithmetic, JSON parsing, etc.).
- 6 were gated with `#[cfg_attr(target_os = "windows", ignore = ...)]`
  pending the proper Windows port engagement (Unix path literals, /
  filesystem-root semantics, /var assumption, /tmp absolute paths,
  cmd.exe vs bash shell syntax).
- 2 were already deferred for macOS via the same pattern and extended
  to Windows.

## Symptom (Effect)

Sample of 19+ FAILED tests (CI run 26357841051, Test windows-latest /
no-features, job 77587735460):

```
agent::tests::activate_replaces_previous_project ... FAILED
agent::tests::activate_sets_project ... FAILED
agent::tests::agent_is_clone_safe ... FAILED
agent::tests::home_root_set_on_first_activate ... FAILED
agent::tests::home_root_set_from_initial_project ... FAILED
agent::tests::home_root_not_changed_by_second_activate ... FAILED
agent::tests::new_with_valid_project ... FAILED
agent::tests::project_status_returns_some_with_project ... FAILED
lsp::manager::tests::evict_idle_clears_stale_last_used_entries ... FAILED
prompts::tests::prompt_surfaces_system_prompt_draft_empty_snapshot ... FAILED
lsp::client::tests::workspace_symbols_returns_project_symbols ... FAILED
server::tests::run_command_output_keeps_absolute_project_paths ... FAILED
tools::config::tests::activate_hint_shows_switched_when_away_from_home ... FAILED
tools::config::tests::activate_hint_shows_returned_when_back_home ... FAILED
tools::config::tests::activate_includes_cwd_hint ... FAILED
tools::config::tests::activate_project_switches_focus_by_id ... FAILED
tools::config::tests::activation_response_emits_legacy_index_when_db_present ... FAILED
tools::edit_file::tests::tree_nonexistent_path_errors ... FAILED
... (more in default/local-embed jobs)
```

## Reproduction

```bash
# On Windows (or via Cargo cross — not yet wired):
cargo test --no-default-features 2>&1 | grep -E "FAILED|test result"
```

Or push to experiments and observe `Test (windows-latest / *)` jobs.

## Environment

- Microsoft Windows Server 2025 (windows-latest GHA runner)
- Stable Rust + MSVC toolchain
- All feature configs (no-features, local-embed, default — same shape)

## Root cause

Multi-faceted pre-existing rot. Tests written on Linux/macOS assume:
1. **Path separators are `/`** — Windows uses `\`; many `assert_eq!`
   on full paths break.
2. **Filesystem locking is BSD-flock-like** — Windows uses mandatory
   locking; tests that re-open the same path can fail.
3. **CRLF in checked-out fixtures** — git on Windows auto-converts LF
   to CRLF. Tests that compare against literal `\n`-separated strings
   see mismatches.
4. **Process spawning APIs** — fork-exec patterns lift weirdly to
   CreateProcess.
5. **Symlinks / hard links** — Windows requires admin or developer
   mode; some test setups silently degrade.

Likely sub-categories of fix:
- Normalize paths to forward-slash before comparing (`path.replace('\\', '/')`)
- Add `.gitattributes`: `* text=auto eol=lf` to force LF in checkouts
- Replace flock-based locking with cross-platform `fs2`/`fs4` (already in deps)
- Gate POSIX-only tests with `#[cfg(unix)]`

## Evidence

- CI run 26357841051 job 77587735460: 19 FAILED tests in
  `Test (windows-latest / no-features)`
- Compile phase succeeded — confirms session's platform-gating stack
  worked. Failures are runtime, not compile-time.
- Linux + macOS pass these same tests (modulo other pre-existing rot
  filed separately).

## Hypotheses tried

N/A — root cause is structural (Windows port never completed).

## Fix

Out of scope for the CI rot rehab session that surfaced this. Recommended
shape for a dedicated engagement:

1. **Pin failing tests with `#[cfg(unix)]`** as triage — get Windows
   to a green baseline by excluding broken tests. Document each one
   as a deferred Windows port-blocker.
2. **Add `.gitattributes`** with `* text=auto eol=lf` to neutralize
   the CRLF-checkout problem at the substrate level.
3. **Refactor path comparisons** to normalize before assert. Helper:
   `fn normalize(p: &Path) -> String { p.to_string_lossy().replace('\\', '/') }`.
4. **Audit fs locking** — replace any direct flock with `fs4`.
5. **Run a full pass** on Windows CI, fixing each test individually.

Total effort estimate: 1-3 days for a developer with Windows env.

## Tests added

The 19+ failing tests themselves are the regression cases. Add CI
verification on each fix.

## Workarounds

For now, treat Windows Test matrix as informational. The 6 OS×config
slots that DO pass on Linux/macOS are the production verification
surface.

## Resume

1. Open a Windows VM or use cross-compile + WINE for local iteration.
2. Tackle by error class, not by test (path mismatches → CRLF → locking).
3. Push small fixes to experiments to keep the matrix shaping signal
   alive.

## References

- `.github/workflows/ci.yml` Test (windows-latest / *) jobs
- CI run 26357841051, job 77587735460 (Test windows-latest / no-features)
- Sibling rot — Windows-specific shipping for compile path:
  - `docs/issues/2026-05-24-ci-windows-jemalloc-build-fail.md` (fixed `621732a6`)
  - Session commits `a1a420ba..a4abca2a` (5 fixes that got us to this point)
- The macOS sibling — same shape, different mechanism:
  - `docs/issues/2026-05-24-ci-macos-tempdir-canonicalization.md`
