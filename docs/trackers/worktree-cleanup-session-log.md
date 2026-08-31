---
kind: tracker
status: active
title: Session Log — Worktree Cleanup
owners: []
entry_prefix:
  - F
  - W
tags:
  - worktree
  - telemetry
  - cleanup
entry_high_water_F: 1
entry_high_water_W: 1
---

# Session Log — Worktree Cleanup

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries with:
>
> ```
> artifact(action="append_entry", id="<artifact id>", id_prefix="F",
>          anchor_heading="## Template for new entries",
>          title="<one-line title>", body="**Observed:** ...")
> ```
>
> One call, one write: the server allocates the next id, formats the
> heading as `## F-N — <title>` (the only shape `link_scan` accepts as a
> definition), records the ledger's high-water mark, and stamps
> `**Valid:** dated <today>` unless your body declares a class. **Then**
> add the Index / Wins Index row, using the id the call returned — the
> indexes are the eval surface, the sections are the evidence.
>
> **Do not hand-allocate ids, and do not pre-write index rows.** A max-id
> is a fact about an instant, and a peer session in the same checkout can
> take the number between your scan and your write. Pre-written rows are
> worse: the allocator counts an id claimed by an index row, so rows
> written ahead of their sections consume the ids they name — which is why
> codescout's `statement-validity-session-log` starts at `statement-validity-session-log:F-2`/`statement-validity-session-log:W-3`
> rather than `statement-validity-session-log:F-1`/`statement-validity-session-log:W-1` (see `statement-validity-session-log:F-3` there).
>
> **`edit_markdown` is not the append path**, though it works at first.
> This template ships without frontmatter, so a fresh copy is directly
> editable — but once you declare `entry_prefix` to make the ledger
> guarded (which `get_guide("tracker-conventions")` tells you to do), the
> librarian guard refuses direct edits and only `append_entry` writes.
> Reach for `edit_markdown` for the prose sections and the index tables,
> never for allocating an entry.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-08-30 | high | data-loss | mitigated | "Merged, so safe to remove" ignored 1,639 rows of pre-fix worktree telemetry |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-08-31 | high | Grep for CITATIONS of a path, not the object itself — liveness is a property of the reference graph | An irreversible 174M deletion of the pinned retrieval-benchmark corpus, on the agreement of three git instruments that all report it as debris (`git worktree list` omits it, `git status` is silenced by two .gitignore lines, the .git pointer names a repo that no longer exists). Recovery would have been move-then-add plus a 163M re-index. | validated |

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
line — the server writes the heading from `title`). Add the matching Index
row afterwards, using the id the call returned. Do not allocate the id
yourself; see *How to use* above.

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

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

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
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer — and, for a multi-instance target, names every instance it landed in. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — "Merged, so safe to remove" ignored 1,639 rows of pre-fix worktree telemetry

**Observed:** 2026-08-30, auditing the repo's three linked worktrees and
reporting which were safe to remove.

**When:** After establishing merge state and catalog cleanliness, while
recommending removal to the user. Reconnaissance ran on the recommendation
itself, because a negative search result was about to authorise a deletion.

**Expected (my report):** `feat/peer-delegation` and `vdi-windows` are fully
contained in `master`, `git worktree prune --dry-run` is clean, the catalog
holds zero rows under either worktree root, and `doctor` reports zero
`worktree_scoped_row` findings — therefore "safe to remove outright".

**Got (scouted reality):** merge state and catalog state were correct, but
removal is not lossless. Both worktrees hold a populated
`.codescout/usage.db` that exists nowhere else:

| Worktree | own `usage.db` rows | range | in main db? |
|---|---|---|---|
| `.claude/worktrees/operator-rules-phase-2` | 0 | — | **yes — 937 rows** |
| `.claude/worktrees/peer-delegation` | **705** | 2026-06-01 | no |
| `.worktrees/vdi-windows` | **934** | 2026-06-12 .. 06-16 | no |

The main checkout's `usage.db` groups to exactly two `project_root` values:
`/home/marius/work/claude/codescout` (6898) and the
`operator-rules-phase-2` worktree (937). Neither June worktree appears.

**Probable cause:** `docs/issues/archive/2026-08-20-worktree-removal-deletes-its-usage-telemetry.md`
— `usage.db` is project-root-scoped and a worktree is its own project root.
Fixed on `experiments` @ `04e8e2c0` (patch-id `39a1c54c860b276715b924ba870766fa`)
by redirecting the db OPEN to `worktree_main_root(...)` while leaving
`project_root` naming the worktree in the row. The 937/0 split for
`operator-rules-phase-2` (created after the fix) against 705/934 stranded in
the two June worktrees (created before it) is that fix's own before/after
boundary, visible in live data. The fix is forward-looking only; it
back-fills nothing.

**Workaround:** copy each stranded `usage.db` out to a file BEFORE
`git worktree remove` — as the bug file's own *Workarounds* section says.
Do NOT merge them into the live main `usage.db`: `write_record`'s retention
sweep is `DELETE FROM tool_calls WHERE called_at < datetime('now','-30 days')`
with no `project_root` predicate, and the horizon today is 2026-07-31, so
every June row would be deleted on the next write. The rows survive in the
worktrees only because nothing has written to those databases since.

