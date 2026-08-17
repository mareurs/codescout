# TAXONOMY — ID prefixes used in this repo

A single-page reference for every monotonic-ID ledger in the project. Use it
when (a) appending a new observation and you're not sure which tracker, or
(b) onboarding and trying to navigate the accumulated session intelligence.

The accumulated rules are spread across `CLAUDE.md`, individual tracker
templates, and skill SKILL.md files. This page is the index, not the spec —
follow the links for the controlling convention.

> **Note** — Some trackers below are *augmented artifacts* (the T-N row is
> the canonical example: data in catalog DB params, prose in markdown body,
> auto-synced via a render template). The body/params split changes how you
> append. See [`architecture/augmented-artifacts.md`](architecture/augmented-artifacts.md)
> for the mental model.

## Main taxonomy

A **declared ledger** carries `entry_prefix: <PREFIX>` in its frontmatter (committed,
so it survives a fresh clone) and gets server-assigned ids from
`artifact(action="append_entry", id=…, id_prefix=…)`. Five are declared today: R, U,
H, HY, CAP. The rest still hand-allocate; F/W deliberately so, see HY-10.

| Prefix | Lives in | Captures | Append tool | Promotes to |
|---|---|---|---|---|
| **F-N** | `docs/trackers/<topic>-session-log.md` | Per-work-stream friction observation: plan-vs-reality drift, surprise tool behavior, blocker | `edit_markdown(action="insert_before", heading="## Template for new entries")` — **deliberately NOT a declared ledger.** F/W are namespaced per work stream, so each of the 8+ live session logs owns its own `F-1..F-N`. Declaring them would give one token eight definers, and the resolver reports Ambiguous rather than resolving — which is already happening (`W-13` has two). **Decided 2026-08-17 — keep per-log numbering, cite qualified.** `link_scan` now resolves `<file-stem>:F-33` to that log's entry, so write `bug-fix-session-log:F-33` and not a bare `F-33` whenever you cite an F/W entry from outside its own log. No renumbering, no lost attribution. See **HY-10** | Bug file (`docs/issues/`) if reproducible; W-N pair if pattern caught it next time |
| **W-N** | Same file as F-N | Per-work-stream win: a discipline / scout / pattern that prevented a worse outcome with named counterfactual | Same | CLAUDE.md / ADR / skill SKILL.md after 2+ confirming datapoints |
| **R-N** | `docs/trackers/reconnaissance-patterns.md` (declared ledger, artifact `5696563f06b2c222`) | Meta — observations about the **recon skill itself**: hits, misses, vocabulary expansions | **First** `artifact(action="append_entry", id="5696563f06b2c222", id_prefix="R")` — no `entry_collection`, because entries are `## R-N` body sections; the server assigns the id atomically, so never compute it and never suffix it. **Then** write the section + Index row with `artifact(action="update", patch={body_edits:[…]})`. The heading must be `## R-N — <title>` or the entry defines no citable token. `edit_markdown` is refused: the ledger is augmented | PR against `codescout-companion/skills/reconnaissance/SKILL.md` |
| **U-N** | `docs/trackers/codescout-usage-frictions.md` | Friction using codescout tools / MCP server: tool slips, prompt drift, hook false-positives | **First** `artifact(action="append_entry", id="c43df94e69ca915f", id_prefix="U")` — no `entry_collection` (prose ledger); the server assigns the id. **Then** write the `### U-N — <title>` section via `artifact(action="update", patch={body_edits:[…]})`. `edit_markdown` is refused: declared ledger | H-N hookify rule, CLAUDE.md note, or prompt-surface edit |
| **H-N** | `docs/trackers/codescout-usage-hookify.md` | Hook design proposal: warn → deny criteria, new gate ideas, false-positive carve-outs | **First** `artifact(action="append_entry", id="e522954737601d13", id_prefix="H")` — no `entry_collection` (prose ledger); the server assigns the id. **Then** write the `### H-N — <title>` section via `artifact(action="update", patch={body_edits:[…]})`. `edit_markdown` is refused: declared ledger | Shipped hook in `claude-plugins/codescout-companion/hooks/` |
| **T-N** | `docs/trackers/tool-usage-patterns.md` (augmented artifact `f2ecdd76a6189efb`) | Tool-selection quality observation: legitimate / debatable / wrong-tool call with prompt gap | `artifact(action="append_entry", id="f2ecdd76a6189efb", entry_collection="observations", id_prefix="T", entry={…})` — atomic id. Body prose via `artifact(action="update", patch={body_edits:[…]})`. **Never** `artifact_augment(merge=true, params={observations:[…]})`: that REPLACES the array, and took this queue 19 → 1 on 2026-08-16 | `src/prompts/source.md` edits (server-instructions surface) |
| **WIN-N** | `docs/trackers/windows-platform-support.md` (augmented artifact `52451519052d207c`) | Windows-platform issue: process-spawn / lsp / platform-gated / path-handling / build-install / test-portability / companion defect, fix, or cfg-gate decision | `artifact(action="append_entry", id="52451519052d207c", entry_collection="issues", id_prefix="WIN", entry={…})` — atomic id, then sync the body `## Issue index` table. **Never** `artifact_augment(merge=true, params={issues:[…]})`: that REPLACES the array | Bug file (`docs/issues/`) for new incidents; `status` flips in place as fixes land |
| **A-N** | `docs/trackers/prompt-hamsa-audit-log.md` (craft-level twin in `claude-plugins/docs/trackers/`) | Prompt-audit record from a Hamsa audit: named gap, recommended move, prediction, confidence, outcome (filled when evidence lands) | Per the tracker's maintenance convention (`## A-N — <title>` section + Index row) | Hamsa SKILL.md heuristic / buddy memory when the finding generalizes |
| **PV-N** | `docs/trackers/provenance-subsystem.md` (augmented artifact `e12cd7e0060ed9b8`) | Provenance/attribution **programme** state: measurement verdicts vs pre-registered kill conditions, standing design decisions, hazards not to rediscover, open decisions, buildable work. Typed `finding \| gap \| decision \| hazard \| task` | `artifact(action="append_entry", id_prefix="PV", entry_collection="items", entry={...})` — atomic monotonic id; query with `entry_filter` | Implementation plan (`docs/plans/`) once phase moves past MEASUREMENT; a `decision` flips to `settled` in place |
| **CAP-N** | `docs/trackers/capability-proposals.md` (augmented artifact `01291679a5ee4707`) | **Pre-plan** proposal for a codescout capability we do not have: the ask, a substrate check citing what exists today at `path:line` and what is genuinely missing, and the open decisions. Reflective — judgment, not gathering | Append a `## CAP-N` section above `## Anti-goals` + an Index row, via `artifact(action="update", patch={body_edits: [...]})` | A spec + plan under `docs/superpowers/` once it has tasks and a file structure; or `rejected` in place with the reason kept |
| **BUG (slug)** | `docs/issues/YYYY-MM-DD-<slug>.md` | Per-bug investigation file: Symptom / Repro / Root cause / Fix / Workaround | Create from `docs/issues/_TEMPLATE.md`; status field in frontmatter | Archived to `docs/issues/archive/` once the fix is verified on `experiments` — reaching master is NOT required. Label the SHA `experiments` and keep a Resume note that the master-side SHA is still owed (it orphans on rebase); the ship sequence's step 4 reconciles it. Move via `artifact(action="move", …)`, never `git mv` |

