---
status: open
opened: 2026-07-02
closed:
severity: medium
owner: marius
related: [docs/trackers/windows-platform-support.md]
tags: [windows-gnu, wine, ci, test-portability]
kind: bug
---

# BUG: 20 lib tests fail under wine on the windows-gnu target (pre-existing; exposed by the new CI gate)

## Summary
The first full `cargo test --lib` run under wine (new `windows-gnu` CI job, run
28582988236) shows 20 failures out of 2807. Bisect proves they pre-date the
perf-vdi-closure work: the symbols cluster fails identically at `8431a1d5`
(pre-Task-6). The windows-gnu wine suite was never green — only targeted win32
tests had ever been run under wine (tracker 2026-06-12 entries).

## Symptom (Effect)
CI `windows-gnu` job, wine test step:

```
test result: FAILED. 2777 passed; 20 failed; 10 ignored; 0 measured; 0 filtered out; finished in 22.16s
```

Representative panic (search-mode symbols; same shape for directory/glob/nested):

```
thread 'tools::symbol::tests::symbols_path_type_file' panicked at src/tools/symbol/tests.rs:1745:5:
symbols with file relative_path should find symbols
... symbols with directory relative_path should find symbols: Object {"symbols": Array [], "total": Number(0)}
```

Full failing set (clustered):
- `tools::symbol::tests::` symbols_path_type_{file,glob,directory,nested_directory},
  symbols_name_path_pattern_in_directory, include_docs_attaches_docs_in_search_mode (6)
- `server::guide_hint_tests::` ×9 — all panic in shared setup at src/server.rs:2966
- `agent::tests::activate_populates_head_sha` (src/agent/mod.rs:2496)
- `embed::preflight::tests::check_index_scope_respects_gitignore`
- `librarian::tools::doctor::tests::validate_prune_request_gates` (also fails on REAL
  windows-latest MSVC — the one pre-session real-Windows failure, run 28039317667)
- `librarian::tools::reindex::tests::reindex_backfills_commits_table`
- `tools::markdown::tests::format_compact_live_renders_claude_md_as_map_shape`
- `tools::run_command::tests::background_command_with_quotes_captures_output`

## Reproduction
On Linux with mingw-w64 + wine + rustup target `x86_64-pc-windows-gnu`:

```
git checkout 6f30b6dd   # or 8431a1d5 — same result for the symbols cluster
scripts/build-windows.sh test symbols_path_type
# → 0 passed; 4 failed (identical panics at both commits)
```

## Environment
CI: ubuntu-latest + gcc-mingw-w64 + wine64 (job added in b7944e1e). Local: Arch,
wine /usr/bin/wine, target x86_64-pc-windows-gnu. NOT reproducible on real Windows
for the symbols cluster (windows-latest MSVC pre-session run had only the doctor
test failing).

## Root cause
Unknown — under investigation. The symbols cluster returns empty result sets for
search-mode queries over a TempDir project under wine, suggesting wine-specific
path handling (TempDir canonicalization / verbatim `\\?\` forms / walker path
matching) in the AST search path. The guide_hint cluster is one shared-setup
unwrap at src/server.rs:2966 (likely env/config dir resolution under wine).

## Evidence

### Bisect — pre-Task-6 identical failure
Local wine run at `8431a1d5` (pre-Task-6, worktree + separate CARGO_TARGET_DIR):
`0 passed; 4 failed` — same four `symbols_path_type_*` panics, same messages
(session log 2026-07-02, @bg_00000042 buffer).

### Pre-session real-Windows baseline
Run 28039317667 (e559c8a8, 2026-06-23), Test (windows-latest / default):
`2735 passed; 1 failed` — only `validate_prune_request_gates`. The symbols
cluster PASSED on real Windows.

## Hypotheses tried
1. **Hypothesis:** Task 6 (warming fallback) regressed the symbols overview under
   Windows. **Test:** wine bisect at 8431a1d5 (pre-Task-6). **Verdict:** rejected —
   identical failures pre-Task-6. Also, the failing tests are search-mode, which
   Task 6 did not touch.
2. **Hypothesis:** failures are wine-environment-specific, not real-Windows bugs.
   **Test:** compared with pre-session windows-latest (MSVC) job: symbols cluster
   green there. **Verdict:** confirmed for the symbols cluster (wine-only);
   `validate_prune_request_gates` is the exception — red on real Windows too.

## Fix
Not started (root cause per cluster unknown). Interim: the CI job skips these 20
tests via `--skip` filters (commit follows this file) so the gate is green against
NEW regressions; each skip cites this file. Tracked as WIN-27 in
docs/trackers/windows-platform-support.md.

## Tests added
N/A — this file tracks pre-existing test failures; the regression gate is the
un-skipped remainder (2777 tests) of the wine suite.

## Workarounds
None needed for users — wine is a CI proxy; the VDI runs real Windows where only
the doctor test is affected.

## Resume
Pick one cluster (`server::guide_hint_tests` — 9 tests, one shared unwrap at
src/server.rs:2966): run under wine with RUST_BACKTRACE=1
(`scripts/build-windows.sh test guide_hint_tests`), identify the unwrap's env
dependency (config dir? APPDATA?), fix or gate; then remove its --skip from
.github/workflows/ci.yml and confirm the job stays green.

## References
- CI runs: 28582988236 (first gnu run), 28039317667 (pre-session baseline)
- docs/trackers/windows-platform-support.md — WIN-27
- docs/trackers/perf-windows-session-log.md — F-3 (broader pre-existing CI rot)
- Plan: docs/superpowers/plans/2026-07-02-perf-vdi-closure.md Task 7 step 3
  (pre-authorized narrowing: "record the exclusion + reason ... do NOT delete the job")
