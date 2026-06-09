---
id: '42dfdfc8b1522192'
kind: tracker
status: active
title: Windows Platform Support — WIN-N Issue Index
owners:
- marius
tags:
- windows
- platform
- vdi
- portability
- ci
topic: windows
time_scope: null
---

# Windows Platform Support — WIN-N Issue Index

Living index of Windows-platform issues for codescout: what is broken, fixed,
mitigated, or deferred when running the MCP server + test suite on Windows. The
primary driver is a locked-down VDI whose EDR/AV injects into spawned processes
and stalls them. This tracker is the durable cross-session **map**; per-incident
detail lives in `docs/issues/` bug files, which get archived once their fix ships
to master — the tracker outlives them.

## Scope & boundary

- **Belongs here:** any Windows-specific defect, portability gap, build/install
  quirk, or platform-gated-code decision — as a one-line WIN-N row pointing at
  the bug file or commit that holds the detail.
- **Does NOT belong here:** full incident detail (→ `docs/issues/<date>-<slug>.md`),
  the scoped narrative of one work stream (→ `docs/trackers/<topic>-session-log.md`),
  or design docs (→ `docs/superpowers/specs|plans/`).
- **Largest contributor:** the VDI reliability work stream (spec + plan +
  session-log under Relationships); its bug files are indexed here as WIN-N rows.

## Status legend

| status | meaning |
|---|---|
| `fixed` | root cause addressed + verified (on `experiments` unless noted) |
| `mitigated` | workaround in place; root cause not fully addressed |
| `open` | known, unaddressed |
| `deferred` | scoped out to its own spec/plan with a re-open trigger |
| `wontfix` | intentionally not fixing |

## Areas

