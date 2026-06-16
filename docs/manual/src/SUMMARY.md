# Summary

[Introduction](introduction.md)
[From code-explorer to codescout](history.md)

# User Guide

- [Why codescout?](why-codescout.md)

- [Installation](getting-started/installation.md)
  - [Your First Project](getting-started/first-project.md)
  - [codescout-companion Plugin](getting-started/routing-plugin.md)

- [Agent Integrations](agents/overview.md)
  - [Claude Code](agents/claude-code.md)
  - [GitHub Copilot](agents/copilot.md)
  - [Cursor](agents/cursor.md)

- [Progressive Disclosure](concepts/progressive-disclosure.md)
  - [Output Modes](concepts/output-modes.md)
  - [Tool Selection](concepts/tool-selection.md)

- [Shell Integration](concepts/shell-integration.md)
  - [Output Buffers](concepts/output-buffers.md)
  - [Interactive Sessions](concepts/elicitation-interactive-sessions.md)

- [Semantic Search](concepts/semantic-search.md)
  - [Lite Stack (daemon-free, default)](concepts/lite-stack.md)
  - [Retrieval Stack (server, opt-in)](concepts/retrieval-stack.md)
  - [Setup Guide](semantic-search-guide.md)
  - [Asymmetric Query Prefix](concepts/asymmetric-query-prefix.md)
  - [Metadata-Enriched Chunks](concepts/metadata-enriched-chunks.md)
  - [Index Scope Guard](concepts/index-scope-guard.md)
  - [Auto-Reindex on Edit](concepts/auto-reindex-on-edit.md)
  - [Hybrid Dense + Sparse Retrieval](concepts/hybrid-bm25-vector.md)
  - [SPLADE on ROCm](concepts/sparse-amd.md)

- [Library Navigation](concepts/library-navigation.md)
  - [Auto-Registration](concepts/multi-ecosystem-auto-registration.md)
- [Multi-Project Workspaces](concepts/multi-project-workspace.md)
  - [workspace Output](concepts/activate-project-output-optimization.md)
  - [Project Hints](concepts/project-hints.md)

- [MCP Resources](concepts/mcp-resources.md)
  - [Tool Description Diet](concepts/tool-description-diet.md)
  - [Tool Usage Doctor](concepts/tool-usage-doctor.md)

- [Librarian](concepts/librarian-embedded.md)
  - [Librarian Tools Collapse (16 → 5)](concepts/librarian-tools-collapse.md)
  - [doc://librarian-guide Resource](concepts/librarian-guide-resource.md)
  - [artifact_refresh (list_stale)](concepts/artifact-refresh-stale.md)
  - [artifact (action="move")](concepts/artifact-move.md)
  - [Audit Doc Refs](concepts/audit-doc-refs.md)
  - [Goal-Tracker Archetype](concepts/goal-tracker.md)
  - [Artifact CLI](concepts/artifact-cli.md)
  - [tracker_design](concepts/tracker-design.md)
  - [workspace_state_at](concepts/workspace-state-at.md)
  - [Augmentation: Templates & Schemas](concepts/augmentation-render-template.md)
- [LSP Idle TTL](concepts/lsp-idle-ttl.md)

- [Memory](concepts/memory.md)
  - [After Onboarding](concepts/after-onboarding.md)
  - [Sections Filter](concepts/memory-sections-filter.md)

- [Dashboard](concepts/dashboard.md)
  - [LSP Startup Statistics](concepts/lsp-startup-stats.md)
- [Git Worktrees](concepts/worktrees.md)

- [Security & Permissions](concepts/security.md)
  - [Security Profiles](concepts/security-profiles.md)
  - [Compact Schemas & `workspace` Safety](concepts/compact-schemas-and-activate-project-safety.md)
  - [PostCompact Cache Flush](concepts/post-compact-cache-flush.md)
  - [Cross-Process Write Serialization](concepts/cross-process-write-serialization.md)

- [Routing Plugin](concepts/routing-plugin.md)
  - [Superpowers Workflow](concepts/superpowers.md)

- [Project Configuration](configuration/project-toml.md)
  - [Global Config](configuration/global-config.md)
  - [Embedding Backends](configuration/embedding-backends.md)
  - [EDR-Constrained Windows](configuration/embeddings-edr-windows.md)
  - [Embeddings](configuration/embeddings.md)
    - [Model Comparison](configuration/embedding-model-comparison.md)

- [Language Support](language-support.md)
  - [Bash](concepts/bash-language-support.md)
  - [Kotlin LSP Multiplexer](concepts/kotlin-lsp-multiplexer.md)
  - [Rust LSP Multiplexer](concepts/rust-lsp-multiplexer.md)

# Tool Reference

- [Tools Overview](tools/overview.md)
  - [API Naming Reference](tools/api-redesign.md)
  - [Symbol Navigation](tools/symbol-navigation.md)
    - [Progressive Directory Overview](tools/list-symbols-progressive.md)
    - [Call Graph](tools/call-graph.md)
  - [File Operations](tools/file-operations.md)
    - [grep: Literal Fallback](tools/search-pattern-literal-fallback.md)
  - [Editing](tools/editing.md)
    - [edit_code](tools/edit-code.md)
    - [Structural Edit Gate](tools/edit-file-structural-gate.md)
    - [Document Section Editing](tools/document-section-editing.md)
    - [Markdown Tools](tools/markdown-tools.md)
    - [read_markdown](tools/read-markdown.md)
  - [Semantic Search](tools/semantic-search.md)
    - [File-Diversity Re-Rank](tools/semantic-search-diversity.md)
  - [Library Navigation](tools/library-navigation.md)
  - [Git](tools/git.md)
  - [AST Analysis](tools/ast.md)
  - [Memory](tools/memory.md)
  - [`get_guide`](tools/get-guide.md)
  - [Workflow & Config](tools/workflow-and-config.md)
    - [Read-Only `workspace`](tools/activate-project-read-only.md)
    - [Tool Workflows](tools/tool-workflows.md)
    - [Onboarding Improvements](concepts/onboarding-improvements.md)

# Experimental

- [Experimental Features](experimental/index.md)
# Development

- [Architecture](architecture.md)
- [Adding Languages](extending/adding-languages.md)
  - [Writing Tools](extending/writing-tools.md)
  - [The Tool Trait](extending/tool-trait.md)

- [Debug Mode](concepts/diagnostic-logging.md)
  - [Heartbeat Memory Fields](concepts/heartbeat-memory-fields.md)
- [Troubleshooting](troubleshooting.md)
