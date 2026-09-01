---
id: da501585415287c3
kind: plan
status: archived
title: Tracker Lifecycle Stage 1 — activate the dormant hygiene push trigger + add the session-log decay detector (D10)
tags:
- tracker-redesign
- stage-1
- hygiene
- lifecycle
topic: tracker lifecycle stage 1
---

# Tracker Lifecycle Stage 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement TMR-3 (push-based maintenance) and TMR-6 (session-log decay policy) from `docs/trackers/tracker-management-redesign.md` on the current catalog model — by activating the *existing, dormant* SessionStart hygiene trigger in codescout and adding a session-log decay detector (D10) to the tracker-hygiene skill.

**Architecture:** No new machinery. Recon (F-1, `docs/trackers/archive/tracker-redesign-session-log.md`) found the push trigger already shipped in `codescout-companion/hooks/session-start.mjs:115-128`: it nudges when `docs/trackers/tracker-hygiene-log.md` has an overdue `next-sweep-due:` date — codescout just never created that file. Stage 1 = (1) extend the hygiene skill with detector **D10 session-log-decay** carrying the TMR-6 distill-then-archive procedure, (2) bootstrap codescout's hygiene ledger (which activates the nudge), (3) wire the plan into the TMR tracker. Decay *detection* is automatic (sweep-driven); every mutation stays human-gated per the skill's Phase 4.

**Tech Stack:** Markdown skill files (claude-plugins repo), codescout librarian artifacts, existing bash test harness. **No Rust changes.**

## Global Constraints

- Two repos: Task 1 works in `/home/marius/work/claude/claude-plugins` (branch per its conventions), Tasks 2–3 in `/home/marius/work/claude/codescout` (branch `experiments` — never `master`).
- codescout repo: all file edits via codescout MCP tools (`edit_markdown`, `artifact`, `create_file`) — native Write/Edit are hook-denied; shell via `run_command`.
- The hook contract is pinned by `codescout-companion/hooks/session-start.test.sh:69-128` — the ledger must have real YAML frontmatter (not fenced) with `next-sweep-due: YYYY-MM-DD`; due **today counts as due** (`due <= today`).
- Do NOT change the tracker-hygiene SKILL.md `description:` frontmatter (trigger-string; changing it requires re-scoring its eval scenario in prompt-engineering).
- D10 verdicts/mutations are human-gated one-at-a-time (D3-class judgment detector) — nothing in this plan authorizes auto-archiving.
- Cite `TMR-3` / `TMR-6` / `F-1` in commit messages.

---

### Task 1: Add detector D10 (session-log-decay) to the tracker-hygiene skill

**Repo:** `/home/marius/work/claude/claude-plugins`

**Files:**
- Modify: `codescout-companion/skills/tracker-hygiene/SKILL.md` (Phase 3 detector table ~L82-92; new procedure subsection after the D9 rule ~L94-107; Phase 2 inventory list ~L67-75)
- Modify: `codescout-companion/skills/tracker-hygiene/references/tracker-hygiene-log-template.md` (Detector trust state table; Sweep entry template table)

**Interfaces:**
- Produces: detector ID `D10 session-log-decay` with the exact table row and procedure text below — Task 2's ledger content includes the matching `D10` trust-table row and MUST use the same detector name string.

- [ ] **Step 1: Add the D10 row to the Phase 3 detector table**

In `SKILL.md`, insert after the `**D9**` table row:

```markdown
| **D10** | session-log-decay | File matches `*session-log*.md` in the live dir, frontmatter `status: active` or `draft`, AND no git touch in ≥21 days | propose **distill-then-archive** (procedure below) — never a bare archive; every sub-step is its own gate | low, by design |
```

And update the v2 line that currently reads `outside D1–D5/D9 is a `miss` HY-N entry` to read `outside D1–D5/D9/D10 is a `miss` HY-N entry`.

- [ ] **Step 2: Add the D10 procedure subsection**