- **process-spawn** — child-process creation/kill under EDR (the core VDI hazard).
- **lsp** — language-server binary resolution + spawn.
- **platform-gated** — code/deps that are Unix-only and must be `cfg(unix)`-gated.
- **path-handling** — canonicalization, 8.3 short names, verbatim `\\?\`, HOME/USERPROFILE.
- **build-install** — building / reloading the live binary on Windows.
- **test-portability** — unit/integration tests that bake in Unix assumptions.
- **companion** — codescout-companion plugin (cross-repo) surfaced on Windows.

## Issue index

<!-- Rendered mirror of the augmentation `issues` params (tool-usage-patterns
     style). Maintain via:
       artifact_augment(id="<id>", merge=true, params={issues:[...]})
     then re-sync this table. Filter rows live with:
       artifact(action="get", id="<id>", entry_filter={"status":{"eq":"open"}}) -->

| id | area | status | summary | ref | since |
|----|------|--------|---------|-----|-------|
| WIN-1 | process-spawn | fixed | run_command spawn hang + cmd.exe quote mangling (EDR grandchild holds pipe; `.arg()` MSVC-CRT quoting; inherited stdin REPL block) | docs/issues/2026-06-08-windows-run-command-child-process-hang.md | 2026-06-08 |
| WIN-2 | process-spawn | fixed | process kill/liveness shelled out to taskkill/tasklist (spawn under EDR); now Win32 OpenProcess/TerminateProcess/GetExitCodeProcess | 9de846d4 | 2026-06-09 |
| WIN-3 | process-spawn | fixed | all 3 run_command spawn sites + BackgroundKillGuard routed through one `platform::shell_command_configured`; stdin=null default on both platforms | 8c4c738f | 2026-06-09 |
| WIN-4 | lsp | fixed | LSP binary name hardcoded `.cmd` (npm shim only); now PATH-probes `.cmd`/`.exe`/`.bat` | docs/issues/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md | 2026-06-06 |
| WIN-5 | lsp | deferred | bounded LSP spawn timeout under EDR — `spawn()` is sync `CreateProcessW`, needs `spawn_blocking`; init handshake already bounded | docs/trackers/vdi-reliability-session-log.md (F-3) | 2026-06-09 |
| WIN-6 | platform-gated | fixed | peer module (Unix domain sockets) does not compile on Windows; gated behind `cfg(unix)` | 5f8911b2 | 2026-06-09 |
| WIN-7 | platform-gated | fixed | tikv-jemalloc-sys fails to build on Windows MSVC; gated as `cfg(unix)` target dep | docs/issues/archive/2026-05-24-ci-windows-jemalloc-build-fail.md | 2026-05-24 |
| WIN-8 | path-handling | mitigated | librarian umbrella members need verbatim `\\?\` prefix on Windows (`canonicalize().starts_with`); workspace.toml written with verbatim + plain fallbacks | %APPDATA%\librarian\workspace.toml | 2026-06-08 |
| WIN-9 | test-portability | open | Windows test suite not green: 6 failures — 8.3 short-path/temp canonicalization, HOME/`~/.ssh` deny, hidden-file counting, seeded-drift, path-relative annotation (supersedes the ~19-test 2026-05-24 portability rot) | docs/issues/2026-06-09-windows-test-suite-preexisting-failures.md | 2026-06-09 |
| WIN-10 | build-install | mitigated | running `.exe` is locked during rebuild; no `~/.cargo` symlink on Windows; workflow = move exe aside, background rebuild, `/mcp` reload | CLAUDE.md | 2026-06-08 |
| WIN-11 | test-portability | fixed | server tool-count tests hardcoded the Unix count of 22 (incl. peer); made `cfg(unix)`-aware (21 on Windows) | 5881ed09 | 2026-06-09 |
| WIN-12 | companion | open | codescout-companion hooks reference consolidated-away tool names (replace_symbol/insert_code/remove_symbol/edit_lines/create_or_update_file); cross-repo fix | docs/issues/2026-06-09-windows-test-suite-preexisting-failures.md | 2026-06-09 |
| WIN-13 | path-handling | mitigated | librarian + guide_hint emitted mixed-slash paths on Windows (18 historical test failures) | docs/issues/archive/2026-05-24-ci-windows-default-feature-failures.md | 2026-05-24 |

## Currently stable on Windows

What works now (post the VDI reliability stream, on `experiments`):

- `cargo build` (lib + bin) compiles clean.
- `run_command` foreground / background / interactive — no hangs, correct
  quoting, `stdin=null` default; all spawns route through
  `platform::shell_command_configured`.
- Process kill/liveness via Win32 (no `taskkill`/`tasklist` spawn).
- Git operations (libgit2 — never shells out, so immune to the spawn hazard).
- LSP binary resolution probes `.cmd`/`.exe`/`.bat` on PATH.

## Open items / next steps

- **WIN-5** — LSP spawn-timeout under EDR: needs a `spawn_blocking` spec before
  implementation.
- **WIN-9** — make the Windows test suite green (path / HOME / hidden-file /
  slash environmental class).
- **WIN-12** — update codescout-companion hooks to the consolidated tool names
  (cross-repo, in `../claude-plugins/codescout-companion/`).

## Relationships

- Spec: `docs/superpowers/specs/2026-06-08-vdi-reliability-hardening-design.md`
- Plan: `docs/superpowers/plans/2026-06-08-vdi-reliability-hardening.md`
- Session log: `docs/trackers/vdi-reliability-session-log.md`
- Active bug files: `docs/issues/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md`,
  `docs/issues/2026-06-08-windows-run-command-child-process-hang.md`,
  `docs/issues/2026-06-09-windows-test-suite-preexisting-failures.md`
- Archived CI-Windows bugs: `docs/issues/archive/2026-05-24-ci-windows-*.md`

## How to append

When a Windows issue is found or its status changes:

1. `artifact(action="get", id="<id>", entry_filter={...})` — confirm it is not
   already tracked.
2. `artifact_augment(id="<id>", merge=true, params={issues:[...existing..., {new WIN-N}]})`
   — next free integer; never reuse or delete; flip status + cite the fixing
   commit (master-side SHA after cherry-pick) in `ref`.
3. Re-sync the "## Issue index" table above with the render_template columns.
4. For a brand-new incident, also open a `docs/issues/<date>-<slug>.md` and cite
   it in `ref`.

## History

### 2026-06-09 — tracker created
Seeded with 13 WIN-N entries from the VDI reliability work stream plus the
2026-05-24 CI-Windows archive. Created after a `librarian reindex` (649
artifacts) confirmed no existing Windows tracker among the 36 live ones.

