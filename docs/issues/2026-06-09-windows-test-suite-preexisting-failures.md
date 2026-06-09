---
status: open
opened: 2026-06-09
closed:
severity: medium
owner: marius
related: []
tags: [windows, tests, ci, environmental, companion]
kind: bug
---

# BUG: 6 pre-existing test failures on Windows (environmental + companion drift)

## Summary
Running the full `cargo test` suite on the Windows VDI yields 6 failures that
are **not** caused by the VDI reliability work stream: 5 are Windows
environmental (path canonicalization, HOME resolution, hidden-file counting,
seeded-doc drift, path-relative annotation) and 1 is real stale-tool-name drift
in the `codescout-companion` plugin hooks. The suite has evidently never been
run green on Windows. Logged so a future scoped pass can make `cargo test`
green on this platform; none of these block the reliability stream.

## Symptom (Effect)
`cargo test` (full suite) on Windows: **2590 passed, 8 failed**. Two of the 8
(`server_registers_all_tools`, `server_tool_count_is_l3_target`) were the
peer-gating count tests, fixed in `5881ed09`. The remaining **6**:

```
test embed::preflight::tests::check_index_scope_counts_hidden_non_gitignored_files ... FAILED
test librarian::tools::doctor::tests::doctor_call_surfaces_seeded_drift ... FAILED
test server::tests::companion_surfaces_reference_only_real_tools ... FAILED
test server::tests::stripped_responses_emit_paths_relative_annotation_once_per_activation ... FAILED
test util::fs::tests::detect_project_root_finds_cargo_toml ... FAILED
test util::path_security::tests::read_ssh_key_denied ... FAILED
```

Per-failure assertions:

```
detect_project_root_finds_cargo_toml (src/util/fs.rs:207)
  left:  Some("C:\\Users\\MAILIN~1.002")
  right: Some("C:\\Users\\MAILIN~1.002\\AppData\\Local\\Temp\\.tmpAA31yX")

read_ssh_key_denied (src/util/path_security.rs:907)
  assertion failed: result.is_err()

doctor_call_surfaces_seeded_drift (src/librarian/tools/doctor.rs:382)
  left: Some(6)   right: Some(5)

check_index_scope_counts_hidden_non_gitignored_files (src/embed/preflight.rs:393)
  panicked (no assert message)

stripped_responses_emit_paths_relative_annotation_once_per_activation (src/server.rs:2400)
  panicked

companion_surfaces_reference_only_real_tools (src/server.rs:1916)
  companion-surface drift detected: stale tool names replace_symbol /
  insert_code / remove_symbol / edit_lines / create_or_update_file in the
  companion hooks (hooks.json, cs-activate-project.sh, pre-edit-hint.sh,
  session-start.sh, subagent-guidance.sh, worktree-activate.sh,
  worktree-write-guard.sh)
```

## Reproduction
- Commit: `5881ed09` (experiments), Windows 11 VDI.
- `cargo test` (full suite). The 6 above fail; the rest pass.

## Environment
Windows 11 Enterprise; codescout `experiments`; profile path
`C:\Users\MAILINCA.BRN.002` (8.3 short name `MAILIN~1.002`); `%TEMP%` under the
profile; `codescout-companion` sibling checkout at `../claude-plugins/`.

## Root cause
Two distinct classes. None touch `platform/`, `run_command/`, `prompts/`, or the
IL3 code changed this stream — verified by module location.

**Class 1 — Windows environmental (5):**
1. `detect_project_root_finds_cargo_toml` — the test compares a freshly created
   temp dir against `detect_project_root`'s result; on Windows the two disagree
   on 8.3 short-name (`MAILIN~1.002`) vs canonicalized long path. Path-form
   normalization mismatch, not a logic bug.
2. `read_ssh_key_denied` — the `~/.ssh` deny relies on HOME expansion; on Windows
   the expansion / deny-prefix match (`src/platform/windows.rs::denied_read_prefixes`
   uses `~/...` POSIX-style prefixes) does not fire, so the read is not denied.
