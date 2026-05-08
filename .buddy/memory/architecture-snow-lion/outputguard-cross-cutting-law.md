---
specialist: architecture-snow-lion
scope: project
slug: outputguard-cross-cutting-law
created: 2026-05-07
updated: 2026-05-07
tags: [output, progressive-disclosure, invariant, tool-design]
---

**Lesson:** `OutputGuard` (`src/tools/output.rs`) is this project's single answer to progressive disclosure. It is not a per-tool helper; it is a cross-cutting architectural law. Every tool that returns variable-length output flows through `cap_items` or `cap_files`, emits the standard `OverflowInfo` shape with `by_file` distribution map and actionable hint, and respects `Exploring` (capped 200) vs `Focused` (offset/limit pagination) modes.

**Why:** Token efficiency is treated as an architectural invariant on this project, not a UX preference. The OutputGuard contract is what makes tools composable — agents learn one overflow grammar and apply it everywhere. Bypassing OutputGuard in a single tool fractures the agent's mental model and erodes trust in the rest of the toolkit by association. The `OutputBuffer` `@ref` system extends the same idea to large outputs.

**How to apply:** When reviewing any new tool or modification that changes a tool's output shape, the first structural question is: does this respect OutputGuard? If the tool emits `Vec<T>`, it should call `cap_items`. If it emits a directory listing, `cap_files`. If overflow happens, the response must include an actionable narrowing hint (`narrow with path=...`, `re-run with detail_level="full"`, etc.). Read `docs/PROGRESSIVE_DISCOVERABILITY.md` before opining on tool design — it codifies the patterns and anti-patterns. A tool that bypasses OutputGuard is an architectural defect, not a style preference.
