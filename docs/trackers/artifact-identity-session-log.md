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
entry_high_water_F: 30
entry_high_water_W: 13
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
| F-8 | 2026-11-11 | med | plan-prose | fixed-verified | **A spec requirement is built by no plan: "onboarding says when another checkout uses the name"** — plus two open spec-level questions |
| F-9 | 2026-11-11 | med | self-friction | fixed-verified | **Told the operator a peer held three dirty files; they were my own stopped implementer's leftovers** — attributed by adjacency; a transcript search found the real author |
| F-10 | 2026-11-11 | low | plan-prose | fixed-verified | **Task 1 left a stale "librarian feature" message in `tests/cli_doc.rs` and a "librarian-less" comment in `src/prompts/mod.rs`** — passed both reviews; fixed in 6671e2ce8 |
| F-11 | 2026-11-11 | high | architectural | open | **The spec and the old catalog code disagree in about a dozen places a plan must settle** — a third guard signal, the entry-id race guard, four consumers, G5, vectors; 3 verified, the rest reported |
| F-12 | 2026-11-12 | low | review-gap | fixed-verified | **No review ran tests/result_caps, so an unannotated cap constant reached the first full gate** — `CHECKOUTS_LISTED_MAX`; fixed in 6eef416c2 |
| F-13 | 2026-11-12 | med | shared-checkout | promoted-to-bug-tracker | **Landing Plan B1 took four gates because the base moved twice and a peer's LSP external-write tests fail intermittently** — branch 2 of 6 lane runs, clean `experiments` 1 of 6; cause not found |
| F-14 | 2026-11-12 | med | plan-prose | open | **Key-space census after the B1 exclusions: 26 duplicate-key groups (54 files), 17 invalid-key files, 9 of them not renamable** — 14 groups are plan and spec pairs kept in scope; the spec does not yet say so |
| F-15 | 2026-11-12 | med | architectural | mitigated | **Shadow diff of experiments HEAD: the closed grammar sees 26 of the old catalog's 3812 file-grain edges; the rest is migration work, not a builder defect** — 4764 of 4770 only-old edge rows have a text form the grammar does not read; 4924 Unexplained of 5678 rows |
| F-16 | 2026-11-13 | med | shared-checkout | mitigated | **A background gate ran while an implementer edited the same worktree, so its green could not be trusted** — the output carries no commit id; discarded and rerun on an untouched tree |
| F-17 | 2026-11-14 | med | plan-prose | fixed-verified | **The A-R2 writability probe trips the read-door source gate** — `OpenOptions::new` in `src/librarian/util.rs` counts as a raw read; Task 1 Step 4 does not run that test |
| F-18 | 2026-11-14 | med | plan-prose | fixed-verified | **An atomic replace needs a writable directory; the plan calls the new refusal a failed write and mis-predicts its test as already green** — in-place `fs::write` needs only the file's permission |
| F-19 | 2026-11-14 | high | architectural | fixed-verified | **Plan C3's age eviction deletes the vectors of stable content** — the stamp and marker skips mean a stable file's rows are never hit, so they age out at 90 days |
| F-20 | 2026-11-14 | med | plan-prose | mitigated | **Plan C3 R5 understates the cold-cache hole: every stamped artifact is unqueued, not 26.6% of chunks** — the cache starts empty for the whole corpus; the scout's "first reindex embeds it anyway" is refuted |
| F-21 | 2026-11-14 | med | plan-prose | fixed-verified | **Plan C3 Task 2's text-scan guard does not survive rustfmt** — the chained call is about 113 columns, so `.with_vector_cache(model, ` matches 0 times after formatting |
| F-22 | 2026-11-14 | med | plan-prose | fixed-verified | **Plan C3 contradicts two SQLite documents: WITHOUT ROWID with 3 KB blobs, and "delete the file at any time"** — withoutrowid.html § 4; howtocorrupt.html § 2.5 |
| F-23 | 2026-11-14 | low | plan-prose | fixed-verified | **Plan C3: five small drifts between its text and the code** — guide claims a model switch-back reads the cache; R11 "one extra embed" ignores concurrency; a failed open is sticky; two tests cannot fail as named |
| F-24 | 2026-11-14 | med | plan-prose | fixed-verified | **Plan C3 Task 1: four store tests pass on the broken build they should catch** — the flush counter is kept by the caller of the write, so a per-row commit stays at 1; symmetric tally inputs; WAL unpinned; only a short blob tested |
| F-25 | 2026-11-14 | high | plan-prose | mitigated | **Plan D renames and rewrites markdown that code, harnesses and mdBook read by path, and can create a key collision** — `include_str!` targets, `.github` templates, `_TEMPLATE.md`, two `render_prompt.md` to one key, SUMMARY.md links |
| F-26 | 2026-11-14 | high | plan-prose | fixed-verified | **Plan D drops augmentation params outside the entry collection with no report line** — also unshown fields of body-defined entries and a stale sidecar's newer shape; Task 8 asserts no derived row |
| F-27 | 2026-11-14 | med | plan-prose | mitigated | **Plan D's plan of the committed export misreports exported observations and renamed documents as unrecoverable** — `splice_event` refuses the duplicate id; old `rel_path`s miss HEAD; the e2e checks writes, not the tally |
| F-28 | 2026-11-14 | med | plan-prose | mitigated | **Plan D's citation rewrite can write the wrong bytes or guess** — lowercase-only qualifier regex vs its own `My_Notes:F-2` test; merged Text runs across escapes; tokens inside URLs; history maps a deleted file's id by key equality |
| F-29 | 2026-11-14 | low | plan-prose | mitigated | **Plan D: nine smaller drifts between its text and the code** — a byte-for-byte stamp test vs `frontmatter::write`; fence length; a refusal with no command; two dry runs reading two catalogs; no side-file re-check; report overwritten |
| F-30 | 2026-11-14 | high | review-gap | fixed-verified | **Plan D Task 1: the side-file refusal misses a symlinked catalog path, so an immutable open reads stale rows silently** — SQLite keeps `-wal` beside the symlink target; measured 0 rows read vs 1 held |

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
| W-8 | 2026-11-12 | high | whole-branch Opus review after per-task reviews | a hook that refuses pure moves of colliding files; a dismissed dialog stored as a permanent decline; a nudge on read-only activations | validated |
| W-9 | 2026-11-12 | high | independent Opus review of a scout report before planning | a flock the CLI never takes; a vector table that breaks the schema's determinism rule; a cache key that ignores model, budget and dimension; four missed writers | validated |
| W-10 | 2026-11-13 | high | two independent Opus reviews of a plan before execution | cross-repo cites written as unqualified keys; a duplicate-key check counting worktree shadow rows; a renumber check reading the survivor body; tasks that end red on clippy; gates that stay green while the code refuses the recipe | validated |
| W-11 | 2026-11-14 | med | independent review of a plan: read every named test and caller | a probe that turns the read-door source gate red after the Task 1 commit; a test predicted green that is red | validated |
| W-12 | 2026-11-14 | high | independent review of a plan: read each fill and eviction rule against the path that reaches it | a cache that starts empty and evicts stable content at 90 days while every test passes; a guard rustfmt breaks | validated |
| W-13 | 2026-11-14 | high | independent review of a plan: test the riskiest claim with a scratch experiment and each rule against the real file list | an immutable open that reads stale data without the side-file refusal; renames of `include_str!` targets and harness-named files | validated |

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

**Status:** fixed-verified

**Update 2026-11-12:** the spec's Decision log answered both open questions: the repo name file lives at the git root, and an already-onboarded project is asked for the name on its next `onboarding` call. Plan B1 built the rest; checked in the code on `experiments`: `onboarding(repo_name=...)` returns the name block and records a declined time (`repo_name_declined_at` in `src/tools/onboarding.rs`), and `src/librarian/identity/checkouts.rs` scans for other checkouts by first commit. The doctor check named under Workaround was replaced by the on-next-onboarding rule. Checked by reading the code, not by running the dialog.

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

**Update 2026-11-12:** the spec's Decision log records the amendments as approved on 2026-11-11 (one guard signal, a separate machine-state file, a lock plus the high-water mark for entry ids, committed config as the index's inputs, `contrib/**` outside the key space, scalars in frontmatter, the tool changes). The build side is not done: the guard replacement, the entry-id lock, the consumers and the vector cache belong to Plans C, D and E. Status stays open until those plans build them.

**Fix:** open. Plan B amends the spec's "Open for the operator" section with a recommended answer for each item, then plans from the amended spec. The scout and review reports are the evidence.

**Status:** open

