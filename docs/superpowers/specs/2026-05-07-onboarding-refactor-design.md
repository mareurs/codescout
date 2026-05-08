# Onboarding Prompt Refactor — Design

**Date:** 2026-05-07
**Status:** Draft (awaiting user review before implementation plan)
**Motivation:** Audit of session `80d79d25` (backend-kotlin onboarding, 2026-05-07) surfaced seven concrete bugs in the workspace onboarding flow. The two prompts (`onboarding_prompt.md`, `workspace_onboarding_prompt.md`) duplicate memory-template content and have drifted from each other.

## Bugs Being Fixed

From the audit:

1. Root subagent forgot `project_id` → memories landed at workspace scope and were partially overwritten by synthesis.
2. Root content (`dev.sh`, `docker-compose.yml`, `scripts/`, activation per project, generic navigation) is not captured in any memory.
3. CLAUDE.md gaps detected at end of onboarding but never applied — assistant ended on an unanswered question.
4. Per-project subagent coverage was inconsistent (ktor 6, python-services 3, eduplanner-mcp 2).
5. HARD-GATE only verified `project-overview` per project — eduplanner-mcp passed with two memories.
6. Stale `mcp-server` and `mcp-server-deprecated` workspace projects retained from a rename were never proposed for cleanup.
7. Workspace synthesis tried to read `## Phase 0: Embedding Model Selection` from the single-project prompt; the heading existed but the cross-prompt reference is fragile.

## Decisions

| Decision | Resolution |
|---|---|
| Where root content lives | Workspace-level memories. No `root` codescout project. |
| Where activation/entry-points live | Enriched workspace `architecture` + `development-commands`. |
| Per-project memory set | 6 mandatory: `project-overview`, `architecture`, `conventions`, `development-commands`, `domain-glossary`, `gotchas`. |
| Verification | Parent reads back all 6 topics per project, retries up to 2 times, aborts if still missing. |
| CLAUDE.md update | Parent computes diff, asks once, applies on `y`. |
| Stale project handling | Detect orphans, ask once, auto-delete on `y`. |
| Refactor scope | Both prompts; share memory templates via include. |

## File Layout

**New:** `src/prompts/memory-templates.md` — single source of truth for the 7 memory definitions. ~250 lines. One `### project-scope: <topic>` and one `### workspace-scope: <topic>` section per topic. Each ends with a YAML `manifest` tail listing `required_subsections` (used for static tests, not runtime parsing).

**Modified:**

- `src/prompts/source.md` — drops inline templates (lines 232–490), replaces with `{{include: memory-templates.md}}`. Final ~250 lines (down from 579).
- `src/prompts/workspace_onboarding_prompt.md` — same include. Reorganized into 6 numbered phases. Final ~200 lines (up from 152).
- `src/prompts/mod.rs` — adds `load_prompt()` helper that performs string substitution at compile time (`include_str!`).

**Empty-stub convention:** for `domain-glossary` / `gotchas` with nothing to say, subagents write a canonical stub:
```
# {project_id} {Topic}

_No project-specific {topic}. See workspace {topic} memory._
```
Verification logic treats stub-equal content as a successful write, not a missing one. This makes the "6 mandatory" rule produce predictable empty files instead of subagents silently skipping.

## Workspace Flow — 6 Phases

```
1. Workspace Survey                  (breadth-first; write Survey doc)
2. Stale-Project Cleanup             (NEW; detect orphans, single ask)
3. Per-Project Deep Dives            (parallel subagents, 6 memories each)
4. Coverage Verification             (NEW; read-back loop, retry)
5. Workspace Synthesis               (5 workspace memories from per-project)
6. CLAUDE.md Refresh                 (NEW; propose diff, single ask)
```

Each phase ends with a `<HARD-GATE>` the parent must satisfy before moving on. Phase 2 runs only when `onboarding(force: true)`. Phase 6 runs in both single-project and workspace flows.

### Phase 1 — Workspace Survey

Unchanged in shape. Output: a "Workspace Exploration Summary" passed into Phase 3 subagent prompts. Gate: summary written.

### Phase 2 — Stale-Project Cleanup

Parent lists `.codescout/projects/*/`, intersects with live projects from Phase 1. For orphans, parent reads the most recent mtime under `<project>/memories/` and prints:

```
Found N orphaned codescout projects (no longer in workspace):
  - <id>   last touched <date>
  - <id>   last touched <date>

Delete all? [y/N]
```

On `y`: `memory(action: delete, project_id, topic)` for every memory under each orphan, then remove the empty `<project>/memories/` directory. On `N`: log `cleanup: skipped (N orphans retained)` to the final summary, never re-ask. Gate: answered or skipped.

