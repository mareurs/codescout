# Researcher Tracker — Design

**Status:** draft
**Date:** 2026-05-08
**Owner:** Marius
**Scope:** codescout repo (Phase 1–2); cross-project archetype (Phase 3)

## Goal

Give the codescout repo a single, agent-agnostic source of truth for "what
research lives here, what good research looks like, and how to add a new one".
Today, research files live in two places (`docs/research/` and `docs/` root)
with no shared frontmatter, no quality bar, and no index. Readers and agents
have to discover them by ad-hoc browsing.

## Non-goals

- **Not** an autonomous file-mover. Files are saved or migrated only on
  explicit user instruction (`save this research`, or a confirmed bootstrap
  sweep).
- **Not** a per-research living document. Each entry stays a standalone
  markdown file; the tracker only catalogs them.
- **Not** a new tool or MCP API surface in Phase 1–2. Pure markdown +
  librarian augmentation. Rust changes deferred to Phase 3.
- **Not** coupled to claude-plugins. Per CLAUDE.md § *Agent-Agnostic Design*,
  the source of truth is a repo file any agent (Claude Code, Copilot,
  Antigravity, etc.) can read.

## Architecture

One librarian artifact, one canonical research folder, frontmatter as
single source of truth per entry.

```
docs/research/
├── README.md                              # kind=tracker, augmented
│   ├── (params, hidden)                   # 8-field array per entry
│   ├── [LIVE] Index                       # render_template → table
│   ├── ## How to save a research          # workflow steps (agent-agnostic)
│   ├── ## Research quality criteria       # Hamsa-authored standards
│   ├── ## Suspected stragglers            # heuristic flags from gather sweep
│   └── ## History                         # dated session log
├── 2026-04-03-embedding-model-benchmark.md   # frontmatter-headed entries
├── 2026-03-21-claude-prompt-engineering.md
├── 2026-03-21-superpowers-prompt-patterns.md
├── 2026-04-02-usage-analysis.md
├── multi-agent-context-loss.md
├── research-progressive-disclosure.md             # migrated from docs/
└── research-validation.md                         # migrated from docs/
```

### Per-entry frontmatter

```yaml
---
title: Embedding Model Benchmark for Semantic Search
date: 2026-04-03
topic: retrieval-quality
summary: Compares 5 embedding models on 20 codescout-specific test cases across 4 difficulty tiers.
status: complete   # draft | complete | superseded
---
```

5 keys in frontmatter; `id` (slug from filename), `path`, and `sources_count`
are derived by the gather pass.

### Index params (8 fields, derived per entry)

| Field | Source | Notes |
|---|---|---|
| `id` | filename slug | stable key, unchanged across edits |
| `title` | frontmatter | required |
| `path` | filesystem | rel to repo root |
| `date` | frontmatter | ISO 8601 |
| `topic` | frontmatter | freeform tag — used for grouping |
| `summary` | frontmatter | one-liner; ≤200 chars enforced by quality criteria |
| `sources_count` | computed | counts URLs/links in body or `## Sources` section |
| `status` | frontmatter | `draft \| complete \| superseded` |

### Filename convention

- **Date-prefixed** (`YYYY-MM-DD-<slug>.md`) — preferred for new entries created
  via the save flow. Date is the research's own date, not the file's mtime.
- **Bare slug** (`<slug>.md`) — acceptable when the date is unknown (e.g.
  migrated stragglers without a clear research date) or the research is
  long-lived and continuously updated. Frontmatter `date` is then the only
  date source.

The bootstrap flow does NOT invent dates. If the original file has no date in
its name and no obvious date in body, the migrated filename keeps the bare
slug; frontmatter `date` is set to the file's earliest git commit date or to
"unknown" with a History note. User confirms during bootstrap.

## Workflows

### Save flow (`save this research`)

```
agent reads README §How to save + §Research quality criteria
  → compose frontmatter + body per quality criteria
  → write docs/research/YYYY-MM-DD-<slug>.md
  → if librarian available: artifact_augment(id=<tracker>, merge=true)
  → next refresh re-globs folder, rebuilds [LIVE] Index
```

The agent is the active LLM in the user's session — Claude Code, Copilot,
Antigravity, etc. The README workflow text is identical for all of them.

### Refresh flow (recurring augmentation)

Gather sources:
- `gather_from: file` glob `docs/research/*.md` → frontmatter parse
- `gather_from: file` glob `docs/**/research-*.md` + content heuristic → straggler scan

Synthesizer:
- Rebuild `params.entries` from frontmatter
- Update body § *Suspected stragglers*: replace section with current heuristic hits
- Append History entry on substantive change (new entry, status flip, straggler discovered)

### Bootstrap flow (one-shot, manual)

Triggered when the tracker is first set up, then again on demand if scattered
research files are suspected.

```
agent reads README §How to save
  → run heuristic scan:
      filename: docs/**/research-*.md, docs/observations.md
      content:  ## Sources / ## References sections, arxiv URLs, bib markers
  → list candidates in §Suspected stragglers with proposed new path
  → user confirms which to move
  → agent: `git mv` old new; prepend frontmatter; commit
  → trigger refresh
```

Bootstrap is **prose in README**, not code. Any agent that can read markdown
and run shell commands can execute it.

## Augmentation prompt (sketch)

Imperative voice; names gather sources; conflict-resolution rule; body/params
boundary. The actual prompt gets refined when the artifact is created — this
sketch sets the shape.