**Sweep 2026-11-16:** Built on integration: `contrib/**` outside the key space (`01c57d013`), builder inputs from the commit only (`f6aca6c4f`), the vector cache (`013113b7f`, `18dc50fcc`), and cites written as grammar text (`2694348ea`). Not built, and assigned to Plans E4 to E8 by the Plan E scout rulings of 2026-11-15: the three-signal guard, `entry_reservation`, `catalog_meta`, `graft` and `prune_missing`, `file-provenance.py`, and the old indexer's worktree refusal. The row stays open until E4 to E8 build them.

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

## F-12 — No review ran tests/result_caps, so an unannotated cap constant reached the first full gate

**Observed:** 2026-11-12, Plan B1 final gate.

**When:** The first full `./scripts/gate.sh` on the finished Plan B1 branch (`38b328d61`).

**Expected:** Every task review and the whole-branch review had passed, so the gate should be green.

**Got:** The gate failed on `result_caps every_cap_constant_is_classified`. The constant `CHECKOUTS_LISTED_MAX` in `src/tools/onboarding.rs` had no `// cap-class:` annotation and no probe row in `src/tools/core/cap_probe.rs`.

**Probable cause:** No implementer and no reviewer ran `tests/result_caps.rs`. The controller had ruled (R6) that implementers run targeted checks only, and the new bound sits in a file whose own tests do not include the cap gate.

**Workaround:** Commit `6eef416c2` annotated the constant and added the probe row `onboarding.checkouts_listed`. The second full gate was green.

**Severity:** low

**Status:** fixed-verified

**Valid:** dated 2026-11-12

**Rests on:** A green targeted run is evidence only for the tests it ran. The whole-suite gate exists for the rules that nobody thinks to run.

**Fix idea / Pointer:** A brief for a task that adds a result-shaping bound should name `tests/result_caps.rs` as a required targeted run.

---

## F-13 — Landing Plan B1 took four gates because the base moved twice and a peer's LSP external-write tests fail intermittently

**Observed:** 2026-11-12, landing Plan B1 on `experiments`.

**When:** Following the W-5 route: merge `experiments` into the branch, gate that tree, then `git merge --ff-only` in the shared checkout.

**Expected:** One gate, then the fast-forward.

**Got:** Four gate runs and three merges, because `experiments` moved twice during the gates (`c7b80749d`, then `56b3eb525`) and a peer's LSP tests failed intermittently:

- Gate A (branch `ec51112f0`): green. FMT, CLIPPY, LEAN and DEFAULT exited 0, and Python passed 1836 tests.
- Gate B (after the second merge): the lean lane failed `lsp::client::tests::goto_definition_follows_a_write_made_outside_codescout` and `hover_follows_a_write_made_outside_codescout`. The default lane passed 9735 tests. The two tests passed 3 of 3 when run alone.
- Gate C (after the third merge): the lean lane passed. The default lane failed `goto_definition_follows_a_write_made_outside_codescout` and passed 9742 tests.
- A clean checkout of `experiments` at `56b3eb525`, with none of the branch's commits, ran each lane three times. The lean lane passed 3 of 3. The default lane passed 2 of 3 and failed `rename_edit_does_not_land_on_another_callee_after_a_same_size_write_outside_codescout` once.

The branch failed in 2 of 6 lane runs and `experiments` failed in 1 of 6, which this sample size cannot tell apart. The failing test was not the same one on `experiments`. All failing tests belong to the same group: the external-write tests in `src/lsp/client.rs`.

**Probable cause:** Not found. The failures are fast (about 0.6 s) and look like a race between an external write and the language server's view of the file. A peer was changing `src/lsp/client.rs` during the gates. This is a hypothesis, not a measurement.

**Workaround:** The operator said "land it if experiments fails too". The fast-forward landed `a79985050` on `experiments`, with nothing pushed. The peers' staged and modified paths were identical before and after.

**Severity:** med

**Status:** promoted-to-bug-tracker

**Valid:** dated 2026-11-12

**Rests on:** A gate result is evidence about the tree it ran on. An intermittent test needs a rate on the base tree before a branch can be blamed for it or cleared of it.

**Fix idea / Pointer:** `docs/issues/2026-11-12-lsp-external-write-tests-fail-intermittently-in-the-full-suite.md`

---

## W-8 — A whole-branch Opus review found four defects that passed every per-task review

**Observed:** 2026-11-12, Plan B1 whole-branch review.

**Pattern:** A fresh Opus reviewer reads the whole branch once, after every per-task review has passed, and reports defects that cross task boundaries. The controller then fixes them in one wave through the resumed implementer, with a ruling in the ledger for each choice.

**Counterfactual:** Without it, four defects would have landed. Each had passed its own task review:

- The gate compared collisions by path set, so a pure move or swap of a colliding file was refused as a new problem (R11). The pre-commit hook would have blocked ordinary renames.
- A dismissed repo-name dialog was written as a permanent decline (R12). The repo would never have been named, and the only way back is a call that nobody knows.
- The `repo_name_unconfirmed` note appeared on read-only activations (R13). It pushed agents to write into repos that the user was only browsing.
- The remembered-decline block still told the agent to ask the user (R14).

**Confirming data points:** The same review also found that no task had run `tests/result_caps.rs` (see F-12). The earlier Task 2 and Task 7 reviews found shape-level defects in single tasks. Only the whole-branch view found the cross-task contradiction between R12 and the plan text.

**Impact:** high

**Promote-when:** A second plan in this work stream shows the same pattern, a defect that passed its task review and failed only the whole-branch review. Then add it to the subagent-driven-development notes in `docs/conventions/`.

**Status:** validated

**Valid:** dated 2026-11-12

**Rests on:** A per-task review checks a task against its brief. Only a whole-branch review checks the tasks against each other.

---

## F-14 — Key-space census after the B1 exclusions: 26 duplicate-key groups, 17 invalid-key files, 9 not renamable

**Observed:** 2026-11-12, Plan B1 Task 4, which authored `.codescout/key-space.toml` and ran the gate over the repo.

**When:** Sizing the migration (Plans D and E) against the key space that B1 committed.

**Expected:** W-4 measured 45 renames, 37 duplicate-key groups and 144 `[[F-16]]` rewrites over the whole tree, before any exclusions.

**Got:** With the committed exclusions, the census reads:

- 2566 markdown files are inside the key space, and 176 are outside it.
- The gate reports 43 problems: 26 duplicate-key groups covering 54 files, and 17 invalid-key files.
- No folder index is partly excluded.
- Of the 17 invalid-key files, 9 cannot be renamed: 3 GitHub templates and 6 prompt templates. Their names are dictated from outside, so the migration must exclude them or accept a rename.
- 14 of the 26 duplicate-key groups are plan and spec pairs that share a stem. They stay in scope, which departs from the spec's named example. The decision is not yet recorded in the spec.

**Probable cause:** The W-4 counts came before the exclusion rules existed. R7 and R8 moved whole subtrees out of the key space.

**Workaround:** None needed for B1. Plans D and E must size their work from these counts and settle the 9 files and the 14 pairs.

**Severity:** med

**Status:** open

**Sweep 2026-11-16:** The spec (design lines 289 and 379) still puts dated plan and spec pairs outside the key space. The committed `key-space.toml` keeps them in. No ruling settles the conflict. Plan D freezes the 9 unrenamable files (D-R26) and reports every key collision (D-R14) (`c14fe1820`, `4fb5ab77c`). The sweep reports that only 6 of the 14 groups are plan and spec pairs, not 14; the controller did not re-count them.

**Valid:** dated 2026-11-12

**Rests on:** A migration is sized from the key space it will run against, not from a count taken before the scope was set.

**Fix idea / Pointer:** Record the plan-and-spec-pair decision in `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`, and carry the 9 unrenamable files into the Plan D design.

---

## F-15 — Shadow diff of experiments HEAD: the closed grammar sees 26 of the old catalog's 3812 file-grain edges; the rest is migration work, not a builder defect

**Observed:** Plan B2 Task 7 ran `codescout derive-shadow --project <main checkout> --rev HEAD --json` from the B2 worktree binary (commit 2c66232d4 plus the ignored test 74c13818b) against the live machine-global catalog. The compared commit was `experiments` HEAD 1c92b2aa7. The command read git objects only. It wrote nothing to the catalog or to the main checkout. Full report: `.superpowers/sdd/2026-11-11-artifact-identity-plan-b2-derived-index-builder/task-7-report.md` in the B2 worktree.

**Valid:** dated 2026-11-12