Insert after the `**supersedes** is an edge, not a status (D2/D5).` paragraph (end of Phase 3), before `### Phase 4`:

```markdown
**D10 distill-then-archive — the session-log decay policy.** A session log's value
inverts once its work stream wraps: unpromoted content is index noise, and its
per-file F-1/W-1 numbering pollutes citation resolution. When D10 fires, walk this
sequence — each mutation is its own Phase-4 gate:

1. **Promote wins.** For every W-N with `Status: validated`, check its
   `Promote-when` criterion. Fired → promote (CLAUDE.md / skill / project memory,
   per the win's own routing) and set `promoted-to-permanent-docs`. Not fired →
   note it in the digest (step 4) so the criterion survives.
2. **Rehome open frictions.** For every F-N with `Status: open`, run a verify-open
   check against current code (distributed fixes leave entries zombie-open).
   Still real → promote to a bug file (`docs/issues/`) or move to the successor
   work stream's log; otherwise flip to `fixed-verified` / `wontfix-false-alarm`
   with one line of evidence.
3. **Confirm with the owner** that the work stream is actually wrapped (D3-style
   question — an idle-but-planned stream gets `defer`, which resurfaces next sweep).
4. **Compact.** Replace the body with an outcomes digest: the Index / Wins Index
   tables (statuses updated), one paragraph per promoted/rehomed entry naming its
   destination, and unfired Promote-when criteria. Full prose history stays in git.
5. **Archive through the catalog:** `artifact(update, patch={status:"archived"})`
   then `artifact(move, new_rel_path="docs/trackers/archive/<name>.md")`.

Evidence base: TMR-6 in codescout's `docs/trackers/tracker-management-redesign.md`
(2026-07-17 survey: session logs were the dominant zombie-active class in all three
surveyed repos; 6/13 codescout session logs untouched 4–5 weeks yet `active`).
```

- [ ] **Step 3: Add link_scan to the Phase 2 inventory**

In the Phase 2 bullet list, insert after the `**Observed augmentation freshness:**` bullet:

```markdown
- **Observed citation-graph drift:** `librarian(action="link_scan")` (report mode) — record `counts.edges_missing` and `counts.dangling` in the sweep entry; a jump vs the previous sweep is an observation worth an HY-N note (materializing with `write=true` is a Phase-5 fix, gated like any other).
```

- [ ] **Step 4: Update the ledger template**

In `references/tracker-hygiene-log-template.md`:
- Detector trust state table — add row: `| D10 session-log-decay | individual | 0 | — |`
- Sweep entry template table — add row: `| D10 session-log-decay | 0 | 0 | 0 | 0 |`

- [ ] **Step 5: Verify by inspection commands**

Run: `grep -c "D10" codescout-companion/skills/tracker-hygiene/SKILL.md`
Expected: ≥4 (table row, v2 line, procedure heading text, evidence line)

Run: `grep -c "D10 session-log-decay" codescout-companion/skills/tracker-hygiene/references/tracker-hygiene-log-template.md`
Expected: 2 (trust table + sweep template)

Run: `bash codescout-companion/hooks/session-start.test.sh`
Expected: all `pass` lines, exit 0 (hook untouched — this is the no-regression check)

- [ ] **Step 6: Commit (claude-plugins repo)**

