---
status: open
opened: 2026-06-03
closed:
severity: medium
owner: marius
related: []
tags: [artifact, librarian, delete, workspace, subagent, abs_path]
kind: bug
---

# BUG: `artifact(action="delete")` refuses an in-workspace artifact — "outside every workspace root" (root cause: stale workspace-root guard ignores active project)

## Summary

> **CORRECTION (2026-06-03) — root cause CONFIRMED via clean serial reproduction.**
> The title's "parallel-subagent abs_path" suspicion is a **red herring**. The failure
> reproduces serially in the parent context (no subagents) on *any* codescout artifact.
> The stored `abs_path` is correct and **absolute**; the guard rejects because it checks
> the legacy `[[roots]]` registry, which does not list codescout. See Root cause.

`artifact(action="delete", id=...)` refused to delete three trackers that live under
the active project's `docs/trackers/`, erroring **"outside every workspace root —
refusing to delete docs/trackers/<file>.md"**. The active project was `codescout`
(verified via `workspace(action="status")` immediately before), and the files were at
`codescout/docs/trackers/*.md` — plainly under the workspace root. The artifacts had
been created moments earlier by **three parallel general-purpose subagents** (via
`artifact` create + `artifact_augment`) during the augmented-tracker eval Run 1.
Cleanup required bare `rm` + `librarian(action="reindex", force=true)`, which dropped
the artifact rows (`removed: 3`) but left **orphan augmentation rows** in the local
catalog (`orphans_removed: 0`).

## Symptom (Effect)
For all three ids (`d0801763526d35b4`, `db3dfa86b0cb58d7`, `2782b9905883e711`):

```
artifact(action="delete", id="<id>")
→ error: "artifact '<id>' is outside every workspace root —
          refusing to delete docs/trackers/<file>.md"
```

The error names the correct `rel_path` — so the catalog *knows* the artifact and its
path — yet the guard claims it is outside every registered workspace root.

## Reproduction (hypothesized — NOT yet confirmed)

**Status: CONFIRMED 2026-06-03 — serial, parent context, no subagents, no parallelism.**

At codescout `4c80dbce`, branch `feat/per-request-workspace-pinning`, live release binary
(built 2026-06-03T16:13):

1. `artifact(action="create", kind="spec", title="probe", rel_path="docs/_delete_probe_tmp.md")`
   → returns `id`.
2. `artifact(action="delete", id=<that id>)` →
   `artifact '<id>' is outside every workspace root — refusing to delete docs/_delete_probe_tmp.md`.
3. Confirm the stored path is absolute (rules out the relative-abs_path theory):
   `sqlite3 ~/.local/share/librarian/catalog.db "SELECT abs_path FROM artifact WHERE id='<id>'"`
   → `/home/marius/work/claude/codescout/docs/_delete_probe_tmp.md`.
4. Cleanup: `rm docs/_delete_probe_tmp.md` + `librarian(action="reindex", scope="repo")` (removed 1).

Parallel creation / subagent context is **not** required to trigger it.
## Environment
codescout v0.14.0, commit `3e1be988`, branch `experiments`. Active project `codescout`.
Artifacts created by 3 concurrent `general-purpose` subagents during the
`augmented-tracker-discovery` eval Run 1 (2026-06-03).

## Root cause

**CONFIRMED 2026-06-03** (serial reproduction, parent context — no subagents, no parallelism).

The delete guard (`src/librarian/tools/delete.rs:31-44`) accepts an artifact only if its
`abs_path` is a lexical prefix of some entry in `ctx.workspace.roots`:

```rust
if !ctx.workspace.roots.iter().any(|r| abs_path.starts_with(&r.path)) {
    return Err("… outside every workspace root …");
}
```

`ctx.workspace` is the **boot-time** workspace loaded by `build_tool_context()`
(`src/librarian/mod.rs:33-39`) from `LIBRARIAN_WORKSPACE` / `workspace::default_config_path()`
→ the global `~/.config/librarian/workspace.toml`. That file still uses the **legacy
`[[roots]]`** registry (10 entries: code-explorer, personal, southpole, …) and **does not
list codescout**. The active project is resolved *separately* into `ctx.current_project`
(CWD/git-root in `build_tool_context`, canonicalized in `LibrarianAdapter::derive_ctx`,
`src/librarian/adapter.rs:92`) — but the delete guard **never consults `current_project`**.

So codescout's artifact `abs_path` (correct, absolute — catalog column =
`/home/marius/work/claude/codescout/docs/...`) is a prefix of **none** of the 10 legacy
roots → the guard always rejects.

**The stored `abs_path` is NOT relative.** The error message *prints* a relative path
(`docs/...`) only because the tool output layer relativizes displayed paths against the
project root; the raw `abs_path` column and `artifact::get` both return the absolute form.

`artifact(action="move")` shares the identical guard (`src/librarian/tools/mv.rs:24-28`,
`ctx.workspace.roots.iter().find(...)`) → **same latent bug**.

