# Graduate Remaining Experimental Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Graduate the 9 remaining experimental features (8 librarian + 1 server diagnostic) from `experimental/` to permanent locations in the manual.

**Architecture:** Uniform per-file pattern (`git mv` + callout removal + content edit if needed) followed by SUMMARY.md and experimental/index.md updates. Single commit. The `librarian-embedded.md` gets one extra content fix because the librarian default flipped to enabled this session.

**Tech Stack:** mdBook markdown, git

---

## Task 1: Move 9 files, remove callouts, fix librarian-embedded content

**Files:**
- Move: 9 files from `docs/manual/src/experimental/` to `docs/manual/src/concepts/`
- Modify: `concepts/librarian-embedded.md` (after move) — content fix

- [ ] **Step 1: Move all 9 files**

```bash
git mv docs/manual/src/experimental/artifact-move.md docs/manual/src/concepts/artifact-move.md
git mv docs/manual/src/experimental/librarian-guide-resource.md docs/manual/src/concepts/librarian-guide-resource.md
git mv docs/manual/src/experimental/librarian-tools-collapse.md docs/manual/src/concepts/librarian-tools-collapse.md
git mv docs/manual/src/experimental/librarian-embedded.md docs/manual/src/concepts/librarian-embedded.md
git mv docs/manual/src/experimental/workspace-state-at.md docs/manual/src/concepts/workspace-state-at.md
git mv docs/manual/src/experimental/augmentation-render-template.md docs/manual/src/concepts/augmentation-render-template.md
git mv docs/manual/src/experimental/tracker-design.md docs/manual/src/concepts/tracker-design.md
git mv docs/manual/src/experimental/artifact-refresh-stale.md docs/manual/src/concepts/artifact-refresh-stale.md
git mv docs/manual/src/experimental/heartbeat-memory-fields.md docs/manual/src/concepts/heartbeat-memory-fields.md
```

Expected: 9 files moved, no errors.

- [ ] **Step 2: Remove the ⚠ callout from each moved file**

For every one of the 9 files, the file currently begins with:

```
> ⚠ Experimental — may change without notice.

# <Title>
```

Remove the first two lines (the callout line and the blank line after it). After removal the file should start at the `# <Title>` line.

For each file in `docs/manual/src/concepts/`:
- `artifact-move.md`
- `librarian-guide-resource.md`
- `librarian-tools-collapse.md`
- `librarian-embedded.md`
- `workspace-state-at.md`
- `augmentation-render-template.md`
- `tracker-design.md`
- `artifact-refresh-stale.md`
- `heartbeat-memory-fields.md`

Use mcp__codescout__edit_file with the exact pair:

```
old_string: "> ⚠ Experimental — may change without notice.\n\n"
new_string: ""
```

