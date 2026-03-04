# Rename Story & Changelog Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `CHANGELOG.md` at the repo root and a `docs/manual/src/history.md` page that tell the story of the `code-explorer → codescout` rename for both existing migrators and new readers.

**Architecture:** Option C — manual-first. `CHANGELOG.md` is lean and links to depth. `docs/manual/src/history.md` carries the full narrative: TL;DR, the "why", what the tool became, a migration table, and a brief note on tool API changes.

**Tech Stack:** Markdown only. No code changes.

---

### Task 1: Write `CHANGELOG.md`

**Files:**
- Create: `CHANGELOG.md`

**Step 1: Create the file**

```markdown
# Changelog

## [0.2.0] — codescout

> **TL;DR:** The project was renamed from `code-explorer` to `codescout`. If you're
> migrating, update your MCP config and any scripts that reference the old binary name.
> [Full story and migration guide →](docs/manual/src/history.md)

### Breaking changes

- **Binary renamed:** `code-explorer` → `codescout`
- **MCP server ID renamed** — update `.mcp.json` or Claude Code settings accordingly
- **9 tools renamed** for naming consistency (`get_symbols_overview` → `list_symbols`, `execute_shell_command` → `run_command`, and others)
- **3 tools consolidated** — `insert_before_symbol` + `insert_after_symbol` merged into `insert_code(position)`, `is_onboarded` folded into `onboarding(force)`
```

**Step 2: Verify it renders cleanly**

Open preview or run: `cat CHANGELOG.md`  
Expected: clean markdown, TLDR block quote visible, link to history.md present.

**Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: add CHANGELOG.md with codescout rename entry"
```

---

### Task 2: Write `docs/manual/src/history.md`

**Files:**
- Create: `docs/manual/src/history.md`

**Step 1: Create the file**

The tone is direct and personal — a colleague explaining what happened, not a PR description. Write:

```markdown
# From code-explorer to codescout

> **TL;DR**
> - The project was renamed. The binary is now `codescout`, not `code-explorer`.
> - Update your MCP config: change the server key from `code-explorer` to `codescout`.
> - Update any scripts or aliases that call the old binary name.
> - 9 tools were renamed and 3 consolidated — see [Tools Overview](../tools/overview.md) for current names.

---

## The name

The original name was `code-explorer`. It made sense at the time — the tool helped an AI navigate
a codebase the way a developer would explore it in an IDE.

Two things changed.

First, the practical one: `code-explorer` was already taken on [crates.io](https://crates.io). A
Rust binary needs a crate name, and that one wasn't available.

Second, the honest one: the name had stopped fitting. By the time the rename happened, the tool
had grown persistent memory that survives across sessions, semantic search over embeddings, a shell
integration with output buffering, a project dashboard, and LSP-backed navigation across 9
languages. It wasn't just exploring files anymore. It was orienting an AI inside a codebase —
tracking context, surfacing what matters, remembering what was learned.

*Scout* felt closer to that. A scout doesn't just wander. It goes ahead, maps the terrain, and
comes back with something useful.

## What it grew into

The project started as file navigation. You could list symbols, search for patterns, read a
function body without dumping the whole file into context.

Then it got LSP: real go-to-definition, hover types, find-all-references — the same signals a
developer gets from their IDE, available to the AI.

Then semantic search: find code by concept, not just by text match. Then persistent memory: notes
that survive between sessions. Then shell integration with output buffers, so large command output
doesn't blow the context window. Then a dashboard for project health.

Each addition was driven by a recurring friction — the AI doing something clumsy that a better
tool could prevent. The scope kept expanding because the problem kept expanding.

## Migrating from code-explorer

If you were running `code-explorer` before, here's everything that changed at the surface:

| What | Before | After |
|---|---|---|
| Binary name | `code-explorer` | `codescout` |
| MCP server key (`.mcp.json`) | `"code-explorer"` | `"codescout"` |
| Claude Code settings key | `"code-explorer"` | `"codescout"` |
| Cargo crate | `code-explorer` | `codescout` |

Update your `.mcp.json` (or Claude Code's `~/.claude/settings.json`) to use `"codescout"` as the
server key. The tool list and behavior are unchanged — it's a rename, not a rewrite.

## What else changed

Alongside the rename, the tool API was tidied up:

- **9 tools renamed** for consistency — plural `list_*` for enumeration, `find_*` for search,
  `search_*` for text/semantic. Full mapping in the [Tools Overview](../tools/overview.md).
- **3 tools consolidated** — `insert_before_symbol` and `insert_after_symbol` merged into
  `insert_code(position: "before"|"after")`. `is_onboarded` folded into `onboarding(force: true)`.
```

**Step 2: Read it back and check the voice**

Read the file aloud (or skim carefully). Ask:
- Does "scout" feel earned by the end of "The name" section?
- Does "What it grew into" feel like a story arc, not a feature list?
- Is the migration table immediately actionable?

Adjust any sentences that feel mechanical.

**Step 3: Commit**

```bash
git add docs/manual/src/history.md
git commit -m "docs: add history.md — rename story and migration guide"
```

---

### Task 3: Wire `history.md` into the manual's table of contents

**Files:**
- Modify: `docs/manual/src/SUMMARY.md`

**Step 1: Add the entry**

In `SUMMARY.md`, add a top-level entry after `[Introduction](introduction.md)` and before `# User Guide`:

```markdown
[Introduction](introduction.md)
[From code-explorer to codescout](history.md)

# User Guide
```

This makes it visible at the top of the sidebar for existing users who open the manual looking for "what changed".

**Step 2: Verify the manual builds (if mdBook is available)**

```bash
cd docs/manual && mdbook build 2>&1 | head -20
```

Expected: no errors about missing files. If mdbook is not installed, skip — the link structure is correct by inspection.

**Step 3: Commit**

```bash
git add docs/manual/src/SUMMARY.md
git commit -m "docs: add history.md to manual table of contents"
```

---

### Task 4: Final review pass

**Step 1: Read the full chain**

Read `CHANGELOG.md` → follow the link to `history.md`. Simulate both audiences:
- **Migrator**: does TL;DR give them everything they need in 30 seconds?
- **New reader**: does the "The name" section feel natural, not defensive?

**Step 2: Check cross-references**

- `history.md` links to `../tools/overview.md` — confirm that path resolves correctly relative to `docs/manual/src/`.
- `CHANGELOG.md` links to `docs/manual/src/history.md` — confirm correct from repo root.

**Step 3: Final commit (if any tweaks)**

```bash
git add -p  # stage only what changed
git commit -m "docs: polish rename story after review pass"
```
