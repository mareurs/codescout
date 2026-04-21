# Credits

librarian-mcp borrows design from:

- **Redis agent-memory-server** (Apache-2.0) — https://github.com/redis/agent-memory-server
  - Filter AST shape (`filter.rs`)
  - Artifact row field layout (derived from `MemoryRecord`)
  - Working-vs-long-term split applied to draft-vs-indexed artifacts

- **Model Context Protocol reference memory server** (MIT) — https://github.com/modelcontextprotocol/servers
  - Entity / Relation / Observation conceptual model