### Phase 3 — Per-Project Deep Dives

Subagent prompt template requires **6 memories** and:

- Empty-stub for `domain-glossary` / `gotchas` if no project-specific content (do NOT skip).
- End final response with `MANIFEST: <comma-separated topic list>`. Topics MUST exactly match the 6 required.

Gate: all subagents returned.

### Phase 4 — Coverage Verification

Parent runs `memory(action: read, project_id, topic)` for each `(project, topic)` pair → 6N reads. Builds a coverage matrix:

```
                project-overview architecture conventions dev-cmds glossary gotchas
ktor-server     ✓                ✓           ✓           ✓        ✓        ✓
python-services ✓                ✓           ✓           ✓        EMPTY    EMPTY
eduplanner-mcp  MISSING          ✓           ✓           MISSING  MISSING  MISSING
```

States: `✓` (non-empty content longer than the stub), `EMPTY` (byte-equal to canonical stub — counts as ✓), `MISSING` (absent or content shorter than stub).

Retry loop:

```
attempt = 0
while attempt < 2 and matrix has MISSING:
    failed = projects with at least one MISSING cell
    re-dispatch one subagent per failed project with prompt:
        "Re-onboarding: {project_id}.
         Previous run was incomplete. You wrote: {present_topics}.
         Missing: {missing_topics}.
         Constraints:
           - Do NOT re-read sibling projects.
           - Do NOT rewrite topics already present.
           - For empty-stub eligible topics, write the stub exactly:
             {empty_stub_text}
         End with: MANIFEST: {missing_topics}"
    wait for all
    re-read missing cells
    attempt += 1

if matrix still has MISSING:
    abort, print failure summary
    skip Phase 5 and Phase 6
```

Subagent's `MANIFEST:` line is **advisory only** — used for retry-prompt context, never trusted as a gate. Source of truth is the read-back.

### Phase 5 — Workspace Synthesis

Parent reads per-project `architecture` + `conventions` (and others as needed for cross-project topics), then writes 5 workspace memories from the workspace-scope sections of `memory-templates.md`:

- `architecture` — Project Map, Cross-Project Deps, Shared Infrastructure, Top-Level Code Map, Generic Navigation.
- `conventions` — shared + per-project pointers.
- `development-commands` — cross-project orchestration.
- `domain-glossary` — terms used across multiple projects.
- `gotchas` — cross-project pitfalls.

Workspace `architecture` is the load-bearing change. Required subsections:

```
## Project Map
For each project: <id>/ — <one-line purpose> · entry-point: <file>:<symbol> · activate: <command>

## Cross-Project Dependencies
<a> → <b> (<what is shared>)

## Shared Infrastructure
[CI workflows, deployment, shared tooling]

## Top-Level Code Map
[Root-layer scripts, docker-compose files, env templates, generated artifacts.
 List each with a one-line purpose. This is the "what does dev.sh do" section.]

## Generic Navigation
[Where to look first for: build/test/lint, common bugs, domain modeling,
 cross-project flows. 5–8 bullets, each pointing to a memory or file.]
```

Then write `system-prompt.md`. Gate: 5 memories written, read-back verified.

### Phase 6 — CLAUDE.md Refresh

Parent computes the canonical memory table from what was actually written this run. Each row's "What's inside" is the first `## H2` of the memory body — deterministic, parent does not invent prose.

Parent reads existing CLAUDE.md, locates `## codescout Memories` (allowed to fail; if missing, parent proposes adding the section), generates a unified diff, prints:

```
Proposed CLAUDE.md memory-table update:

@@ ## codescout Memories @@
- | `memory("architecture")` | <old> |
+ | `memory("architecture")` | <new> |
+ | `memory(project_id="ktor-server", topic="development-commands")` | <new row> |
  ... (N lines total)

Apply? [y/N]
```

On `y`: `edit_markdown(path: "CLAUDE.md", action: replace, heading: "## codescout Memories", content: <new table>)`. On `N` or no answer: log `claude_md: skipped (user declined)` to the final summary. No follow-up question.

## Single-Project Flow

Trimmed to ~250 lines. Final shape:

```
## THE IRON LAW                          (kept verbatim)
## Phase 0: Embedding Model Selection    (kept; STABLE-HEADING comment added)
## Phase 1: Semantic Index Check         (kept)
## Phase 2: Explore the Code             (kept; gate checklist tightened)
## Red Flags                             (kept)
## Memories to Create                    (REPLACED with include directive)
## Coverage Verification                 (NEW; single-project equivalent of Phase 4)
## After Everything Is Created           (kept)
## Refresh CLAUDE.md                     (REWRITTEN to match Phase 6 diff/ask flow)
## Gathered Project Data                 (kept)
## Optional: Private Memories            (kept)
## Optional: Semantic Memories           (kept)
```