3. `check_index_scope_counts_hidden_non_gitignored_files` — hidden/dot-file
   counting differs on Windows (no panic message captured; preflight.rs:393).
4. `doctor_call_surfaces_seeded_drift` — seeded-doc drift count off by one
   (6 vs 5); likely a fixture file count that varies by platform line-endings or
   path globbing.
5. `stripped_responses_emit_paths_relative_annotation_once_per_activation` —
   path-relative annotation assertion sensitive to Windows path form.

**Class 2 — companion plugin drift (1):**
6. `companion_surfaces_reference_only_real_tools` — the `codescout-companion`
   hooks on this machine still reference tool names consolidated away long ago
   (`replace_symbol`/`insert_code`/`remove_symbol` → `edit_code`;
   `create_or_update_file` → `create_file`; `edit_lines` → `edit_code`). The test
   correctly flags this. Fix belongs in the **sibling repo**
   `../claude-plugins/codescout-companion/hooks/`, not here. This is cross-repo
   and may simply be a stale checkout on this VDI.

## Evidence
Triage performed by re-running each failure with `--nocapture` at commit
`5881ed09`; assertion messages quoted verbatim in `## Symptom`. Module paths
confirm none overlap the reliability stream's changed files.

## Hypotheses tried
1. **Hypothesis:** the failures are caused by the VDI reliability work stream
   (platform spawn/kill, run_command, prompt scrub, IL3 hint). **Test:** mapped
   each failing test to its module; none are in `platform/`, `run_command/`,
   `prompts/`, or `detect_il3_violation`. The one in `path_security.rs`
   (`read_ssh_key_denied`) exercises `denied_read_paths`, a different function
   from the IL3 hint string edited this stream. **Verdict:** rejected — zero
   overlap.
2. **Hypothesis:** `companion_surfaces_reference_only_real_tools` fails because
   the Unix-only `peer` tool is absent on Windows. **Test:** read the drift
   output — it names `replace_symbol`/`insert_code`/etc., never `peer`.
   **Verdict:** rejected — it is companion-hook tool-name drift, unrelated to
   peer gating.

## Fix
N/A — not fixed this session (out of scope for the reliability stream). Scoped
follow-ups when someone makes the Windows suite green:
- Class 1: normalize path comparisons in the tests (canonicalize both sides /
  compare via `same-file`), and make the `~/.ssh` deny test + windows
  `denied_read_prefixes` agree on HOME-prefixed paths.
- Class 2: update the `codescout-companion` hooks to the consolidated tool names
  (cross-repo) or refresh the sibling checkout.

## Tests added
N/A — these *are* failing tests; the work is to make them pass on Windows, not
to add new ones. The two peer-gating count tests were already made cfg-aware in
`5881ed09`.

## Workarounds
Run `cargo test --lib` for the unit suite and ignore these 6 known failures, or
filter them out, until the scoped Windows pass lands. `cargo build` is green.

## Resume
Pick Class 1 first (self-contained, in-repo): start with
`detect_project_root_finds_cargo_toml` in `src/util/fs.rs` — canonicalize the
expected temp path before comparing, or compare with `dunce`/`same-file`. Then
`read_ssh_key_denied` in `src/util/path_security.rs` — verify
`denied_read_prefixes` (`src/platform/windows.rs`) matches a USERPROFILE-rooted
`.ssh` path. Class 2 is cross-repo: inspect
`../claude-plugins/codescout-companion/hooks/` for the stale names listed above.

## References
- `src/server.rs` (companion-surface + count tests), `5881ed09` (peer count fix)
- `src/util/fs.rs`, `src/util/path_security.rs`, `src/embed/preflight.rs`,
  `src/librarian/tools/doctor.rs`
- `../claude-plugins/codescout-companion/hooks/` (Class 2 drift)
- `docs/trackers/vdi-reliability-session-log.md`