```
Maintain the index of research artifacts under docs/research/.

Source of truth: each docs/research/*.md file's frontmatter (title, date,
topic, summary, status). Glob the folder via gather_from: file. Parse
frontmatter; derive id (filename slug), path (file location), and
sources_count (count URLs in body) per entry. Sort entries by date desc.

Update body § Suspected stragglers from a second gather pass: glob
docs/**/research-*.md plus docs/observations.md, report any path NOT under
docs/research/ that matches the heuristic. Do not move files.

Conflict resolution: frontmatter wins over filename for date display, but
filename date drives sort order if present (else frontmatter date, else file
mtime). Frontmatter missing/malformed → entry status defaults to "draft" with
summary "(missing frontmatter — needs review)".

Body sections (How to save, Research quality criteria) are human-edited;
NEVER overwrite them on refresh. Only update [LIVE] Index, § Suspected
stragglers, and append to § History.

Length budget: params under 100 entries; body under 400 lines.
```

## Render template (sketch)

```jinja
| ID | Date | Title | Topic | Status | Sources |
|---|---|---|---|---|---|
{% for e in entries %}| `{{ e.id }}` | {{ e.date }} | [{{ e.title }}]({{ e.path }}) | {{ e.topic or "—" }} | {{ e.status }} | {{ e.sources_count }} |
{% endfor %}

**Total:** {{ entries|length }} research entries
({{ entries|selectattr("status","equalto","complete")|list|length }} complete,
 {{ entries|selectattr("status","equalto","draft")|list|length }} draft,
 {{ entries|selectattr("status","equalto","superseded")|list|length }} superseded)
```

## Research quality criteria — authored via Hamsa

The README § *Research quality criteria* section is the standards document
that any save flow reads. **Authored during implementation** by summoning
Hamsa (`/buddy:summon hamsa`). Hamsa drafts the section; user reviews; goes
into the README.

Criteria the section must cover (from this brainstorm):

- What constitutes a "research" entry vs a plan/spec/issue/observation.
- Required sections (Sources, key findings, methodology where applicable).
- Citation discipline (URLs, arxiv IDs, paper titles).
- Length expectations (no minimum; cap on reasonable length).
- When to mark `status: superseded`.
- Voice guidelines (factual, traceable, source-anchored — not opinion).

Hamsa fills in the actual prose during P1 implementation.

## Error handling

| Failure | Handling |
|---|---|
| Frontmatter missing/malformed | Entry status = `draft`, summary = `(missing frontmatter — needs review)`. History entry logs the file. |
| Save invoked but README §quality unreadable | Agent must abort save, surface the error. Explicit refusal — no fallback. |
| `artifact_augment` unavailable (non-librarian agent) | Save still works (file + frontmatter written). Next librarian-equipped session refreshes the index. |
| Bootstrap proposes move; target path exists | Skip; flag in §Suspected stragglers as `(target exists)`, leave original. |
| Frontmatter `date` mismatches filename date | Filename wins for sort; History logs `(date mismatch: file=X frontmatter=Y)`. |
| Entry has no filename date AND no frontmatter date | Sort by file mtime as a last resort; flag in History `(no date — sorted by mtime)`. |
| Suspected straggler scan returns 0 hits but a known straggler exists | Heuristic miss; user manually adds entry to §Suspected stragglers and refreshes. |

## Testing

**Phase 1:** No new Rust code → no unit tests. Validation is empirical:

1. Create `docs/research/README.md` artifact with augmentation prompt + params.
2. Add frontmatter to all 5 existing files in `docs/research/`.
3. Run a refresh; verify the rendered [LIVE] Index table contains all 5
   entries with correct fields.
4. Migrate `docs/research-progressive-disclosure.md` and
   `docs/research-validation.md` via the bootstrap flow (`git mv` + prepend
   frontmatter). Confirm they appear in the index after next refresh.
5. Test the save flow: simulate a "save this research" request from a fresh
   research output; confirm the new file lands in `docs/research/` with
   frontmatter and the index updates.

**Phase 3 (archetype):** Standard Rust unit test that `tracker_design` returns
the new `research_index` archetype with expected `name`, `prompt_template`,
`params_shape_example`, and `render_template` fields. Add to
`crates/librarian-mcp/src/tools/tracker_design.rs` test suite.

## Phase plan

| Phase | Scope | Exit criteria |
|---|---|---|
| **P1** — One-off tracker | Create README.md artifact; Hamsa-authored quality criteria; migrate 2 stragglers; add frontmatter to existing 5. | All 7 entries appear correctly in [LIVE] Index. |
| **P2** — Convention bake | Use the save flow for ~3 new researches over a few weeks. Adjust schema/prompt as friction surfaces. | Schema stable across 3 saves; no manual fixups needed. |
| **P3** — Archetype promotion | Add `research_index` archetype in `crates/librarian-mcp/`. Future projects spawn one via `librarian(tracker_design)`. | `tracker_design` returns the archetype; example project consumes it cleanly. |

P3 is gated on P2 producing a stable schema. Premature promotion = premature
lock-in.

## Open questions

- **Hamsa session shape:** does Hamsa author the quality criteria standalone,
  or in dialogue (Hamsa proposes, user iterates)? Defer to implementation.
- **Frontmatter validation:** Phase 1 is loose (any YAML with the 5 keys).
  Phase 2 may add a JSON schema in the augmentation. Decide after baking.
- **`sources_count` heuristic:** count URLs in body? Count lines under a
  `## Sources` heading? Both? Decide during the synthesizer prompt iteration.
- **Cross-references:** Q4 originally had a `cited_in[]` field. Dropped as
  too costly. Revisit if Phase 2 friction surfaces a need (e.g. plans
  silently going stale because their cited research got `superseded`).

## References

- CLAUDE.md § *Agent-Agnostic Design* (the rule that drove Approach 2-revised)
- `librarian(tracker_design)` archetypes (`reflective`, `task_list`, etc.)
- Existing trackers as shape references: `docs/trackers/tool-usage-patterns.md`
  (params + body), `docs/trackers/skill-frictions.md` (numbered list)