**Blast radius:** `delete` and `move` fail for *every* project absent from the legacy
global `[[roots]]` list — codescout included. The 10 listed projects still work, which is
why it went unnoticed (delete is rare; archiving uses `move` on listed projects).
## Evidence

- **Clean serial reproduction (2026-06-03):** create+delete of `docs/_delete_probe_tmp.md` in
  the parent context failed identically — no subagents involved.
- **Stored `abs_path` is absolute:** catalog column =
  `/home/marius/work/claude/codescout/docs/_delete_probe_tmp.md` (`sqlite3 … quote(abs_path)`).
- **Guard source:** `src/librarian/tools/delete.rs:31-44`; `mv.rs:24-28` (same pattern).
- **`ctx.workspace` source:** `build_tool_context()` `src/librarian/mod.rs:33-39` loads the
  global `~/.config/librarian/workspace.toml` (10 `[[roots]]`, codescout absent).
- **0 of 2814 catalog rows have a non-absolute `abs_path`** — disproves the systematic
  relative-path theory.
- Original incident (3 subagent-created trackers, 2026-06-03) — forensic trail removed during
  cleanup; superseded by the serial repro above.
## Hypotheses tried
None — observed during cleanup, logged at user request. The `rm` + reindex was a
workaround, not an investigation.

## Fix

**Implemented + verified live 2026-06-03** (branch `feat/per-request-workspace-pinning`).
The create+delete probe that previously failed now succeeds through the running server; the
file and catalog row are dropped with 0 orphan augmentation rows. Status stays `open` until
the fix ships to `master` (archive move happens then).

- **Shared guard helper** in `src/librarian/tools/mod.rs`: `managed_roots(ctx)` returns the
  legacy `workspace.roots` **plus** `current_project.git_root` and `abs_path`;
  `containing_root(roots, abs_path)` does the lexical prefix match. Both `delete` and `mv` call
  it now, so the two guards cannot drift.
- `delete.rs` and `mv.rs` rewired to the helper; error wording → "outside every **managed** root".
  `mv` joins `new_rel_path` against the matched root (`git_root` ordered before `abs_path`, so a
  repo-root-relative path resolves against the repo root, not a project subdir).
- **Correction to the original sketch (step 2 — "canonicalize both sides"):** I did *not*
  `canonicalize()` `abs_path` at call time. `delete` tolerates an already-removed file and
  `std::fs::canonicalize` errors on a missing path — canonicalizing would reintroduce a failure.
  Stored `abs_path` is already canonical-absolute (upsert canonicalizes) and `current_project`
  is canonicalized at the adapter boundary, so a lexical `starts_with` over the expanded root
  set is sound.
- Orphan-augmentation cleanup (old step 3) remains out of scope — 0 orphan rows currently exist.
## Tests added

Three regression tests, all green (`cargo test --lib librarian::tools::` → 345 passed; clippy clean):

- `delete::tests::delete_succeeds_for_active_project_absent_from_legacy_roots` — the exact bug:
  empty `workspace.roots`, project in `current_project`; delete now succeeds.
- `delete::tests::delete_refuses_artifact_outside_all_managed_roots` — safety property held: no
  legacy root and no active project → delete refused, file left intact.
- `mv::tests::move_succeeds_for_active_project_absent_from_legacy_roots` — same fix on the `mv`
  path; `new_rel_path` resolves against the active project's `git_root`.

The existing `mk_ctx` helpers were the blind spot: both put `tmp` in `workspace.roots` with
`current_project = None`, the inverse of the real `[[project]]`-model runtime — which is why the
bug shipped despite test coverage.
## Workarounds
- `rm` the file(s), then `librarian(action="reindex", force=true, scope="project")` to
  drop the now-missing-file artifact rows. **Caveat:** this leaves orphan augmentation
  rows in the local catalog (no API cleans them once the artifact row is gone); they are
  local-only (the catalog DB is gitignored) and harmless to the repo.
- To avoid: do not have parallel subagents create artifacts the parent will need to
  delete; or delete from within the creating subagent's context.

## Resume
1. Reproduce per the hypothesized recipe; **before any cleanup**, capture the stored
   `abs_path` of the un-deletable artifact directly from the catalog DB
   (`sqlite3 ~/.local/share/librarian/catalog.db "SELECT id, abs_path FROM artifacts WHERE id='<id>'"`)
   — that is the missing forensic datum.
2. Compare against the delete-guard workspace-root check; confirm the canonicalization
   mismatch.
3. Inspect `abs_path` assignment in `artifact(action="create")` under a subagent context.
4. Consider the doctor/reindex orphan-augmentation cleanup (Fix step 3).

## References
- `docs/evals/augmented-tracker-discovery.md` § "Run 1" — observation source.
- CLAUDE.md § "Concurrent multi-workspace: one server, one active project" — the
  shared-server / process-global active-project hazard the hypothesis links to (see also
  the 2026-05-30 shared-server-global-active-project-race bug).
