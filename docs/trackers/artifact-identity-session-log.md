---
title: Session Log — Artifact Identity (internal uid, portable key)
tags:
- identity
- slug
- librarian
- session-log
topic: artifact identity refactor
entry_prefix:
- F
- W
entry_high_water_F: 11
entry_high_water_W: 7
---
# Session Log — Artifact Identity (internal uid, portable key)

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries with:
>
> ```
> doc(action="append_entry", id="<artifact id>", id_prefix="F",
>          anchor_heading="## Template for new entries",
>          title="<one-line title>", body="**Observed:** ...",
>          index_row="| {id} | <date> | <sev> | <cat> | open | **<title>** — … |",
>          index_after_line="<the table line your row goes AFTER>")
> ```
>
> One call, one write — **and the index row belongs in that same call.**
> The server allocates the next id, formats the heading as `## F-N —
> <title>` (the only shape `link_scan` accepts as a definition), records
> the ledger's high-water mark, stamps `**Valid:** dated <today>` unless
> your body declares a class, and writes `index_row` with `{id}` replaced
> by the id it just allocated. The indexes are the eval surface, the
> sections are the evidence.
>
> **A row needs an anchor from SOMEWHERE, and there are two sources.**
> Best, once per ledger: declare `snapshot_anchor` in this file's own
> frontmatter, naming the index table's header line verbatim —
> `doc(action="update", id=…, patch={"extra": {"snapshot_anchor": "| ID |
> Date | … |"}})`. After that, `index_row` **alone** places every row at
> the block's **TAIL**, which is where a newest-last ledger wants it, and
> nobody chooses an anchor again. Otherwise pass `index_after_line` per
> call; it WINS when both are given. They are **not** symmetric —
> `index_after_line` with no `index_row` is refused, and passing neither
> anchor with a row is refused by name.
>
> **Choosing that per-call anchor fails silently in both directions, which
> is the reason to prefer the declaration above.** It inserts after the
> **first** line equal to what you pass: a non-unique line writes your row into the wrong
> table, and a line that does not exist writes nothing at all and
> allocates no id. Neither raises an error. **Prefer the target table's
> last existing row** — unique by construction, because ids are — over
> the `|---|---|` separator, which anchors to the table's **top** and is
> repeated 9 times in one live ledger. Top is right for a newest-first
> ledger like `bug-fix-session-log.md` and wrong for the oldest-first
> ones; the corpus does not agree, so your ledger decides. A declared
> `snapshot_anchor` sidesteps this whole paragraph.
>
> **Do not hand-allocate ids, and do not pre-write index rows.** A max-id
> is a fact about an instant, and a peer session in the same checkout can
> take the number between your scan and your write. Pre-written rows are
> worse: the allocator counts an id claimed by an index row, so rows
> written ahead of their sections consume the ids they name — which is why
> codescout's `statement-validity-session-log` starts at `statement-validity-session-log:F-2`/`statement-validity-session-log:W-3`
> rather than `statement-validity-session-log:F-1`/`statement-validity-session-log:W-1` (see `statement-validity-session-log:F-3` there).
>
> **`index_row` is not a pre-written row, and that warning does not reach
> it.** The server substitutes `{id}` after allocating, in the same write
> as the section, so no instant exists at which the row names an id the
> section does not. The warning is about a row you type yourself ahead of
> the append — that one still consumes the id it names.
>
> **A copy of this file is a ledger from its first moment.** The frontmatter
> above declares `entry_prefix: [F, W]`, which is why the recipe at the top
> is accepted on a fresh copy with nothing to declare first — and, because
> a declared file is a ledger, the librarian guard refuses direct edits to
> it from the start (`edit_file` says so by name). Retitle it and edit its
> prose and index tables with `doc(action="update", id=<the copy's artifact
> id>, patch={body_edits: [...]})` once the copy is catalogued; add entries
> only with `append_entry`, never by hand.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (`docs/conventions/`, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-11-11 | low | plan-prose | fixed-verified | **The stamped frontmatter id is not where moves leave stale data** — 1545/1550 still match; the residue is bare hex in prose |
| F-2 | 2026-11-11 | med | self-friction | fixed-verified | **Claimed rekey.rs could re-key artifact keys without reading it** — it re-keys entry prefixes |
| F-3 | 2026-11-11 | med | subagent | fixed-verified | **Slug-drift estimate scoped to one repo; the unique index spans the machine** — suffix depends on other repos' index order |
| F-4 | 2026-11-11 | high | architectural | promoted-to-bug-tracker | **Reindex deletes a moved file's local history** — has_history guards escaping rows only |
| F-5 | 2026-11-11 | med | plan-prose | fixed-verified | **Plan A Task 1 missed everything a removed cargo feature drags along** — required-features, install commands, memories; found by scout + review |
| F-6 | 2026-11-11 | med | self-friction | fixed-verified | **Ran the plan on the shared checkout until the operator asked for a worktree** — 33 files edited, restored from a saved patch |
| F-7 | 2026-11-11 | med | subagent | fixed-verified | **The worktree-aware repo-name suggestion mishandles a submodule** — `worktree_main_root` was ungated; found by the re-review, fixed in wave 2 |
| F-8 | 2026-11-11 | med | plan-prose | open | **A spec requirement is built by no plan: "onboarding says when another checkout uses the name"** — plus two open spec-level questions |
| F-9 | 2026-11-11 | med | self-friction | fixed-verified | **Told the operator a peer held three dirty files; they were my own stopped implementer's leftovers** — attributed by adjacency; a transcript search found the real author |
| F-10 | 2026-11-11 | low | plan-prose | fixed-verified | **Task 1 left a stale "librarian feature" message in `tests/cli_doc.rs` and a "librarian-less" comment in `src/prompts/mod.rs`** — passed both reviews; fixed in 6671e2ce8 |
| F-11 | 2026-11-11 | high | architectural | open | **The spec and the old catalog code disagree in about a dozen places a plan must settle** — a third guard signal, the entry-id race guard, four consumers, G5, vectors; 3 verified, the rest reported |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-2 | 2026-11-11 | high | cross-session design review | path fallback would change keys on folder moves | validated |
| W-3 | 2026-11-11 | high | per-task review with named risks | a Kelvin-sign key, a write through a dangling link, a 60 s-bound dialog under a write lock — all with a green gate | validated |
| W-4 | 2026-11-11 | high | census with the real code | Plan E sized from the 895-id estimate; measured 45 renames, 37 duplicate-key groups, 144 `[[F-16]]` rewrites | validated |
| W-1 | 2026-11-11 | high | builder check against the code | repo name in librarian.toml breaks every older binary | validated |
| W-5 | 2026-11-11 | med | merge the base into the branch, then ff-only | a merge commit in the shared checkout would capture six other sessions' staged files | validated |
| W-6 | 2026-11-11 | high | independent Opus review of a scout report | a split built on "retire catalog_audit ships alone", a two-signal guard and four missed consumers | validated |
| W-7 | 2026-11-11 | high | independent Opus review of a plan before execution | a gate that refuses every commit editing a file already in a collision; a cache that diverges from a rebuild after a rename | validated |

---

## Promotion status

**Audited:** <YYYY-MM-DD>, against the target surface itself — opened and read,
not recalled.

One line per `W-N` (and any `F-N` with a `Fix idea` bound for a permanent
surface). Check the **target**, not the entry: a `Promote-when` that fired is
invisible from inside the tracker, because `Status: validated` reads as healthy
either way. Record one of:

- **already promoted, no action** — quote the promoted text verbatim and name
  where it landed, so the next reader verifies instead of re-deriving.
- **UNFIRED, carried forward** — restate the criterion and the current datapoint
  count.
- **FIRED but not yet applied** — the one that leaks. Name the exact target
  surface and the exact text to add. This is an action item, not a note; set the
  entry's `Status:` to `promotion-due` so a query can find it.

> ⚠️ **Name every instance of the target, not the target's type.** This machine
> runs three Claude Code profiles (`~/.claude`, `~/.claude-sdd`,
> `~/.claude-kat`), each with its own `CLAUDE.md`. An audit that concluded
> *"not found in the user's global CLAUDE.md"* — singular — led to a promotion
> that reached one file of three on 2026-08-18. The session that found the gap
> was running on a profile **without** the rule, and applied it only because
> another profile's copy happened to be injected as project instructions. Three
> files that should be byte-identical have an md5; compare them.

> ⚠️ **For an INSTALLED artifact the target is the SERVING copy — not the repo
> source, and not the other copies.** Measured 2026-08-20: three rules promoted
> into a plugin skill were byte-identical across all three profile caches *and*
> stale against source, because the commit never bumped the version the cache is
> keyed on. Comparing the copies to each other reads **green** there — only
> comparing each copy to the claim catches it. And the session that made the edit
> is the **least representative observer**: its own reload resolved the skill from
> the repo source, so the confirming evidence sitting in front of it was evidence
> about the wrong artifact.

> ⚠️ **Anchor on a back-citation, not a verbatim quote.** A quote goes red when the
> promoted rule is legitimately reworded — a false positive produced by the
> promotion working as intended, observed 2026-08-20 when `codescout:R-89`'s bullet was
> rewritten and the tracker's stored quote had to be edited to match. The durable
> form is the promoted text citing its own entry id —
> *"(codescout:R-1 + codescout:R-7 in codescout's `docs/trackers/reconnaissance-patterns.md`.)"* — so
> verification is a `grep` for the id and survives every rewording. Keep the quote
> as a reading aid; do not make it the predicate.

Run this when the work stream wraps, **and** whenever a criterion fires
mid-stream — an audit that only happens at archive time is one that happens
after the lesson was needed. Prior art:
`eduplanner-ui:docs/trackers/archive/calendar-insight-panel-session-log-2026-08-18.md`, whose
audit correctly caught its own `calendar-insight-panel-session-log-2026-08-18:W-4` as fired-and-unapplied and named the exact
text to promote.

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Pass this block as `append_entry`'s `body` (without the `## F-N — <title>`
line — the server writes the heading from `title`). Put the matching Index
row in the same call, as `index_row` (see *How to use* above); writing it
afterwards is the second write that call exists to remove. Do not allocate the id
yourself.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Pass this block as `append_entry`'s `body`, with `id_prefix="W"` — F-N and
W-N have separate counters. A win without a **Counterfactual** is marketing
— name what would have happened without the pattern, with at least one
piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (`docs/conventions/<topic>.md`, ADR, etc.)>

