# State Protocol — Cross-Component Filesystem Contract

**Status:** open · **Owners:** codescout, codescout-companion, buddy · **Version:** 1 · **Tracker:** I-18

This document is the contract between the three cooperating components that share project
and user state through the filesystem:

- **codescout** — MCP server (Rust). Primary writer of `.codescout/`.
- **codescout-companion** — Claude Code plugin (bash hooks). Reader of `.codescout/`,
  writer of `cc_session_id`.
- **buddy** — Claude Code plugin (bash + Python). Owner of `.buddy/` and `~/.claude/buddy/`.

The components do not share a compiler. Schema changes that look local are interfaces
across plugin boundaries. **Every entry in this document is part of a public contract.**
Breaking changes require coordinated releases across the affected components.

## Why this document exists

Filesystem-as-IPC is the strongest form of coupling on this project: implicit, untyped,
runtime-only, distributed across processes that do not share a compiler. Without an
explicit contract, schema changes break silently across plugin boundaries — at runtime,
in production, with no diagnostic. This document names every shared path, its writer,
its readers, its schema, and its compatibility expectations.

It is also the prerequisite for safely retiring the `.code-explorer` / `.codescout`
backwards-compat fallbacks (tracker item I-10): we cannot delete a path until every
reader is documented and migrated.

## Compatibility rules

1. **Adding a new path or new field to an existing record is a minor change.** Readers
   ignore unknown fields. Writers may emit it without coordinated release.
2. **Renaming or removing a path or field is a breaking change.** Requires:
   - Updated entry in this document with a deprecation note and end-of-support version.
   - One release supporting both old and new (writers emit both, readers accept both).
   - Subsequent release removes the old.
3. **Changing the value type of an existing field is a breaking change.** Same protocol
   as renaming.
4. **The schema version of an artifact** (where one is defined, e.g. `.buddy/<sid>/state.json::version`)
   bumps with every breaking change. Readers reject unknown future versions and warn —
   they do not silently misinterpret.
5. **Plugins that read a path they do not own must tolerate absence.** Returning to a
   sane default is preferable to crashing; warning is preferable to silent fallthrough.

## `.codescout/` — per-project, codescout-managed

Primary writer: codescout server. Resides at `<project_root>/.codescout/`.

| Path | Writer | Readers | Purpose / Schema |
|---|---|---|---|
| `project.toml` | codescout `onboarding` tool, codescout config writes | codescout server (config load), companion `detect-tools.sh` (gates onboarding-prompt injection) | TOML config: `[project]` (name, languages), `[lsp.<lang>]` (mux toggle), `[security]` (shell_command_mode, indexing_enabled), `[memory]` (drift_detection_enabled). Schema versioned implicitly by codescout MSRV; field additions are minor. |
| `system-prompt.md` | codescout `onboarding` tool (regenerated when `ONBOARDING_VERSION` bumps) | companion `session-start.sh` (injects into Claude Code session) | Markdown. Free-form. Generated from `src/prompts/onboarding_prompt.md` + `builders.rs::build_system_prompt_draft`. Companion treats as opaque. |
| `memories/<topic>.md` | codescout `memory(action="write")` | codescout `memory(action="read"|"list")`, companion `session-start.sh` (lists names only — content not parsed) | Markdown with optional frontmatter for anchors. Topic path = filename. |
| `private-memories/<topic>.md` | codescout `memory(action="write", private=true)` | codescout only | Same as memories/, but auto-gitignored. Companion does not list. |
| `anchors/<topic>.toml` | codescout memory anchor system | codescout staleness check | TOML sidecars for memory↔source-file anchors. Internal to codescout. |
| `embeddings.db` (legacy) | codescout legacy index | codescout legacy retrieval, companion `session-start.sh` (queries `meta.last_indexed_commit` and `drift_report` view) | sqlite-vec virtual tables. **Being removed** — see `docs/trackers/2026-05-07-legacy-retrieval-removal.md` (L-01..L-15). Companion's drift query depends on `meta` table and `drift_report` view; when the legacy index goes, companion drift detection must move to the new stack or be removed. |
| `embeddings/project.db` | codescout `sync_project` binary, retrieval stack writers | codescout `semantic_search` | sqlite-format Qdrant per-project store (post-Phase 6). |
| `index-state.json` | codescout `sync_project` completion (`index_state::write_index_state`) | codescout `index(action="status")` (`git_sync` envelope); **companion** `session-start.sh` (planned, scope-b) | JSON `{ last_indexed_commit, last_indexed_at (RFC3339), schema_version }`. `last_indexed_commit` = full git HEAD oid at sync time (`""` for non-git roots). Qdrant-era replacement for the legacy `meta.last_indexed_commit` reindex trigger — lets an out-of-process consumer detect external HEAD moves (checkout/pull/HEAD change) via a single file read, no internal-DB access. |
| `embeddings/lib/<name>.db` | codescout library indexing | codescout library-scoped search | Per-registered-library embeddings. |
| `libraries.json` | codescout `library(action="register")` | codescout library scope resolution | JSON array of registered libraries with paths and versions. |
| `cc_session_id` | **companion** `session-start.sh` | codescout `usage` tracking | Plain-text Claude Code session UUID. **Cross-plugin write into a codescout-owned directory** — only companion writes here. Codescout reads it during usage.db correlation. |
| `tantivy/` (legacy) | codescout legacy keyword index | codescout legacy retrieval | **Being removed** with `embeddings.db`. |

