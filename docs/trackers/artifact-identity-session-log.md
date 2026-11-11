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
entry_high_water_F: 4
entry_high_water_W: 2
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

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-2 | 2026-11-11 | high | cross-session design review | path fallback would change keys on folder moves | validated |
| W-1 | 2026-11-11 | high | builder check against the code | repo name in librarian.toml breaks every older binary | validated |

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