(If a file's exact callout text differs slightly, read the file first to capture the literal opening.)

- [ ] **Step 3: Verify no ⚠ callouts remain**

```bash
grep -l "⚠ Experimental" docs/manual/src/concepts/{artifact-move,librarian-guide-resource,librarian-tools-collapse,librarian-embedded,workspace-state-at,augmentation-render-template,tracker-design,artifact-refresh-stale,heartbeat-memory-fields}.md 2>/dev/null
```

Expected: no output.

- [ ] **Step 4: Read librarian-embedded.md to find stale opt-in wording**

Use mcp__codescout__read_markdown on `docs/manual/src/concepts/librarian-embedded.md` to locate any text describing how to enable the librarian. The current text describes opt-in via `LIBRARIAN_ENABLED=1` (or `[librarian] enabled = true`). It needs to flip to opt-out semantics.

- [ ] **Step 5: Fix the librarian-embedded.md content**

Replace any wording that says the feature is **disabled by default and requires opt-in** with text that says it is **enabled by default and can be opted out**.

Specifically:
- "disabled by default" → "enabled by default"
- "opt in via `LIBRARIAN_ENABLED=1`" → "opt out via `LIBRARIAN_ENABLED=0`"
- "set `LIBRARIAN_ENABLED=1`" → "set `LIBRARIAN_ENABLED=0`"
- "`[librarian] enabled = true` in `.codescout/project.toml`" → "`[librarian] enabled = false` in `.codescout/project.toml`" (when used as the opt-in example)

Make the prose read coherently — the section should describe a layered priority:

> The librarian is enabled by default. To opt out, set `LIBRARIAN_ENABLED=0` in the environment, or add `[librarian]\nenabled = false` to `.codescout/project.toml`. The env var overrides the project file.

If the file has any sentences claiming the feature is "experimental" or "opt-in only" that are no longer accurate, remove or rewrite them.

Use mcp__codescout__edit_markdown or mcp__codescout__edit_file as needed. Do not rewrite sections beyond the opt-in/opt-out wording.

- [ ] **Step 6: Verify the content fix**

```bash
grep -n "LIBRARIAN_ENABLED\|enabled by default\|opt out\|opt in" docs/manual/src/concepts/librarian-embedded.md
```

Expected: lines describing opt-out semantics (`LIBRARIAN_ENABLED=0`), no lines describing opt-in via `LIBRARIAN_ENABLED=1` as the way to enable the feature.

---

## Task 2: Update SUMMARY.md, clean experimental/index.md, commit

**Files:**
- Modify: `docs/manual/src/SUMMARY.md`
- Modify: `docs/manual/src/experimental/index.md`

- [ ] **Step 1: Update SUMMARY.md — librarian-mcp section**

In `docs/manual/src/SUMMARY.md`, find the line:

```
- [librarian-mcp](concepts/librarian-mcp.md)
```

Replace with:

```
- [librarian-mcp](concepts/librarian-mcp.md)
  - [Librarian Embedded](concepts/librarian-embedded.md)
  - [Librarian Tools Collapse (16 → 5)](concepts/librarian-tools-collapse.md)
  - [doc://librarian-guide Resource](concepts/librarian-guide-resource.md)
  - [artifact_refresh (list_stale)](concepts/artifact-refresh-stale.md)
  - [artifact (action="move")](concepts/artifact-move.md)
  - [tracker_design](concepts/tracker-design.md)
  - [workspace_state_at](concepts/workspace-state-at.md)
  - [Augmentation: Templates & Schemas](concepts/augmentation-render-template.md)
```

Use mcp__codescout__edit_file with that exact old_string / new_string.

- [ ] **Step 2: Update SUMMARY.md — Heartbeat under Debug Mode**

In `docs/manual/src/SUMMARY.md`, find the line:

```
- [Debug Mode](concepts/diagnostic-logging.md)
```

Replace with:

```
- [Debug Mode](concepts/diagnostic-logging.md)
  - [Heartbeat Memory Fields](concepts/heartbeat-memory-fields.md)
```

- [ ] **Step 3: Verify SUMMARY.md changes**

```bash
grep -c "concepts/\(artifact-move\|librarian-guide-resource\|librarian-tools-collapse\|librarian-embedded\|workspace-state-at\|augmentation-render-template\|tracker-design\|artifact-refresh-stale\|heartbeat-memory-fields\)" docs/manual/src/SUMMARY.md
```

Expected: `9`

- [ ] **Step 4: Read current experimental/index.md to find the 9 entries to remove**

Use mcp__codescout__read_markdown on `docs/manual/src/experimental/index.md` to capture the exact current list of feature entries.

- [ ] **Step 5: Replace the Available Features list with a placeholder**

The current `## Available Features` section has 9 bullet entries (one per remaining experimental feature). Replace the entire section content (between `## Available Features` and the next heading or end of file) with:

```
*No features currently in experimental.*
```

Keep the `## Available Features` heading and all surrounding text (intro, callout, etc.) intact.

Use mcp__codescout__edit_markdown with `action="replace"` and `heading="## Available Features"` if it works, or mcp__codescout__edit_file with old_string covering the 9 bullets and new_string being the placeholder line.

- [ ] **Step 6: Verify experimental/index.md cleanup**

```bash
grep "artifact-move\|librarian-guide-resource\|librarian-tools-collapse\|librarian-embedded\|workspace-state-at\|augmentation-render-template\|tracker-design\|artifact-refresh-stale\|heartbeat-memory-fields" docs/manual/src/experimental/index.md
```

Expected: no output.

```bash
grep "No features currently" docs/manual/src/experimental/index.md
```

Expected: matching line.

- [ ] **Step 7: Final verification — graduated files**

```bash
grep -l "⚠ Experimental" docs/manual/src/concepts/{artifact-move,librarian-guide-resource,librarian-tools-collapse,librarian-embedded,workspace-state-at,augmentation-render-template,tracker-design,artifact-refresh-stale,heartbeat-memory-fields}.md 2>/dev/null
```

Expected: no output.

```bash
ls docs/manual/src/concepts/{artifact-move,librarian-guide-resource,librarian-tools-collapse,librarian-embedded,workspace-state-at,augmentation-render-template,tracker-design,artifact-refresh-stale,heartbeat-memory-fields}.md
```

Expected: 9 files listed.

```bash
ls docs/manual/src/experimental/{artifact-move,librarian-guide-resource,librarian-tools-collapse,librarian-embedded,workspace-state-at,augmentation-render-template,tracker-design,artifact-refresh-stale,heartbeat-memory-fields}.md 2>&1 | head -3
```

Expected: "No such file or directory" for all (proves the moves stuck).

- [ ] **Step 8: Commit**

```bash
git add docs/manual/src/
git commit -m "docs: graduate 9 remaining experimental features"
```

Expected: commit created with all 9 file renames + SUMMARY.md + experimental/index.md changes.

---

## Summary

After both tasks complete, all 9 remaining experimental features will live in `concepts/`, the SUMMARY.md will reference them under their proper parent sections, and `experimental/index.md` will show the placeholder note. One commit on `experiments`. No code changes, no tests to run beyond the existing suite.