**Promoted-to:** <surface + section, one per line, line-start — omit until it lands>

**Status:** validated | promotion-due | promoted-to-permanent-docs | archived

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promotion-due` | `Promote-when` has **fired** and the text is not yet on the target surface. An action item, not a resting state. Exists because `validated` cannot distinguish "criterion not yet met" from "criterion met, nobody harvested it" — and both read as healthy, which is how a lesson sits unpromoted while the failure it describes recurs. |
| `promoted-to-permanent-docs` | Moved into a `docs/conventions/` page, an ADR, a skill, or another permanent surface (a `CLAUDE.md` line only when it passes that file's residency test). Session log keeps the pointer — and, for a multi-instance target, names every instance it landed in. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — The stamped frontmatter id is not where moves leave stale data

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, measuring the source tracker before the identity brainstorm.

**When:** checking cost 1 of `docs/trackers/temp/artifact-identity-refactor.md` § Observed costs.

**Expected:** the tracker says a move makes "every stamped frontmatter `id:`" stale, and counts 895 stamped files as the exposure.

**Got:** 1545 of 1550 frontmatter `id:` stamps equal `sha256(current abs_path)[:16]`, archived files included (858/862 live, 687/688 archive). `doc(move)` already repairs the stamp (`src/librarian/tools/mv.rs:407-428`). The stale residue is bare 16-hex ids in prose: 592 occurrences / 312 distinct in the citation census (all `.md` in scope, untracked included), 676 / 335 over tracked `.md` only (`stalehist.py`).

**Probable cause:** the tracker inferred the exposure from the mechanism (`id = sha256(abs_path)`) without measuring which surface the repair already covers.

**Workaround:** the spec measures the prose residue instead and resolves it through `.codescout/aliases.tsv`.

**Severity:** low

**Status:** fixed-verified

**Rests on:** a cost claimed from a mechanism is a hypothesis until the surface it names is counted.

**Fix idea / Pointer:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` § Measurements.

