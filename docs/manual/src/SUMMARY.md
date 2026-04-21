# Summary

[Introduction](introduction.md)
[From code-explorer to codescout](history.md)

# User Guide

- [Why codescout?](why-codescout.md)

- [Installation](getting-started/installation.md)
  - [Your First Project](getting-started/first-project.md)
  - [Routing Plugin](getting-started/routing-plugin.md)
  - [Companion Plugin](getting-started/companion-plugin.md)

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
  - [Setup Guide](semantic-search-guide.md)

- [Library Navigation](concepts/library-navigation.md)
  - [Auto-Registration](concepts/multi-ecosystem-auto-registration.md)
- [Multi-Project Workspaces](concepts/multi-project-workspace.md)
  - [activate_project Output](concepts/activate-project-output-optimization.md)
- [LSP Idle TTL](concepts/lsp-idle-ttl.md)

- [Memory](concepts/memory.md)
  - [After Onboarding](concepts/after-onboarding.md)
  - [Sections Filter](concepts/memory-sections-filter.md)

- [Dashboard](concepts/dashboard.md)
  - [LSP Startup Statistics](concepts/lsp-startup-stats.md)
- [Git Worktrees](concepts/worktrees.md)

- [Security & Permissions](concepts/security.md)
  - [Security Profiles](concepts/security-profiles.md)
  - [Compact Schemas & `activate_project` Safety](concepts/compact-schemas-and-activate-project-safety.md)
  - [PostCompact Cache Flush](concepts/post-compact-cache-flush.md)

- [Routing Plugin](concepts/routing-plugin.md)
  - [Superpowers Workflow](concepts/superpowers.md)

- [Project Configuration](configuration/project-toml.md)
  - [Embedding Backends](configuration/embedding-backends.md)
  - [Embeddings](configuration/embeddings.md)
    - [Model Comparison](configuration/embedding-model-comparison.md)

- [Language Support](language-support.md)
  - [Kotlin LSP Multiplexer](concepts/kotlin-lsp-multiplexer.md)

# Tool Reference

- [Tools Overview](tools/overview.md)
  - [API Naming Reference](tools/api-redesign.md)
  - [Symbol Navigation](tools/symbol-navigation.md)
  - [File Operations](tools/file-operations.md)
    - [grep: Literal Fallback](tools/search-pattern-literal-fallback.md)
  - [Editing](tools/editing.md)
    - [Structural Edit Gate](tools/edit-file-structural-gate.md)
    - [Document Section Editing](tools/document-section-editing.md)
    - [Markdown Tools](tools/markdown-tools.md)
  - [Semantic Search](tools/semantic-search.md)
  - [Library Navigation](tools/library-navigation.md)
  - [Git](tools/git.md)
  - [AST Analysis](tools/ast.md)
  - [Memory](tools/memory.md)
  - [Workflow & Config](tools/workflow-and-config.md)
    - [Read-Only `activate_project`](tools/activate-project-read-only.md)
    - [Tool Workflows](tools/tool-workflows.md)
    - [Onboarding Improvements](concepts/onboarding-improvements.md)
  - [GitHub](tools/github.md)

# Experimental

- [Experimental Features](experimental/index.md)
  - [Asymmetric query prefix](experimental/asymmetric-query-prefix.md)
  - [Bash language support](experimental/bash-language-support.md)
  - [Cross-process write serialization](experimental/cross-process-write-serialization.md)
  - [File-diversity re-rank](experimental/file-diversity-rerank.md)
  - [Global config](experimental/global-config.md)
  - [Index scope guard](experimental/index-scope-guard.md)
  - [librarian-mcp](experimental/librarian-mcp.md)
  - [list_symbols progressive directory](experimental/list-symbols-progressive-dir.md)
  - [MCP resources, tool diet, progress](experimental/mcp-resources.md)
  - [Metadata-enriched chunks](experimental/metadata-enriched-chunks.md)
  - [Project hints](experimental/project-hints.md)
  - [read_markdown improvements](experimental/read-markdown-improvements.md)
  - [Rust LSP multiplexer](experimental/mux-rust.md)
  - [Tool description diet](experimental/tool-description-diet.md)
  - [Tool usage doctor](experimental/tool-usage-doctor.md)

# Development

- [Architecture](architecture.md)
- [Adding Languages](extending/adding-languages.md)
  - [Writing Tools](extending/writing-tools.md)
  - [The Tool Trait](extending/tool-trait.md)

- [Debug Mode](concepts/diagnostic-logging.md)
- [Troubleshooting](troubleshooting.md)