### `.codescout/embeddings.db` schema (companion's read surface)

The companion reads two read-only surfaces of this database:

- `meta` table, key `last_indexed_commit` → string. Compared against `git rev-parse HEAD`
  to decide whether to trigger background reindex.
- `drift_report` view, columns: `file_path`, `max_drift`. Filtered by `max_drift > 0.1`
  to surface session-start drift warnings.

Both surfaces are **stable until L-01 ships and the legacy index is removed.** The
**reindex trigger** has now been ported to `.codescout/index-state.json` (see the next
section); the **drift-report** surface is still pending (port to per-chunk drift, or
remove from the companion).

### `.codescout/index-state.json` schema (freshness signal — supersedes the `embeddings.db` `meta` read)

Written by codescout on every successful project `sync_project` completion
(`src/retrieval/index_state.rs::write_index_state`); the Qdrant-era replacement for the
now-frozen legacy `meta.last_indexed_commit`.

```json
{
  "last_indexed_commit": "<full git HEAD oid; empty string when the project root is not a git repo>",
  "last_indexed_at": "<RFC3339 timestamp>",
  "schema_version": 1
}
```

- **Reindex trigger (consumer):** compare `last_indexed_commit` to `git rev-parse HEAD`.
  Behind ⇒ trigger a background `codescout index`. A single file read — no internal-DB
  access, no process spawn.
- **codescout `index(action="status")`** reads the same file and emits a
  `git_sync: { status, behind_commits, last_indexed_commit, head_commit }` envelope
  (`index_state::git_sync_status`); `behind_commits` is computed via
  `git2::graph_ahead_behind`.
- **Indeterminate freshness** (no sidecar, non-git root, recorded commit unresolvable) ⇒
  `git_sync` is omitted rather than reported as up-to-date.
- `schema_version` lets consumers degrade gracefully on a shape they don't recognise.

> Covers *external* changes (checkout / pull / HEAD move) only; edits made *through*
> codescout's own write tools are handled by the on-edit reindex
> (`docs/superpowers/specs/2026-05-02-auto-reindex-on-edit-design.md`). The two are
> complementary, not redundant.
## `.buddy/` — per-project, buddy-managed

Resides at `<project_root>/.buddy/`. Buddy is sole writer; nothing else writes here.