## Work-stream-specific prefixes (not durable taxonomy slots)

These appear inside individual session logs / specs and are scoped to one
work stream — not project-wide ID namespaces. The session log defines them
in its own header; they don't need slots here.

- **S-NN** — Session residuals: open follow-ups when a multi-session work
  stream wraps. Example: `docs/trackers/archive/2026-05-07-retrieval-session-residuals.md`.
- **D-NN** — Design decisions inside a spec. Example: a multi-decision
  spec uses D-1 through D-N for the decisions enumerated in that document.

If you find yourself wanting to introduce a new project-wide prefix, ask first
whether it really earns a slot or whether it's a variant of one of the seven
above.

## How to choose

```
You observed something. Where does it go?
│
├─ Is it a bug? (wrong output, silent failure, corrupt state)
│   → BUG file in docs/issues/  (open one per the CLAUDE.md trigger rules)
│   → if it is Windows-specific, ALSO add a WIN-N row to windows-platform-support.md
│
├─ Is it a Windows-platform defect / portability gap / cfg-gate decision?
│   → WIN-N in windows-platform-support.md (augmented artifact 42dfdfc8b1522192)
│
├─ Is it a friction with codescout / MCP / hooks / Iron Laws?
│   → U-N in codescout-usage-frictions.md
│
├─ Is it a design idea for a hook / gate / IL refinement?
│   → H-N in codescout-usage-hookify.md
│
├─ Is it a tool-selection observation worth reviewing later?
│   → T-N in tool-usage-patterns.md (artifact f2ecdd76a6189efb)
│
├─ Is it about the recon skill itself (hit / miss / proposal)?
│   → R-N in reconnaissance-patterns.md
│
├─ Is it a feature idea — a capability codescout does not have yet?
│   → CAP-N in capability-proposals.md (artifact 01291679a5ee4707)
│     Do the substrate check BEFORE writing it: what exists today, what is
│     actually missing. An entry without one is not ready.
│     Already has tasks + a file structure? Skip this — write a spec + plan.
│
└─ Is it a per-work-stream friction or win, scoped to one task / refactor?
    ├─ Friction → F-N in <topic>-session-log.md
    └─ Win      → W-N in same file (with counterfactual)
```