**Severity:** high — irreversible data loss. 1,639 rows of tool-call
telemetry, unrecoverable once the directory is gone, feeding `/analyze-usage`
and `docs/trackers/tool-usage-patterns.md`.

**Status:** mitigated — surfaced and counted BEFORE removal and put to the
user, who decided the telemetry was not worth preserving. All three worktrees
removed 2026-08-30 with `git worktree remove --force`; 705 + 934 = 1,639 rows
discarded as a deliberate, informed choice rather than a silent loss. Both
`usage.db-wal` sidecars were 0 bytes, so nothing beyond the counted rows was
at stake, and `crates/librarian-mcp/.codescout/usage.db` is tracked in git
with content identical to the main checkout's, so that sub-database was never
at risk. Root cause remains unresolved: nothing in git or codescout warns that
removing a worktree destroys gitignored per-worktree state.

**Valid:** dated 2026-08-30

Row counts are true of this machine at this instant; re-count before acting,
and note the retention horizon moves daily.

**Rests on:** `docs/issues/archive/2026-08-20-worktree-removal-deletes-its-usage-telemetry.md`
(root cause + the "copy it out" workaround) and the retention-sweep predicate
quoted in that file's *Fix* section.

**Fix idea / Pointer:** Removal checklist for this repo should be: count
`usage.db` rows in the worktree, compare against `project_root` groups in the
main db, copy out any delta, THEN remove. Only the third step is currently
documented anywhere.

## W-1 — Grepping for citations, not inspecting the object, is what separated a live corpus from dead siblings

**Observed:** 2026-08-30/31, auditing `.worktrees/` after removing the three registered
worktrees. Four other sessions were live in the same checkout.

**Pattern:** To decide whether a directory is safe to delete, grep the repo for
**citations of its path** — `scripts/`, `docs/`, `*.toml`, CI — rather than inspecting the
directory or asking git about it. Liveness is a property of the reference graph, not of
the object.

**Counterfactual, concrete.** `.worktrees/` held eight directories. Three shared one
dangling gitdir pointing at `code-explorer`, a repo that no longer exists. Two of those
three were genuinely dead and were deleted (184 MB reclaimed). The third,
`.worktrees/bench`, is the pinned retrieval-benchmark corpus at `ede25e69`, 174 MB, and
`scripts/run-tc-benchmark.sh:18`, `scripts/sweep-bm25-boost.sh`, `docs/PROBES.md` and
`docs/trackers/retrieval-benchmark.md` all resolve against it. Deleting it costs a
move-then-add plus a **163 MB re-index**. The citation grep is the only check that
separated it from its two siblings.

**Why the obvious instruments all failed, which is the transferable part.** Three
independent-looking git signals agreed the directory was debris:

- `git worktree list` — the canonical instrument for *what worktrees exist here* — reports
  only the main checkout. Bench is absent entirely.
- `git status` is silent: `.worktrees/` is gitignored twice (`.gitignore:7` and `:117`).
- the `.git` file points at a repository that does not exist.

**All three consult git, and the corpus's liveness is not a git fact.** A peer's
formulation: *instruments sharing a substrate are one instrument*, and their agreement
carries no more evidence than any one alone. This is the more dangerous shape than a
single misleading signal, because agreement reads as corroboration — and it means the
*careful* reader, who reaches for `git worktree list` rather than eyeballing the pointer,
gets a clean and confident wrong answer.

**Confirming data points:**

1. This session — the grep kept `bench` and authorised the other two deletions.
2. A peer's independent re-derivation: 78–84 citations across 16 files, both script line
   references read directly rather than taken from my report.
3. A second peer verified the corpus intact at `ede25e69` by **set difference**, not by
   counting: 851 tracked paths, `comm -23` returns 0 tracked-but-absent. Their first
   attempt *counted* 778 present against 851 and read as "73 missing, corpus damaged" —
   the opposite of the truth, and an argument for deleting. They had pruned `.codescout/`,
   where 61 tracked paths live.

**Impact:** high — the counterfactual is an irreversible 174 MB deletion of a load-bearing
measurement corpus, taken on the agreement of three instruments.

**Promote-when:** a second work stream where a citation grep overrides an agreeing set of
native instruments. At two datapoints this is craft-shaped, not project-shaped — *count
distinct substrates, not distinct commands, when corroborating a negative* — and belongs in
the reconnaissance SKILL.md beside the negative-results law, not in a codescout memory.

**Status:** validated — single stream, three independent verifications within it, no
promotion yet.

**Valid:** dated 2026-08-31

The instrument-blindness is a fact about this repo's layout and git's design, not about an
instant; the citation counts are as measured that night.

**Rests on:** `observer-blindness:OB-4` (the three-blind-instruments analysis and the
shared-substrate rule) and `F-1` in this log, which is the same lesson one layer down —
merge state is a claim about commits and says nothing about gitignored state.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     artifact(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark and stamps `**Valid:** dated
     <today>` — one write. Then add the Index / Wins Index row with the id
     it returned. Do not hand-allocate; do not pre-write the row. -->