| Path | Writer | Readers | Purpose / Schema |
|---|---|---|---|
| `<sid>/state.json` | buddy hooks (`session-start`, `post-tool-use`, `user-prompt-submit`) | buddy statusline, `/buddy:*` slash commands | JSON `{version: 1, signals: {context_pct, last_edit_ts, last_commit_ts, session_start_ts, prompt_count, tool_call_count, last_test_result, recent_errors, idle_ts}, derived_mood, suggested_specialist, last_mood_transition_ts}`. **Schema versioned via top-level `version`.** Atomic writes via `mkstemp + os.replace` (iron rule). |
| `<sid>/narrative.jsonl` | buddy `post-tool-use.sh::accumulate_narrative` | buddy judge worker | JSONL, one entry per tool call: `{timestamp, type, text, ...optional fields}`. **Append-only, currently unbounded** — capping is tracker item I-03. |
| `<sid>/verdicts.json` | buddy plan-judge worker | buddy `pre-tool-use.sh`, statusline | JSON judge findings: `{verdict, severity, evidence, correction, affected_files}`. Refreshed every N tool calls. |
| `<sid>/cs_verdicts.json` | buddy cs-judge worker | buddy statusline (badge), `/buddy:check` | JSON, codescout-specific judge findings. Same shape as `verdicts.json`. |
| `<sid>/active_plan.json` | `/buddy:summon`, plan-detection logic | buddy hooks, judge | JSON: active plan metadata for judge prompt assembly. |
| `.current_session_id` | buddy `session-start.sh` | slash commands (`resolve_session_id_for_command`) | Plain-text current session UUID. Resolved with PPID fallback. |
| `by-ppid/<PPID>/session_id` | buddy `session-start.sh` | slash commands (PPID-resolved fallback) | Plain-text session UUID indexed by parent PID. GC'd when stored `started_at` doesn't match `ps -o lstart`. |
| `by-ppid/<PPID>/started_at` | buddy `session-start.sh` | session-start GC | Plain-text process start time (output of `ps -o lstart`). |
| `memory/INDEX.md` | buddy memory protocol | buddy memory dedup, future-summon load | Markdown index per memory protocol. Sole catalog of project memories. |
| `memory/<specialist>/<slug>.md` | buddy `/buddy:remember`, autonomous mid-turn writes | buddy summon (loads when specialist is summoned) | Markdown with frontmatter `{specialist, scope: project, slug, created, updated, tags}`. Body contains lesson + reasoning. |
| `memory/common/<slug>.md` | same | loaded for every summoned specialist | Same shape as per-specialist; stored under `common/` for cross-buddy lessons. |

**No cross-component reads of `.buddy/`.** Buddy is fully encapsulated within its own
namespace. Codescout-companion does not touch this directory.

## `~/.claude/buddy/` — global, buddy-managed (mirrored to `~/.claude-sdd/buddy/`)

| Path | Writer | Readers | Purpose / Schema |
|---|---|---|---|
| `identity.json` | buddy statusline (`bones.py::roll_form`) | buddy statusline | JSON `{user_id, form, label, eyes_for_mood}`. Form binding deterministic via FNV-1a + mulberry32 from user-id. |
| `memory/INDEX.md` | buddy memory protocol | buddy memory dedup, future-summon load | Same shape as project INDEX. |
| `memory/<specialist>/<slug>.md` | buddy `/buddy:remember --global`, autonomous writes | buddy summon | Same shape as project memories; mirrored to `~/.claude-sdd/buddy/memory/<specialist>/<slug>.md` if multi-instance configured (`buddy/data/instances.json`). |
| `memory/common/<slug>.md` | same | loaded for every summoned specialist | Same. |
| ~~`state.json`~~ | (deprecated) | — | **Removed.** `session-start.sh` performs one-shot deletion if found. Was per-user state before per-session refactor. |

**Mirror discipline:** Global writes call `mirror_global_write()` from `scripts/memory.py`.
INDEX regeneration on the mirror is lazy — performed when the mirror instance next summons
a specialist.

## Cross-component touch surface (the seam)

The only paths written by one component and read by another are:

