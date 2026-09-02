---
id: '0578d8e4f59738c8'
kind: tracker
status: active
title: Resume queue — Workspace Pinning Phase 4b + Phase 5 (WP-N)
owners:
- marius
tags:
- resume-queue
- workspace
- concurrency
- locking
- regime-3
topic: per-request workspace pinning
entry_high_water_WP: 5
entry_prefix: WP
---

# Resume queue — Workspace Pinning Phase 4b + Phase 5 (WP-N)

Work left in the per-request workspace pinning stream.

**Plan:** `docs/plans/2026-05-30-per-request-workspace-pinning.md` — still
`status: draft` **on purpose**. Its own § *Phase 4b — DEFERRED* says: *"This plan
stays active (do NOT archive) while 4b is open… At ship time, optionally promote
this section to a standalone `docs/trackers/` entry if the plan would otherwise
be archived."* This file is that promotion. The plan remains the authority on the
lock-ordering proof and the re-scout recipe; do not duplicate them here.

## Shipped, so nobody re-does it

Verified at the bytes 2026-08-28:

- **Phases 0–3** — registry, `workspace_override` on `ToolContext`, multi-residence,
  read-tool migration.
- **Phase 4a** (writes) — `workspace_override` reaches every write tool:
  `src/tools/symbol/edit_code.rs`, `src/tools/edit_file/mod.rs`,
  `src/tools/create_file.rs`, `src/tools/markdown/edit_markdown.rs`,
  `src/tools/memory/mod.rs`. Regime-3 correctness is closed and was live-verified.
- **The lock-ordering proof** — committed `69c91896`, and it is the mandatory gate
  for WP-1. Read it before touching the field type.
- **Schema advertisement** — `inject_workspace_param` (`src/server.rs:626-638`)
  injects the optional `workspace` param into pinnable tools' schemas.

## How to use this queue

**To act:** scan the `## WP-N` headings — each carries a `**Status:**` line.
WP-1 and WP-2 are one focused session together; WP-3 and WP-4 are small and
independent of them.

**To append:** one call, from the main checkout —

```
artifact(action="append_entry", id="<this artifact's id>", id_prefix="WP",
         anchor_heading="## Template for new entries", title=…, body=…)
```

**Deliberately unaugmented** — see `docs/conventions/cross-machine-catalog-resume.md`.

## Provenance

Opened 2026-08-28 from a full-surface partial-implementation sweep, which
confirmed the plan's own deferral record against `src/` rather than assuming it.

## WP-1 — Flip `workspaces` to `Arc<RwLock<Workspace>>` — atomic, ~60 sites, one session

**Status:** deferred by explicit decision (user, 2026-05-31) — do not start without re-confirming the trigger
**Valid:** conditional — profiling shows different-root write serialization is a real bottleneck
**Rests on:** the plan's § *Phase 4 — Lock-Ordering Proof* (`69c91896`)

**Observed 2026-08-28.** `src/agent/mod.rs:102` still reads:

```rust
pub workspaces: HashMap<PathBuf, Workspace>,
```

— plain values, so all writes still serialize on the single `AgentInner` write
lock via `with_project_at_mut`.

**What it buys, precisely.** Only concurrent **cross-workspace writes**. Reads
already don't serialize (shared read lock); same-root writes serialize on
`write_lock` regardless. That is a narrow case, which is why it was deferred.

**Why it is one session and not incremental.** Flipping the field type yields 12
first-layer `cargo check` errors, cascading to ~60 sites across 11 files once the
sync accessors change *signature* — `default_workspace() -> Option<&Workspace>`
and `active_project() -> Option<&ActiveProject>` cannot return references from
behind an `Arc<RwLock>`. The tree will not compile until all ~60 are migrated, so
**there is no safe mid-way checkpoint**.

**Shape the proof obligates:** closure accessors clone the
`Arc<RwLock<Workspace>>` under a brief registry-map read lock, **release the map
lock**, then `.read()/.write().await` the per-`Workspace` lock and run the
closure. The sync accessors must be **removed**, not adapted. Order is
registry-map L0 → per-`Workspace` L1 → `write_lock` L2 → flock L3; never
block/await on a per-entry lock while holding a registry lock.

**Next:** re-run the reversible re-scout recipe in the plan before committing to
it — the ~60-site figure was measured 2026-05-31 and the tree has moved a long
way since.