## Promotion ladder

```
F-N / W-N    (session log — per work stream, archived when wrapped)
  │
  ├──→ BUG file   if friction stabilizes into a reproducible bug
  │
  └──→ CLAUDE.md / ADR / SKILL.md   if win confirmed 2+ times across work streams
                                    (promote-when criterion fires)

U-N          (codescout usage frictions — durable across sessions)
  │
  ├──→ H-N hookify rule   if friction can be substrate-enforced
  │
  └──→ CLAUDE.md / prompt-surface edit   if convention or guidance fix

H-N          (hookify proposal)
  │
  └──→ Shipped hook in codescout-companion/hooks/  (PR + merge)

R-N          (recon-skill meta)
  │
  └──→ PR against codescout-companion skill SKILL.md   (promote-when fires)

T-N          (tool-usage patterns)
  │
  └──→ src/prompts/source.md edits   (server-instructions surface)

BUG          (per-bug investigation)
  │
  └──→ Fix lands on master + archive move to docs/issues/archive/
```

## SHA-citation rule

Every prefix above may cite git commits as evidence. After cherry-pick + rebase,
experiments-side SHAs orphan. Cite the **master SHA** captured immediately after
`git cherry-pick` lands on master — see `CLAUDE.md § After cherry-pick`.

For cross-repo citations (e.g. tracker in codescout pointing at a fix in
codescout-companion), prefix with the repo name: `codescout-companion:0b75991`.

## Citation format (mandatory)

Every closure that cites a git commit must use one of these shapes — bare SHAs without branch scope are ambiguous and not allowed:

- `(master:<sha>)` — fix has shipped to master, `git branch --contains <sha>` from master returns master.
- `(experiments:<sha>, not-yet-on-master)` — fix exists on experiments only. The qualifier is mandatory.
- `(<repo>:<sha>)` — cross-repo fix; repo prefix names which repo's SHA. E.g. `(claude-plugins:bd20a8a)`. Branch context is master unless further qualified.
- `(in-place)` — for files outside git (e.g. `~/.claude/CLAUDE.md`). No SHA citation.

When a fix shipped only to experiments and later cherry-picks to master, update the citation from `(experiments:<sha>, not-yet-on-master)` to `(master:<new-sha>)`. Both reads remain in the tracker's history via citation-history footnote.

Policy: [`docs/trackers/archive-cadence-policy.md`](trackers/archive-cadence-policy.md) — surface 1.

## Archive cadence

Closed entries graduate to `docs/trackers/archive/<tracker>-<YYYY>-q<n>.md`:

- **Trigger**: status is closed AND (SHA is on master OR closure is `wontfix` / `by-design` / `substrate-caught` / cross-repo verified).
- **Cadence**: manual quarterly pass + accelerated by release cuts.
- **Archive file**: per-tracker, time-partitioned. Frontmatter `kind: tracker, status: archived`.
- **Recovery**: `artifact(action="find", kind="tracker", include_archived=true)`.

Full policy: [`docs/trackers/archive-cadence-policy.md`](trackers/archive-cadence-policy.md).## Status vocabularies (per prefix)

Different prefixes use slightly different status enums:

- **F-N statuses** — `open | mitigated | fixed-verified | wontfix-false-alarm | promoted-to-bug-tracker | pinned-as-eval-baseline` (canonical in `docs/templates/session-log.md`).
- **W-N statuses** — `validated | promoted-to-permanent-docs | archived`.
- **U-N statuses** — `open | fixed-shipped | promoted | wontfix` (informal).
- **H-N statuses** — `warn | deny | shipped | rejected`.
- **R-N verdicts** — `hit | miss | proposal | promoted`.
- **T-N verdicts** — `legitimate | debatable | wrong-tool`.
- **WIN-N statuses** — `fixed | mitigated | open | deferred | wontfix` (canonical in the `params_schema` of `docs/trackers/windows-platform-support.md`).
- **BUG statuses** — `open | investigating | fixed | mitigated | wontfix | zombie` (canonical in `docs/issues/_TEMPLATE.md`).

When in doubt, mirror the existing entries in that file — consistency beats
correctness here.
