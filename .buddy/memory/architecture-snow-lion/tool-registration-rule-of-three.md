---
specialist: architecture-snow-lion
scope: project
slug: tool-registration-rule-of-three
created: 2026-05-07
updated: 2026-05-07
tags: [registration, premature-abstraction, rule-of-three, project-philosophy]
---

**Lesson:** This project favors flat collections over registry abstractions until duplication earns the extraction. Tools register as `Vec<Arc<dyn Tool>>` in `src/server.rs::CodeScoutServer::from_parts`; conditional inclusion uses inline `#[cfg(...)]` blocks rather than a feature-aware registry trait. The pattern is intentional and reflects a project-level discipline: do not abstract on a sample of one or two.

**Why:** I considered proposing a `ToolRegistry` trait on first review. The Refactoring Yak applied rule-of-three (Heuristic 6) and the "name the structural defect" rule (Heuristic 1). With one or two conditionals, the abstraction's shape would be a guess; with three, the duplication itself dictates the shape. Premature extraction would freeze an interface around an unrepresentative sample and force later inversions.

**How to apply:** When reviewing any registration or collection pattern in this codebase — tools, hooks, resources, languages, embedders — do NOT propose a registry abstraction unless three or more cfg-gated, conditional, or otherwise diverging entries already exist. With two, the duplication is debatable; with three, propose the abstraction *and* let the existing duplication dictate its shape. This applies beyond tools: language LSP configs in `src/lsp/servers/`, MCP resource builders in `src/mcp_resources/`, and similar collections all live under the same discipline.
