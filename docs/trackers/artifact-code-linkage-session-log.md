# Artifact-Code Linkage — Session Log

> **Purpose:** Two-sided observation log for the multi-session work of
> mapping how artifacts in this project reference, validate, or store
> code-state pointers (file paths, symbols, git SHAs, gather sources).
> Captures frictions (F-N) and wins (W-N) discovered while probing the
> four artifact↔code channels: `gather_from`, `audit_doc_refs`,
> `evidence_commits`, `anchor_commit`.
>
> **Scope:** Anchored at commit `844ebc1e`. Started 2026-05-17.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-05-17 | med | architectural | fixed-verified | Augmentation prompt references `context.git_log` but augmentation row has no `gather_from: git_log` source — silent drift (instance fixed; F-2/F-3 structural) |
| F-2 | 2026-05-17 | med | architectural | open | Archetype designs in `tracker_design.rs` supply no `gather_from` defaults (root of F-1) |
| F-3 | 2026-05-17 | med | architectural | open | 1 of 4 augmented artifacts uses the `gather_from` channel in production (was 0/4; goal-tracker re-augmented with gather_from post-recovery) |
| F-4 | 2026-05-17 | low | cross-repo | open | Stored SHAs (`evidence_commits`, task notes) carry no repo-scoping field |
| F-5 | 2026-05-17 | med | codescout-tool | fixed-verified | `state_at(commit=...)` short-SHA lookup (#32, commit `2f085f45`) — verified `d482ca8a` resolves + ambiguity guard fires |
| F-6 | 2026-05-17 | high | codescout-tool | fixed-verified | `librarian(reindex)` UNIQUE constraint + dim mismatch (default + force) — bug-tracker #5/#6, verified post-rebuild |
| F-7 | 2026-05-17 | low | architectural | open | `last_changed` is single-purpose (gather_config_value only) and per-line, not per-symbol |
| F-8 | 2026-05-17 | med | architectural | fixed-verified | 6 of 8 codescout memories anchored to deleted/moved files post-dissolve refactor (refreshed via `memory(refresh_anchors)` per topic) |
| F-9 | 2026-05-17 | high | codescout-tool | fixed-verified | `librarian(reindex, force=true)` cascade-delete eliminated (commit `d482ca8a`); 4 augmentations preserved post-test |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-05-17 | med | `gather_goal_children` deterministic kernel resolves child statuses without LLM inspection | Same shape as F-9 — LLM would re-derive child status from each child's body text every refresh, costing N+1 round trips per goal-tracker | validated |
| W-2 | 2026-05-17 | med | Artifact link graph is artifact-only by design; no edges point at code | Every refactor would either break edges or require coordinated cross-repo updates — F-13/F-14 fault line at a different layer | validated |
| W-3 | 2026-05-17 | med-high | `audit_doc_refs::merger` encodes 5 lifecycle invariants (stable PK, wontfix preservation, severity escalates only, fixed/regression transitions) | Each missing invariant is a category of re-scan bug — merger would silently overwrite human decisions or accumulate noise | validated |

---

## Category conventions

Inherits the template's category vocabulary. This log primarily uses:

| Category | When to use here |
|---|---|
| `architectural` | Structural finding about how artifact↔code linkage is *designed* (e.g. a channel that's stored but not validated by intent) |
| `codescout-tool` | Friction in an artifact / librarian tool surfaced during a probe |
| `tracker-instance` | Drift on a specific artifact's params or augmentation row (not a tool defect) |
| `cross-repo` | Behavior at the project boundary — SHA/path references that span repos |

---

## F-1 — Augmentation prompt cites `context.git_log` but row has no `gather_from: git_log` source

**Observed:** 2026-05-17, while running Demo 2 of the artifact→code probe
(`artifact_refresh(action=gather, id=d2cd00fc837e53f2)`).

**When:** Verifying whether the goal-tracker's live `gather_from` channel
actually pulls git commits when the prompt instructs the LLM to use them.

**Expected:** `context.git_log` populated with commits since the last
refresh (the prompt body explicitly says: *"`gather_from: git_log` should
set `since: \"last_refreshed_at\"` so `context.git_log` only carries
commits after `refresh_meta.last_refresh_at`"*).

**Got:** Gather response has no `git_log` key. `refresh_meta.commit_count_since_last:
0`. The augmentation row's gather config has no `git_log` source configured —
only `Artifacts` (via `gather_goal_children`) ran. The prompt promises
context the gather doesn't supply.

**Probable cause:** Same root as F-9 (in archived `i1-session-friction.md`):
the prompt template and the gather config are stored separately on the
augmentation row. When `archetype_goal()` (`src/librarian/tools/tracker_design.rs`)
specifies both, only the prompt half propagates verbatim to new artifacts;
the gather config is filled in by the creator at `artifact(action=create,
augment=...)` time. If the creator forgets the gather half, the prompt
makes promises the row can't keep.

**Workaround:** Re-augment with the missing source:
`artifact_augment(id=<goal-id>, merge=true,
params={gather_from: [..., {source: "git_log", since: "last_refreshed_at",
limit: 20, grep: "<criterion-path-pattern>"}]})`. The merge=true preserves
prompt + other params per F-15 / F-16 fixes.

**Severity:** med — silent. The LLM dutifully writes `evidence_commits: []`
because `context.git_log` is empty, even when commits *did* land touching
the criterion's paths. No error fires. The user only notices the gap by
inspecting the augmentation row directly.

**Status:** fixed-verified (instance only — 2026-05-17 third session, post-F-9 recovery)

**Verification:** After re-augmenting `d2cd00fc837e53f2` via `artifact_augment(merge=false, prompt=..., params={..., gather_from: [{source: "git_log", since: "last_refreshed_at", limit: 30, grep: "goal|i1|librarian|hamsa|pika|archetype|gather"}]})`, `artifact_refresh(action=gather)` returned `hints: ["30 items gathered from git_log"]` and `context.git_log` populated with 30 matching commits. The pipeline works when configured. Note: this fix is **instance-only** — F-2 (no archetype defaults) and F-3 (adoption count) remain structural.

**Fix idea / Pointer:** Routes to existing design tracker
`docs/trackers/augmentation-prompt-template-resolution.md` — second
concrete of the same fault line as F-9. The design tracker's Option 2
(archetype-name field on augmentation row + Rust-resolved template) would
also need to cover gather-config defaults, not just prompt. May warrant
amending that tracker's Promote-when to count this as concrete #2.

---

## W-1 — `gather_goal_children` deterministic kernel resolves child statuses without LLM inspection

**Observed:** 2026-05-17, Demo 2 of the artifact→code probe.

**Pattern:** The Rust kernel function `gather_goal_children`
(`src/librarian/tools/gather.rs:179-260`) reads each linked child's
augmentation params, runs archetype-specific predicates from
`goal_aggregation::child_status_pure`, and surfaces a status per child in
`context.deterministic_child_statuses`. The LLM then *copies* these
verbatim (per rule 1a of the goal archetype prompt) — no re-derivation,
no body-text reading.

**Counterfactual:** Without this kernel, the goal archetype prompt's
rule 1 would have to instruct the LLM to make N `artifact(action=get)`
calls (one per child), parse each child's body / params manually, and
synthesize a status. That's N+1 round trips per refresh plus N LLM
interpretations of free-form text — observed pre-Phase-1 in the
dogfood-log DF-1 entry ("Each goal-tracker refresh is N+1 round trips").
Today's measurement: **3/3 children resolved deterministically in a
single gather pass** with `basis: "deterministic"` for all three.

**Confirming data points:**
1. Demo 2 output — 3 children, all `basis: deterministic`, statuses
   match the artifacts' actual params.
2. DF-1 in `goal-tracker-dogfood-log.md` documents the pre-Phase-1 N+1
   shape that this pattern eliminated.
3. T-3 in `2026-05-17-i1-refactor.md` shipped the kernel and gather-time
   injection — observed in production for this artifact.

**Impact:** med. Eliminates per-refresh round-trip cost; makes
goal-trackers usable at multi-child scale without latency blowup.

**Promote-when:** The pattern is already documented in
`src/prompts/source.md` § Goal-trackers. Win is *validated and in
production*. Status flip to `promoted-to-permanent-docs` is justified —
the live prompt surface already encodes it.

**Status:** validated (candidate for `promoted-to-permanent-docs` flip)

---

## F-2 — Archetype designs in `tracker_design.rs` supply no `gather_from` defaults

**Observed:** 2026-05-17, while probing root cause of F-1.

**When:** Searching `tracker_design.rs` for any archetype that seeds a `gather_from` field on creation.

**Expected:** At least the `archetype_goal()` design (which references `gather_from: git_log` in its prompt body) would supply a default gather config so newly-created goal-trackers come with the live channel pre-wired.

**Got:** `grep '"gather_from"' src/librarian/tools/tracker_design.rs` returns **zero matches**. None of the six archetypes (`deployment_state`, `failure_table`, `metric_baseline`, `audit_issues`, `task_list`, `reflective`, `goal`) emit a `gather_from` field in their design output. The string `gather_from` only appears inside `prompt_template` prose as advisory text to the creator.

**Probable cause:** Design choice: archetypes prescribe *shape* (`params_schema_example`, `prompt_template`, `body_skeleton`, `render_template_example`) but leave gather wiring to the creator. The prompt body is the only place gather is hinted; nothing in the create flow validates that the creator followed through.

**Workaround:** Manual re-augment per F-1 workaround. Or amend `tracker_design.rs` to emit a `gather_from` template that the creator can pass through `artifact(action=create, augment={..., params={gather_from: [...]}})`.

**Severity:** med — root cause for F-1. Affects every goal-tracker created via the current `tracker_design` path.

**Status:** open

**Fix idea / Pointer:** Same routing as F-1 — folds into `augmentation-prompt-template-resolution.md`. The structural fix (Option 2 or Option 3 in that tracker) needs to cover gather defaults alongside prompt template.

---

## F-3 — 0 of 4 augmented artifacts use the `gather_from` channel in production

**Observed:** 2026-05-17, while sampling all augmented artifacts in the project.

**When:** `artifact(action=find, augmented=true, limit=30)` returned 4 artifacts; checking each one's `augmentation.params.gather_from`.

**Expected:** At least the goal-tracker, given its prompt's explicit `gather_from: git_log` reference, would actually configure gather sources.

**Got:** All 4 augmented artifacts have **no `gather_from` field**:

| Artifact | Archetype | `gather_from` |
|---|---|---|
| `d2cd00fc837e53f2` | goal | absent |
| `0df5ebc95d284b8e` | audit_issues | absent |
| `4b6294bf495dbfb3` | reflective | absent |
| `64f10cc45d802a11` | task_list | absent |

Meanwhile the codebase ships full gather machinery: `gather_all`, `gather_git_log`, `gather_file`, `gather_grep`, `gather_config_value`, `gather_observations`, `gather_artifacts` — all in `src/librarian/tools/gather.rs`. The only gather actually invoked in production is `gather_goal_children`, which is **dispatched directly by archetype** (not via `gather_from` config) — it works *around* the empty config, not *through* it.

**Probable cause:** Combination of F-2 (no defaults) + the goal archetype's smart-dispatch in `refresh.rs::call`. The smart-dispatch hides F-2's impact for goal-trackers specifically; other archetypes get no equivalent fallback.

**Workaround:** None needed for goal-tracker (smart-dispatch handles children). For other augmented archetypes, manual `artifact_augment(merge=true, params={gather_from: ...})` per use case.

**Severity:** med — extent of F-2. Feature exists end-to-end in Rust but adoption is zero in declarative config. The live channel runs only via hardcoded dispatch, not via the configurable surface the API exposes.

**Status:** open (improved 0/4 → 1/4 post-F-9 recovery)

**Update 2026-05-17 (post-recovery):** The F-9 recovery re-augmented the goal-tracker with `gather_from: git_log` per F-1's workaround — so the count is now **1 of 4** augmented artifacts using gather_from. The other three (audit_issues, reflective, task_list) still don't use it because their archetype prompts don't reference gather sources. The structural finding (F-2) remains the bottleneck.

**Fix idea / Pointer:** Two routes — (a) fix F-2 and let adoption follow, or (b) retire the user-facing `gather_from` surface and document gather as archetype-dispatched-only. Folds into `augmentation-prompt-template-resolution.md` design conversation.

---

## F-4 — Stored SHAs (`evidence_commits`, task notes) carry no repo-scoping field

**Observed:** 2026-05-17, Demo 3 of the artifact→code probe.

**When:** Trying to resolve `0b75991` (cited in goal-tracker progress_log[0].evidence_commits) via `git show`.

**Expected:** A SHA stored in an artifact's params is verifiable against the repo whose state the artifact tracks.

**Got:** `git show 0b75991` → *"fatal: ambiguous argument '0b75991': unknown revision or path not in the working tree."* The note on the same row reveals the SHA belongs to `codescout-companion` (a sibling repo), not codescout. The artifact stored the SHA as a plain string with **no `repo` field, no qualified ref**. The same pattern appears in `64f10cc45d802a11.augmentation.params.tasks[].notes` — T-14's note says "Shipped 0b75991 in codescout-companion" but T-1's `71ea2fa7` has no qualifier; reader has to assume in-repo by default.

**Probable cause:** The `evidence_commits` schema is a `string[]` of short hashes, deliberately free-form (per the dont-fabricate-commit-rationale memory's note: "journal entry, not link"). The schema makes no provision for cross-repo references because the original use case was single-repo.

**Workaround:** Manual: prefix cross-repo SHAs with `<repo>:` in the note text (e.g. `"codescout-companion:0b75991"`). Not enforced; reader-of-the-future has to notice.

**Severity:** low — by design unvalidated, but lossy when work spans repos (which this session does — codescout + codescout-companion + buddy + claude-plugins). A reader following SHA citations across artifact archives will hit silent dead-ends.

**Status:** open

**Fix idea / Pointer:** Two possible: (1) add `repo` field to `evidence_commits` shape (breaking schema change); (2) document the convention `<repo>:<sha>` for cross-repo SHAs (zero-code, propagates via prompt template). Option 2 is the cheaper near-term — option 1 only earns its keep at the third concrete of cross-repo confusion. Currently 1 concrete (this one).

---

## W-2 — Artifact link graph is artifact-only by design; no `dst_id` points at a code resource

**Observed:** 2026-05-17, while running `artifact(action=graph, id=d2cd00fc837e53f2, depth=2)` and surveying the link schema.

**Pattern:** The `artifact_links` table edges are typed `(src_id, dst_id, rel)` where **both endpoints are artifact IDs**. No edge type connects an artifact to a file, symbol, line, or commit. Code references live exclusively in markdown body text (free-form), with `audit_doc_refs` as the validation layer.

**Counterfactual:** If edges could point at code symbols, every rename / move / delete would either break the edge or require a coordinated cross-repo update — exactly the F-13 / F-14 "shared resource, no read-act transaction" fault line at a different layer. The audit_doc_refs scan would be redundant; the link-table integrity check would be a pre-commit gate. Maintenance cost on every refactor would be much higher.

**Confirming data points:**
1. `artifact(graph)` on the dogfood goal-tracker returned 4 nodes / 3 edges, all `rel=child`, all artifact→artifact.
2. `grep 'gather_from' tracker_design.rs` returned zero matches — confirming no archetype prescribes code anchors via the link channel.
3. `audit_doc_refs` exists as a separate after-the-fact scanner — direct evidence that the design intentionally separates artifact identity from code drift detection.

**Impact:** med. The decoupling preserves artifact stability across code churn. The cost is that artifact→code linkage is *softer* (gather + audit) rather than *harder* (link edges), which means the F-1..F-4 frictions of this session are about the soft channel's calibration, not about a hard channel's brittleness.

**Promote-when:** Already implicit in the schema design. Worth surfacing in `docs/ARCHITECTURE.md` if the soft-vs-hard distinction isn't already explicit there. Promote-when: someone proposes adding artifact→symbol edges (the design tracker `multi-agent-concurrent-coordination.md`'s Option B is a near-miss — that's artifact→artifact for a coordinator, not artifact→symbol).

**Status:** validated

---
## F-5 — `state_at(commit=...)` channel is broken; `commits` table is empty

**Observed:** 2026-05-17, Probe 1 of the deep dive (post-MCP-restart).

**When:** Testing `artifact(action=state_at, artifact_id=..., commit=<sha>)` on the goal-tracker. Tried both a recent commit (`844ebc1e`) and an older one (`2005d9fa`).

**Expected:** Returns the artifact's state as of that commit's authored timestamp, per `replay_state_at`'s design.

**Got:** Both calls fail with: *"commit <sha> not indexed; run librarian_reindex"*. Tracing the error: `resolve_cutoff_ts` looks up `SELECT authored_at FROM commits WHERE hash=?1` and fails when the `commits` table has no row. The `timestamp=<ms>` variant works fine; only the SHA-anchored channel is broken.

**Probable cause:** The `commits` table is populated by `backfill_commits` in `src/librarian/tools/reindex.rs:182`, called from `reindex::call` for each target. But the call is wrapped in error suppression: `if let Err(e) = backfill_commits(&cat, abs_root) { tracing::debug!("backfill_commits skipped for {}: {}"); }`. A real backfill failure (git2 binding error, permissions, etc.) gets swallowed; the reindex returns success and the user has no signal commits weren't indexed.

**Workaround:** Use `timestamp=<unix-ms>` instead of `commit=<sha>`. The replay logic works the same; only the SHA→timestamp lookup is broken.

**Severity:** med — the commit-anchored time-travel is a documented feature (artifact-tool schema lists `commit` as an alternative to `timestamp`) that doesn't work in practice. Silent failure path on backfill compounds the issue.

**Status:** fixed-verified (2026-05-17 post-Round-3 rebuild — commit `2f085f45`)

**Update 2026-05-17 (post-d482ca8a rebuild):** Original diagnosis was WRONG. The commits table is **not** empty — it has 2931 rows including all session-this commits. The actual bug is in `resolve_cutoff_ts` (`src/librarian/tools/state_at.rs:30`): the query uses `WHERE hash = ?1` with exact match, but callers pass short 8-char SHAs while the stored hashes are full 40-char. So `state_at(commit="d482ca8a")` fails but `state_at(commit="d482ca8ac91241a7a96a487e46ca394095019912")` succeeds. The error message "commit not indexed; run librarian_reindex" is misleading because it implies the table is empty when really the lookup mode is wrong. **Fix:** change `=` to `LIKE ?1 || '%'` (or `glob`/prefix match) so short SHAs resolve. The session-log finding of "silent error swallow in `backfill_commits`" (which #25 partially addressed) was a real but secondary concern.

**Fix idea / Pointer:** Two parts. (a) Surface backfill errors instead of swallowing them — the `tracing::debug!` should be at minimum `tracing::warn!` with a counter in the reindex response. (b) Investigate why backfill is failing on this project (see F-6 — the reindex itself has other failures preventing diagnosis).

**Verification 2026-05-17 (post-`2f085f45`):** `state_at(commit="d482ca8a", artifact_id="0df5ebc95d284b8e")` → `as_of: 1779038809000` (pre-fix: "not indexed" error). Ambiguity guard: `commit="d"` → `"ambiguous (matches at least dd43... and d9e2...); use a longer prefix or the full 40-char SHA"`. Both the success path (short SHA resolves) and failure path (ambiguous prefix produces actionable error naming conflicts) verified live.

---

## F-6 — `librarian(action=reindex)` has two stacked failure modes on this project

**Observed:** 2026-05-17, while trying to populate `commits` (F-5 workaround attempt).

**When:** `librarian(action=reindex, scope=project)` and `librarian(action=reindex, scope=project, force=true)`.

**Expected:** Walks the project's markdown, upserts artifact rows, backfills `commits` table. Returns added/updated/removed/unchanged counts.

**Got:**
- **Without `force=true`:** *"UNIQUE constraint failed: artifact.abs_path"* — the indexer tries to insert a row whose `abs_path` already exists. The default upsert path doesn't handle re-walk + path collision.
- **With `force=true`:** *"Dimension mismatch for inserted vector for the "embedding" column. Expected 768 dimensions but received 1."* — the embedder returned a 1-element vector instead of 768. Workspace status confirms model is `jina-embeddings-v2-base-code` (768-dim); something in the embedding service is returning a sentinel value (likely an error fallback) that the writer doesn't gate against.

**Probable cause:**
- Default path: indexer's path-uniqueness check doesn't account for prior walks having left rows; UPSERT or skip-on-collision logic missing.
- Force path: embedding service silently returns malformed vectors on error; writer doesn't validate dimensions before INSERT.

**Workaround:** None known short of manual SQL surgery on the catalog DB.

**Severity:** high — reindex is a foundational operation. With it broken, the `commits` table cannot be backfilled (F-5), libraries cannot be indexed (0/62 per `workspace(status)`), and any artifact whose canonical row drifted from the filesystem cannot be reconciled.

**Status:** fixed-verified (2026-05-17 post-rebuild — bug-tracker #5 + #6 + #7 closed)

**Verification (post-d482ca8a rebuild):**
- `librarian(reindex, scope=project)` now succeeds: `added: 0, updated: 4, removed: 0, unchanged: 493, backfill_error_count: 0`. Was failing with `UNIQUE constraint failed: artifact.abs_path` pre-fix. ✅
- `librarian(reindex, scope=project, force=true)` now succeeds AND preserves augmentations: post-call `artifact(find, augmented=true)` still returns 4 artifacts. Was cascade-deleting them pre-fix. ✅
- F-6b dim-validation code path verified via the unit test suite (2329 passed); live trigger not reproduced today because the embedder isn't currently failing.

**Fix idea / Pointer:** Separate bug-tracker entries warranted. Promote to `docs/issues/bug-tracker.md` as #5 (reindex UNIQUE) and #6 (embedding dimension validation).

---

## F-7 — `last_changed` is single-purpose (gather_config_value only) and per-line, not per-symbol

**Observed:** 2026-05-17, Probe 3 reading `src/librarian/tools/gather.rs:436`.

**When:** Checking whether artifact freshness can tie to file mtime / last-touched commit as a general primitive.

**Expected:** A reusable "when was this thing last touched" query exposed as a `GatherSource` variant or general utility.

**Got:** `last_changed` exists but is called only from `gather_config_value` at line 495 (single caller per `references()`). Implementation uses `git2::blame_file` and returns the most-recent line's `(commit_id, ISO-8601)`. **Per-line, not per-symbol or per-file** — returns last edit to ANY line in the file, no way to scope to a specific function. For LLM consumption, the granularity is wrong for code-symbol tracking.

**Probable cause:** Function was built for `gather_config_value`'s narrow use case (config file last-edit annotation), not designed as a general primitive. Hasn't been generalized because no second caller emerged.

**Workaround:** None needed today; if a caller wants per-symbol freshness, they need a different mechanism (LSP-backed or `git log -L`).

**Severity:** low — architectural observation, not a bug. The function does what it was designed for.

**Status:** open

**Fix idea / Pointer:** Per two-concretes-threshold: keep as-is until a second caller wants this. If/when an artifact-refresh path needs per-symbol freshness, lift the implementation then.

---

## F-8 — Memory anchors drift after refactors; 6 of 8 codescout memories anchor to deleted/moved files

**Observed:** 2026-05-17, side-finding from `workspace(action=status)` during reindex investigation.

**When:** Reading workspace status to verify embedder configuration.

**Expected:** Memory topics' anchors track the live filesystem; staleness signal fires on a small handful.

**Got:** **6 of 8 memory topics are stale:**
- `architecture`: 19 of 30 anchored files changed; deleted: `src/tools/markdown.rs`, `src/prompts/server_instructions.md`, `src/prompts/onboarding_prompt.md`, `docs/ARCHITECTURE.md`, `src/embed/index.rs`, `src/tools/memory.rs`
- `conventions`: 11 of 16 changed; deleted: `src/prompts/server_instructions.md`, `src/prompts/onboarding_prompt.md`
- `gotchas`: 10 of 10 changed; deleted: `src/embed/index.rs`
- `domain-glossary`: 8 of 17 changed; deleted: `src/embed/drift.rs`
- `development-commands`: 2 of 2 changed
- `project-overview`: 4 of 4 changed

The deletions are all from the librarian dissolve (`d48bf992`) and embed reorg — same refactor that left CLAUDE.md citing the wrong `src/prompts/server_instructions.md` path. **Same root cause as the side-finding flagged at the end of the archive-tracker commit (`844ebc1e`).**

**Probable cause:** Memory anchors are stored in `.codescout/system-prompt.md` and similar surfaces; they're populated at onboarding and not automatically migrated when source files move. The staleness *signal* works (workspace status surfaces all 6), but no automated remediation runs.

**Workaround:** Manual `memory(refresh_anchors, topic=...)` per stale topic, or `mcp__codescout__memory(write)` with new content pointing at the post-dissolve paths.

**Severity:** med — the staleness is surfaced but accumulates. Each future refactor adds more drift. Adjacent concern to artifact→code linkage (same "stored reference to moving code" pattern at the memory layer).

**Status:** fixed-verified (2026-05-17 third session)

**Resolution:** Called `mcp__codescout__memory(action=refresh_anchors, topic=<X>)` for each of the 6 stale topics: architecture, conventions, gotchas, domain-glossary, development-commands, project-overview. Plus `system-prompt` (which became stale during this session due to the CLAUDE.md edit). Post-refresh `workspace(status)` shows all 7 topics in `fresh`; only `language-patterns` and `onboarding` remain `untracked` (intentional — those are tracker-independent).

**Caveat:** The fix is point-in-time. The next refactor that moves/deletes anchored files will re-create the staleness. The staleness *signal* works; what's missing is an automated remediation (e.g. `memory(refresh_anchors, all_stale=true)` batch command, or a hook on file-deletion events).

**Fix idea / Pointer:** Out of scope for this session log's primary topic, but worth its own followup. The staleness signal exists; a `refresh_anchors` command per topic would close 6 of 6 with one batch.

---

## W-3 — `audit_doc_refs::merger::merge_into_tracker` lifecycle logic is robust

**Observed:** 2026-05-17, Probe 2 reading `src/librarian/tools/audit_doc_refs/merger.rs:5`.

**Pattern:** The merger encodes a clear lifecycle policy with five invariants:
1. **Stable primary key** `(md_file, raw_ref)` — re-scans match existing rows by content-derived identity, not by row number. No renumbering on update.
2. **Wontfix preservation** — `if existing.status != "wontfix"` gates all auto-transitions. A human's explicit "wontfix" survives every re-scan.
3. **Open → Fixed** — when verdict flips to `Resolved`, status auto-flips to `"fixed"` with a note `"auto-resolved at <commit>"`.
4. **Fixed → Open (regression)** — when a previously-fixed issue's verdict goes back to non-Resolved/non-External, status auto-reverts with `"regression at <commit>; prior: <prior-notes>"`.
5. **Severity escalates only** — severity rank can rise across scans but never drop. Applies even to wontfix rows (tracks worst-ever).

**Counterfactual:** Without these invariants, re-running `audit_doc_refs` would either:
- Renumber rows on each scan (breaking cross-references to specific findings); OR
- Reset wontfix flags every scan (forcing humans to re-mark them); OR
- Drop severity when a less-severe verdict came back (hiding history); OR
- Miss regressions (treating a fix-then-break sequence as a stable fix).

Each of these is a category of bug that an unprincipled merger would ship. The codified invariants close each category by construction.

**Confirming data points:**
1. `merger.rs:5-66` body reads as a clear policy implementation — each invariant is one logical block.
2. Tests in `merger.rs` (per `symbols` listing) include `tests` module at lines 77-214 — ~140 lines of test code validating the transitions.
3. Commit history: `46d59411 feat(audit_doc_refs): lifecycle transitions + wontfix preservation + severity escalates only` shipped these invariants in a single named commit — the discipline was design, not accident.

**Impact:** med-high. Merger is the load-bearing layer that makes `audit_doc_refs` re-runnable safely. Without these invariants, re-scans would either silently overwrite human decisions or accumulate noise.

**Promote-when:** Already shipped + tested. Worth surfacing in `docs/ARCHITECTURE.md` as an example of "stable primary key from content, not from row order" for re-mergeable trackers — the pattern applies beyond audit_doc_refs.

**Status:** validated

---
## F-9 — `librarian(reindex, force=true)` cascade-deletes all augmentations (DATA LOSS)

**Observed:** 2026-05-17, during the artifact-code linkage deep dive, when trying to re-augment the goal-tracker per F-1 workaround.

**When:** After running `librarian(action=reindex, scope=project, force=true)` earlier in the session (which failed with the F-6b dimension mismatch error), attempted `artifact_augment(merge=true, id=d2cd00fc837e53f2, params={gather_from: [...]})`.

**Expected:** The merge=true call patches `gather_from` into the existing augmentation row, preserving the prompt + other params per F-15/F-16 semantics.

**Got:** *"no augmentation for artifact 'd2cd00fc837e53f2' — call artifact_augment first"*. Subsequent `artifact(find, augmented=true)` returns **count: 0**. All 4 augmented artifacts in the project (`d2cd00fc837e53f2`, `0df5ebc95d284b8e`, `4b6294bf495dbfb3`, `64f10cc45d802a11`) have `augmentation: null` and `created_at` timestamps from after the failed reindex — i.e. they were re-inserted by the post-delete re-walk, without their augmentation data.

**Probable cause:** Tracing the chain:
1. `reindex.rs::call` with `force=true` runs `DELETE FROM artifact WHERE abs_path LIKE ?1` per target.
2. `artifact_augmentation` schema (`src/librarian/catalog/schema.sql:116`) declares `artifact_id TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE` — so the DELETE cascade-removes all matching augmentation rows.
3. The subsequent re-walk + embedding INSERT runs as separate statements; when the embedding INSERT fails (F-6b dimension mismatch), the prior DELETE has already auto-committed.
4. The artifact rows are re-inserted from filesystem on next walk, but **augmentation data is permanently gone** — no transactional boundary wrapped DELETE+rebuild.

**Workaround:** Reconstruct augmentations from this session's transcript (which contains the full prompt + params from earlier `artifact_get` calls). Re-call `artifact_augment(merge=false, ...)` with the reconstructed data per artifact. Data not captured in the transcript is **lost** (e.g. any `progress_log` entries written after my last read).

**Severity:** **high** — silent data loss on a foundational operation. The `force=true` flag is documented as "force full reindex, ignoring cached file hashes" — nothing warns that augmentation rows will be destroyed.

**Status:** fixed-verified (2026-05-17 post-d482ca8a rebuild)

**Verification:** Re-ran `librarian(reindex, scope=project, force=true)` against the catalog containing the 4 recovered augmentations. Result: `added: 0, updated: 1, removed: 0, unchanged: 496`. Post-call `artifact(find, augmented=true)` count: **4** (unchanged from pre-call). The destructive cascade-delete is closed.

**Fix landed:** commit `d482ca8a` (`fix(librarian): reindex destructive failures + F-2 archetype default`). Removed the pre-walk `DELETE FROM artifact WHERE abs_path LIKE` block in `reindex.rs::call`. `force=true` is now a no-op pending proper plumbing through `index_repo_sync` (task #31).

**Fix idea / Pointer:** Promoted to `docs/issues/bug-tracker.md` #7. Three-part fix:
1. Wrap force-reindex's DELETE + re-walk in a single SQLite transaction so failures roll back.
2. Document the data-loss risk in the reindex tool description (until #1 ships).
3. Consider whether the cascade-delete is correct — maybe augmentations should survive a re-index by being preserved across artifact-row recreation (key on `abs_path` or content-hash rather than synthetic `id`).

Backs F-6b (dimension mismatch) into a higher-severity bug than originally filed — the symptom is noisy, but the side-effect is destructive.

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
