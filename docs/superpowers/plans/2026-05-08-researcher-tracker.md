# Researcher Tracker — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Note:** Task 3 is interactive (Hamsa session). It must run in the main session, not a subagent. All other tasks are subagent-friendly.

**Goal:** Stand up a research-index tracker at `docs/research/README.md` that catalogs every research file in the codescout repo, with frontmatter as source-of-truth and an agent-agnostic save workflow embedded in the README body.

**Architecture:** Single librarian artifact (`kind=tracker`, augmented). Body holds workflow + Hamsa-authored quality criteria + history. Params hold an 8-field array per entry, refreshed from frontmatter via gather. No Rust changes in Phase 1; pure markdown + librarian augmentation.

**Tech Stack:** Markdown, YAML frontmatter, librarian MCP (`artifact`, `artifact_augment`, `artifact_refresh`), MiniJinja render template.

**Scope:** Phase 1 only (per spec § Phase plan). Phase 2 = "use it for a few weeks", Phase 3 = Rust archetype promotion — both deferred until Phase 1 bakes.

**Spec:** [`docs/superpowers/specs/2026-05-08-researcher-tracker-design.md`](../specs/2026-05-08-researcher-tracker-design.md)

---

## File Map

| File | Action | Purpose |
|---|---|---|
| `docs/research/2026-04-03-embedding-model-benchmark.md` | Modify (prepend frontmatter) | Existing research |
| `docs/research/2026-03-21-claude-prompt-engineering.md` | Modify (prepend frontmatter) | Existing research |
| `docs/research/2026-03-21-superpowers-prompt-patterns.md` | Modify (prepend frontmatter) | Existing research |
| `docs/research/2026-04-02-usage-analysis.md` | Modify (prepend frontmatter) | Existing research |
| `docs/research/multi-agent-context-loss.md` | Modify (prepend frontmatter) | Existing research |
| `docs/research-progressive-disclosure.md` | `git mv` → `docs/research/` + frontmatter | Straggler migration |
| `docs/research-validation.md` | `git mv` → `docs/research/` + frontmatter | Straggler migration |
| `docs/research/README.md` | Create via `artifact(create, kind=tracker)` | The tracker artifact |

---

## Task 1: Add frontmatter to existing research files

**Files:** 5 existing research files under `docs/research/`.

Frontmatter per file (use these exact values):

| File | title | date | topic | summary | status |
|---|---|---|---|---|---|
| `2026-04-03-embedding-model-benchmark.md` | Embedding Model Benchmark for Semantic Search | 2026-04-03 | retrieval-quality | Compares 5 embedding models on 20 codescout-specific test cases across 4 difficulty tiers. | complete |
| `2026-03-21-claude-prompt-engineering.md` | Claude System Prompt Engineering Research | 2026-03-21 | prompt-engineering | Patterns and anti-patterns observed across Claude system prompts in production agentic tools. | complete |
| `2026-03-21-superpowers-prompt-patterns.md` | Superpowers Plugin — Prompt Architecture Patterns | 2026-03-21 | prompt-engineering | Architectural patterns used by the Superpowers plugin's skills and slash commands. | complete |
| `2026-04-02-usage-analysis.md` | Usage Analysis — 2026-04-02 | 2026-04-02 | telemetry | Cross-project analysis of `usage.db` from codescout (self, Rust) and backend-kotlin (client). | complete |
| `multi-agent-context-loss.md` | Context Loss and Compound Error in Multi-Agent LLM Systems | 2026-03-15 | agent-architecture | Empirical and theoretical analysis of compound error in delegation trees vs single-session skill-based architectures. | complete |

- [ ] **Step 1: Verify each file currently lacks frontmatter**

Run: `for f in docs/research/2026-04-03-embedding-model-benchmark.md docs/research/2026-03-21-claude-prompt-engineering.md docs/research/2026-03-21-superpowers-prompt-patterns.md docs/research/2026-04-02-usage-analysis.md docs/research/multi-agent-context-loss.md; do head -1 "$f"; done`

Expected: each line is `# <title>` (no `---` frontmatter delimiter). If any already has frontmatter, skip that file in Step 2 and note it in the commit message.

- [ ] **Step 2: Prepend frontmatter to each file**

For each row in the table above, write the frontmatter block ABOVE the existing first H1. Use `edit_file` with `insert: "prepend"`.

Example for the first file:

```yaml
---
title: Embedding Model Benchmark for Semantic Search
date: 2026-04-03
topic: retrieval-quality
summary: Compares 5 embedding models on 20 codescout-specific test cases across 4 difficulty tiers.
status: complete
---

```