## F-2 — Claimed rekey.rs could re-key artifact keys without reading it

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, choosing how a rename re-keys the entry graph.

**When:** proposing the slug-replacement design in chat, before writing the spec.

**Expected:** my own claim in chat: "`doc(move)` must re-key `entry_cite`, and `rekey.rs` already exists for that".

**Got:** reading `src/librarian/catalog/rekey.rs` showed that it re-keys an entry PREFIX (`T-` → `SRI-`, `retarget_token`), not an artifact key. Only its durable-vs-derived row split is reusable. Caught before the spec was written; v2 then removed the re-key step entirely (edges hold uids).

**Probable cause:** a module name read as a capability; asserted a checkable fact without reading the symbol this session.

**Workaround:** corrected in chat and in the spec.

**Severity:** med

**Status:** fixed-verified

**Rests on:** `codescout:R-19` — a specific, checkable claim needs the symbol read in the same session before it is stated.

**Fix idea / Pointer:** none beyond the rule.

## F-3 — Slug-drift estimate scoped to one repo; the unique index spans the machine

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, testing the risk "two machines might mint different slugs".

**When:** a Sonnet subagent measured slug stability; its brief scoped every query to codescout rows.

**Expected:** its estimate (about 52 slugs differ by title drift, 3 to 7 by suffix order) to bound the cross-machine divergence.

**Got:** the slug's unique index covers the WHOLE machine catalog (7681 rows, about 12 repo roots, 47 git roots in `commits`). A codescout slug's suffix therefore depends on which OTHER repos were indexed first: `tracker-hygiene-log` exists in 7 repos with slugs `-2`..`-7`, and codescout holds the bare slug only by index order. The real divergence is wider than the estimate.

