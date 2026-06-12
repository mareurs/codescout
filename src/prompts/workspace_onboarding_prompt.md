# WORKSPACE MODE — Multi-Project Onboarding

You are onboarding a **multi-project workspace**. The projects are listed
in the "Discovered projects" table that follows this prompt.

> Single-project Phase 1 / Phase 2 instructions do NOT apply here — this
> file is the complete flow. Do NOT spawn a "root" exploration subagent;
> root-layer content is absorbed into workspace `architecture` (Top-Level
> Code Map + Generic Navigation) per the included memory templates.

---

{{include: memory-templates.md}}

---

## Phase 1 — Workspace Survey

Breadth-first sweep of all projects before any deep dive.

For each project in the workspace:
1. `tree("{project_root}")` — top-level structure
2. `symbols` on the main source directory
3. `read_file` the build config (Cargo.toml / package.json / go.mod)
4. Note: purpose, tech stack, rough file count, key entry points

**Write a Workspace Exploration Summary** before proceeding:
- One-sentence description of each project
- Cross-project dependencies
- Shared infrastructure (CI, deployment, tooling)
- Key cross-project patterns

<HARD-GATE>
Do NOT proceed to Phase 2 until the Workspace Exploration Summary is
written. The summary is required input for Phase 3 subagent prompts.
</HARD-GATE>

---

## Phase 2 — Stale-Project Cleanup

**Runs only when `onboarding(force: true)`.** Skip this phase otherwise.

1. List `.codescout/projects/*/` directories.
2. Intersect with the live project list from Phase 1.
3. Orphans = directories not in the live set.
4. For each orphan, read the most-recent mtime under `<orphan>/memories/`.

If any orphans found, ask the user **once**:

```
Found N orphaned codescout projects (no longer in workspace):
  - <id>   last touched <date>
  - <id>   last touched <date>

Delete all? [y/N]
```

- On `y`: run `memory(action: "delete", project_id: "<id>", topic: "<topic>")`
  for every memory file under each orphan, then remove the empty
  `<project>/memories/` directory.
- On `N` or no answer: log `cleanup: skipped (N orphans retained)` for the
  final summary. Do NOT re-ask in subsequent phases.

<HARD-GATE>
Phase 2 is complete when the user has answered the cleanup prompt OR no
orphans were found. Record the outcome for the final summary.
</HARD-GATE>

---

## Phase 3 — Per-Project Deep Dives

Dispatch one Agent subagent per project **in a single parallel batch** —
all Agent tool calls in one response turn.

**Subagent prompt template** (fill placeholders for each project):

```
You are deep-diving the "{project_id}" project in a multi-project workspace.

## Workspace Context
{paste the Workspace Exploration Summary from Phase 1}

## Your Assignment

Write all 6 memories (project-scope) defined in memory-templates.md:
1. project-overview
2. architecture
3. conventions
4. development-commands
5. domain-glossary
6. gotchas

For domain-glossary and gotchas with nothing project-specific to record,
write the empty stub from memory-templates.md (do NOT skip).

Use: memory(action: "write", project_id: "{project_id}", topic: "<topic>", content: "...")

## Sibling Projects (for context, do NOT deep-dive these)
{list other projects with 1-sentence descriptions}

## Exploration Steps (scoped to {project_root}/)
1. tree("{project_root}") — structure
2. symbols on ALL source files in the project
3. read_file on build config, README if present
4. symbols(name=..., include_body=true) on 3-5 key functions/types
5. semantic_search for 3+ concepts specific to this project
6. Read test files to understand testing patterns

## Rules
- Be specific: file paths, function names, concrete patterns
- Do NOT document sibling project internals — note dependencies only
- 15-40 lines per memory (or empty stub for eligible topics)
- When you encounter types from sibling projects, note them as
  "imports FooType from {sibling}" but do not document FooType itself

## End-of-Run Manifest
End your final response with a single line listing exactly what you wrote:

  MANIFEST: project-overview, architecture, conventions, development-commands, domain-glossary, gotchas

The line is advisory — the parent verifies by reading back. Topics MUST
exactly match the 6 above (use empty-stub when nothing to say).
```

<HARD-GATE>
Do NOT proceed to Phase 4 until every Agent subagent has **returned**
(not just been dispatched). If a subagent fails or times out, note the
failure but proceed to Phase 4 — the read-back will catch it.
</HARD-GATE>

---

## Phase 4 — Coverage Verification

For each `(project_id, topic)` pair (6 × N projects), run:

```
memory(action: "read", project_id: "<id>", topic: "<topic>")
```

Build a coverage matrix with three states per cell:

- **✓** — content present, longer than the empty stub.
- **EMPTY** — content byte-equal to the canonical empty stub from
  memory-templates.md. Counts as ✓ for `domain-glossary` and `gotchas`
  only. For other topics, treat EMPTY as MISSING.
- **MISSING** — read returns "not found" / null / error / content shorter
  than the stub.

If the matrix has any MISSING cells, run the **retry loop** (max 2 attempts):

```
attempt = 0
while attempt < 2 and matrix has MISSING:
    failed_projects = projects with at least one MISSING cell
    re-dispatch ONE subagent per failed project with prompt:

        "Re-onboarding: {project_id}.
         Previous run was incomplete. You wrote: {present_topics}.
         Missing: {missing_topics}.
         Constraints:
           - Do NOT re-read sibling projects.
           - Do NOT rewrite topics already present.
           - For empty-stub eligible topics with nothing to say, write the
             stub exactly:
             {paste EMPTY_STUB block from memory-templates.md}
         End with: MANIFEST: {missing_topics}"

    wait for all subagents to return
    re-read missing cells
    attempt += 1
```

If the matrix still has MISSING after the retry budget is exhausted, abort:

```
Onboarding aborted at Phase 4 — coverage incomplete after retries:

  <project>   missing: [<topic>, <topic>]

Workspace synthesis and CLAUDE.md refresh skipped.
Re-run /onboarding force to retry just the failed projects.
```

Skip Phase 5 and Phase 6 on abort.

<HARD-GATE>
Phase 4 is complete only when the coverage matrix is 100% green
(✓ or eligible-EMPTY for every cell). Otherwise, abort.
</HARD-GATE>

---

## Phase 5 — Workspace Synthesis

Read per-project memories that feed cross-project synthesis:
- `memory(action: "read", project_id: "<id>", topic: "architecture")` for each project
- `memory(action: "read", project_id: "<id>", topic: "conventions")` for each project
- (Read other topics only when needed for cross-project content.)

Then write the 5 workspace-scope memories from `memory-templates.md`:
- `architecture` (with all 5 required subsections — Project Map,
  Cross-Project Dependencies, Shared Infrastructure, Top-Level Code Map,
  Generic Navigation)
- `conventions`
- `development-commands`
- `domain-glossary` (empty stub if no cross-project terms)
- `gotchas` (empty stub if no cross-project pitfalls)

Then generate the **system prompt** per the `workspace-scope: system-prompt`
section and write it **directly** to `.codescout/system-prompt.md` with
`create_file` — NOT `memory(action: "write", topic: "system-prompt")`. The
system prompt is the always-on root file injected into every session, not a
memory topic.

After writing, read each back and verify:
- 5 markdown memories non-empty (or eligible-EMPTY)
- system-prompt file exists and contains its required subsections

<HARD-GATE>
Phase 5 is complete only when all 5 workspace memories and the system prompt
are written and verified. On failure, retry once; if still failing, abort to user.
</HARD-GATE>

---

## Phase 6 — CLAUDE.md Refresh

Compute the canonical memory table from what was actually written this run.
Each row's "What's inside" cell is the first `## H2` of the memory body
(empty-stub memories show `_no project-specific entries_`).

Read existing `CLAUDE.md`. Locate `## codescout Memories`:
- If present: compute a unified diff for the table block.
- If absent: propose adding a new `## codescout Memories` section.

Print the diff and ask the user **once**:

```
Proposed CLAUDE.md memory-table update:

  [unified diff, ~10 lines]

Apply? [y/N]
```

- On `y`: `edit_markdown(path: "CLAUDE.md", action: "replace", heading: "## codescout Memories", content: <new table>)`
- On `N` or no answer: log `claude_md: skipped (user declined)` to the
  final summary. Do NOT re-ask.

<HARD-GATE>
Phase 6 is complete when the user has answered. No follow-up questions.
</HARD-GATE>

---

## Final Summary

Print exactly one block, no chatter:

```
Onboarding complete.

  Per-project memories
    <id>          6/6 ✓
    <id>          6/6 ✓ (1 retry)

  Workspace memories     6/6 ✓

  Cleanup                <N orphans deleted | skipped | none>
  CLAUDE.md              <updated | skipped>
  System prompt          v<old> → v<new>
```

Skipped/failed phases show `✗ <reason>` instead of `✓`.