(Note the blank line after `---` and before the existing H1.)

Repeat for all 5 files using the values from the table.

- [ ] **Step 3: Verify all 5 parse as YAML and the H1 still follows**

Run: `for f in docs/research/2026-04-03-embedding-model-benchmark.md docs/research/2026-03-21-claude-prompt-engineering.md docs/research/2026-03-21-superpowers-prompt-patterns.md docs/research/2026-04-02-usage-analysis.md docs/research/multi-agent-context-loss.md; do echo "=== $f ==="; head -10 "$f"; done`

Expected: each file starts with `---`, has 5 frontmatter keys, ends frontmatter with `---`, blank line, then `# <title>`.

- [ ] **Step 4: Commit**

```bash
git add docs/research/2026-04-03-embedding-model-benchmark.md \
        docs/research/2026-03-21-claude-prompt-engineering.md \
        docs/research/2026-03-21-superpowers-prompt-patterns.md \
        docs/research/2026-04-02-usage-analysis.md \
        docs/research/multi-agent-context-loss.md
git commit -m "docs(research): add frontmatter to existing research files

Source-of-truth for the upcoming research-index tracker per spec
2026-05-08-researcher-tracker-design.md. Frontmatter keys: title,
date, topic, summary, status."
```

---

## Task 2: Migrate stragglers from `docs/` root

**Files:**
- `docs/research-progressive-disclosure.md` → `docs/research/research-progressive-disclosure.md`
- `docs/research-validation.md` → `docs/research/research-validation.md`

Frontmatter values:

| Destination | title | date | topic | summary | status |
|---|---|---|---|---|---|
| `research-progressive-disclosure.md` | Research Validation: Progressive Disclosure & Tool Discovery for LLM Agents | unknown | tool-discovery | Maps academic research on progressive disclosure and tool discovery to codescout's design. | complete |
| `research-validation.md` | Research Validation: The Science Behind codescout | unknown | foundational-research | Maps current academic research to codescout's core design decisions across 6 themes. | complete |

Both files lack a clear research date in body or filename. Use `date: unknown` per spec § Filename convention; the augmentation will sort by mtime as last resort and log via History.

- [ ] **Step 1: Verify both source files exist and target paths are free**

Run: `ls -la docs/research-progressive-disclosure.md docs/research-validation.md docs/research/research-progressive-disclosure.md docs/research/research-validation.md 2>&1`

Expected: first two lines show the source files; last two lines show `No such file or directory` (target paths free).

- [ ] **Step 2: `git mv` both files**

```bash
git mv docs/research-progressive-disclosure.md docs/research/research-progressive-disclosure.md
git mv docs/research-validation.md docs/research/research-validation.md
```

- [ ] **Step 3: Prepend frontmatter to each migrated file**

For `docs/research/research-progressive-disclosure.md`:

```yaml
---
title: "Research Validation: Progressive Disclosure & Tool Discovery for LLM Agents"
date: unknown
topic: tool-discovery
summary: Maps academic research on progressive disclosure and tool discovery to codescout's design.
status: complete
---

```

For `docs/research/research-validation.md`:

```yaml
---
title: "Research Validation: The Science Behind codescout"
date: unknown
topic: foundational-research
summary: Maps current academic research to codescout's core design decisions across 6 themes.
status: complete
---

```

(Title is quoted because it contains a colon — YAML requires it.)

- [ ] **Step 4: Verify both files start with the new frontmatter**

Run: `head -8 docs/research/research-progressive-disclosure.md docs/research/research-validation.md`

Expected: each file starts with `---`, frontmatter, `---`, blank line, then existing H1.

- [ ] **Step 5: Commit**

```bash
git add docs/research/research-progressive-disclosure.md docs/research/research-validation.md
git commit -m "docs(research): migrate stragglers from docs/ root

Moves docs/research-progressive-disclosure.md and docs/research-validation.md
into docs/research/ and adds frontmatter. Per spec 2026-05-08-researcher-tracker-design.md
§ Bootstrap flow. Date: unknown — sort falls back to mtime per spec § Error handling."
```

---

## Task 3: Hamsa session — quality criteria draft (interactive, main session only)

**Files:**
- Create: `docs/research/.quality-criteria.draft.md` (temporary scratch — gets folded into README in Task 4)

This task must run in the **main session**, not a subagent — it requires the user-facing `/buddy:summon hamsa` slash command.

- [ ] **Step 1: Summon Hamsa**

In the main session, invoke:

```
/buddy:summon hamsa
```

- [ ] **Step 2: Brief Hamsa with this prompt**

Paste this brief (copy-paste exactly):

```
Author the "Research Quality Criteria" section for docs/research/README.md
in the codescout repo. The section becomes the standards document any agent
(Claude Code, Copilot, Antigravity) reads when the user says "save this
research".

The section must cover:

1. Definition — what counts as a "research" entry vs a plan/spec/issue/observation/note.
2. Required content — what every research file must include (e.g. an explicit
   findings or summary section, sources/references, methodology where applicable).
3. Citation discipline — URLs, arxiv IDs, paper titles. Anti-pattern: vague
   "the literature says" without traceable sources.
4. Length expectations — no minimum; reasonable cap; ratio guidance for sources
   vs prose if useful.
5. Voice — factual, traceable, source-anchored. Anti-pattern: opinion piece
   dressed as research.
6. When to mark `status: superseded` — what triggers it, how to point to the
   newer entry.
7. Frontmatter discipline — title, date, topic, summary, status are required;
   summary ≤ 200 chars; topic is a freeform tag, not enumerated.

Length: aim for 60-120 lines of markdown. Headings: H3 per criterion under an
H2 "Research quality criteria". Voice: imperative ("Cite sources by..." not
"It is recommended to...").

Existing research examples to ground the criteria:
- docs/research/2026-04-03-embedding-model-benchmark.md (long, methodology-heavy)
- docs/research/multi-agent-context-loss.md (medium, source-heavy with arxiv links)
- docs/research/research-validation.md (short, source-mapping)
```

- [ ] **Step 3: Iterate with Hamsa until the section satisfies all 7 points**

User reviews each draft; Hamsa revises. No fixed iteration count.

- [ ] **Step 4: Save the final draft**

Once accepted, save Hamsa's output to `docs/research/.quality-criteria.draft.md`. Use `create_file`. The file is temporary and gets folded into the README body in Task 4.

- [ ] **Step 5: Dismiss Hamsa**

```
/buddy:dismiss hamsa
```

(Or let auto-dismiss handle it.)

---

## Task 4: Compose README body sections

**Files:**
- Create: `docs/research/.readme-body.draft.md` (temporary scratch — gets passed to `artifact(create)` in Task 5)
- Read: `docs/research/.quality-criteria.draft.md` (from Task 3)

This task assembles the markdown that goes into the tracker's `body` parameter. The body has 4 sections: How to save, Research quality criteria (Task 3 output), Suspected stragglers (initial placeholder), History (initial entry).

- [ ] **Step 1: Compose `docs/research/.readme-body.draft.md`**

Write the file with this exact content, replacing `{{QUALITY_CRITERIA}}` with the contents of `docs/research/.quality-criteria.draft.md`:

````markdown
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

{{QUALITY_CRITERIA}}

## Suspected stragglers

*(Refreshed by gather pass. None at index creation time.)*

## History

### 2026-05-08 — Index created

Tracker artifact created from spec
`docs/superpowers/specs/2026-05-08-researcher-tracker-design.md`. 5 existing
research files indexed; 2 stragglers migrated from `docs/` root with frontmatter
(`research-progressive-disclosure.md`, `research-validation.md`) — both with
`date: unknown`. Quality criteria authored via Hamsa session.
````

- [ ] **Step 2: Replace `{{QUALITY_CRITERIA}}` with Task 3 output**

Run:
```bash
QC=$(cat docs/research/.quality-criteria.draft.md)
python3 -c "
import sys
body = open('docs/research/.readme-body.draft.md').read()
qc = open('docs/research/.quality-criteria.draft.md').read()
open('docs/research/.readme-body.draft.md','w').write(body.replace('{{QUALITY_CRITERIA}}', qc))
"
```

- [ ] **Step 3: Verify the substitution worked**

Run: `grep -c '{{QUALITY_CRITERIA}}' docs/research/.readme-body.draft.md`

Expected: `0` (no remaining placeholder).

Run: `grep -c '## Research quality criteria' docs/research/.readme-body.draft.md`

Expected: `1` (Hamsa-authored section is in place).

---

## Task 5: Create the tracker artifact

**Operations:**
1. `artifact(action="create", kind="tracker", augment={prompt, params})` — creates `docs/research/README.md` atomically with body + initial augmentation (prompt + empty params).
2. `artifact_augment(id, merge=false, prompt, params, render_template)` — re-attach the augmentation including the render_template (the `augment` field on `artifact(create)` only accepts prompt + params, not render_template).

