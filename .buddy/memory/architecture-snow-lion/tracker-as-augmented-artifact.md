---
specialist: architecture-snow-lion
scope: project
slug: tracker-as-augmented-artifact
created: 2026-05-07
updated: 2026-05-07
tags: [librarian, augmentation, doc-architecture, state-management, novel-pattern]
---

**Lesson:** Some markdown files in this project are not just documents — they are **augmented artifacts** managed by `librarian-mcp`, with persistent prompts and structured params that auto-refresh the body via gather. `docs/trackers/tool-usage-patterns.md` is the canonical example: the live params table is rendered at the top by the librarian, prose analysis lives in the body, and the persistent prompt instructs how to keep them in sync. This decouples *live state* from *prose body* in a way I had not seen before reviewing this project.

**Why:** It surprised me on first review. Treating a markdown doc as a stateful artifact with a prompt+params surface area is a genuine architectural pattern, not a documentation convention. It implies that some "docs" are actually data with a rendering layer, and that editing them through `read_markdown` + `edit_markdown` directly bypasses the augmentation contract. The CLAUDE.md mandate ("Tool Usage Patterns is a librarian artifact") names this explicitly.

**How to apply:** When reviewing or recommending changes to any tracker, plan, ADR, or design doc in this project, the first question is: is this an **augmented artifact**? Check for `mcp__codescout__artifact(action="get", id=...)` references in CLAUDE.md or the doc's preamble. If it is augmented:
- Edits to the **body** can use `edit_markdown` directly only for the prose sections.
- Edits to **structured live state** must go through `artifact_augment(merge=true, params={...})` or `artifact(action="update", commit_refresh=true)`.
- Schema changes to params should also update the persistent prompt — the prompt and params are paired contracts, not independent.

If a doc is *not* augmented, normal markdown edits apply. Recommending augmentation for a new doc is a real architectural choice — augmented makes sense for state that drifts, not for one-shot specs.
