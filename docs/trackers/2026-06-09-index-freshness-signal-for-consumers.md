---
id: '286ac62b5a821cec'
kind: tracker
status: active
title: Index freshness signal for external consumers (re-enable companion auto-reindex)
owners: []
tags:
- retrieval
- companion
- freshness
- phase-7
topic: null
time_scope: null
---

**Created:** 2026-06-09 · **Status:** draft

Re-enable the codescout-companion plugin's session-start **auto-reindex** (and,
optionally, **drift warnings**) by exposing a cheap, out-of-process **index
freshness signal** from the Qdrant-era retrieval stack. The sqlite-vec surface
the companion used to read was frozen/removed by the Phase 6/7 retrieval-stack
migration, and nothing a shell hook can read replaced it.

## Background — what the companion did, and why it broke

The companion's `hooks/session-start.sh` had two features that depended on the
legacy sqlite-vec store at `.codescout/embeddings.db`:

1. **Auto-reindex on stale** — read `meta.last_indexed_commit`, compared it to
   `git rev-parse HEAD`, and if behind spawned `codescout index --project <cwd>`
   in the background. Purpose: keep the index current after the working tree
   moves ahead via *external* changes (git pull, branch switch, edits made
   outside codescout) that codescout's own on-edit reindex never sees.
2. **Drift warnings** — read the `drift_report` view (`file_path`, `max_drift`)
   and surfaced high-drift files at session start.

The migration broke both:

- **The path moved**: `.codescout/embeddings.db` (single file) →
  `.codescout/embeddings/project.db` (subdir). The companion still reads the old
  path, so both blocks are dead — the `[ -f "$DB_PATH" ]` guard is always false,
  masked by `2>/dev/null`, so the user sees nothing.
- **More fundamentally, `codescout index` no longer writes that store.**
  `src/main.rs:241` (`Commands::Index`) now drives `client.sync_project(...)`
  (Qdrant). `meta.last_indexed_commit` in `project.db` is written only by the
  now-dead legacy indexer (`src/embed/index.rs`), so it is frozen at the
  migration commit. Even re-pointing the companion at `project.db` would read a
  permanently-stale marker → every session reports "behind HEAD" → a reindex
  loop.
- This is the **consumer side of L-14** in
  `docs/trackers/2026-05-07-legacy-retrieval-removal.md` ("Stack search has no
  equivalent of the legacy `check_index_staleness` warning"). The contract is
  documented in `docs/state-protocol.md` (§ `.codescout/embeddings.db` schema —
  companion's read surface), which already states: *"when the legacy index
  goes, the companion's drift query and reindex trigger must be ported to the
  new stack's equivalents or removed."* Per state-protocol compatibility rule 2,
  removing this surface was a breaking change that needed a coordinated
  replacement; the consumer was never migrated.

## Not covered by "Auto-Reindex on Edit"

The existing `Auto-Reindex on Edit` spec
(`docs/superpowers/specs/2026-05-02-auto-reindex-on-edit-design.md`) re-embeds
files edited **through codescout write tools**, drained lazily at the next
`semantic_search`. Its "Out of Scope" explicitly excludes *external editor
edits* (deferred to a file watcher). The companion covers exactly that excluded
case at session boundaries — so this is a complementary signal, not a duplicate.

## Goal

Expose a stable, cheap (no full sync required) way for an out-of-process
consumer (a shell hook) to answer: **"is the index behind the working tree, and
by how much?"** — the Qdrant-era replacement for `meta.last_indexed_commit`.

## Options

| ID | Option | Shape | Notes |
|---|---|---|---|
| O-1 | Sidecar state file | codescout writes `.codescout/index-state.json` after each `sync_project` run: `{ last_indexed_commit, last_indexed_at, schema_version }` | **Cheapest for a session-start hook** — one file read, no process spawn. Mirrors how the companion already reads `cc_session_id` / `project.toml`. Recommended primary. |
| O-2 | `codescout index --status [--json]` CLI | New read-only subcommand printing `{ indexed, last_indexed_commit, head_commit, behind_commits, stale }` | Mirrors the MCP `index(action="status").git_sync` envelope, made callable from a shell hook. One process spawn per session. |
| O-3 | Qdrant payload `last_synced` + freshness query | Per-point timestamp + a small query path | Heaviest; worth it only if an in-search freshness banner (L-14 proper) is also wanted. |

Recommendation: **O-1** (sidecar) as the primary signal, optionally **O-2** for
on-demand checks. Either restores the companion's reindex trigger to a one-line
read.

## Acceptance

- An external consumer can read index freshness without opening internal DBs or
  running a full sync.
- Companion session-start auto-reindex fires only when genuinely behind HEAD,
  and the freshness marker advances after `codescout index`.
- Drift surface is either re-sourced from the new stack or formally dropped from
  the companion (decide as part of this work).

## Checklist

- [x] Decide O-1 vs O-2 (vs both) — **O-1 (sidecar)** chosen (2026-06-09)
- [x] Implement the chosen signal; write it on every `sync_project` completion — new `src/retrieval/index_state.rs` (write/read/`git_sync_status`); write-site is `sync_project` itself (the chokepoint), gated by `SyncOpts.record_index_state` set by the 3 project entry points (MCP tool, CLI `main.rs`, `bin/sync_project.rs`); library syncs leave it false. `git_sync` revived in `IndexStatus::call`. NOTE: the initial `b5d63cb6` placed the write in `IndexProject::call` only — the CLI + bin paths silently wrote nothing (caught by a live CLI proof); fixed at the chokepoint in `10dcfb9f` (Snow Lion ADR). Verified: clippy `--all-targets -D warnings` clean; index_state lib tests + live `codescout index` (sidecar == HEAD) pass. Shipped on `experiments`: b5d63cb6 + 10dcfb9f.
- [x] Document the signal's path/format in `docs/state-protocol.md` — added `index-state.json` schema section + `.codescout/` table row; updated the legacy `embeddings.db` section to note the reindex-trigger port
- [x] Update `codescout-companion/hooks/session-start.sh` to read the new signal (claude-plugins repo) — auto-reindex block now reads `.codescout/index-state.json` via `jq` (was the frozen `embeddings.db` `meta`). `bash -n` clean; functional test (sidecar==HEAD → up-to-date; HEAD moved → behind=1) PASS. Committed at `claude-plugins:2f1a5b9` (`feat(codescout-companion): auto-reindex via index-state.json … bump 1.11.9`).
- [~] Resolve the companion-side blocks — auto-reindex block **resolved**; the **drift-report** block left inert (its `embeddings.db` no longer exists, so the `[ -f ]` guard skips it) pending a per-chunk-drift port or formal removal. Deferred by decision, not forgotten.

## Cross-references

- Tracker: `docs/trackers/2026-05-07-legacy-retrieval-removal.md` — item **L-14** (this is its consumer-facing requirement)
- Contract: `docs/state-protocol.md` — § `.codescout/embeddings.db` schema (companion's read surface); compatibility rule 2
- Spec: `docs/superpowers/specs/2026-05-02-auto-reindex-on-edit-design.md` — the complementary on-edit mechanism
- Consumer: `codescout-companion/hooks/session-start.sh` (~lines 96–145) in the `mareurs/claude-plugins` repo