| Path | Writer → Reader | Frequency | Failure mode if contract drifts |
|---|---|---|---|
| `.codescout/cc_session_id` | companion → codescout | Once per Claude Code session start | codescout `usage` tool fails to correlate sessions; stats are noisy but not broken. |
| `.codescout/system-prompt.md` | codescout → companion | Once per onboarding (rare) + read on every session start | If codescout changes the format from markdown to something else, companion injects garbage into Claude Code. **Treat content as opaque — only existence is the contract.** |
| `.codescout/memories/*.md` | codescout → companion | Listed on every session start | Companion reads filenames only. Filename rename: companion still works (lists whatever is there). Path layout change (e.g. moving to `memories/v2/`): companion stops listing → minor regression on session-start memory hint. |
| `.codescout/embeddings.db` | codescout → companion | Read on every session start | Schema break: companion's drift/reindex hooks fail silently. Currently mitigated by `2>/dev/null`; the user sees no warning. **If `meta.last_indexed_commit` or `drift_report` are renamed without coordination, companion drift warnings disappear.** |
| `.codescout/project.toml` | codescout → companion | Read on every session start | Companion checks `drift_detection_enabled` flag. Removal of this key: companion behaves as if drift detection is on (default-true assumption). |

Everything else is single-owner and not part of the cross-component contract.

## Versioning protocol — concrete steps for a breaking change

When a schema change is unavoidable:

1. **Bump the schema version.** For artifacts with explicit `version` (currently
   `.buddy/<sid>/state.json` only), increment. For artifacts without explicit version
   (everything else), the *change date* in this document serves.
2. **Update this document first.** Add a "Deprecated since v<X>" note on the old shape;
   add the new shape; specify the dual-write window.
3. **Writer changes go first.** New writer emits both old and new fields. Old readers
   continue to work.
4. **Reader changes follow.** New reader prefers new shape, falls back to old. Releases
   between writer and reader updates can ship in either order.
5. **Old shape removal is the last release.** Document the end-of-support version. Search
   the repo for old-shape references and confirm none remain in supported releases.
6. **Add or extend an integration test** in the affected component(s) that round-trips
   read→write→read across the version boundary.

## Integration test discipline

Each component should carry a `state_contract` test that round-trips its read+write
surface against fixtures matching this document:

- **codescout** — `tests/state_contract.rs` (proposed): assert `.codescout/project.toml`
  parses with all documented fields; `cc_session_id` is read as plain text; legacy
  `embeddings.db::meta::last_indexed_commit` query syntax stays valid.
- **codescout-companion** — `tests/state_contract.sh` (proposed): mock a `.codescout/`
  directory with documented files; run hooks; assert no errors and expected stdout.
- **buddy** — already has `tests/test_hook_helpers.py`, `tests/test_state.py`,
  `tests/test_data_catalogs.py`. Extend with explicit cross-version state.json roundtrip
  per the rule above.

Tests catch contract drift at CI time rather than at user-visible runtime.

## Backwards-compat fossils — explicit cleanup target

These exist today and **must be retired** as part of tracker item I-10. They are listed
here so I-10 has an authoritative checklist:

- `.code-explorer/` directory (legacy of `.codescout/` rename)
- `~/.claude/buddy/state.json` (legacy global state — `session-start.sh` deletes on first run)
- Routing config name fallbacks: `codescout-companion.json` (canonical) →
  `codescout-routing.json` → `code-explorer-routing.json` (legacy)
- Hard-coded `embeddings.db` schema reads (companion drift detection) — moves to new
  stack as legacy retrieval is removed.

When I-10 lands, each of these is removed in `codescout-companion` v2.0; this document
is updated to drop the fossil entries; and a one-time migration warning is printed if
the legacy path is detected.

## Known gaps in the contract (open work)

- The `.buddy/<sid>/narrative.jsonl` schema is implicit — every JSONL entry contains
  whatever `accumulate_narrative` chose to write. Tracker I-03 (rotation) is a good
  occasion to also pin a stable schema with required vs optional fields.
- Buddy memory frontmatter schema is documented in `buddy/data/memory-protocol.md` but
  not formally validated. A `python3 -m scripts.validate_memory_frontmatter` pass would
  catch malformed frontmatter at write time.
- `.codescout/project.toml` field set is documented in the codescout config module but
  not duplicated here field-by-field. If this document grows divergent from
  `src/config/project.rs`, prefer the Rust definition; treat this section as a guide.

## Maintenance

This document is a living artifact. Append entries when new shared paths are introduced;
amend when shapes change; mark deprecations explicitly. The discipline mirrors the other
trackers in `docs/trackers/`: any session that touches a shared path must verify this
document is still correct before completing the change.