```bash
git add codescout-companion/skills/tracker-hygiene/SKILL.md codescout-companion/skills/tracker-hygiene/references/tracker-hygiene-log-template.md
git commit -m "feat(tracker-hygiene): add D10 session-log-decay detector with distill-then-archive procedure

Implements TMR-6 (codescout docs/trackers/tracker-management-redesign.md).
Also adds link_scan drift counts to the Phase 2 inventory (TMR-3).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 2: Bootstrap codescout's hygiene ledger — activates the dormant SessionStart nudge

**Repo:** `/home/marius/work/claude/codescout` (branch `experiments`)

**Files:**
- Create: `docs/trackers/tracker-hygiene-log.md`

**Interfaces:**
- Consumes: detector name `D10 session-log-decay` from Task 1 (trust-table row must match).
- Produces: the ledger file whose `next-sweep-due:` frontmatter the SessionStart hook reads; first-sweep cadence for every later sweep entry.

- [ ] **Step 1: Create the ledger via the librarian**

Call `artifact(action="create", kind="tracker", title="Tracker hygiene log", rel_path="docs/trackers/tracker-hygiene-log.md", status="active", tags=["hygiene","skill-meta","lifecycle"], extra={"next-sweep-due":"2026-07-17","sweep-interval-days":30}, body=<content below>)`. If `extra` does not serialize the two hyphenated keys into frontmatter exactly as `next-sweep-due: 2026-07-17` (verify in Step 2!), instead create the file with `create_file` using literal frontmatter and run `librarian(action="reindex")` after.

Body content (everything below the frontmatter — copied from the skill's template body, D10 row included, template boilerplate/bootstrap blockquote dropped):

```markdown
# Tracker hygiene log

Per-project ledger for the `codescout-companion:tracker-hygiene` skill.
Two kinds of entries live here:

- **Sweep entries** (`## Sweep YYYY-MM-DD`) — one per sweep: per-detector
  findings/verdicts, every reject's reason, fixes applied with commit SHA.
- **HY-N meta-entries** (`## HY-N — <title>`) — observations about the
  *skill itself*: detector hits, misses, false-positive patterns, and
  SKILL.md change proposals. Monotonic per project; never reuse an ID.

The frontmatter `next-sweep-due:` field is read by the companion's
SessionStart hook — an overdue date produces a one-line nudge at session
start. Every sweep entry ends by updating it to
`sweep date + sweep-interval-days`.

## Detector trust state

Batch-approval graduation is per detector, earned from this table.
A detector enters `batch` after **two consecutive advancing sweeps** — a sweep
advances only if the detector fired and every finding was approved (zero rejects,
zero defers). Any reject resets to `individual`; a no-finding or deferred sweep is
neutral (streak unchanged).

| Detector | Mode | Consecutive zero-reject sweeps | Last reject (sweep, reason) |
|----------|------|-------------------------------|------------------------------|
| D1 index-drift | individual | 0 | — |
| D2 terminal-not-archived | individual | 0 | — |
| D3 stale-active | individual | 0 | — |
| D4 frontmatter-catalog-mismatch | individual | 0 | — |
| D5 canonical-conflict | individual | 0 | — |
| D9 augmentation-stale | individual | 0 | — |
| D10 session-log-decay | individual | 0 | — |

## Template for new entries

<!-- Insert new sweep entries and HY-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## Sweep YYYY-MM-DD\n...")
     Update frontmatter in the same call:
     edit_markdown(..., frontmatter={set: {"next-sweep-due": "YYYY-MM-DD"}}) -->
```

- [ ] **Step 2: Verify frontmatter is literal YAML, not fenced**

Run: `head -12 docs/trackers/tracker-hygiene-log.md` (via `run_command`)
Expected: first line `---`, containing `kind: tracker`, `next-sweep-due: 2026-07-17`, `sweep-interval-days: 30`, closing `---`. If `next-sweep-due` is missing (extra-key serialization mismatch), fix with `edit_markdown(path="docs/trackers/tracker-hygiene-log.md", frontmatter={set:{"next-sweep-due":"2026-07-17","sweep-interval-days":30}})`.

- [ ] **Step 3: Verify the dormant nudge fires against the real repo**

Run (via `run_command`):
```bash
printf '{"cwd":"/home/marius/work/claude/codescout","source":"startup","session_id":"sst-plan-verify"}' | node /home/marius/work/claude/claude-plugins/codescout-companion/hooks/session-start.mjs 2>/dev/null | grep -o 'TRACKER HYGIENE: sweep overdue (due 2026-07-17)'
```
Expected output: `TRACKER HYGIENE: sweep overdue (due 2026-07-17)` (due==today counts as due per the hook's boundary test).

- [ ] **Step 4: Commit (codescout repo, branch experiments)**

```bash
git add docs/trackers/tracker-hygiene-log.md
git commit -m "feat(hygiene): bootstrap tracker-hygiene ledger — activates the dormant SessionStart sweep nudge