## WP-2 — Eviction sweep: pinned non-default workspaces stay resident until restart

**Status:** open — the other half of 4b, and the half with an observed symptom
**Valid:** dated 2026-08-28

**Observed.** No eviction sweep exists. A pinned, non-default workspace stays in
the registry for the life of the process — observed live 2026-05-31 with the
`mirela` workspace.

**Currently harmless**: registry entries are lightweight and LSP clients
self-evict on their own TTL. But it grows unbounded over a long session that pins
many distinct workspaces, and this repo now routinely runs multi-workspace
sessions.

**Design already settled** (plan § *Lifetime contract*): idle-TTL reusing the LSP
pool's `idle_timeout_secs`; quiescence = every activated project has
`write_lock.try_lock()` free **and** `dirty_files` empty; `default_workspace_root`
is exempt. Eviction quiescence uses `try_lock()`, never `.lock().await` — that is
a clause of the lock-ordering proof, not a style choice.

**Next:** this is separable from WP-1 and cheaper. It can land against the
current plain-`Workspace` map, since quiescence is checked under the registry
lock either way. Consider doing it first.

## WP-3 — Phase 5: the `workspace` param is in the schemas but in none of the prose surfaces

**Status:** open — small, independent of 4b
**Valid:** dated 2026-08-28

**Observed.** `inject_workspace_param` (`src/server.rs:626-638`) puts the param in
24 tools' schemas, but `src/prompts/source.md` never documents it — the only
`workspace` hits there are the repo/multi-project concept, not the per-call pin.

So a model learns the param exists only by reading a tool schema, never from the
instruction surfaces that teach *when* to reach for it. For a param whose entire
purpose is concurrent-subagent correctness, that is the wrong way round.

**Next:** update the three prompt surfaces (`src/prompts/source.md`
`server_instructions` + `onboarding_prompt` slices, and `build_system_prompt_draft()`
in `builders.rs`), then **bump `ONBOARDING_VERSION`** — param semantics reach the
onboarding surface, which is the documented trigger. Mind the 1900-**character**
slice cap; see `src/prompts/README.md`.

## WP-4 — Phase 5: `concurrent_activation_warning` is still live, and its retirement is unresolved

**Status:** open — needs a decision, not just code
**Valid:** dated 2026-08-28

**Observed.** The guard is still emitted: `src/tools/config/mod.rs:316` inserts
`concurrent_activation_warning` into the `activate` response, with the decision
in `Agent::concurrent_switch_warning` and the record at `src/agent/mod.rs:116`.

The plan's Phase 5 says to **remove** it for pinned flows, because pinning
supersedes the mitigation it provides. But it also leaves an open question the
plan never resolved: what happens to **unpinned** calls under concurrency —
declare that unsupported, or retain the warning solely on the
`default_workspace_root` path?

Two archived bug files land on "keep it on the unpinned path":
`docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md` and
`docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md`.

**Next:** make the call explicitly and record it, then scope the removal to
pinned flows only. Do not delete the guard wholesale — the unpinned path is still
racy by construction.

## WP-5 — Open bug in this subsystem: `read_only` flips mid-session with no `activate`

**Status:** investigating — tracked as a bug, listed here for adjacency
**Valid:** dated 2026-08-28

`docs/issues/archive/2026-08-26-workspace-read-only-flips-mid-session.md`
(`6a3bb4d968d1d514`): workspace `read_only` flipped to `true` twice mid-session
with no `activate` call from the session, silently blocking every write.

Listed here because it lives in exactly the registry/activation machinery WP-1
would restructure. **Read it before starting WP-1** — if the mechanism turns out
to be registry-state related, WP-1's migration is either the fix or the thing
that makes it much harder to find.

**Next:** the bug file owns the investigation. This entry is a cross-reference,
not a duplicate — do not update status here.

## Template for new entries

```
## WP-N — <one-line title>

**Status:** open | in-progress | done | deferred
**Valid:** dated YYYY-MM-DD | invariant | conditional — <event>

**Observed.** <what you ran, and what it returned>

**Next:** <the concrete action>
```

## History

### 2026-08-28 — opened

Promoted from the plan's § *Phase 4b — DEFERRED* resume kit, as that section
itself authorises. Phase 4a, the lock-ordering proof, and the still-live
`concurrent_activation_warning` were all re-verified against `src/` that day.
