---
status: open
opened: 2026-05-24
closed:
severity: medium
owner: marius
related: [docs/issues/2026-05-24-ci-windows-test-portability-rot.md]
tags: [ci, windows, librarian, path-separators, cross-platform]
kind: bug
---

# BUG: 18 Windows test failures under `default` feature config — librarian + guide_hint modules emit mixed-slash paths

## Summary

After shipping the round 1+2+3 Windows portability fixes (commits
`621732a6..bc05c0b3`), Windows **no-features** + **local-embed** test
configs are green. The third config — **default** — still has 18
failures, all in `librarian::*` modules + `server::guide_hint_tests::*`.
These were not surfaced in the original scout (CI run 26359108902 /
job 77587735460) because that scout was on the `no-features` config
only — the 16-failure inventory missed an entire OS×config slot.

The pattern is consistent across the 18: production code joins
`/`-separated string paths with `PathBuf::join`, which uses the native
separator (`\` on Windows). Result: mixed-slash paths like
`/home/u/work/code-explorer\docs/trackers/foo.md`. Tests assert
against all-forward-slash expectations.

## Symptom (Effect)

Sample failure pattern (from CI run 26360507552 / job 77594958823):

```
---- librarian::catalog::migrate_v6::tests::migration_v6_translates_repo_to_abs_path stdout ----
panicked at src\librarian\catalog\migrate_v6.rs:286:9:
assertion `left == right` failed
  left:  "/home/u/work/code-explorer\\docs/trackers/foo.md"
  right: "/home/u/work/code-explorer/docs/trackers/foo.md"
```

The mixed-slash form `code-explorer\docs/trackers/foo.md` is the
key signal: backslash where `PathBuf::join` added the separator,
forward-slashes elsewhere because the input strings were
forward-slash-formed.

Full list of 18 failing tests:

```
librarian::catalog::migrate_v6::tests::migration_v6_translates_repo_to_abs_path
librarian::indexer::tests::index_derives_title_from_h1_when_no_frontmatter
librarian::indexer::tests::index_removes_deleted_files
librarian::indexer::tests::reindex_refreshes_stale_metadata
librarian::indexer::tests::removed_file_also_removes_embedding_row
librarian::indexer::tests::rule_change_reclassifies_existing_rows_without_content_change
librarian::tests::reindex_cli_indexes_repo
librarian::tools::audit_doc_refs::tests::smoke_tracker_idempotent_on_second_run
librarian::tools::audit_doc_refs::tests::outputguard_caps_findings_inline
librarian::tools::context::tests::repo_scope_excludes_other_repos
librarian::tools::gather::tests::guard_relative_path_rejects_absolute
librarian::tools::reindex::tests::force_wipes_then_reindexes
librarian::tools::reindex::tests::project_scope_force_does_not_nuke_sibling_rows
server::guide_hint_tests::activate_project_resets_hints
server::guide_hint_tests::first_artifact_call_emits_librarian_hint
server::guide_hint_tests::artifact_event_after_artifact_no_hint
server::guide_hint_tests::run_command_with_overflow_emits_progressive_hint_once
server::guide_hint_tests::second_artifact_call_no_hint
```

Test counts: `2416 passed; 18 failed; 12 ignored`.

## Reproduction

```bash
# On Windows:
cargo test 2>&1 | grep -E "FAILED|test result"
```

Or push to `experiments` and observe `Test (windows-latest / default)`.

## Environment

- Microsoft Windows Server 2025 (windows-latest GHA runner)
- Stable Rust + MSVC toolchain
- `default` feature config (full librarian + embeddings)

## Root cause

Production code uses `PathBuf::join` (native separator) on string paths
that are forward-slash-formed throughout the codebase. On Linux/macOS
the native separator IS `/` so output stays forward-slash. On Windows,
join inserts `\` producing mixed-slash paths that don't match test
expectations or any consistent convention.

This is a real production bug, not test-side brittleness — MCP
responses going to LLM consumers should arguably always be
forward-slash-normalized regardless of platform. The librarian
modules in particular write paths into the catalog DB; mixed
separators there will break cross-platform DB portability.

## Evidence

- CI run 26360507552, job 77594958823: 18 failures, ALL with the
  same mixed-slash pattern (verified by grepping for `\\` in
  assertion-failure lines).
- Linux + macOS pass these same tests because the native separator
  matches the forward-slash convention.

## Hypotheses tried

N/A — pattern was identified on first read of the assertion output.

## Fix (recommended shape)

This is a substantive engagement — touches 6 modules. Recommended
approach:

1. Audit all `PathBuf::join` callsites in `src/librarian/**` that flow
   into MCP responses or DB writes. Add a `to_forward_slash` normalizer
   (similar to `dunce::canonicalize`'s prefix-strip pattern) at the
   serialization boundary.
2. Decide policy: is the catalog DB platform-portable? If yes,
   normalize on write. If no, document this in the indexer's module
   docstring + add a CI check.
3. Update test fixtures to use either platform-native join (matching
   production) or normalize both sides before assert.
4. Add a helper `normalize_for_display(path: &Path) -> String` and use
   consistently across the librarian's response shapes.

Total effort estimate: 1-2 days for a developer with Windows env.

## Tests added

The 18 failing tests are the regression cases. Add CI verification
on each fix.

## Workarounds

Treat `Test (windows-latest / default)` as informational pending fix.
The other 8 OS×config slots (Linux × 3, macOS × 3, Windows × 2)
pass — production verification surface is intact for the dominant
deployments.

## Resume

1. Open a Windows VM or local cross-compile environment.
2. Tackle by module:
   - `librarian::catalog::migrate_v6` (1 test) — fix the test first
     to see the production-side normalize pattern.
   - `librarian::indexer` (5 tests) — likely all from one normalizer.
   - `librarian::tools::*` (5 tests across audit_doc_refs, context,
     gather, reindex) — apply the established pattern.
   - `server::guide_hint_tests` (5 tests) — likely tied to hint-text
     formatters that embed paths.
3. Each module fix should pass on Linux + macOS without changes.
4. Push to experiments to keep the matrix-shaping signal alive.

## References

- `.github/workflows/ci.yml` `Test (windows-latest / default)` job
- CI run 26360507552, job 77594958823 (Test windows-latest / default)
- Sibling: `docs/issues/2026-05-24-ci-windows-test-portability-rot.md`
  (parent rot ticket — that one tracks no-features failures, now
  mitigated; this file tracks default-only residual)
- Round-1 + round-2 + round-3 commits: 621732a6..bc05c0b3 — the
  no-features fixes that surfaced this layer