**Probable cause:** my brief named the population of interest (codescout) instead of the population the mechanism ranges over (the catalog's unique index). Class: a selector narrower than its population (IC-18).

**Workaround:** measured the cross-repo collisions directly in the main thread (`slugfocus2.py`).

**Severity:** med

**Status:** fixed-verified

**Rests on:** brief a measurement on the mechanism's own scope, not on the scope of the question.

**Fix idea / Pointer:** the spec's portable key removes the dedup suffix, so the race disappears.

## F-4 — Reindex deletes a moved file's local history

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, verifying a subagent's claim about plain `git mv` plus reindex.

**When:** mapping identity code paths before the design.

**Expected:** `docs/conventions/cross-machine-catalog-resume.md` calls reindex "the repair" for a move pulled from another host.

**Got:** `src/librarian/indexer.rs:810-854` deletes every `gone` row; the `has_history` guard (`:800-809`) is consulted only for escaping rows (`:820`). Seven tables cascade (`artifact_augmentation`, `artifact_chunk`, `artifact_link`, `artifact_observation`, `entry_reservation`, `event_edges`, `events`; measured on the live schema). 1052 bug files carry events on this machine.

**Probable cause:** the guard's coverage is narrower than its own comment.

**Workaround:** none needed for this session; filed.

**Severity:** high

**Status:** promoted-to-bug-tracker

**Rests on:** a repair that deletes state is a data-loss path until it proves the state exists elsewhere.

**Fix idea / Pointer:** bug `99a4d0be1318c3c1` (`docs/issues/2026-11-11-reindex-deletes-local-history-of-a-file-moved-outside-this-catalog.md`).

## W-1 — Builder check against the code caught a cross-binary break in an operator choice

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, writing spec v2 after the builder check.

**Pattern:** a fresh Opus subagent read the spec as the builder, with the code and the live catalog open read-only, and reported only the questions where a wrong guess would change the plan.

**Counterfactual:** the operator had chosen `.codescout/librarian.toml` for the repo name. The check found `classify::RuleFile` is `#[serde(deny_unknown_fields)]` (`src/librarian/classify.rs:5-9`) and the precedent in `src/librarian/requirements.rs:8-12` (17 of 30 live sessions on a replaced binary). Without it, the committed `[repo]` table would have broken librarian setup for every older binary on every machine that pulled it. It also found that 55 of 273 entry endpoints have non-lowercase stems that today's qualifier regex cannot cite, and that goal G4 was untestable as written.

**Confirming data points:** 8 ranked questions; 2 became operator decisions (`repo.toml`, normalization), 6 became designer's calls; every one cited code.

**Impact:** high

**Promote-when:** a second design session records a builder check catching a cross-binary or cross-machine compatibility break.

**Status:** validated

**Rests on:** a reviewer anchored on the code, not on the author's conclusion, finds the constraints the author never read.

## W-2 — Cross-session review caught a design that broke its own anti-goal

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, two parallel design sessions (identity here, tracker organization in `codescout-37`) exchanging reviews over the session socket.

**Pattern:** each session sends its committed spec to the session whose design it touches and asks for an ack and a review against that session's own layout and measurements.

**Counterfactual:** my v1 key rule gave colliding files a path-form fallback key. `codescout-37` showed that a file without edges that later gains edges then holds a key that changes on the next folder move, which violates the spec's own first anti-goal, during exactly the reorganization the spec serves. It also exposed the missing file-grain citation form (`CitationKind` has none, `extract.rs:159-172`) and the folder-ownership scope. Its measurement (144 re-point commits after archives and moves, reproduced) settled the operator's decision to fold the id layer into this spec.

**Confirming data points:** two review rounds; 2 + 6 points adopted; one input corrected by its sender (`tracker-organization-session-log:F-1`).

**Impact:** high

**Promote-when:** a third pair of parallel design sessions uses the exchange, or one skips it and ships a conflict it would have caught.

**Status:** validated

**Rests on:** a design that serves another design must be reviewed by the session that owns the other one.

## F-5 — Plan A Task 1 missed everything a removed cargo feature drags along

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, scouting Plan A Task 1 before the first subagent dispatch.

**When:** executing the plan with subagents; the task said "remove 112 gates in 23 files" and the Cargo.toml feature.

**Expected:** the task text lists what removing the `librarian` feature changes.

**Got:** the task text missed four `required-features = ["librarian"]` lines on test targets, `tests/feature_lanes.rs`, install commands that pass `--features …,librarian` (`cargo install` would reject them), CONTRIBUTING.md, ROADMAP.md, `docs/conventions/gate-ordering.md`, CI comments, a gate test that matched a printed line, and — found only by the task review — six agent-facing codescout memories (`development-commands` is the one CLAUDE.md points every session at), `attribution.rs:643` and `test-env-isolation.md`. A grep for the cfg attribute alone finds 112 sites and none of these.

**Probable cause:** the task was sized from the one grep that measured the code gates; the references to the feature as a name (docs, memories, commands, test targets) were never measured.

**Workaround:** the controller added the scope items to the implementer's dispatch; the review loop found the memories on round 1.

**Severity:** med

**Status:** fixed-verified

**Rests on:** when a plan removes a named switch, measure every spelling of its name (`--features`, `required-features`, prose, memories), not only the attribute.

**Fix idea / Pointer:** commits `f383136a7`, `d03b67e35` on `feat/artifact-identity-plan-a-20261111`.

## F-6 — Ran the plan on the shared checkout until the operator asked for a worktree

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, at the start of executing Plan A.

**When:** I read the project rules about shared-checkout commits, chose to run the plan on the shared `experiments` checkout with pathspec commits, and recorded it as a ruling.

**Expected:** a plan whose first task rewrites Cargo.toml and 23 source files, in a checkout with other live sessions and uncommitted peer edits, runs in an isolated workspace.

**Got:** the first implementer had edited 33 files in the shared tree when the operator said "wait, no! lets do an worktree". I stopped it, saved its diff, restored exactly those 33 paths (verified clean of peer edits first, nothing staged), and created `.worktrees/artifact-identity-plan-a`. A peer's edits to `src/prompts/mod.rs`, `tests/cli_doc.rs` and `attribution.rs` appeared in the main tree meanwhile; the branch overlaps two of them, so the merge now needs that peer to commit first.

**Probable cause:** I weighed the commit rules, which allow a shared tree, and did not weigh the blast radius of a task that changes build configuration for every session.

**Workaround:** worktree; pinned codescout calls (`workspace=<path>`) in every subagent brief; `append_entry` refuses a worktree session, so ledger entries are written from the main checkout.

**Severity:** med

**Status:** fixed-verified

**Rests on:** isolation is the default for a change that every session builds; the commit rules say how to share a tree, not whether to.

**Fix idea / Pointer:** the SDD skill's setup step says to isolate; follow it before the first dispatch.

## F-7 — The worktree-aware repo-name suggestion mishandles a submodule

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, scoped re-review of the final fix wave on Plan A (commit `ae8228b96`).

**When:** the whole-branch review found that `suggest` offered the worktree folder's name inside a linked worktree; the fix called `path_security::worktree_main_root(root)` to use the main checkout.

**Expected:** a fix for the worktree case leaves every non-worktree case as it was (the brief said so).

**Got:** `worktree_main_root` does not distinguish a submodule from a linked worktree (`is_linked_worktree` does, by requiring a `worktrees` path component; every other librarian caller gates on it first). In a submodule `.git` holds `gitdir: ../.git/modules/x`; the helper returns `Some("..")`, so `suggest` offers `repo` instead of the submodule's name and reads `../.codescout/workspace.toml` against the server's working directory. With an absolute gitdir it offers the superproject's name, the very name collision the fix was meant to remove. The same happens for a linked worktree created with relative gitdir pointers.

**Probable cause:** the fix reused a helper whose name says "worktree" without reading its gate; the implementer verified the linked-worktree case and the plain-directory case, not the third `.git`-file shape.

**Workaround:** none at the time. The final-review process allows no second fix wave, so it was surfaced to the operator, who authorised wave 2 (commit `be094b671`, re-reviewed clean, full gate green): `suggest` calls `worktree_main_root` only when `is_linked_worktree(root)`, resolves a relative main root against the root, and has submodule and relative-pointer tests. call `worktree_main_root` only when `is_linked_worktree(root)`, resolve a relative main root against `root`, and add a submodule-shaped test. `src/agent/mod.rs:794,815` has the same ungated call (older code).

**Severity:** med

**Status:** fixed-verified

**Rests on:** a fix that reuses a helper inherits its gate; read the callers' guards, not only its signature.

**Fix idea / Pointer:** `src/librarian/identity/repo_name.rs:189-191`; re-review report in the Plan A ledger (final fix wave).

## F-8 — A spec requirement is built by no plan

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, final whole-branch review of Plan A.

**When:** comparing the branch with the spec's § Identity.

**Expected:** every requirement of the spec is built by a task or named in a plan that will build it.

**Got:** the spec (§ Identity, about L170-172) says onboarding "says when another checkout in the catalog uses" the repo name. Task 3 does not build it and no plan file schedules it: Plan A is the only plan written for this spec, and Plans B-E are only named in its header. Two further spec-level questions surfaced with it and are open: whether `.codescout/repo.toml` lives at the codescout project root (a workspace sub-project) or at the git root, and how already-onboarded projects, which rarely call `onboarding`, ever get a confirmed name.

**Probable cause:** the split into five plans moved requirements to later plans in the header only; no check mapped each spec sentence to a task.

**Workaround:** ticketed for Plan B (which has the catalog): the "another checkout uses this name" check, a `doctor` check or prompt for already-onboarded projects, remembering a declined dialog (`declined: true` in the block), and the root-versus-git-root ruling.

**Severity:** med

**Status:** open

**Rests on:** a split plan needs a spec-coverage table across all its parts, written when the split is made.

**Fix idea / Pointer:** write the Plan B requirement list from the spec's requirement sentences before dispatching anything.

## W-3 — Per-task review with named risks caught eight defects a green gate passed

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, executing Plan A with one implementer, one Opus task review and one scoped re-review per task, then a whole-branch review.

**Pattern:** every task review read the diff against named risks (each risk named from what the diff changes), and the reviewer was allowed to probe the parser or the filesystem in a scratch copy, never in the repo.

**Counterfactual:** the reviews found, before any merge: (1) the document key lowercased with Unicode rules, so a Kelvin sign in a filename became `k` and a non-ASCII name got a valid key (Task 4); (2) `repo.toml` read as "absent" when it was a dangling symlink, and written through links, so onboarding's read-then-write could create a file outside the repo from a cloned repository (Task 2); (3) an elicitation with no timeout inside a 60 s tool call that holds the write guard (Task 3; a slow answer would lose the whole onboarding response and refuse peers' writes); (4) a labelled wiki link in a table cell that could never be valid (Task 5); (5) a YAML block scalar containing a fence that swallowed the whole body and gave zero citations (Task 5); (6) a broken folder index silently handing its descendants' tokens to a far ancestor (Task 6, a flaw in my own ruling); (7) `[[text]](url)` read as a citation, 32 times in this repo (final review); (8) the worktree folder's name offered as the repo name (final review). A green gate, which every task had, caught none of these. The re-review of the last fix also caught a regression the fix introduced (F-7).

