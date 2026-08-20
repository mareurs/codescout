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
`artifact(action="append_entry", id=…, id_prefix=…)`. **Thirteen are declared today** (measured 2026-08-19 by
reading `entry_prefix:` out of every frontmatter block): AA, CAP, F, FND, FT, GF, H, HY,
R, S, SD, U, W.

> ⚠️ **F and W now appear among the declared prefixes, which contradicts the F/W row
> below.** One session log declared `entry_prefix: [F, W]`. That is a real, unresolved
> contradiction between this document and the corpus — not a typo here. Decide it
> deliberately before adding more: declaring F/W makes them dangling-checked but gives one
> token many definers; leaving them undeclared keeps ambiguity. Measured 2026-08-19 across
> 10 repos in 2 umbrellas: **33% of cross-file entry citations are ambiguous** and F/W are
> the dominant contributors (`F-1` alone has 169 definers). codescout at 28% is among the
> healthiest — this is a property of the per-file `PREFIX-N` convention, not a local defect.

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
| **BUG (slug)** | `docs/issues/YYYY-MM-DD-<slug>.md` | Per-bug investigation file: Symptom / Repro / Root cause / Fix / Workaround | Create from `docs/issues/_TEMPLATE.md`; status field in frontmatter | Archived to `docs/issues/archive/` once the fix is verified on `experiments` — reaching master is NOT required. Record the SHA **labelled with its branch** *and* its `git patch-id --stable` — the SHA is positional and dies when `experiments` is rebased; the patch-id is a content hash of the diff and survives rebase and cherry-pick. **No pending-master-SHA Resume line, and no later reconciliation** — record both once and the record stays resolvable whichever path the fix takes to `master`. Measured 2026-08-19: 10 of 63 archived files had already lost their SHA to a rebase, while zero patch-ids collided across 3594 commits. Move via `artifact(action="move", …)`, never `git mv` |

## Work-stream-specific prefixes (not durable taxonomy slots)

These appear inside individual session logs / specs and are scoped to one
work stream — not project-wide ID namespaces. The session log defines them
in its own header; they don't need slots here.

- **S-NN** — Session residuals: open follow-ups when a multi-session work
  stream wraps. Example: `docs/trackers/archive/2026-05-07-retrieval-session-residuals.md`.
- **D-NN** — Design decisions inside a spec. Example: a multi-decision
  spec uses D-1 through D-N for the decisions enumerated in that document.

If you find yourself wanting to introduce a new project-wide prefix, ask first
whether it really earns a slot or whether it's a variant of one of the **eleven**
in the table above.

### Measured drift — 2026-08-19

The table documents 11 prefixes. **29 are actually defining entries** in this repo
(counted by `## PREFIX-N — title` headings, which is the only shape `link_scan` reads as a
definition). **17 of them have no row in that table.** By number of files defining each:

| Prefix | Definers | Prefix | Definers |
|---|---:|---|---:|
| `C` | 10 | `TMR`, `SD`, `BL` | 1 each |
| `ADR` | 4 | `TU`, `GF`, `DF` | 1 each |
| `BUG-N` (token, not the slug row) | 3 | `B`, `AB`, `I` | 1 each |
| `LIMIT`, `L`, `AA`, `FND`, `FT` | 1 each | | |

This list is a **snapshot, not a slot allocation** — several are one-off spec-local
namespaces that correctly do not earn a row. It is recorded so the next reader knows the
table is a subset of reality rather than assuming an unlisted prefix is a mistake. Re-derive
it rather than trusting this table to stay current.

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

Every prefix above may cite git commits as evidence. **Cite the SHA *and* its patch-id.**

A SHA is positional: after cherry-pick + rebase the experiments-side original orphans and
`git branch --contains` returns empty. `git show <sha> | git patch-id --stable` is a content
hash of the diff and survives rebase **and** cherry-pick, so the pair stays resolvable —
with no promotion path to check and nothing owed later.

Full rationale, measurements and the recovery procedure:
[`docs/RELEASE.md` § *Citing a fix: SHA + patch-id*](RELEASE.md).

For cross-repo citations (e.g. tracker in codescout pointing at a fix in
codescout-companion), prefix with the repo name: `codescout-companion:0b75991`.
## Citation format (mandatory)

Every closure that cites a git commit must use one of these shapes — bare SHAs without branch scope are ambiguous and not allowed:

