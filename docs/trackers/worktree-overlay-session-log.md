# Session Log — worktree-overlay

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries via `edit_markdown(action="insert_before", heading="## Template
> for new entries", content=...)`. Add a row to the Index / Wins Index
> table for each new entry — the indexes are the eval surface, the
> sections are the evidence.
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
| F-1 | 2026-07-17 | med | plan-prose | mitigated | Design asserted a payload-bearing lineage edge; `LinkRow` has no payload field |
| F-2 | 2026-07-17 | high | architectural | mitigated | Bare `graft` of a base-seeded shadow row would duplicate every pre-existing tracker entry |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-07-17 | high | Pre-spec recon on every machinery claim the design cites | Spec + plan would have shipped a merge step that corrupts trackers on first real merge | validated |
---

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

Copy this block when appending a new friction. Allocate the next free
ID. Add a matching row to the Index table.

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

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Copy this block when appending a new win. A win without a
**Counterfactual** is marketing — name what would have happened
without the pattern, with at least one piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Status:** validated | promoted-to-permanent-docs | archived

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
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — Design asserted a payload-bearing lineage edge; `LinkRow` has no payload field

**Observed:** 2026-07-17, worktree-overlay design brainstorm (pre-spec recon).

**When:** Design section 1 claimed the `worktree_of` lineage edge would "carry a base cursor (main row's params snapshot hash + event seq at fork)".

**Expected:** `artifact_link` rows can carry a payload for the base cursor.

**Got (scouted reality):** `LinkRow` at `src/librarian/catalog/links.rs:8-13` is `{src_id, dst_id, rel, created_at}` — no payload column. Nothing in the link surface stores per-edge data.

**Probable cause:** Asserted the field from design convenience without reading the struct this session — the R-19 pattern (checkable fact presented as recommendation without a same-session read).

**Workaround:** Amend the design: keep the bare `worktree_of` link for graph traversal; store the base cursor as a `worktree_fork` **event** on the shadow row (events carry JSON payloads and are already re-pointed atomically by `graft_rows`), or as columns on the new `worktree_registration` table. Recommended: fork event — no schema change to existing tables, survives graft, queryable via `state_at`.

**Severity:** med — would have surfaced at plan time or as a compile error; ~1 plan revision / subagent retry.

**Status:** mitigated — design amended before spec write; final storage choice (fork event vs registration columns) pinned at spec time.

**Fix idea / Pointer:** Spec §1 (data model), this work stream.

---

## F-2 — Bare `graft` of a base-seeded shadow row would duplicate every pre-existing tracker entry

**Observed:** 2026-07-17, worktree-overlay design brainstorm (pre-spec recon).

**When:** Design section 4 claimed "entry_collections renumber rebase-style after main's live max — graft's remap machinery already does this".

**Expected:** `graft_rows`/`merge_augmentation` renumbers incoming entries to slot after the survivor's max id; safe to call on a shadow row seeded with the main row's params (fork-on-first-write).

**Got (scouted reality):** `merge_augmentation` at `src/librarian/catalog/graft.rs:179-320` does **collision-only** renumbering: free incoming ids are preserved verbatim; colliding ids renumber to the next free `<prefix>-N`; incoming entries whose content (minus id) matches a surviving entry are only *flagged* `suspicious`, not dropped. A shadow row seeded with the base entry set therefore collides on **every** base entry at merge → all of them get renumbered and re-appended as duplicates (and flagged), corrupting the main tracker's entry_collection on the first real merge.

**Probable cause:** Memory `worktree-merge-catalog-reconciliation` describes graft in the *reseat* context (worktree row has ONLY worktree-born entries — no seeding), where whole-row folding is correct. The overlay design changes the precondition (shadow = base copy + delta) without re-checking the machinery's contract.

**Workaround:** `merge_worktree` must extract the delta first — entries beyond the base cursor (ids not in the fork snapshot) — and fold **only the delta** via the graft renumber path. Bare `graft_rows` on a seeded shadow is never valid; the spec must state this as an invariant, and the fork event's payload (F-1 workaround) is exactly the data that makes delta extraction exact instead of heuristic.

**Severity:** high — silent data corruption class: duplicated F-N/W-N/T-N entries in live trackers after the first merge, discovered only by later readers.

**Status:** mitigated — design amended pre-spec (merge = delta-extraction wrapper over graft primitives, not bare graft).

**Fix idea / Pointer:** Spec §4 (merge semantics) + a dedicated test: fork-seed → append both sides → merge → assert no duplicate ids AND no duplicated content.

---

## W-1 — Pre-spec recon on machinery claims caught a corruption-class design error

**Observed:** 2026-07-17, worktree-overlay design brainstorm, immediately before spec write.

**Pattern:** Before writing a spec, scout every *existing-machinery* claim the design leans on ("X already does this") by reading the cited symbol's actual contract — here `merge_augmentation`'s doc + tests, `LinkRow`'s fields, and `append_entry`'s tool path.

**Counterfactual:** Without the scout, the spec would have pinned "merge = graft the shadow row" and the plan would have transcribed it. `graft_no_op_when_entry_collections_differ` and the renumber tests all pass for that implementation — nothing in the existing suite exercises a base-seeded source row, so the duplication bug (F-2) would have shipped and fired on the first real worktree merge of a live tracker (e.g. `tool-usage-patterns`, id f2ecdd76…). Recovery would have meant hand-deduplicating a corrupted entry_collection. Additionally the plan would have cited a nonexistent link-payload field (F-1) — a guaranteed first-task compile failure.

**Confirming data points:**
1. F-1 (this session) — nonexistent `LinkRow` payload field cited by design.
2. F-2 (this session) — graft contract mismatch under the new fork-on-first-write precondition.

**Impact:** high — prevented a silent-corruption merge step from reaching spec/plan/implementation.

**Promote-when:** A second pre-spec recon catches a false "existing machinery already does X" claim → promote to memory topic `reconnaissance` as: "before a spec cites existing machinery as 'already does X', read that symbol's contract + its tests this session (W-1, worktree-overlay)."

**Status:** validated — two same-session data points.

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