**Confirming data points:** 7 tasks, 7 task reviews, 7 re-reviews (six fix rounds and the final fix wave); every task but Task 7 needed one fix round; 1 Critical-grade defect class (write through a link) and 2 spec-level corrections (Tasks 4, 6).

**Impact:** high

**Promote-when:** a plan runs without per-task review and ships one of these classes, or the review cost per task exceeds the cost of the defects it finds.

**Status:** validated

**Rests on:** a green gate shows the tests that exist pass; only a reader who names the input that is missing finds the defect.

## W-4 — The census sized Plan E from the corpus: 45 renames, 37 duplicate groups, 144 token links

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, Plan A Task 7: an ignored test (`census_of_this_repo_under_the_new_identity_model`, `src/librarian/identity/census.rs` on the Plan A branch) ran the new key, grammar and index over every tracked `*.md` file at commit `c503c587a`. The repo name it used was the worktree directory name (`artifact-identity-plan-a`, source `suggested`; no `repo.toml` is tracked), so a citation `[[<that-name>/key]]` would resolve locally; none exists. The numbers are comparable only between runs that print the same name.

**Pattern:** measure the corpus under the new rules with the real code before planning the migration, and print the counts by cause, not only the total.

**Counterfactual:** without the census, Plan E would be sized from the old estimate (about 895 stamped ids) and from the idea that few files fail the key rule. Measured: 2704 tracked documents, 0 skipped as non-UTF-8; 45 `InvalidKey`, all `NonCanonical` (19 contain `_`, 25 contain a second `.`, 1 has a double hyphen), so about 45 renames; 82 of 86 uppercase-stem files have valid keys (uppercase alone is not an error); 37 duplicate-key groups covering 127 documents, only 1 involving a folder-index README, mostly fixture and evidence trees (14 `SKILL.md`, 10 `onboarding.md`, 10 `architecture.md` copies) and dated plan/spec pairs, so Plan B needs a key-space scope rule before it enforces uniqueness; 20 folder-index documents, 18 usable (the descendants of the 2 broken ones get no owner); 12,824 citations in total, of which 263 are grammar-form (File 68, Entry 2, CrossRepo 4, UnknownKey 11, Malformed 178), 7,947 are bare tokens not defined locally (`NotLocal`, expected noise) and 4,614 are `Local`. Of the 178 Malformed: 144 are the existing organic `[[F-16]]`-style entry-token links in `docs/trackers` (the largest rewrite for Plan E), 32 are Markdown links whose text starts with `[` (`[[arXiv]](url)`, fixed in the final fix wave by the rule that a wiki link directly followed by `(` is not a citation), and 2 are relative paths. The 4 CrossRepo are memory links `[[research/topic]]` that collide with the `repo/key` form; at least 5 of 10 sampled UnknownKey are TOML names written in prose (`[[roots]]`, `[[slug]]`). The census covers tracked files only; an indexer that walks the filesystem sees more.