- [ ] **Step 1: Read the body draft from Task 4**

Capture the contents of `docs/research/.readme-body.draft.md`.

- [ ] **Step 2: Call `artifact(action="create")`**

```
artifact(
  action="create",
  kind="tracker",
  rel_path="docs/research/README.md",
  title="Research Index",
  topic="research",
  status="active",
  body=<contents of .readme-body.draft.md>,
  augment={
    prompt: """
Maintain the index of research artifacts under docs/research/.

Source of truth: each docs/research/*.md file's frontmatter (title, date,
topic, summary, status). Glob the folder via gather_from: file. Parse
frontmatter; derive id (filename slug), path (file location), and
sources_count (count URLs in body or under a ## Sources / ## References
heading) per entry. Sort entries by filename date if present, else
frontmatter date, else file mtime.

Update body § Suspected stragglers from a second gather pass: glob
docs/**/research-*.md plus docs/observations.md. Report any path NOT under
docs/research/ that matches the heuristic. Do not move files.

Conflict resolution: frontmatter wins over filename for date display.
Frontmatter missing/malformed → entry status defaults to "draft" with
summary "(missing frontmatter — needs review)".

Body sections (How to save, Research quality criteria) are human-edited;
NEVER overwrite them on refresh. Only update the [LIVE] Index render,
§ Suspected stragglers, and append to § History.

Length budget: params under 100 entries; body under 600 lines.
""",
    params: {entries: []}
  }
)
```

Capture the returned `id` — it's needed for every subsequent step.

- [ ] **Step 3: Call `artifact_augment` to attach the render_template**

```
artifact_augment(
  id="<id-from-step-2>",
  merge=false,
  prompt="<same prompt as Step 2>",
  params={entries: []},
  render_template="| ID | Date | Title | Topic | Status | Sources |\n|---|---|---|---|---|---|\n{% for e in entries %}| `{{ e.id }}` | {{ e.date }} | [{{ e.title }}]({{ e.path }}) | {{ e.topic or \"—\" }} | {{ e.status }} | {{ e.sources_count }} |\n{% endfor %}\n\n**Total:** {{ entries|length }} research entries\n({{ entries|selectattr(\"status\",\"equalto\",\"complete\")|list|length }} complete,\n {{ entries|selectattr(\"status\",\"equalto\",\"draft\")|list|length }} draft,\n {{ entries|selectattr(\"status\",\"equalto\",\"superseded\")|list|length }} superseded)"
)
```

The render_template is passed as a single string with `\n` escapes (MiniJinja parses it as multiline at render time).

- [ ] **Step 4: Verify the file was created and the augmentation is attached**

Run:
```
artifact(action="get", id="<tracker-id>", full=true, include_links=false)
```

Expected:
- `kind: tracker`, `status: active`
- `augmentation` field present with prompt, params (`{entries: []}`), and render_template
- `body` starts with `# Research Index` and contains the 4 sections

Run: `ls -la docs/research/README.md && head -40 docs/research/README.md`

Expected: file exists; first lines show frontmatter (if librarian writes one) + the rendered `[LIVE] Index` block (empty table because params are still empty), followed by body sections.

- [ ] **Step 5: Clean up scratch files**

```bash
rm docs/research/.quality-criteria.draft.md docs/research/.readme-body.draft.md
```

---

## Task 6: First refresh — populate index from frontmatter

**Operation:** `artifact_refresh(action="gather")` then `artifact(action="update", commit_refresh=true)`.

- [ ] **Step 1: Trigger a gather pass**

```
artifact_refresh(action="gather", id="<tracker-id-from-Task-5>")
```

This returns a context bundle including all 7 research files' frontmatter.

- [ ] **Step 2: Synthesize params from gather output**

Build the `entries` array from the gather output. Expected count: **7 entries** (5 originals + 2 migrated stragglers). Each entry has id (filename slug), title, path, date, topic, summary, sources_count, status.

- [ ] **Step 3: Update the artifact and commit the refresh**

```
artifact(
  action="update",
  id="<tracker-id>",
  patch={params: {entries: [...synthesized array...]}},
  commit_refresh=true
)
```

- [ ] **Step 4: Verify the rendered table**

```
artifact(action="get", id="<tracker-id>", full=true)
```

Expected:
- `[LIVE] Index` section now contains a 7-row table.
- All 7 entries listed; the 2 stragglers (`research-progressive-disclosure`, `research-validation`) appear with `date: unknown`.
- Total count: `**Total:** 7 research entries (7 complete, 0 draft, 0 superseded)`.

