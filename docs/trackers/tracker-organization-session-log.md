---
status: active
title: Session Log — Tracker Organization (lifecycle folders, per-entry ledgers)
tags:
- session-log
- tracker-organization
- lifecycle
topic: tracker organization
entry_prefix:
- F
- W
entry_high_water_F: 1
entry_high_water_W: 2
---
# Session Log — Tracker Organization

> **Work stream:** reorganize `docs/trackers/` into lifecycle folders (layer 1) and topic or
> unit folders (layer 2); every ledger becomes one file per entry with issues-style identity
> (date+slug stem is the key, no sequential number). Brainstorm started 2026-11-11.
> Measurements: `tracker-hygiene-log` HY-34 (temp/ policy, parked) and HY-35 (why trackers go
> stale). Identity layer: `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`
> (owned by a parallel session).
>
> **Exit:** archive when the design is implemented and the migration is done, or when the
> work stream is dropped. Record which one.

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
| F-1 | 2026-11-11 | med | stale-claim | open | **frictions/ is not one-file-per-entry** — FR-N are defined in index.md; only issue-clusters/ has the shape |
| F-<n> | YYYY-MM-DD | low/med/high | <category> | open | <one-line title> |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-11-11 | med | read the gate before writing the surface | guide naming docs/templates/ would fail no_guide_names_a_path_or_field_only_this_repo_has | validated |
| W-2 | 2026-11-11 | med | measure the repo under the new model with an ignored census test before building tools on it | a migration sized from the estimate of about 76 trackers would have missed 81 flat files plus 261 in subfolders, and the loose temp tracker would surface only at migration time | validated |
| W-<n> | YYYY-MM-DD | low/med/high | <pattern> | <what-would-have-happened> | open |

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

## F-1 — I claimed frictions/ is one file per entry; its 16 FR-N are defined in index.md

**Valid:** dated 2026-11-11

**Severity:** med · **Category:** stale-claim · **Status:** open

**Observed:** During the brainstorm I told the operator, and the parallel identity session
(message 2026-11-11), that `docs/trackers/frictions/` and `docs/trackers/issue-clusters/`
"already have one file per entry". The scout shows that only `issue-clusters/` has that shape:
each `IC-N-<slug>.md` defines its own `## IC-N — <title>` heading (for example
`IC-3-declared-not-wired.md` line 14). `frictions/index.md` defines all 16 `FR-N` entries as
headings inside the index (`grep -cE '^#{2,3} FR-[0-9]+ [—–-] '` → 16), and the files beside it
are evidence files ("Ledger entry: FR-5 in index.md"). Their names carry no `FR-N`, although
the index says they live at `FR-N-<slug>.md`.

**Cost:** The identity spec (280e77b90, § Key rule) uses `frictions/FR-3-<slug>.md` as its
example of a per-entry ledger folder. The rule it derives (a folder key owns the tokens of every
file inside the folder) is still right for the planned layout, but the example describes a
shape that does not exist yet. If a builder used `frictions/` as a fixture, the test would not
match reality.

**Fix:** Tell the identity session that `frictions/` is index-defined today, and that
`issue-clusters/` is the real per-entry example. In the migration, `frictions/` must move its
16 definitions out of `index.md` into the entry files.

## W-1 — The guide gate allows only server-defined paths, so the lifecycle layout must be a server convention

**Valid:** dated 2026-11-11

**Impact:** med · **Pattern:** read-the-gate-before-writing-the-surface · **Status:** validated

**Scout:** The operator wants the lifecycle system "promoted in get_guide". Before drafting any
guide text I read the test that constrains guides. `src/prompts/mod.rs`
`no_guide_names_a_path_or_field_only_this_repo_has` refuses, in EVERY guide, the needles
`docs/templates/`, `docs/conventions/`, `docs/issues`, `docs/superpowers/` and others. Paths
the server itself defines are allowed: the test's own doc comment names `docs/trackers/` and
`docs/augmentations` as constants in `src/librarian` (`LEDGER_DIR` in
`src/librarian/tools/append_entry.rs:3772`, `SIDECAR_DIR` in `augmentation_sidecar.rs:33`).
`tracker_conventions_names_no_repo_specific_artifact_kind` also refuses `docs/issues`,
`_TEMPLATE`, `kind: bug` and "bug file" in `tracker-conventions`.

**Consequence for the design:** the guide can teach the lifecycle layout by name only if the
server defines it: the lifecycle folder names under `docs/trackers/` and the `lifecycle:` key
become server constants that doctor checks. Per-kind templates stay in the repo, and no guide
names their path. A repo-specific procedure goes to the repo's own template.

**Counterfactual:** A guide section that pointed to `docs/templates/tracker-<kind>.md`, or that
said "like docs/issues", would fail the gate at the next run. The design would then have to
be reshaped after the plan was written.

## W-2 — A census of this repo under the new model gave the migration its worklist before any tool existed

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, building Plan L-A (the lifecycle core). Task 7 added an ignored test that walks `git ls-files '*.md'`, builds a `LifecycleTree` and prints counts. Run at tree `d5e6abc96e7a76df9a956b8b27321cc40096396c` on branch `feat/tracker-lifecycle-plan-a-20261111`: 0 collections, 0 entries, 1 placement problem, 81 flat `docs/trackers/*.md` files and 261 files in other `docs/trackers/` subfolders (343 documents in all under `docs/trackers/`).

**Pattern:** measure the repo under the new model with a throwaway test before any tool or migration depends on the model, and keep the test as an `#[ignore]`d census so the figure can be re-taken.

**Counterfactual:** the plan carried the estimate "about 76 legacy trackers"; the census counted 81 flat files plus 261 files in subfolders, so a migration sized from the estimate would have missed more than two thirds of the files. The one placement problem is the first temp tracker (`docs/trackers/temp/artifact-identity-refactor.md`): a loose file in `temp/`, outside any collection folder, which the model refuses. Without the census it would have surfaced at migration time, after the design was frozen.

**Confirming data points:** the same census shape found the identity session's figures (45 invalid keys and 37 duplicate-key groups over 2704 documents), so both migrations are sized from measurement; the final whole-branch review found that the lifecycle checks did not report a loose file in `temp/`, which the census had already shown as the only misplaced file.

**Impact:** med

**Promote-when:** the migration command (plan L-E) exists and reuses the census as its dry run.

**Status:** validated

**Rests on:** a design is checked against the real corpus before tools are built on it.

---

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