**Confirming data points:** one run; the arithmetic was re-summed by the task reviewer (19 + 25 + 1 = 45; 144 + 2 + 32 = 178; 127 documents in 37 groups).

**Impact:** high

**Promote-when:** Plan E sizing is replaced by a measured migration run, or a second census on another repo disagrees with this one.

**Status:** validated

**Rests on:** a migration is sized by running the new rules over the real corpus, never by an estimate made before the rules existed.

## F-9 — Told the operator a peer held three dirty files; they were my own stopped implementer's leftovers

**Valid:** dated 2026-11-11

**Observed:** For several turns I told the operator that "a peer" held uncommitted edits in `src/prompts/mod.rs`, `src/tools/run_command/attribution.rs` and `tests/cli_doc.rs`, and the merge waited on that peer. I also tried to message a peer name (`codescout-37`) that no longer resolved. The edits were the leftovers of the first Task 1 implementer (agent `ac57719078e6dde19`), which I stopped after it edited the shared checkout (F-6). When I restored its 33 paths I left these three out, because I assumed a peer owned them.

**Evidence:** the new wording in all three diffs appears in that agent's transcript and in no peer's transcript. I searched both account profiles; one transcript over 64 MiB was searched with `rg` and had no match, and three large kat files were over the grep tool's size cap and were not searched, so the absence is strong but not proven. The diff was wording-only and byte-identical across two reads.