- [ ] **Step 5: Verify Suspected stragglers section is empty**

Run: `grep -A 3 '## Suspected stragglers' docs/research/README.md`

Expected: section header followed by an empty/none-found message. If any straggler shows up, that's a bug — investigate before proceeding.

- [ ] **Step 6: Commit the refreshed README**

```bash
git add docs/research/README.md
git commit -m "docs(research): create research-index tracker artifact

Creates docs/research/README.md as a kind=tracker librarian artifact.
Body holds: How to save workflow + Hamsa-authored quality criteria +
Suspected stragglers (empty) + History. Params hold an 8-field index
array refreshed from each file's frontmatter via gather pass.

Per spec 2026-05-08-researcher-tracker-design.md § Architecture.
Initial refresh indexes all 7 research files (5 original + 2 migrated
stragglers).

Phase 1 of 3 (one-off tracker → schema bake → archetype promotion).
Phase 3 (Rust archetype in crates/librarian-mcp/) deferred until
Phase 2 produces a stable schema."
```

---

## Task 7: Smoke-test the save flow end-to-end

This task validates the workflow described in the README by running a small
synthetic save and confirming the index updates.

- [ ] **Step 1: Read the `## How to save a research` section**

Run: `read_markdown(docs/research/README.md, heading="## How to save a research")`

Expected: section is readable and contains the 5 numbered steps.

- [ ] **Step 2: Read the `## Research quality criteria` section**

Run: `read_markdown(docs/research/README.md, heading="## Research quality criteria")`

Expected: Hamsa-authored content present, all 7 criteria covered. If unreadable, abort per spec § Error handling and investigate.

- [ ] **Step 3: Create a synthetic test research entry**

Create `docs/research/2026-05-08-tracker-smoketest.md` with:

```markdown
---
title: Tracker smoke-test entry
date: 2026-05-08
topic: meta
summary: Synthetic entry created during Phase 1 validation. Delete after refresh confirms index updates correctly.
status: draft
---

# Tracker smoke-test entry

This file exists only to validate that the save-flow end-to-end works.
The next refresh should pick it up; this file gets deleted in Step 5.
```

- [ ] **Step 4: Refresh and verify the index grows to 8 entries**

```
artifact_refresh(action="gather", id="<tracker-id>")
# synthesize new entries with the smoketest file added
artifact(action="update", id="<tracker-id>",
         patch={params: {entries: [...]}}, commit_refresh=true)
```

Then:
```
artifact(action="get", id="<tracker-id>", heading="[LIVE] Index")
```

Expected: 8 entries, `tracker-smoketest` row visible with `status: draft`. Total: `**Total:** 8 research entries (7 complete, 1 draft, 0 superseded)`.

- [ ] **Step 5: Remove the smoke-test entry and refresh again**

```bash
rm docs/research/2026-05-08-tracker-smoketest.md
```

```
artifact_refresh(action="gather", id="<tracker-id>")
artifact(action="update", id="<tracker-id>",
         patch={params: {entries: [...without smoketest...]}}, commit_refresh=true)
```

Expected: back to 7 entries.

- [ ] **Step 6: Final verification commit**

If the smoke-test passed and the README is back to 7 entries, no commit needed (the changes were ephemeral). If anything required adjustment to the augmentation prompt or render template, commit those fixes:

```bash
git add docs/research/README.md
git commit -m "fix(research): adjust tracker after smoke-test"
```

---

## Done criteria (Phase 1 exit)

- ✅ All 7 research files (5 original + 2 migrated) live under `docs/research/` with valid 5-key frontmatter.
- ✅ `docs/research/README.md` is a `kind=tracker` librarian artifact with augmentation prompt, render template, and 7-entry params.
- ✅ Rendered `[LIVE] Index` shows 7 rows, sorted correctly per the spec sort rule.
- ✅ Hamsa-authored `## Research quality criteria` section is in the body and covers all 7 points from the brief.
- ✅ Smoke-test (Task 7) confirmed save → refresh → index update works end-to-end.
- ✅ Commits land on `experiments` branch; not pushed; not cherry-picked to master until Phase 2 friction is observed.

## Phase 2 watchpoints (informational, not part of this plan)

While using the tracker over the next few weeks, watch for:
- Schema friction: any field consistently empty or consistently overloaded?
- Save-flow friction: any step the user has to remind agents about repeatedly?
- Heuristic miss in §Suspected stragglers: research files outside the patterns we scan.
- Hamsa criteria gaps: any save where an agent didn't know what to do.

These observations feed Phase 3 archetype design.