**Derived index of that commit.** 2587 documents, 2231 entries, 4886 edges, 46872 chunks, 43 problems, 176 skipped (all `outside_key_space`; no `not_utf8`, no symlink).
- Edges by kind: local 4683, malformed 144, file 43, unknown_key 14, entry 2.
- Problems: duplicate_key 26 (plan and spec pairs that share a file stem: 52 documents), invalid_key 17 (underscore in the file name).
- Two documents carry `parse_note = frontmatter_error`.

**Old catalog, same scope** (3099 in-scope artifacts at the diff, 3100 a minute later: the catalog is live). `artifact_link` rows with both ends in scope 3820 (3810 `cites`). `entry_cite` rows whose `src_slug` is in scope 3583. Chunk rows of in-scope artifacts 51724. Excluded: 5066 rows under other roots, 2 worktree copies, 0 `missing_since`.

**Diff.** old 3099, new 2587, matched 2582. Only-old 5517, only-new 52, changed 109 rows. By cause: unexplained 4924, file_set_walk_vs_commit 466, closed_grammar_drops_edge 230, key_outside_space 56, parse_rule 2.

**The six rulings, measured on this repo.**
1. Listing (tracked `*.md`, symlinks skipped): 461 only-old documents are untracked or staged-new files (458 `??`, 1 `A `, 2 `AM`); all 461 checked by script. 5 only-new documents are in the commit and absent from the old catalog (the old walk does not hold `.codescout/measurements/**`; reason not opened).
2. One frontmatter rule: 2 documents indexed as `frontmatter_error`, both show as `parse_rule` title rows. No `not_utf8` file.
3. Key space: 176 skipped plus 56 only-old documents with cause `key_outside_space`.
4. Committed inputs only: 25 `kind` rows and about 10 `status` rows differ because machine-global `~/.config/librarian/workspace.toml` rules (`**/CLAUDE.md`, `**/README.md`, `**/docs/plans/**/*.md`, `**/*-tracker.md`) are not applied. Verified: each of the 25 paths matches the glob that gives the old kind.
5. Deferred to Plan C: 26 `chunk_count` rows. 11 are dirty files. The other 15 have working-tree sha equal to the old `file_sha256`, but the old chunks come from another text (never embedded, or edited since). Verified by query for all 26.
6. Commit-only reads: 13 `file_sha256` rows are the 13 tracked files that carry uncommitted edits. For all 13 the working-tree hash equals the old catalog hash, so the old row holds the working text and the derived row holds the committed text. Tags (8), title (8), kind (8) and status (9) rows on the same 13 paths are the same gap.

**The finding.** 4764 of the 4770 only-old edge rows (3406 file grain, 1364 entry grain) have a committed-text form that the closed grammar does not read: relative path links (563), 16-hex ids (720), bare tokens (849 in prose, 860 only in inline code or headings), `stem:TOKEN` (383), and tokens in inline code or headings. The old scanner reads those forms. It does not enable wiki links. The derived index holds 43 `file` edges and 2 `entry` edges of the `[[...]]` form in the whole repo, and 26 file-grain edges were compared against the old 3812. Verified: one sample row per group, both sides opened (old catalog row by read-only query; derived row and committed blob), and the mechanism in `link_scan/extract.rs` and `identity/grammar.rs`. Per-form row counts come from a script over the committed blobs, not from a replay of the old scanner. A flip to the derived index loses these edges unless the migration rewrites the text to `[[key]]` or the grammar admits more forms. This is a Plan D and Plan E decision. No builder or diff defect was found.

**Not established (suspected only).**
- 4 file rows and 2 entry rows have no text support in the commit or the working tree (8 old `cites` rows from `src/prompts/guides/tracker-conventions.md`; `pre-T-4` and `post-T-4`). They may be stale rows or hand-written `cites` links; `artifact_link` has no origin column.
- 23 only-new entry rows, all same-document `local` edges in ledgers. In the sample, the old catalog holds no `entry_cite` row from the source ledger at all. Why is not known.
- 24 only-new file rows are `[[key]]` links the old scanner cannot read (verified for one).
- 11 only-old file rows name a bare token that has several definers (indicator only).

**Follow-up (open, not done here).** Do not widen the causes to hide these rows.
- Ruling for the user: whether the diff gains derived-side cause rules. 64 rows are the same citation recorded as a derived `malformed` edge (`[[R-79]]` style): 16 file-grain, 19 cross-document entry, 29 same-document entry. Evidence exists (source blob, `edge.line`, malformed edges). Verified for one sample.
- Plan D: the migration must emit `[[entry-key]]` for each token that becomes an entry, and must size the 3406 plus 1364 rewrites. Compare W-4 (144 `[[F-16]]` rewrites).
- 26 duplicate-key groups and 17 invalid-key documents exclude those documents from key resolution (see F-14).
- Resolve the six suspected items above before Plan E reads from the derived index.

**Status:** mitigated

**Sweep 2026-11-16:** The cutover rewrite is built (`c14fe1820`). The 64 malformed rows have a ruling: no derived-side cause rules, revisit after the Plan D dry run. The six suspected items are still unresolved and are not on the E4 duty list.

## W-9 — Independent Opus review of the Plan C scout before any plan was written

**Valid:** dated 2026-11-12

**Pattern:** Before writing Plan C, a Sonnet scout read the code the plan touches and wrote a report with each claim tagged verified or reported. A separate Opus reviewer then opened the code again for the load-bearing claims and ran its own sweeps for writers and readers. The plan writer builds from the corrected facts, not from the scout alone.

**What it caught.**
- The scout said every doc write takes the project flock. The reviewer found that the CLI write commands call tool bodies with no lock. I re-checked this: `acquire_write_guard` has one production caller, `src/server.rs:1138`, and no file under `src/cli` uses it.
- The scout proposed a plain blob table in the derived file for the vector cache. The reviewer showed it passes the determinism test but breaks the schema's own rule that every stored row is deterministic, because an embedding is not a pure function of its input.
- The scout's cache key (`embed_input_hash`) ignores the model, the budget and the dimension. The reviewer found that the code index already treats a remote model name as untrustworthy.
- The writer inventory missed `sources`, `event_edges` (the `resolves` edge has readers), `commit_refresh`, and the renumbering of params ids in graft and merge. The renumbering would make params and markdown disagree once entries are dual-written.
- The scout never stated the file-versus-catalog write order, and the four write tools disagree: create writes the catalog first, update and append write the file first.
- The scout said the vector and git-history caches can start now. They have no persistent derived file, no build trigger and no consumer.

**Counterfactual.** A plan built from the scout alone would have assumed a lock that the CLI does not take, specified a vector table that violates the schema invariant, keyed it on a name that can lie, and left a params-versus-markdown disagreement in graft and merge. Each would have surfaced in an implementer's retry or in a final review, not at planning time. The cost of the pattern was one reviewer run (about 490 s).

**Evidence:** the scout report and the condensed review are session-scratch files; the corrected facts are in the Plan C1 file when it lands.

**Status:** validated

## F-16 — A background gate ran while an implementer edited the same worktree, so its green could not be trusted

**Valid:** dated 2026-11-13

**Observed:** Plan C1 Task 4, 2026-11-13. A full gate ran in the background on commit 1f165952a while the Task 4 implementer, resumed for a fix round, edited, staged and committed in the same worktree. The gate came back green, and I nearly recorded it as the Task 4 result.

**Expected:** a gate result names the tree it ran on.

**Got:** the implementer told me afterwards that the gate may have compiled a mix of 1f165952a and the next round's sources. I could not tell from the gate output, which carries no commit id. I discarded the green, ran a new gate on 7c9017252 with nobody editing, and recorded the first as unreliable.

**Probable cause:** I started the gate and the scoped re-review in parallel to save time, and the implementer still held an open fix round. The re-review is read-only. The implementer is not.

**Workaround:** a gate runs only while no implementer holds an open round on that worktree. From Task 6 on, the controller dispatches the next implementer or fix round only after the gate wake. The re-review may run beside a gate because it only reads.

**Severity:** med

**Status:** mitigated

**Sweep 2026-11-16:** Only the process rule guards this: run a gate only when no implementer holds an open round on that worktree. `scripts/gate.sh` still does not print the commit or the dirty state it ran on. Its only `git` call is line 69 (`git rev-parse --show-toplevel`).

**Rests on:** a green result is evidence for the tree it ran on; a gate that does not print the commit it built cannot prove which tree that was.

**Fix idea / Pointer:** `scripts/gate.sh` could print `git rev-parse HEAD` and `git status --porcelain` at its start and end, so a result names its tree and a mid-run edit is visible.

## W-10 — Two independent Opus reviews of Plan C1 before any implementer ran

**Valid:** dated 2026-11-13