**Cost:** several turns waiting for an owner who did not exist, one false statement to the operator, and a real gap (F-10) that stayed hidden until I read the stray diff.

**Fix:** saved the diff, ported the two missing hunks onto the branch (commit `6671e2ce8`, now in `experiments`), then discarded the dirty files. A second dirty file later (`src/util/path_security.rs`) was attributed by the same transcript search and confirmed by asking its owner, who agreed.

**Lesson:** a dirty file in a shared checkout is not evidence of a peer author. Before telling the operator who holds it, search the transcripts for the exact new text, or ask. After any restore of "exactly these paths", list what is still dirty and test each remaining file against your own subagents' transcripts first.

## F-10 — Task 1 left a stale "librarian feature" message in tests/cli_doc.rs and a "librarian-less" comment in src/prompts/mod.rs

**Valid:** dated 2026-11-11

**Observed:** After Task 1 removed the `librarian` cargo feature, `tests/cli_doc.rs:91` still said the binary "was built WITHOUT the librarian feature", and a doc comment at `src/prompts/mod.rs:3390` still described a "librarian-less binary". The per-task review, the final review and the gate all passed with these in place. I found them only by comparing the stray edits of the first Task 1 implementer (F-9), which had already fixed both.

**Cost:** a stale failure message that would have sent a reader to rebuild a binary "with the librarian feature", which no longer exists. Nothing failed.

**Fix:** commit `6671e2ce8` rewrites both texts (the message now says the binary cannot run `doc --help`; the comment says a binary built without the default features). `gate.sh --check` was green, and the commit is in `experiments`.

**Cause:** not established. The two phrasings are "WITHOUT the librarian feature" and "librarian-less". Same class as F-5: prose that names a removed feature's effect, not its name.

**Lesson:** after removing a feature, search string literals and comments for the feature's effect as well as its name, and treat a stray implementer diff as a second opinion worth reading.

## W-5 — Merging the base into the branch in the worktree, then ff-only, kept six peers' staged files out of the merge

**Valid:** dated 2026-11-11

**Pattern:** to land a gated branch on a shared checkout whose base keeps moving, merge the base into the feature branch inside its own worktree, gate that tree, then run `git merge --ff-only <branch>` in the shared checkout.

**Counterfactual:** a real `git merge` in the shared checkout makes a merge commit from the index. The index held six files that other sessions had staged (a bug-triage memory, a 2026-11-04 issue, a research note, two data archives and a plan), so the merge commit would have swept all six in. The merge would also have refused over a peer's uncommitted `src/util/path_security.rs`. `experiments` moved four times while the gates ran. The last three moves were peer commits (`librarian_guard.rs`, a plan file, `path_security.rs`); the worktree absorbed each move without touching a peer's files.

**Evidence:** `git merge --ff-only` updated 59 files with the six staged files still staged afterwards. I checked for an overlap before the fast-forward, found `src/util/path_security.rs` dirty in the shared checkout, and did not run it until its owner had committed. I did not run the fast-forward while the file was dirty, so I did not observe git's refusal; I expect git would have refused.

**Limit:** the fast-forward only works while `experiments` is an ancestor of the branch. Each new peer commit costs another merge and, for Rust changes, another seven-minute gate.

## F-11 — The spec and the old catalog code disagree in about a dozen places a plan must settle

**Valid:** dated 2026-11-11

**Observed:** A scout of the old catalog code (Sonnet, read-only) and an independent Opus review of its report found places where the approved spec and the code disagree, or where the spec is silent. A plan written from the spec alone would have missed them. Status of each: V = I re-checked it in the code; R = reported by the scout or reviewer and not re-run by me.

**In the spec's Removed list but with a hidden dependant:**
- The `id:` stamp and params-as-store are two of THREE signals in `src/util/librarian_guard.rs`. The third is a declared `entry_prefix` (V, `librarian_guard.rs:141`). The guard has four call sites (V: `read_file.rs:200`, `edit_file/mod.rs:862`, `edit_markdown.rs:1572`, `read_markdown.rs:891`). Removing the stamp leaves no replacement for the other two signals' job.
- `entry_reservation` is the race guard for entry-id allocation (R, `augmentation.rs:1831-1975`); removing it needs per-entry files or a file lock.
- `catalog_meta` holds gc settings, reindex progress, audit watermarks and a backfill cursor, none rebuildable from files (R). Goal G1 needs a carve-out.

**Not mentioned in the spec:**
- `doc(graft)`, `rekey_prefix`, `fix=prune_missing` and `scope=repo|umbrella|all` (R).
- A third citation grammar on the write path, `resolve_cite_ref` (R).
- Consumers of hex ids or `catalog.db`: the companion `goal-stop-hook`, the `constitution-check` CLI, `scripts/file-provenance.py` (used by `fmt-mine.sh` in the gate, a companion hook and CI), and `audit_doc_refs` with its own resolver (R).
- Classification rules and the walk's ignore set come partly from the machine-global `~/.config/librarian/workspace.toml` and the untracked `.codescout/project.toml`, so goal G5 does not hold and a two-clone test would fail (R).