TMR-3 (push-based maintenance): the nudge mechanism shipped in
codescout-companion session-start.mjs but never fired here because this
file did not exist (tracker-redesign-session-log F-1).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 3: Wire the plan into the TMR tracker and the catalog graph

**Repo:** `/home/marius/work/claude/codescout` (branch `experiments`)

**Files:**
- Modify: `docs/trackers/tracker-management-redesign.md` (via `artifact(update)` — it is a managed augmented artifact, id `3e01d4fe6de9d69b`; `edit_markdown` will be refused)

**Interfaces:**
- Consumes: plan artifact id (from this file's own frontmatter `id:` — read it with `artifact(action="find", filter={"rel_path":{"contains":"tracker-lifecycle-stage1"}})`); TMR tracker id `3e01d4fe6de9d69b`.

- [ ] **Step 1: Link plan → tracker**

Call `artifact(action="link", src_id=<plan id>, dst_id="3e01d4fe6de9d69b", rel="implements")`.

- [ ] **Step 2: Append a History entry to the TMR tracker**

Call `artifact(action="update", id="3e01d4fe6de9d69b", patch={body_edits:[{heading:"## History", action:"insert_after", at:"after-heading-line", content:"\n### <today YYYY-MM-DD> — Stage-1 implemented\nD10 session-log-decay landed in the tracker-hygiene skill (claude-plugins <short SHA of Task 1 commit>); codescout hygiene ledger bootstrapped (<short SHA of Task 2 commit>) — the SessionStart nudge is live, verified against the real repo. First sweep will exercise D10 against the 6 stale session logs the survey found.\n"}]})` — substitute the two real commit SHAs and today's date.

- [ ] **Step 3: Verify the link and status**

Call `artifact(action="get", id="3e01d4fe6de9d69b", include_links=true)` — expect one incoming `implements` edge from the plan. Then `artifact(action="update", id=<plan id>, patch={status:"active"})` (plan moves draft → active once execution starts; flip to `done` only after the first sweep runs).

- [ ] **Step 4: Commit**

```bash
git add docs/trackers/tracker-management-redesign.md docs/plans/2026-07-17-tracker-lifecycle-stage1-plan.md
git commit -m "docs(tmr): record Stage-1 landing; link plan to tracker (TMR-3, TMR-6)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Out of scope (deliberate, YAGNI)

- **Rust changes** — `list_stale` kind-filtering, doctor scheduling, catalog GC: not needed for Stage 1; D10 detection uses git dates + filename match at skill level. Catalog GC is tracked separately in `docs/issues/archive/2026-07-17-catalog-dead-rows-no-gc.md`.
- **Running the first sweep** — interactive by design (every finding human-gated); do it in a live session after this plan lands: `/codescout-companion:tracker-hygiene`.
- **Plugin version bump / release** of claude-plugins — its own release flow owns that.
- **Stage 2** (entry-grain IDs, `cites` at write time) — separate plan once Stage 1's first sweep produces data.

## Verification of the whole stage

Start a fresh Claude Code session in codescout → SessionStart banner must show the `TRACKER HYGIENE: sweep overdue` line → run `/codescout-companion:tracker-hygiene` → D10 must list the ≥21-day session logs (survey predicts: codescout-lessons-2026-05-20, usage-analysis-improvements, vdi-reliability, dzo-legibility, pi-integration [has uncommitted branch changes — expect a `defer`], structural-edit-gate).
