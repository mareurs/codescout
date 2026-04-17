# Experimental Features

> Features on this page are available on the [`experiments` branch](https://github.com/mareurs/codescout/tree/experiments)
> and may change without notice. When a feature graduates to stable, its page
> moves into the main manual.

## Available Features

- [MCP resources, tool diet, progress notifications](./mcp-resources.md) — token-efficient resource sharing, short descriptions with on-demand guides, and progress notifications for long operations.
- [Project hints in `activate_project`](./project-hints.md) — manifest-derived primary language, entry points, and build commands surfaced in the activation response so agents have context without running onboarding.
- [Rust LSP multiplexer](./mux-rust.md) — share a single `rust-analyzer` process across multiple `codescout` instances on the same project, eliminating stale-hover / stale-goto bugs.
- [Tool usage doctor](./tool-usage-doctor.md) — `doctor://tool-usage` MCP resource reporting per-tool call counts, error/overflow rates, and prune candidates for the next prompt-surface review.
- [Cross-process write serialization](./cross-process-write-serialization.md) — advisory file lock serializes write-tool calls across concurrent codescout instances on the same project; contention returns a recoverable error instead of corrupting files.