**Where the spec's premise is wrong about today's code:**
- Embedding vectors are not a content-hash cache; reuse is within one artifact only (R, `catalog/chunk.rs:416`).
- The indexer refuses linked worktrees (V, `indexer.rs:385`), so "a worktree is a checkout" needs that guard removed.
- "Frontmatter holds scalars only" contradicts `tags`, `owners` and `entry_prefix: [F, W]` (R).
- `AGENTS.md` and `contrib/pi/AGENTS.md` collide on key `agents`, and neither can be renamed (R).

**Cost if unaddressed:** plans C to E argue from a spec that omits a guard, a race guard and four consumers; the gate itself (`fmt-mine.sh` via `file-provenance.py`) would break at the flip.

**Fix:** open. Plan B amends the spec's "Open for the operator" section with a recommended answer for each item, then plans from the amended spec. The scout and review reports are the evidence.

## W-6 — An independent Opus review of the scout report corrected the guard, the entry points and the slice order

**Valid:** dated 2026-11-11

**Pattern:** before planning from a subagent's map of old code, send its report to an independent reviewer on a stronger model. Give the reviewer the report, the spec and the code, and no conclusion of the controller's.

**Counterfactual:** the scout (Sonnet) said the guard's two signals were the whole story, that `Catalog::open` writes on every open, that `reindex_cli` is a production entry point, that the guard has three call sites, and that retiring `catalog_audit` ships alone. The Opus review said otherwise. I re-checked three of its corrections in the code and they hold: a third guard signal (`librarian_guard.rs:141`), four guard call sites (`edit_markdown.rs:1572` was missed), and `reindex_cli` is `#[cfg(test)]` (`librarian/mod.rs:598`). The review also reported, and I did not re-run, that retiring `catalog_audit` does not ship alone (older binaries re-create the audit triggers on open), that the cutover export must precede the read flip, and four consumers the scout missed, one of which (`scripts/file-provenance.py`) the gate uses. A plan from the scout alone would have split the work with S2 first and without those edges.

**Cost:** the scout took about 25 minutes and 495k subagent tokens; the review about 13 minutes and 312k.

**Limit:** my own spot-check covered five of the scout's claims and three of the reviewer's. The rest of the review is reported, not verified. Before F-11 is closed, the reported items need one pass of their own.

## W-7 — An independent Opus review of Plans B1 and B2 found a gate that would refuse every edit to a colliding file, and five compile errors

**Valid:** dated 2026-11-11

**Pattern:** before a plan is executed, an independent Opus reviewer reads it against the spec and the real code, with no conclusions of the author, and names the test that goes red.

**Counterfactual:** the review of Plans B1 and B2 found a design flaw in my own gate and several errors in cited code. I re-checked three of them in the code and they hold: five `ProjectSection { … }` struct literals (three in `src/tools/run_command/tests.rs`, one in `config/project.rs:1000`, one in `onboarding.rs:1359`) would fail to compile with E0063 when a field is added, and my file list named only some; `grammar::Citation` has only `reference` and `line`, so the planned `raw` edge column could not be filled; and the model `open_db` for the new SQLite file is at `call_edges/cache.rs:115` and sets only the journal mode. The reviewer also reported, and I accepted from the plan text rather than from a run: the gate as written (`--diff-filter=ACMR`) would refuse any commit that edits a file already in a collision (33 duplicate keys and 18 bad names remain in this repo after the planned exclusions); the parse cache keyed by content and rules only would give a different result after a rename than a rebuild; 444 lines in `docs/trackers` repeat one token on a line, which breaks an edge key on `(line, raw)`; and the builder read the repo name and key-space file from the working tree, which defeats the two-clone goal.

**Fixes made:** the gate compares the staged index with `HEAD` and refuses only problems that are new; the cache key is `(sha256, rel_path, rules_hash, PARSER_VERSION)`; edges are keyed by their position in the document; the builder reads every input from the commit's blobs; shallow clones are `unverified`, not collisions; the onboarding scan runs only when a name is being chosen and only with an interactive client; B1 runs before B2.

**Limit:** the plans are not yet run. This review is reported work; the first execution will show what it missed.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     doc(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...",
              index_row="| {id} | <date> | <sev> | <cat> | open | **<t>** — … |",
              index_after_line="<the table line your row goes AFTER>")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark, stamps `**Valid:** dated
     <today>` AND writes the index row with `{id}` filled in — one write.
     A row needs an anchor from SOMEWHERE: `index_after_line` per call, or
     a `snapshot_anchor` declared once in this file's frontmatter, after
     which `index_row` alone lands the row at the block's TAIL. Passing
     `index_after_line` with no `index_row` is refused; they are not
     symmetric. A per-call anchor must be UNIQUE (first match wins,
     silently) and decides top vs bottom — prefer the last existing row.
     Do not hand-allocate ids; do not type the row yourself ahead of the
     call. -->