**Pattern:** Plan C1 was written by one agent and then reviewed twice by independent Opus reviewers, each told to distrust the plan and to open the code for every symbol, signature and line the plan names. Each review produced concrete plan-text fixes, and the writer applied them in a revision commit.

**What they caught (before any implementer ran).**
- First review (commit 9febe63d3): two Critical and eight Important findings. Cross-repo cites would have been written as unqualified `[[key]]` and resolved in the wrong repository; the duplicate-key check counted worktree shadow rows and `//` twin rows, so it would have refused ordinary cites; Task 1 left the tree red at its own step; Task 4 turned tests red in four unlisted files and three recipe gates would have stayed green while the code refused the recipes they check; Task 6's renumber check read the survivor's body, so after Task 4 it would have refused almost every collision; the guide heading pins and a 128-byte budget margin were not named; `rekey_prefix` would have left the new `**Cites:**` text stale; Task 4 was too large for one pass.
- Second review (commit a5b5afdae): eight more Important findings. Tasks 1 and 3 ended red on clippy dead code; the prose retry remedy was wrong after a peer's append; the `tool_surface_under_budget` gate (an exact measured total) was not named; two managed ledgers held sectionless recipes the extended gate would refuse; `artifact.rs:604`'s planned fix could not work on in-memory fixtures; a gone worktree with a legacy params-only entry slipped past the renumber check; one CLI lock test could not fail under its own mutation; the sidecar-prompt route through `doc(augment)` forks shadow rows and reformats files.

**Counterfactual.** Each of these would have surfaced as a failed implementer run and a fix round, or a Critical in the whole-branch review. Several (the cross-repo qualification, the survivor-body renumber check) would have produced wrong committed text or a silent params-versus-markdown split. The cost of the pattern was two reviewer runs of about 14 and 14 minutes and two revision rounds by the writer.

**Evidence:** the plan's own history (9febe63d3, a5b5afdae, 3e2feac9e) on the C1 branch; the condensed review `plan-c1-review.md` in the session scratchpad.

**Status:** validated

## F-17 — The A-R2 writability probe trips the read-door source gate in src/librarian

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent review of plan `docs/superpowers/plans/2026-11-14-atomic-catalogued-write-door.md` (bug a806db1ec4ea162b) before execution.

**When:** checking ruling A-R2 (probe writability with `OpenOptions::new().write(true).open(path)` inside `write_catalogued_file`, `src/librarian/util.rs:85`).

**Expected:** the plan names no test outside `util`, `dual_write`, `tools::link`, `event_create`, `augmentation_sidecar` and `identity` that the probe affects.

**Got:** `no_librarian_production_read_bypasses_the_read_door` (`src/librarian/read_door.rs:1414`) scans every production `.rs` file under `src/librarian` and counts any path that names `fs::OpenOptions`, `OpenOptions::new` or `File::options` as a raw content read (`path_reads`, `read_door.rs:1326-1338`). Only `read_door.rs` itself and the `ALLOWED` list (`read_door.rs:1037`) are exempt. A probe written as A-R2 says, in `util.rs`, turns that test red. Task 1 Step 4 does not run it, so the red first shows at the Task 4 gate, after the Task 1 commit.

**Probable cause:** the plan was written from the door and its callers; the source gate is a separate test that judges spelling, not intent.

