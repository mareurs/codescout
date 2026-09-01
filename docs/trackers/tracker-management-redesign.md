---
id: '3e01d4fe6de9d69b'
kind: tracker
status: active
title: Tracker Management Redesign — survey evidence + design requirements (TMR-N)
tags:
- tracker-redesign
- librarian
- graph
- lifecycle
topic: tracker management redesign
expects_augmentation: docs/augmentations/docs-trackers-tracker-management-redesign.yaml
---

# Tracker Management Redesign

Consolidates the 2026-07-17 multi-repo tracker survey into design requirements (TMR-N,
see params table) for a new management approach — candidate shapes: entry-grain graph,
external service, render-from-registry, lifecycle automation. Requirements marked
`accepted` bind any proposal; `proposed` rows await a decision with Marius.

## Survey evidence

### 2026-07-17 — four-vantage survey (codescout, backend-kotlin, MRV-poc live + frozen clone, global catalog)

**Scale.** ~20 repos hold trackers in the shared catalog (`~/.local/share/librarian/catalog.db`).
Top holders: MRV-poc 100 trackers / 0 kind=bug (per-tracker-ledger regime), codescout 80 trackers +
194 bugs, backend-kotlin 80 + 108, eduplanner-ui 16, long tail of ~15 more repos. Three distinct
regimes coexist: per-file bugs + session logs (codescout), canonical-per-topic + hygiene sweeps
(backend-kotlin), per-tracker ledgers + machine registries (MRV-poc).

**Drift mechanisms (ranked by evidence strength):**

1. **Params-vs-body divergence** — MRV `reviewer-ui-bugs`: params track 2 bugs, body has 74
   BUG entries; `ingestion-defects`: params fossilized as `.issues`/`.issues_extra`/`.issues_extra2`
   (merge foot-gun), synced 05-23 vs body 06-07. The "params win" rule masks the drift. Counter-cases
   `corpus-gaps` and `uat-acceptance-findings` were same-day synced and clean → drift correlates with
   sync discipline, not tooling.
2. **Fix-then-forget on every pull-based surface** — refresh cycle has never run anywhere
   (`refresh_count=0` on 21/23 augmented trackers ecosystem-wide); 106 (codescout) + 26
   (backend-kotlin) closed bugs unarchived; index files (README / CLAUDE.md tracker tables) lag disk
   everywhere; `librarian(doctor)` never scheduled → 181 violations accreted.
3. **Vocabulary fragmentation** — bug `fixed` (codescout) vs `closed` (backend-kotlin); tracker
   status `open` (off-vocab); 9 manual rel types where ~4 concepts exist; 52–66 `kind=unknown` rows
   per repo.
4. **Path-based identity** (`id = sha256(abs_path)`) — MRV frozen clone = whole-repo uncataloged
   shadow; worktree rows leaked into catalog; 179 dead `missing_file` rows; every CLAUDE.md carries
   move-orphaning warnings as process compensation for a storage-layer decision.
5. **Index-file drift recurs after repair** — MRV repaired its CLAUDE.md index 10→26 rows in six
   weeks, already ~12 files behind again.

**Connection tiers (consistent across repos):**