- `(master:<sha>)` — fix has shipped to master, `git branch --contains <sha>` from master returns master.
- `(experiments:<sha>, not-yet-on-master)` — fix exists on experiments only. The qualifier is mandatory.
- `(<repo>:<sha>)` — cross-repo fix; repo prefix names which repo's SHA. E.g. `(claude-plugins:bd20a8a)`. Branch context is master unless further qualified.
- `(in-place)` — for files outside git (e.g. `~/.claude/CLAUDE.md`). No SHA citation.

**A citation is never updated after the fix reaches `master`.** Record the SHA and its patch-id once, at the time of the fix:

```
git show <sha> | git patch-id --stable
```

The SHA is **positional** — `experiments` is rebased after every ship, so a cherry-picked commit's original is orphaned and eventually garbage-collected. The patch-id is a **content hash of the diff** and survives both rebase and cherry-pick, so it still finds the change after the SHA dies. Measured 2026-08-19: zero genuine collisions across 3594 commits, and all 104 duplicate patch-ids were the same change appearing on two branches — the anchor working, not failing.

Recording both is what **replaces** the former cherry-pick-vs-fast-forward decision and the "master-side SHA still owed" follow-up. There is no promotion path to check and nothing to come back for.

Policy: [`docs/trackers/archive-cadence-policy.md`](trackers/archive-cadence-policy.md) — surface 1.

## Archive cadence

Closed entries graduate to `docs/trackers/archive/<tracker>-<YYYY>-q<n>.md`:

- **Trigger**: status is closed AND (SHA is on master OR closure is `wontfix` / `by-design` / `substrate-caught` / cross-repo verified).
- **Cadence**: manual quarterly pass + accelerated by release cuts.
- **Archive file**: per-tracker, time-partitioned. Frontmatter `kind: tracker, status: archived`.
- **Recovery**: `artifact(action="find", kind="tracker", include_archived=true)`.

Full policy: [`docs/trackers/archive-cadence-policy.md`](trackers/archive-cadence-policy.md).

## Status vocabularies (per prefix)

Different prefixes use slightly different status enums:

- **F-N statuses** — `open | mitigated | fixed-verified | wontfix-false-alarm | promoted-to-bug-tracker | pinned-as-eval-baseline` (canonical in `docs/templates/session-log.md`).
- **W-N statuses** — `validated | promotion-due | promoted-to-permanent-docs | archived`.
  (`promotion-due` means the `Promote-when` criterion has **fired** and the text is not
  yet on the target surface — an action item, not a resting state. It was missing from
  this list until 2026-08-20 while `docs/templates/session-log.md` defined it.)
- **U-N statuses** — `open | fixed-shipped | promoted | wontfix` (informal).
- **H-N statuses** — `warn | deny | shipped | rejected`.
- **R-N verdicts** — `hit | miss | proposal | promoted`.
- **T-N verdicts** — `legitimate | debatable | wrong-tool`.
- **WIN-N statuses** — `fixed | mitigated | open | deferred | wontfix` (canonical in the `params_schema` of `docs/trackers/windows-platform-support.md`).
- **BUG statuses** — `open | investigating | fixed | mitigated | wontfix | zombie` (canonical in `docs/issues/_TEMPLATE.md`).

When in doubt, mirror the existing entries in that file — consistency beats
correctness here.

### `**Valid:**` — the one field whose vocabulary is the same everywhere

`**Status:**` says where the *work* stands and varies per prefix. `**Valid:**` says
whether the *claim* is still true, and takes the same three values in every ledger:

```text
**Valid:** invariant                   a law; no expiry
**Valid:** dated 2026-08-20            true of an instant; every measured count
**Valid:** conditional — <event>       true until that named event fires
```

There is no fourth, and no exemption: **declaring nothing is not "no claim here"** —
absence is read as decay. What decides whether an undeclared entry is actually flagged
is *exposure*, the number of other files citing it, so an uncited entry is left alone
because nothing rests on it. A section the server writes via
`artifact(action="append_entry")` is stamped `**Valid:** dated <today>` unless the
caller passes a class.

Malformed declarations are refused rather than accepted — a bare `conditional` naming
no event, an unknown class (`conditionally speaking` fails on a word boundary, not a
prefix), and a calendar-invalid date (`dated 2026-02-30`).
`librarian(action="doctor")` reports these as `entry_conditional_past_due`,
`entry_dated_stale`, `entry_cited_from_outside_but_undeclared` and
`validity_unparseable` — read-only worklists, never verdicts.

Its sibling `**Rests on:**` is one durable sentence naming the route back to the proof —
an ADR, a decision, a principle, not a `path:line`. Parsed and stored today; no consumer
reads it yet.

Full treatment: [Statement Validity](manual/src/concepts/statement-validity.md).
Authoring rules: `get_guide("tracker-conventions")` § Required fields.