**Workaround:** none needed yet (review only). Plan change: put the probe in `crate::util::fs` beside `atomic_write` (outside the gate's scan), or add an `ALLOWED` entry for `librarian/util.rs` with a reason ("write-only open, no content read"); add `cargo test --lib read_door` to Task 1 Step 4.

**Severity:** med

**Status:** fixed-verified

**Sweep 2026-11-16:** Fixed in `4bae72783`. The probe is `refuse_unwritable_existing` in `src/util/fs.rs`, outside the read-door gate's scan. The sweep ran the gate test and the probe tests at `56dd96fe3`: 10 passed on a clean tree.

**Rests on:** a source gate that judges spelling counts every new spelling of its pattern, whatever the intent.

**Fix idea / Pointer:** plan Task 1 Step 3 and Step 4; ruling A-R2.

---

## F-18 — An atomic replace needs a writable directory, which the in-place write did not

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent review of plan `docs/superpowers/plans/2026-11-14-atomic-catalogued-write-door.md` before execution.

**When:** checking the Task 1 test `a_write_that_cannot_stage_leaves_the_old_bytes` (directory mode 0555, writable file) and the Step 2 claim "against the current code the staging test already passes".

**Expected:** the current door fails in a read-only directory, so the test is a regression guard.

**Got:** the current door is `std::fs::write` (`src/librarian/util.rs:92`), which opens the EXISTING file with `O_TRUNC`; that needs write permission on the file only, not on the directory. So on the current code the write succeeds and the test (which expects an error) is RED. After the change, `atomic_write` needs to create a sibling staging file (`create_staging_file`, `src/util/fs.rs:343-377`) and rename it (`fs.rs:493`), which needs write and search permission on the DIRECTORY. A writable document in a directory the writer cannot write was updatable before and is refused after. The plan presents this case as "a failed write" (Review Focus 1) and has no ruling with its cost. Same class: a file that is a bind-mount point (rename gives `EBUSY`), extra free space equal to the document size, owner/group/ACL/xattr not kept (the new inode belongs to the writer).

**Probable cause:** the plan treats in-place and replace as equal in what they may write; they differ in which permission they need.

**Workaround:** none needed yet. Plan change: correct Step 2's prediction; add a ruling (for example A-R7) that names the lost capability and its cost, and keep the test as the pin of the new behaviour.

**Severity:** med

**Status:** fixed-verified

**Sweep 2026-11-16:** Fixed in `4bae72783` and `0f26f04fd`. Ruling A-R7 accepts the lost capability and names its cost. `write_catalogued_file` documents it (`src/librarian/util.rs:108-122`). The read-only-directory test (`util.rs:911-946`) passes at `56dd96fe3`.

**Rests on:** an atomic replace changes the permission a write needs from the file to its directory.

**Fix idea / Pointer:** plan Task 1 Step 1/Step 2; Rulings.

---

## W-11 — Reading the five read-only tests and the 19 callers confirmed the plan's Task 1 claims and found two gaps

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent review of plan `docs/superpowers/plans/2026-11-14-atomic-catalogued-write-door.md` before execution.

**Pattern:** before execution, read each test the plan says will "still pass" and each caller the plan says the compiler will judge, instead of trusting the plan's list.

**What it confirmed:** all 19 references to `write_catalogued_file` pass text (`String`, `&String`, `&str`, `format!`), so A-R3 `impl AsRef<str>` holds. The five mode-0444 tests (`dual_write.rs:929`, `link.rs:1119` and `:1278`, `event_create.rs:1591`, `link_dir_tests.rs:2912`) all write through the door, and after Task 1 each fails at the A-R2 probe with an `io::Error`, as the plan says. The probe is load-bearing for `doc_move_completes_and_reports_when_only_the_id_repair_write_fails`: without it `atomic_write` replaces the 0444 file in its writable directory, the id repair succeeds and the test fails. That test is run by Task 1 Step 4 only because the filter `tools::link` is a substring of `tools::link_dir_tests`.

**What it caught:** F-17 (the probe trips the read-door source gate) and F-18 (directory permission); and that the link test `a_write_failure_after_the_destination_write_names_the_destination_change` asserts no "partly written" sentence, though Task 2 lists it.

**Counterfactual:** without the scout, Task 1 commits a change whose first red shows in the Task 4 gate (F-17), and Step 2's report states a wrong RED/GREEN split (F-18).

**Status:** validated

---

## F-19 — Plan C3's age eviction deletes the vectors of stable content, because nothing ever touches a row the stamp skips

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent review of plan `docs/superpowers/plans/2026-11-14-artifact-identity-plan-c3-vector-cache.md` (uncommitted draft) before execution.

**When:** checking C3-R4 (age eviction, `VECTOR_CACHE_RETENTION_DAYS = 90`) against the reindex queue.

**Expected:** a row stays while its content is live, so the spec's "a rebuild reuses them" (spec L230) holds for the cutover rebuild.

**Got:** a row's `last_used` is set only by a put or a hit (plan Task 1 Step 3, get/flush rules). A stable file is never looked up: an artifact whose `embedded_sha256` stamp equals its file hash is not queued at all (`src/librarian/indexer.rs:671-697`, `needs_embed`), and a queued chunk with a current marker skips the seam (`src/librarian/tools/reindex.rs:628`, `item.up_to_date && !reembed`). So 90 days after its embed, any reindex that touched the cache (in any repo: the file is per user) deletes the vectors of every file that did not change or move. The cache then holds only content embedded in the last 90 days, and a rebuild or a move of older content re-embeds. Every plan test embeds fresh content inside one run, so all stay green. The plan states R4's cost only as "a switch back to a model unused for 90 days misses".

**Probable cause:** "unused" was defined as "not hit", but the stamp and marker skips mean stable content is never asked for.

**Workaround:** none needed yet (review only). Plan change, controller's choice: (a) no eviction in C3, follow-up with a live-set rule; or (b) at the end of a reindex, touch `(model_key, embedded_input_hash)` for every marked chunk under the reindexed targets, in one transaction; plus a test that a row whose hash is a live marker survives an eviction at day 91.

**Severity:** high

**Status:** fixed-verified

**Sweep 2026-11-16:** Plan C3 shipped with no eviction (C3-R4) and no `last_used` column, and a test pins that (`013113b7f`, `vector_cache.rs:642`). The cache file only grows, as the manual says. A later eviction must avoid this trap (C3 state doc, follow-up 2).

**Rests on:** an LRU clock advances only on reads, and a cache behind a skip is never read.

**Fix idea / Pointer:** plan C3-R4, Task 1 (evict), Task 3 (eviction call and test), Review Focus 6.

## F-20 — Plan C3 R5 understates the cold-cache hole: an artifact with a current stamp is not queued at all

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent review of the Plan C3 draft before execution.

**When:** checking C3-R5's cost sentence ("a chunk that carries a marker never reaches the cache ... 26.6% of this repo's chunks carry a marker") and the scout's "an unmarked chunk ... the first reindex after C3 embeds it anyway (`catalog/mod.rs:504-513`)".

**Expected:** unmarked chunks (73%) reach the cache on the first reindex after C3; only marked ones are a hole.

**Got:** the queue is gated per ARTIFACT, not per chunk: `index_repo_sync` queues an artifact only when `needs_embed` (its `embedded_sha256` stamp differs from the file hash) or `force_embed` (`src/librarian/indexer.rs:671-697`). The v14 migration adds the marker column and clears no stamp (`catalog/mod.rs:504-513`; the only `embedded_sha256 = NULL` is `clear_kept_row`, `indexer.rs:1042`). So every artifact embedded before C3 and unchanged since is not queued, marked or not. On the day C3 ships the cache is empty for the whole existing corpus, and the first move or rebuild of any such file re-embeds. The scout's claim is refuted; the plan's 26.6% understates the hole by about four times. The fill lever that exists is one `reindex` with `reembed=true` per project (R9: it skips reads and writes every row), at full embed cost; `backfill-chunks` selects only artifacts without rows or vectors and does not fill it.

**Probable cause:** the cost was reasoned from the chunk marker, not from the artifact stamp that gates the queue before the marker is consulted.

**Workaround:** none needed yet. Plan change: rewrite R5's cost ("every artifact whose stamp is current is not queued, so the cache starts empty; a `reembed=true` reindex fills it"), and say so in the manual section.

**Severity:** med

**Status:** mitigated

**Sweep 2026-11-16:** The plan's R5 text and the manual now state the whole hole (`9714ec4c2`). The cold cache itself remains: it is empty for every artifact embedded before C3 (`indexer.rs:672-697`). It is accepted under ruling C3-R5. One `reembed=true` reindex per project, or the seeding follow-up, fills it. Plan D's state doc makes that a precondition of the cutover rebuild. No bug file carries it.

**Rests on:** a cache filled only on the embed path holds nothing that the skip in front of that path keeps out.

**Fix idea / Pointer:** plan C3-R5, Task 5 manual text; follow-up (b) seeding.

## F-21 — Plan C3 Task 2's text-scan guard counts a one-line call that rustfmt splits

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent review of the Plan C3 draft before execution.

**When:** checking Task 2's guard `every_production_construction_site_attaches_the_vector_cache_with_its_model`, which counts `src.matches(".with_vector_cache(model, ")` in `src/librarian/mod.rs`.

**Expected:** the chained call stays on one line, as `.with_concurrency(env.embed_concurrency.unwrap_or(1))` does.

**Got:** both construction sites chain at 16 columns of indent (`src/librarian/mod.rs:342` and `:629`). `.with_vector_cache(model, std::sync::Arc::new(vector_cache::VectorCache::beside(Some(&db_path))))` is 97 characters, so the line is about 113 columns. The repo has no `rustfmt.toml` (checked), so `max_width` is 100, and rustfmt puts each argument on its own line. After `./scripts/fmt-mine.sh` the text `.with_vector_cache(model, ` matches 0 times and the guard fails against 2 `with_budget` sites; written unformatted, the gate's fmt step fails instead. `VectorCache::beside(Some(&db_path))` alone fits (about 90 columns at 20 indent) and survives.

**Probable cause:** a text-scan guard pinned to a call layout that the formatter owns.

**Workaround:** none needed yet. Plan change: match on whitespace-stripped source (`src.split_whitespace().collect::<String>()` then `.with_vector_cache(model,`), or bind `let vector_cache = Arc::new(VectorCache::beside(Some(&db_path)));` before the chain and count `.with_vector_cache(model, vector_cache)`.

**Severity:** med

**Status:** fixed-verified

**Sweep 2026-11-16:** Fixed in `18dc50fcc`. Each construction site binds the cache before the chain (`mod.rs:411`, `:706`). The guard counts `.with_vector_cache(model, cache)` (`embedding.rs:1133`), a short line that rustfmt leaves alone. `rustfmt --check` exits 0.

**Rests on:** a source-text gate must survive the formatter that the gate itself runs.

**Fix idea / Pointer:** plan Task 2 Step 1 (guard) and Step 3 (wiring).

## F-22 — Plan C3 contradicts two SQLite documents: WITHOUT ROWID for 3 KB rows, and deleting an open database file

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent review of the Plan C3 draft before execution.

**When:** checking C3-R8 (`vec(...) WITHOUT ROWID`, a 768-float BLOB per row) and the Task 5 manual text "You can delete `vector-cache.db` at any time".

**Expected:** both follow SQLite's own guidance.

**Got:** (1) sqlite.org/withoutrowid.html § 4: WITHOUT ROWID "is likely to be helpful for tables that ... do not store large strings or BLOBs", with a rule of thumb of rows under about 1/20 of a page (about 200 bytes at 4 KiB), because content is also stored in interior nodes and fan-out drops. A row here is about 3,100 bytes (768 x 4 plus the key), about 15 times the rule. A rowid table with the same composite PRIMARY KEY keeps the key index small and the blob in the leaves. (2) sqlite.org/howtocorrupt.html § 2.5: unlinking an open database file and creating a new one with the same name makes two processes share one WAL, and "a rollback or recovery for one of the databases might use content from the other database, resulting in corruption". The MCP server holds the cache open for its whole life (`try_build_runtime_with` is called once, `src/server.rs:441`), so "at any time" names exactly that case. The plan's only shape check on a hit is `blob.len() == dim * 4`, which a wrong overflow page does not change.

**Probable cause:** storage rules carried from the scout without a check against SQLite's documents.

**Workaround:** none needed yet. Plan change: drop `WITHOUT ROWID` in C3-R8 (or measure both); manual text "Stop every codescout process, then delete `vector-cache.db`, `vector-cache.db-wal` and `vector-cache.db-shm`."

**Severity:** med

**Status:** fixed-verified

**Sweep 2026-11-16:** The cache table is a plain rowid table, pinned by a test (`013113b7f`, `vector_cache.rs:624-643`). The manual and the guide say to stop every codescout process, then delete the three files. A guide test refuses 'at any time' (`9714ec4c2`, `src/prompts/mod.rs:4188`).

**Rests on:** a storage choice cites the engine's own guidance, not the shape of the previous table.

**Fix idea / Pointer:** plan C3-R8, Task 1 Step 3 (open), Task 5 Step 3 (manual text).

## F-23 — Plan C3: five small drifts between its rulings, guide text and tests and the code

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent review of the Plan C3 draft before execution.

**When:** reading the rulings, the guide paragraph and the Task 1-3 test lists against the code.

**Expected:** each sentence and each test name says what the code will do.

**Got:** five smaller drifts.
1. Guide paragraph (Task 5): "a switch back to an earlier model take[s] vectors from it and call[s] no embedder". A same-dimension switch is applied only by `reembed=true` (an ordinary reindex does not queue stamped artifacts, `indexer.rs:671-697`), and R9 makes `reembed=true` skip cache reads. Only a dimension change, which drops vectors and goes through `backfill-chunks`, reads the cache.
2. R11: "at most one extra embed per process". The drain runs `buffer_unordered(svc.concurrency())` (`reindex.rs:635`); each future calls `get` before any embed returns, so with `LIBRARIAN_EMBED_CONCURRENCY=N` up to N lookups see dimension 0.
3. R13: a failed open is `Unavailable` for the life of the `VectorCache`, which in the MCP server is the life of the process (`src/server.rs:441`). A transient `SQLITE_BUSY` at first creation (the WAL switch and the schema `BEGIN IMMEDIATE`) or a corrupt file the operator deletes keeps the cache off until restart. No test covers a busy open, and none pins the 250 ms production timeout (the lock test passes 50 ms explicitly).
4. `the_note_names_reused_vectors`: the "nothing needed a new vector" branch requires `total_unchanged > 0` (`reindex.rs:976`); the one-file move fixture has 0 unchanged, so the negative half cannot fail. Add an unchanged sibling file.
5. `a_backfill_flushes_the_cache_after_each_page` runs one page (`store_backfill` uses batch 100, `indexer.rs:4832`), so an end-of-run flush passes it. Use batch 1 with two artifacts and assert `transactions() == 2`.

**Probable cause:** sentences carried from the scout's gain list and from single-process, concurrency-1 fixtures.

**Workaround:** none needed yet. Plan edits as named in each item.

**Severity:** low

**Status:** fixed-verified

**Sweep 2026-11-16:** All five items are fixed (`18b9e3c2e`, `9714ec4c2`, `013113b7f`, `8c80a17ca`, `b2e3469bb`). One residual: no test makes an open fail with SQLITE_BUSY. The recovery path does not depend on the cause, and a corrupt-file test covers it (`vector_cache.rs:242-256`). Ruling: the status stays fixed-verified, because the missing test checks the cause and the recovery is tested.

**Rests on:** a negative assertion is evidence only in a fixture where the other branch is reachable.

**Fix idea / Pointer:** plan Task 5 guide text; C3-R11; C3-R13; Task 3 and Task 4 test lists.

## W-12 — Reading the queue gate, the drain and the eviction rule together showed Plan C3's cache would miss the spec's rebuild case

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent review of plan `docs/superpowers/plans/2026-11-14-artifact-identity-plan-c3-vector-cache.md` before any implementer ran.

**Pattern:** read the queue gate (`index_repo_sync`), the drain, the backfill and the seam methods together, and read each eviction and fill rule against the path that would have to reach it, instead of testing each rule alone.

**What it confirmed:** the plan's three corrections of the scout hold at the bytes: `reindex_cli` is `#[cfg(test)]` (`src/librarian/mod.rs:600-601`, callers only at `:826, :828, :1005, :1036`); `index_repo` has no production caller (`indexer.rs:2117`, called from `mod.rs:731` and three tests); `RemoteEmbedder::dimensions()` reads `cached_dims`, 0 until the first embed (`crates/codescout-embed/src/remote.rs:458-463`, set at `:615-616`). The seam fits: a hit can ride `unwritten` into `flush_vectors` and land under the chunk's own id, and the marker is pushed only on a landed upsert (`reindex.rs:49-101`), so a cached vector is marked after it lands like any other. `reembed` is threaded as the plan says (`reindex.rs:628`). The read-door gate does not see `rusqlite::Connection::open` or `create_dir_all` (`path_reads`, `read_door.rs:1325`), and no plan constant is cap-shaped (`tests/result_caps.rs:680`). `librarian-runtime` is a pull-only guide with no declared section, so no size cap applies to its edit.

**What it caught:** F-19 (age eviction removes stable content), F-20 (the cache starts empty for the whole stamped corpus), F-21 (a guard that rustfmt breaks), F-22 (two SQLite-document contradictions), F-23 (five smaller drifts).

**Counterfactual:** without the joint read, every Task 1-4 test passes (each embeds fresh content inside one run) and the cache ships; the first measurement of the spec's "a rebuild reuses them" would be the cutover rebuild, which would re-embed nearly the whole corpus. Task 2's guard would go red at the Task 6 gate after four commits.

**Status:** validated

## F-24 — Plan C3 Task 1: four store tests pass on the broken build they should catch

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, Opus review of Plan C3 Task 1 (worktree `c3-vector-cache-20261114`, commit 013113b7f).

**When:** Checking that every Task 1 test can fail on a broken `src/librarian/vector_cache.rs`.

**Expected:** The Task 1 test list says of `one_flush_is_one_transaction`: "Mutation named: a per-row commit fails this." The tally test and the lock test pin what their names claim.

**Got:** (1) `transactions` is incremented in `flush` after `write_rows` returns `Ok` (`vector_cache.rs:351`), and `write_rows` (`:177-188`) cannot reach the counter. A `write_rows` that autocommits each row returns `Ok` after 400 rows and the counter still reads 1, so the test passes on the mutation it names. A counter that the production code maintains is not an oracle for the property it counts. (2) `the_tally_counts_and_prints_what_a_run_did` calls `note_item(true, false)` and `note_item(false, true)`. A swap of the two parameters' roles gives the same `(hits 1, errors 3, stored 3)`. (3) No test pins `journal_mode = WAL`. A scratch experiment (system SQLite 3.54.1) shows that a DELETE-mode reader is not blocked by another connection's `BEGIN IMMEDIATE`, only by a writer in EXCLUSIVE (the commit). So `an_opening_process_takes_no_write_lock_and_a_locked_file_fails_only_the_flush` passes with the WAL line removed. (4) `a_row_whose_blob_is_not_dim_times_four_bytes_is_failed_not_returned` tests only a shorter blob. A `<` mutation of the `!=` length check (`:304`) returns a wider vector for a longer blob and passes.

**Probable cause:** The plan names each oracle by what it reads on a correct build, not by what a broken build changes.

**Workaround:** Reported as Important (1) and Minor (2)-(4) in the Task 1 review. The controller decides the fix.

**Severity:** med

**Status:** fixed-verified

**Sweep 2026-11-16:** All four test gaps are closed in `69bb3af22`: the trigger atomicity oracle, asymmetric tally inputs, a WAL read-back through a second connection, and a `zeroblob(20)` case.

**Rests on:** A test is evidence only for the failures it can express (`docs/conventions/testing-discipline.md`).

**Fix idea / Pointer:** (1) An atomicity oracle: a raw connection adds `CREATE TRIGGER refuse BEFORE INSERT ON vec WHEN NEW.input_hash = 'bad' BEGIN SELECT RAISE(ABORT, 'refused'); END`; stage good, bad, good; flush is `{stored: 0, failed: 3}` and no row landed. (2) Use asymmetric inputs. (3) Read `PRAGMA journal_mode` through a second connection. (4) Add a `zeroblob(20)` case.

---

## F-25 — Plan D renames and rewrites markdown that code, harnesses and mdBook read by path, and can create a key collision

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent Opus review of `docs/superpowers/plans/2026-11-14-artifact-identity-plan-d-cutover-tool.md` (uncommitted draft) before any implementer ran.

**When:** Checking Task 4 (D-R14 renames) and Task 3 (path-link rewrite) against this repository's real population.

**Expected:** A rename or rewrite that breaks a reader outside the markdown corpus is reported, never applied silently. A rename never makes a valid key ambiguous.

**Got:** (1) `git ls-files` lists in-scope markdown with invalid keys that other files read by path: `src/prompts/workspace_onboarding_prompt.md` (`include_str!`, `src/prompts/mod.rs:1088`), `src/librarian/tools/audit_doc_refs/render_prompt.md` and `src/librarian/tools/legibility_scan/render_prompt.md` (`include_str!`, `audit_doc_refs/mod.rs:966`, `legibility_scan/mod.rs:424`), `docs/PROGRESSIVE_DISCOVERABILITY.md` (`include_str!`, `src/server.rs:2972`; `build.rs:34`), `docs/issues/_TEMPLATE.md` (18 code, script and test files), `scripts/measure/judge_prompt.md` (`scripts/measure/judge.py:55`), `.github/ISSUE_TEMPLATE/bug_report.md` and `.github/pull_request_template.md` (names GitHub dictates). None is excluded by `.codescout/key-space.toml`. The plan renames them all and reports no non-markdown reference. (2) The two `render_prompt.md` files both canonicalise to `render-prompt`. D-R14 refuses only an existing target path or two renames to one PATH, so both renames run and create a duplicate key. A rename onto a key that another folder already holds also runs and turns that document's valid `[[key]]` citations into `key_collision`. (3) The path-link rewrite turns `docs/manual/src/SUMMARY.md` (mdBook navigation, 20+ `[x](y.md)` links) and `src/prompts/*.md` (prompt surfaces pinned by snapshot tests) into wiki links, which breaks the book build and the gate. The plan's own Global Constraint "no prompt surface changes" holds for its code but not for the tool's output.

**Probable cause:** The plan treats every in-key-space `.md` as a document that only other documents read.

**Workaround:** Reported as Critical in the review.

**Severity:** high

**Status:** mitigated

**Sweep 2026-11-16:** All three findings are fixed in the cutover (`c14fe1820`, `4fb5ab77c`, `db0576417`): a file that code, a dot directory or mdBook reads is frozen, and a rename onto a held key is refused. Two parked gaps remain. Config in `.json` or `.txt` files does not freeze a file (I1-a, a ledger Ruling; no such case exists in this repository). Code that builds a path by join is not seen (accepted in Task 4).

**Rests on:** `src/librarian/identity/key.rs:127` (`key_of` refuses `_` and `.` stems); `.codescout/key-space.toml` exclude list read this session.

**Fix idea / Pointer:** `plan_renames` refuses a new key that any HEAD document or another rename holds (reason `key collision`). For each rename, scan HEAD's non-markdown blobs for the old file name and the old path, and refuse the rename with the hits as `path:line`. Before the run, the operator either extends the key space (`src/`, `.github/`, `scripts/measure`, `docs/manual/src`) or the tool takes `--no-rewrite <glob>`. The dry run prints both lists.

## F-26 — Plan D drops augmentation params outside the entry collection with no report line

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent Opus review of the Plan D draft.

**When:** Tracing each catalog-only fact class from the old row to committed text to what the derived index reads back.

**Expected:** Lossless-or-reported (D-R7): every catalog fact either lands in committed text or gets a report line that says it is lost.

**Got:** (1) `AugmentationRow.params` is the whole raw JSON (`src/librarian/catalog/augmentation.rs:15-40`). `AugmentationSidecar` holds only prompt, entry_collection, params_schema, render_template, append_mode and history_cap (`src/librarian/augmentation_sidecar.rs:49-64`). Task 5 exports only the rows of `entry_collection`. Params keys outside the collection, and all params of an augmentation with no `entry_collection`, are dropped, and no `Class` counts or reports them. (2) An entry whose token the body defines is `Skipped` with "the body defines it". Its params fields that the section does not show (status, severity, category) are dropped, and the reason does not say so. (3) The `doctor` skip rule that Task 5 reuses ("already exported and declared", `src/librarian/tools/doctor.rs:2075-2085`) never refreshes a sidecar. A shape that changed in the catalog after an earlier `export_augmentations` is reported as skipped and its newer prompt or schema is lost. (4) The derived index has no event, observation, augmentation or relation-label table (`src/librarian/derived/schema.rs:31-108`). The exported text is chunked and its wiki links become edges, but nothing round-trips the event fields, and Task 8 asserts no `derived.db` row at all.

**Probable cause:** The spec's store-boundary table lists "ledger entries in augmentation params" and no other params content, and the plan follows the table, not the rows.

**Workaround:** Reported as Critical/Important in the review.

**Severity:** high

**Status:** fixed-verified

**Sweep 2026-11-16:** Params outside the entry collection are reported by key name (`f63957625`). A defined entry names the fields its text lacks. A differing sidecar is reported as shape drift and never overwritten. The derived index has `event` and `augmentation` tables (`04e2ef5c7`). The cutover e2e test asserts the exported rows (`df6a1f5b1`). The derived `event` table is tested only in the build tests (`src/librarian/derived/build.rs:1370`, `:1426`).

**Rests on:** Spec § The store boundary ("Nothing in the catalog lacks [a committed home]") and the plan's D-R7.

**Fix idea / Pointer:** Add a `ParamsOther` class: one `Unrecoverable` item per augmentation and params key outside the entry collection (or export a params snapshot beside the sidecar). Name the dropped fields in the `Skipped` reason of a body-defined entry. Compare the rendered sidecar with HEAD's and report a difference as `shape drift`. In Task 8, assert `derived.db` rows: entry `(docs/ledger.md, F-2)`, an `entry` edge for `[[ledger:F-1]]`, a `file` edge from `docs/log.md` to `ledger`, and no `malformed` edge on a rewritten line.

## F-27 — Plan D's plan of the committed export misreports exported observations and renamed documents as unrecoverable

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent Opus review of the Plan D draft.

**When:** Checking D-R2 and D-R10 ("a run after the commit is safe") per export class.

**Expected:** A plan of the committed export writes nothing and reports nothing new as unrecoverable, so the report the operator reads after the commit is true.

**Got:** (1) An observation with no matching catalog note becomes a note section with id `observation-<rowid>`. On the next plan its "covered" test still reads only the catalog's notes, so the plan builds the same section again, and `splice_event` refuses it ("event id … is already in this document", `src/librarian/catalog/event_text.rs:385-387`). Task 6 turns that error into `Unrecoverable`. The text does not change, so Task 8's "second `--apply` reports every file unchanged" and "a dry run after the commit reports `writes` 0" both pass while the tally is wrong. (2) Old rows hold `rel_path` from before the rename. After the commit, renames come only from HEAD's `IndexProblem`s, and HEAD has none, so every row of a renamed document becomes `document not in HEAD`. (3) After a partial commit, `[[My_Notes]]` in a file that was not yet rewritten can no longer follow the rename: `renamed_keys` is empty, and `key_of("docs/My_Notes.md")` fails, so the history map cannot map the hex id either.

**Probable cause:** Each detector is specified for the first run; the post-commit plan was checked only for writes, not for items.

**Workaround:** Reported as Important in the review.

**Severity:** med

**Status:** mitigated

**Sweep 2026-11-16:** All three findings are fixed (`4fb5ab77c`, `ef33f2315`, `16938991e`), and the e2e test asserts the post-commit tally (`df6a1f5b1`). Two parked cases still make a later plan report a new item: a note with a reference definition (C1-c, bug `28c8a999b2f3132b`) and a held observation whose bytes changed after export (M52).

**Rests on:** Plan D Review Focus line 1 and D-R10 (a plan is a function of `HEAD` and the old catalog).

**Fix idea / Pointer:** Skip an observation whose `observation-<rowid>` id `parse_event_sections` already finds (reason `the text carries it`). Map an old `rel_path` that HEAD lacks through `canonical_stem` in the same folder (the same D-R14 rule) before you report `document not in HEAD`. Task 8 asserts the post-commit dry run's full tally: no `unrecoverable` count grows and the observation is `skipped`.

## F-28 — Plan D's citation rewrite can write the wrong bytes or guess a target

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent Opus review of the Plan D draft, Task 3 algorithm against `src/librarian/identity/grammar.rs:98-208` and `src/librarian/tools/link_scan/extract.rs:287-290, 698-711`.

**When:** Looking for a rewrite that guesses or writes the wrong bytes.

**Expected:** A rewrite is applied only where the target is certain and the byte range is the source's; everything else is reported.

**Got:** (1) Task 3 says `<qualifier>:TOKEN` uses "the old `cross_repo_re` shape", which is `\b[a-z][a-z0-9_-]{1,119}:[A-Z]{1,3}-\d+\b`, lowercase only. Task 4's test expects `My_Notes:F-2` to follow the rename, which that shape does not match. With that shape, the bare-token pass then rewrites `F-2` alone and gives `My_Notes:[[my-notes:F-2]]`. (2) "Merge adjacent `Text` events into one run" then edits "byte ranges of the original text". pulldown-cmark splits Text at a backslash escape (`R\-91` gives Text `R` at 0..1 and Text `-91` at 2.., as `grammar.rs` documents) and gives an entity a Text that is not its source slice. Offsets in a merged run then point at the wrong bytes. The grammar skips such events (`bare_tokens`, `grammar.rs:189-203`). The plan does not. (3) A bare token or 16-hex id inside a bare URL (`https://host/runs/R-91`) is a Text match, because the grammar scans bare URLs (`a_token_in_a_bare_url_is_scanned`), and a rewrite inside it breaks the URL. (4) `ids_from_history` maps a historical path to "the HEAD path with the same key". A deleted `docs/a/notes.md` and an unrelated surviving `docs/b/notes.md` share the key, so a stale id of the deleted file is rewritten to point at the unrelated file. That is a guess, which D-R7 forbids.

**Probable cause:** The algorithm restates the grammar's event loop but not its offset and boundary rules, and history recovery uses key equality as evidence of identity.

**Workaround:** Reported as Important in the review.

**Severity:** med

**Status:** mitigated

**Sweep 2026-11-16:** All four findings are fixed (`0f80a8a6f`, `c14fe1820`): an uppercase qualifier is reported, escapes and URLs are never rewritten, and history binds an id only to its own path. One case still guesses: an id whose rename destination was later deleted and reused maps to the new file (T4-M10, `head_path` in `src/librarian/cutover/history.rs`). The sweep found it by reading the code and did not run it. Only the final review's park list names it, and no `Ruling:` line does.

**Rests on:** D-R7 ("ambiguous ... reported, never guessed") and spec § The citation grammar.

**Fix idea / Pointer:** State the qualifier regex (ASCII, case-insensitive, then compare to stems as written). Merge only Text events whose range is contiguous with the previous one and whose text equals its source slice, and scan the source slice. Skip and report a match inside a `://` word. Map a historical id only when git rename detection (`find_similar`) or an unchanged path links it to a HEAD file. Otherwise report it with a suggestion. Add a test for each case.

## F-29 — Plan D: nine smaller drifts between its extraction, refusal and report text and the code

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent Opus review of the Plan D draft.

**When:** Reading each extraction, refusal and report rule against the code it names.

**Expected:** Each step's text agrees with the code it extracts, and an operator in the refusal case knows the exact next command.

**Got:** (1) `declaration_stamped` is extracted from `stamp_augmentation_declaration`, which runs `frontmatter::parse` then `frontmatter::write` (`src/librarian/tools/doctor.rs:2151-2172`). `write` re-serialises every key: first-class fields first, extras after, and comments dropped (`src/librarian/frontmatter.rs:174-219`). The test `declaration_stamped_sets_only_expects_augmentation` ("other keys ... byte for byte") cannot pass unless the extraction changes doctor's behaviour, which contradicts "the extractions change no behaviour". (2) The `params-entry` block (D-R13) does not state its fence length. `render_event_section` uses the longest backtick run plus one (`src/librarian/catalog/event_text.rs:178-183`). A JSON string with three backticks closes a fixed fence and turns the rest into prose that the scanner reads. (3) The side-file refusal says "open the catalog once with the current binary and close it", which names no command. The common case is a server that the harness killed and that left a `-wal`. (4) D-R18 keeps `derive-shadow`'s data-dir default while `cutover` reads `LIBRARIAN_DB` first. With `LIBRARIAN_DB` set, `derive-shadow --cutover` and `cutover export` print the dry runs of two different catalogs. (5) The side files are checked once, before the open. Nothing re-checks them after `old_rows::load`, so a session that starts during the run goes unseen. (6) A second `--apply` overwrites `cutover-report-<repo>.md`, which is the record of what the first run moved. (7) `NoOldRowsUnderRoot` with no `--old-root` keeps `git_root.display()` (`src/librarian/derived/diff.rs:330-341`). (8) `take_at` at the git root is not `lock_root` (the nearest `.codescout/project.toml`, `src/cli/write_lock.rs:31-41`). (9) A shallow clone and a hot `-journal` are not named.

**Probable cause:** Extractions and refusals are named by their callers, not read for their outputs.

**Workaround:** Reported as Important (4) and Minor (the rest) in the review.

**Severity:** low

**Status:** mitigated

**Sweep 2026-11-16:** All nine drifts are fixed in code and tests (`b8c897911`, `b77e49679`, `5ed828ae8`, `6ba948ecd`). One parked race remains: two worktrees that export at the same time can replace one report (M57, a Task 8 Ruling).

**Rests on:** Plan D Global Constraints (no absolute path, STE refusals) and D-R1 (one text format per writer).

**Fix idea / Pointer:** (1) Implement `declaration_stamped` as a line upsert, as `upsert_int_line` is, or weaken the test to "parses to the same frontmatter plus the key". (2) Reuse the event fence rule. (3) Name `sqlite3 <file> 'PRAGMA wal_checkpoint(TRUNCATE)'`, then run again. (4) Drop `derive-shadow --cutover`, or give it D-R18's resolution. (5) Re-check `side_files_present` and the file's size and mtime after load and before apply. (6) Name the report `<repo>-<commit12>` and never overwrite it. (7)-(9) as stated.

## W-13 — A scratch SQLite experiment and a check against the real file list tested Plan D's two riskiest claims

**Valid:** dated 2026-11-14

**Observed:** 2026-11-14, independent Opus review of the Plan D draft before any implementer ran.

**Pattern:** test the riskiest claim with a scratch experiment, and test each rename and rewrite rule against the repository's real file list, not against the plan's fixtures.

**What it confirmed:** (1) A scratch experiment (Python `sqlite3`, SQLite 3.54.1, in the session scratchpad) showed: a cleanly closed WAL database opened with `file:…?mode=ro&immutable=1` reads every row and creates no `-wal` or `-shm`. With a writer holding the database and one row only in the `-wal`, the immutable open reads 1 row while a plain `mode=ro` open reads 2. So the immutable open reads stale data without an error, and D-R8/D-R16's side-file refusal before the open carries the whole safety claim. A live server is detectable, because a WAL connection keeps both side files while it is open. (2) The plan's named seams exist with the stated shapes: `open_old_read_only` (`diff.rs:372`), `refuse_if_open_would_write` (`derive_shadow.rs:115`), `write_entry_into_ledger` ending in `record_written_file` (`dual_write.rs:120-183`, `UPDATE artifact` at `:58-66`), `splice_pending_section` (`augmentation.rs:2362`), `body_defined_indices` (`:2767`), `TWIN_KINDS` and the ledger refusal (`event_create.rs:336, 385-392`), `splice_event`, `parse_event_sections` (`event_text.rs:383, 297`), `label_for_rel`, `splice_relation` (`link_text.rs:23, 44`), `link_target` (`extract.rs:698`), `BuildInput` (`build.rs:76-83`), `VectorCache::beside` (`vector_cache.rs:268`), `TOOL_SURFACE_CHAR_BUDGET` (`server.rs:6625`), and every old-caller test the plan names.

**What it caught:** F-25 (in-scope rename candidates that code reads: `include_str!` prompt files, `.github` templates, `_TEMPLATE.md`, and two `render_prompt.md` files that map to one key), F-26 (params loss), F-27 (post-commit misreport), F-28 (rewrite guesses), F-29.

**Counterfactual:** without the population check, the first `cutover export --apply` on this repository renames `src/prompts/workspace_onboarding_prompt.md` and the two `render_prompt.md` files. The build then fails on `include_str!`, the GitHub issue templates stop working, and the mdBook SUMMARY links break. The fixture-only Task 8 test passes throughout.

**Status:** validated

## F-30 — Plan D Task 1: the side-file refusal misses a symlinked catalog path, so an immutable open reads stale rows silently

**Valid:** dated 2026-11-14

**Observed:** Plan D Task 1 (`5ed828ae8`, worktree `cutover-tool-20261114`) builds the side-file refusal that the immutable open's safety rests on (Global Constraints, W-13). `side_files_present(path)` (`src/librarian/derived/diff.rs:459`) and `has_side_file` (`src/cli/derive_shadow.rs:122`) build `<path>-wal` from the path as given. SQLite resolves a symlinked database path and keeps the `-wal`/`-shm` beside the TARGET. Measured with SQLite 3.54.1 (python, scratchpad): a holder opened `link/catalog.db` (a symlink to `real/catalog.db`) with `wal_autocheckpoint=0` and inserted 1 row; `real/` held `catalog.db-wal` and `-shm`, `link/` held only the link; the side-file test on the link path answered false; an `immutable=1` open through the link read 0 rows while the holder read 1. No error.

**Cost:** a catalog path that is a symlink (an `LIBRARIAN_DB` value, or a symlinked `catalog.db`) passes the refusal while a server runs, and the cutover (and derive-shadow) read stale rows silently. `file_stamp` (D-R8 re-check) does not see it: the main file does not change while the rows sit in the `-wal`.

**Fix:** canonicalize the path (`std::fs::canonicalize`) before `side_files_present` and before building the URL, in `open_old_immutable` and in derive-shadow's `open_old`; add a test with a symlinked catalog and a live holder.

**Status:** fixed-verified — fixed in D task 1 fix round 1 (`b77e49679`, on `integration/identity-lifecycle-20261114`; the scoped Opus re-review found it addressed). Verified 2026-11-16: `open_old` in `src/cli/derive_shadow.rs` canonicalizes the path before the side-file check and the URL. Reaches `experiments` with the next integration merge.

**Also seen in the same review:** the refusal's remedy (`sqlite3 <file> 'PRAGMA wal_checkpoint(TRUNCATE)'`) leaves a zero-length `-journal` in place (measured: WAL and rollback databases both keep `c.db-journal`), so a leftover empty journal refuses forever.

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