- Dense **prose entry-citation webs** — the real knowledge structure (851 F-N mentions in
  backend-kotlin trackers; MRV's RI/II/BUG/GFI lattice with 48–75 cross-mentions per file).
- Thin **deliberate typed-edge layer** that works where used: backend-kotlin gantt cluster
  (`tracks`/`implements`/`supersedes`, 15 edges), MRV July data-readiness family (12 edges) — both
  created *during* focused work streams, not retroactively.
- **Zero cross-repo edges anywhere** (umbrella link_scan: 1,072 artifacts, `cross_repo: 0`)
  despite two umbrellas and a documented `<repo>:<ID>` convention.
- Scanner-derived edges: 2,144 citations → 487 resolvable (project scope); 248 ambiguous
  (per-file F-1 namespaces, avg ~10 candidates), 372 dangling (archived definers, deleted ids,
  params-buried entries invisible to prose scan) — ~29% loss.

**Session-log decay (the dominant zombie class):** codescout 13 active-dir session logs, 6
untouched 4–5 weeks; backend-kotlin's May zombies and README-missing trackers are mostly session
logs; MRV's index-missing files likewise. Root cause: the terminal state ("work stream wrapped")
has no detectable event — streams fizzle, nobody fires the archive step. Existing but unused
primitives: `append_mode + history_cap` (built-in history trimming, shrink-guard-exempt), the
tracker-hygiene skill (never run in codescout), backend-kotlin HY-2 finding ("staleness keeps
re-deferring — model as distinct state").

**Bugs filed from the survey (out-of-scope for this design):**
`docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md`,
`docs/issues/archive/2026-07-17-tmp-probe-artifacts-pollute-global-catalog.md`,
`docs/issues/archive/2026-07-17-catalog-dead-rows-no-gc.md`.

### 2026-07-17 — rehomed from codescout-lessons session log (D10 distill)

The first D10 hygiene sweep archived `codescout-lessons-2026-05-20-session-log.md` (36d stale). Four of its open frictions are direct prior-art evidence for the TMR requirements and are rehomed here so they survive the archive:

- **F-9** — *tracker-level frontmatter `status:` drifts from entry-level prose `**Status:**`; multiple incompatible enums coexist with no documented authority.* Direct evidence for **TMR-5** (enforced small vocabularies) — independently observed on codescout's own trackers before the multi-repo survey.
- **F-12** — *`kind: unknown` is the #1 row in the librarian catalog (550 of ~2k, >25%).* The vocabulary-discipline gap at the first axis; reinforces **TMR-5** and the `kind=unknown` counts the survey found per repo (52–66).
- **F-7** — *frontmatter `topic` (1/36) and `time_scope` (0/36) are zombie columns — fields exist as catalog columns, nobody fills them.* A schema-adoption gap relevant to any redesign that adds structured fields.
- **F-8** — *tags semantically overloaded (topic + shape + lifecycle all stuffed into `tags:`)* — argues for dedicated typed fields, relevant to TMR-1/TMR-5 modelling.

Full original entries in git history at `docs/trackers/archive/codescout-lessons-2026-05-20-session-log.md`.
## Requirements rationale

### TMR-1 — entries as nodes, globally unique IDs
File-grain nodes miss the actual structure: citation webs are entry-to-entry. Per-file monotonic
namespaces (every session log has an F-1) are the direct cause of the 248 ambiguous citations.
Cheapest viable form: prefix-qualified IDs (`<tracker-slug>:F-1` or unique prefixes like TMR-N).

### TMR-2 — single write path per fact
The only zero-drift mechanism observed in the fleet is MRV's `registry.json → rendered RI-NNN.md`
pipeline: markdown as view, not store. Where dual-write must remain, per-entry atomic appends
(`append_entry`) beat whole-array merges — the synced-vs-drifted MRV pair is the controlled
comparison.

**Decision 2026-07-17 — accepted in the weak form:** per-entry atomic appends (`append_entry`)
are required wherever dual-write remains; render-from-store (strong form) is encouraged per kind
and stays the target for registry-style ledgers, but is not mandated fleet-wide. Reversible:
if registries keep proving out, the strong form can be promoted later via a new TMR row.
### TMR-3 — push-based maintenance
Every detector that requires a manual pull decayed to zero use (refresh, doctor, hygiene,
link_scan write). Evidence says triggers (post-commit, SessionStart banner, scheduled sweep)
matter more than new mechanism.

### TMR-4 — identity decoupled from path
Most observed drift classes are downstream of `sha256(abs_path)`: shadow clones, worktree leakage,
move-orphaning, dead rows. Stable IDs + path-as-mutable-attribute inverts the failure mode.

**Decision 2026-07-17 — accepted** as binding on any *new* design; explicitly not a retrofit
mandate for the current catalog.
### TMR-5 — enforced vocabularies
Free-form status/kind/rel fragmented within weeks in every repo. Small closed sets, validated at
write time; per-project extension only by declaration.

**Decision 2026-07-17 — accepted, fleet + extensions model:** small fleet-wide core set,
enforced at write time; per-repo additions only by explicit declaration (workspace.toml-style).
This also answers the 'who arbitrates vocabulary' open question.
### TMR-6 — per-kind lifecycle policies
Statuses describe; policies act. Session logs: decay signal (no appends ≥21–30d, optionally no
topic-file commits) → hygiene-gated distill (promote validated W-N, rehome open F-N) → compact
body to outcomes digest → archive. Ledgers: per-entry close events. Registries: render-sync check.

### TMR-7 — write-time edge capture
Healthy graphs were built during work, by hand, with intent. Scanner derivation loses 29% and
should be the repair path, not the primary. E.g. `append_entry` accepting a `cites` field.

## Candidate directions

**Decided 2026-07-17 — the staged path** (endorsed by Marius same session):

- **Stage 1 — lifecycle + push (no schema work):** per-kind lifecycle policies (TMR-6) and
  self-scheduling maintenance (TMR-3) on the *current* catalog model. Session-log decay is the
  first policy; doctor/refresh/link_scan get triggers. Regime-independent, cheapest, reversible.
- **Stage 2 — entry grain:** globally unique entry IDs (TMR-1), `append_entry` as the required
  write path (TMR-2 weak form), write-time `cites` capture (TMR-7). Schema/convention work.
- **Stage 3 — storage, only if needed:** identity decoupled from path + tombstones (TMR-4),
  entry-grain edges as first-class rows — pursued only if Stage 2's graph outgrows the catalog.
  External-service option parked here.

Rejected alternatives: big-bang catalog rewrite (violates TMR-4's no-retrofit scoping and
TMR-2's weak form); external service first (highest cost, unproven need); render-from-registry
mandate fleet-wide (strong TMR-2 form was declined).

Stage-1 plan: `docs/plans/archive/2026-07-17-tracker-lifecycle-stage1-plan.md` (see History) — archived 2026-09-01, all three tasks shipped.
## Open questions

- Does the graph live in the shared catalog or per-repo, and how do umbrellas federate it?
- Cross-repo edges: why did zero materialize despite conventions — missing tooling, or missing
  need? (Survey couldn't distinguish.)
- ~~Who arbitrates vocabulary (TMR-5)?~~ Resolved 2026-07-17: fleet-wide core + declared per-repo extensions.
- Live MRV checkout is outside every umbrella visible from codescout — should the redesign define
  fleet-wide scope explicitly?

## History

### 2026-07-17 — Stage-1 implemented
D10 session-log-decay detector landed in the tracker-hygiene skill (claude-plugins a0009165, branch tracker-hygiene-d10); codescout hygiene ledger bootstrapped (codescout e32d42cf, branch experiments) — the SessionStart nudge is live, verified firing against the real repo. First sweep will exercise D10 against the ~6 stale session logs the survey found.

### 2026-07-17 — direction decided: staged path
Stage 1 lifecycle+push → Stage 2 entry grain → Stage 3 storage-if-needed. Rationale: TMR-3/6
are implementable today with no schema changes; TMR-1/7 need convention+schema work; TMR-4
only binds new storage design. Stage-1 plan drafted same session.

### 2026-07-17 — accept/reject pass: all seven requirements accepted
Decision pass with Marius (same session as creation). TMR-1/3/6/7 accepted as written;
TMR-2 accepted in the weak form (atomic appends required, render-from-store encouraged);
TMR-4 accepted as binding-on-new-design (no retrofit mandate); TMR-5 accepted with the
fleet-core + declared-extensions ownership model. Params rows updated with `decided` dates.
All seven now constrain any candidate direction; next step is the direction decision
(catalog-native entry graph vs external service vs minimal lifecycle path).

### 2026-07-17 — created from multi-repo survey
Four-vantage survey (this session): codescout local audit, explore subagents at backend-kotlin,
mrv-vertex-probe (frozen clone), and live MRV-poc; global catalog SQL audit. Seeded TMR-1..TMR-7
as `proposed`. Three codescout bugs filed as side-findings.

### 2026-07-17 — Stage-2 implemented (TMR-1 + TMR-7 landed)

Entry-grain IDs (TMR-1) + write-time cites (TMR-7) shipped on `experiments` (codescout librarian catalog), 8 commits `0663be16..70d16686`. Delivered: v9 schema (`artifact.slug` UNIQUE + `entry_cite` table, FK on slug = move-durable), the `entry_cite` module, `slugify`/`ensure_slug` (lazy title-derived minting), `append_entry` gains `cites` → resolves refs (16-hex id | `<slug>:<local>` | unique rel_path) and writes `entry_cite` edges atomically inside the IMMEDIATE tx (worktree-refused), and `get(include_links)` surfaces edges as `entry_links {outgoing, incoming}`. `artifact_link`/`link_scan` untouched (table separation). Executed via subagent-driven-development with two Opus review passes that each caught a real defect (legacy-migration slug-drop; a non-discriminating worktree-guard test + two unguarded resolver branches) and a whole-branch review (id-keyed backlinks hidden for slug-less targets; unescaped LIKE in cite resolution) — all fixed. Full suite 3294 pass; branch merge-ready (master promotion = user-driven release flow per docs/RELEASE.md). Spec: `docs/superpowers/specs/2026-07-17-tracker-entry-graph-stage2-design.md`; plan: `docs/superpowers/plans/2026-07-17-tracker-entry-graph-stage2.md`. Deferred follow-ons: frontmatter slug mirror, worktree cites, graph-traversal tool, rel_path-sha id re-key (→ TMR-4, Stage 2b/3). Filed bug: `docs/issues/archive/2026-07-17-worktree-cites-refusal-materializes-shadow-fork.md`.