Three concrete fixes baked in:

1. STABLE-HEADING comment on Phase 0 to prevent the cross-prompt heading drift seen in this session.
2. Project-scope memories use the same 6-memory set as workspace per-project subagents (single source of truth in `memory-templates.md`).
3. CLAUDE.md flow unified with Phase 6.

Single-project does NOT get: stale-project cleanup (no `.codescout/projects/` dir), workspace synthesis, or per-project subagents.

## Final Summary Format

Always printed at end of onboarding (both flows):

```
Onboarding complete.

  Per-project memories
    ktor-server          6/6 ✓
    python-services      6/6 ✓
    eduplanner-mcp       6/6 ✓ (1 retry)

  Workspace memories     5/5 ✓

  Cleanup                2 orphans deleted
  CLAUDE.md              updated
  System prompt          v23 → v24
```

Skipped/failed phases show `✗ <reason>` instead of `✓`. Single block, no chatter.

## Rust Loader

```rust
const INCLUDE_MARKER: &str = "{{include: memory-templates.md}}";

const RAW_ONBOARDING: &str = include_str!("onboarding_prompt.md");
const RAW_WORKSPACE:  &str = include_str!("workspace_onboarding_prompt.md");
const TEMPLATES:      &str = include_str!("memory-templates.md");

fn load_prompt(name: &str) -> String {
    let raw = match name {
        "onboarding_prompt.md" => RAW_ONBOARDING,
        "workspace_onboarding_prompt.md" => RAW_WORKSPACE,
        _ => panic!("unknown prompt: {}", name),
    };
    raw.replace(INCLUDE_MARKER, TEMPLATES)
}
```

`include_str!` keeps everything compiled into the binary. No runtime IO, no template engine, no new dependency. Existing `prompt_surfaces_reference_only_real_tools` test runs against the post-substitution string.

## Tests

Static tests in `src/prompts/mod.rs`:

```rust
#[test]
fn templates_include_resolves_in_both_prompts() { ... }

#[test]
fn workspace_phase_0_reference_resolves() {
    let single = load_prompt("onboarding_prompt.md");
    let workspace = load_prompt("workspace_onboarding_prompt.md");
    let referenced = "## Phase 0: Embedding Model Selection";
    if workspace.contains(referenced) {
        assert!(single.contains(referenced),
            "workspace prompt references heading missing from single-project prompt");
    }
}

#[test]
fn memory_templates_have_manifest_tail() { ... }

#[test]
fn empty_stub_pattern_is_canonical() { ... }
```

No subagent-compliance integration tests — the runtime read-back loop is the test.

## ONBOARDING_VERSION Bump

This refactor changes the stored per-project system prompt format (new memory shape, new Phase 6 step). Per `CLAUDE.md § Onboarding Version`, bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs`. Triggers regeneration on next `onboarding()` call for every project.

## Tracker Hooks

After the refactor lands, append entries to:

- `docs/trackers/skill-frictions.md` — two entries marking the audited bugs as fixed.
- `docs/trackers/tool-usage-patterns.md` — verdict-resolved observation noting the workspace-onboarding over-reporting pattern.

## Out of Scope

- Real templating engine (`tinytemplate`, `minijinja`). String substitution is sufficient.
- `root` as a first-class codescout project. Considered and rejected — root is a holder, not a runnable artifact.
- Hybrid detection of root-layer code. Workspace memories absorb root content unconditionally.
- Subagent JSON manifests as the verification mechanism. Read-back is stricter and the user picked it.

## References

- Session audit: `~/.claude/projects/-home-marius-work-mirela-backend-kotlin/80d79d25-4d97-4296-8409-428e92117e4c.jsonl`
- Subagent transcripts: `/tmp/claude-1000/-home-marius-work-mirela-backend-kotlin/80d79d25.../tasks/`
- Current prompts: `src/prompts/source.md`, `src/prompts/workspace_onboarding_prompt.md`
- Surface consistency rules: `CLAUDE.md § Prompt Surface Consistency`

> **Status (2026-05-08): paths retargeted post-I-01.** Drafted 2026-05-07. The next day I-01 (commits `7db51a5`..`f047d47`) consolidated `src/prompts/source.md` and `src/prompts/source.md` into `src/prompts/source.md` with `<!-- @surface NAME -->` blocks; build.rs slices them into `OUT_DIR/{surface}.md` at compile time. Path references below now point at the new location. The 7 bugs being fixed and all design decisions remain valid. The companion implementation plan (`docs/superpowers/plans/2026-05-07-onboarding-refactor.md`) carries notes on substitution-mechanism and test-plumbing re-derivation.

---
