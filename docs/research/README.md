---
id: b2b57ae3019c9391
kind: tracker
status: active
title: Research Index
owners: []
tags: []
topic: null
time_scope: null
---


# Research Index

This folder catalogs every research artifact in the codescout repo. The
[LIVE] table above is rendered automatically from each file's frontmatter
by the librarian augmentation refresh — do not edit it by hand.

## How to save a research

When a user says "save this research", any agent (Claude Code, Copilot,
Antigravity, etc.) follows these steps. The standards in § Research quality
criteria below are non-negotiable; if you cannot read them, abort and surface
the error.

1. **Read § Research quality criteria below.** The new entry must satisfy
   every criterion. If the in-context research material doesn't, flag the
   gap to the user and either iterate or abort.

2. **Pick a filename.**
   - Date-prefixed (`YYYY-MM-DD-<slug>.md`) when the research has a clear
     date.
   - Bare slug (`<slug>.md`) when the research is long-lived/continuous or
     the date is genuinely unknown.

3. **Write the file** at `docs/research/<filename>` with frontmatter:

   ```yaml
   ---
   title: <human-readable title>
   date: YYYY-MM-DD                # or "unknown"
   topic: <freeform tag>           # e.g. retrieval-quality, prompt-engineering
   summary: <one-liner, ≤200 chars>
   status: complete                # draft | complete | superseded
   ---
   ```

   Then a blank line, then the H1, then the body.

4. **Refresh the index.** If the librarian MCP is available:

   ```
   artifact_refresh(action="gather", id="<this-tracker-id>")
   # synthesize new params from gather output
   artifact(action="update", id="<this-tracker-id>",
            patch={params: {...}}, commit_refresh=true)
   ```

   If librarian is unavailable, the file is still saved correctly; the next
   librarian-equipped session refreshes the index. No manual table edit.

5. **Bootstrap migration** (only on explicit user request, not autonomous):
   scan `docs/**/research-*.md` and `docs/observations.md` for stragglers.
   Propose `git mv` candidates in § Suspected stragglers, wait for user
   confirmation, then move + add frontmatter.

## Research quality criteria

A new entry must satisfy every criterion below. If the material does not
meet them, flag the gap before saving — do not save a partial entry
silently.

### C-1 — Definition: what counts as research

A research entry maps external evidence (papers, benchmarks, production
data, empirical observations) to a question the project needed to answer.
It is **not**:

- A plan or spec → `docs/superpowers/plans/` or `docs/superpowers/specs/`
- An issue or bug note → `docs/issues/`
- An observation log → `docs/trackers/` or `docs/observations.md`
- An architecture decision record → ADR format

If the document's primary purpose is to record what was *decided*, it is
not research. If it records what was *found* — and the finding could
outlive the decision — it is research.

### C-2 — Required content

Every research file must include:

1. **A findings or conclusions section.** Name it `## Key Takeaways`,
   `## Summary`, `## Conclusions`, or equivalent. Minimum one paragraph.
   State what you learned, not only what you read.

2. **A sources section** when the entry cites external material. Name it
   `## Sources` or `## References`. Omit only when the entry is purely
   empirical (benchmarks on local hardware with no external citations).

3. **Methodology** when the entry involves measurements, benchmarks, or
   controlled tests. Document: setup, inputs, scoring criteria, and what
   was held constant. See `2026-04-03-embedding-model-benchmark.md` for
   the canonical pattern.

### C-3 — Citation discipline

Cite every external source by its full, traceable identity:

- **arXiv papers:** include arXiv ID (`arXiv:YYMM.NNNNN`) and the full
  paper title.
- **Web sources:** include the full URL and a human-readable title.
- **Industry reports / blog posts:** include URL, org or author, and
  publication date if available.

Anti-pattern: `"the literature suggests…"` or `"research shows…"` without
a specific, followable source. Every factual claim sourced from outside
this codebase must have a traceable anchor in `## Sources`.

### C-4 — Length

No minimum. Hard cap: ~800 lines (split by sub-topic beyond that).

Ratio: at least one paragraph of synthesis per cited source. A file that
is 80% source list and 20% analysis is a bookmark, not research.

### C-5 — Voice

Write factual, traceable, source-anchored prose:

- State findings as measurements: `"Model X scored 0.82 on Tier 4 vs
  0.61 for Model Y"`, not `"Model X is noticeably better"`.
- Qualify setup-dependent claims: `"under this benchmark's scoring
  criteria"`, not `"in general"`.
- Distinguish what a source says from what you concluded from it.

Anti-pattern: opinion dressed as research. If a claim has no traceable
source and is not a direct measurement from this codebase, label it
explicitly as an inference or hypothesis.

### C-6 — When to mark `status: superseded`

Mark `status: superseded` when:

- A newer entry covers the same question with updated data or methodology.
- The finding has been invalidated by a code or architecture change.
- The entry's conclusions conflict with a newer entry and the newer wins.

When superseding, add a blockquote below the H1:

> **Superseded** by [YYYY-MM-DD-slug.md](./YYYY-MM-DD-slug.md) —
> one-line reason.

Do not delete the superseded file. The index renders superseded entries
last.

### C-7 — Frontmatter discipline

Every file must open with a five-key YAML block in this order:

```yaml
---
title: <human-readable title>
date: YYYY-MM-DD          # or "unknown" if genuinely undatable
topic: <freeform tag>     # e.g. retrieval-quality, prompt-engineering
summary: <one-liner, ≤ 200 chars>
status: complete          # draft | complete | superseded
---
```

- Quote `title` or `summary` values that contain colons (YAML requires it).
- `summary` must fit 200 characters — rephrase rather than truncate mid-word.
- `topic` is freeform; do not invent an enum.
- `date: unknown` is valid; do not guess.
- Do not add extra keys — they are silently ignored but create schema drift.

## Suspected stragglers

*(Refreshed by gather pass. None at index creation time.)*

## History

### 2026-05-08 — Index created

Tracker artifact created from spec
`docs/superpowers/specs/2026-05-08-researcher-tracker-design.md`. 5 existing
research files indexed; 2 stragglers migrated from `docs/` root with frontmatter
(`research-progressive-disclosure.md`, `research-validation.md`) — both with
`date: unknown`. Quality criteria authored via Hamsa session.

