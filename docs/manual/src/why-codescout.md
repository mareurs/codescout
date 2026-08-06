# Why codescout?

AI coding agents using only raw file tools — `cat`, `grep`, `find` — burn most of their
context window on navigation overhead: reading full files to find one function,
re-reading the same module from different entry points, asking questions they already
answered two tool calls ago.

The result is shallow understanding, hallucinated edits, and constant course-correction.
See the comparison table in the project README for a side-by-side view.

## Design choices

codescout exposes the same information an IDE uses — symbol definitions, references, type
info, git history — through a standard MCP interface. Three choices drive the design:

- **Single-session over agent chains** — skills run in the same context window as the
  main session, avoiding the compound error that accumulates at every inter-agent handoff
- **LSP navigation over file reads** — symbol-level queries are 10–50x more token-efficient
  than reading files, and return structured results rather than noise
- **Compact by default** — every tool defaults to the most useful minimal representation;
  full bodies available on demand via `detail_level: "full"`

## Research

These choices are informed by research on compound error in multi-agent systems —
research and empirical evidence confirm failure rates of 41–87% in production multi-agent pipelines.

[Read the analysis →](../../research/multi-agent-context-loss.md)
