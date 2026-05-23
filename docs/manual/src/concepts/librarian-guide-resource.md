# `doc://librarian-guide` MCP Resource

A dense reference document for the librarian subsystem, surfaced as an MCP resource so agents can pull it on demand without consuming system-prompt tokens.

## Usage

```
resources/read doc://librarian-guide
```

Returns a self-contained markdown guide covering:

- **Artifact model** — frontmatter fields, `id`, `status`, `kind`, `tags`, `owners`
- **Filter syntax** — leaf format `{"field": {"op": value}}` with composition (`and`/`or`/`not`)
- **Tracker workflow** — design → create → augment → refresh lifecycle
- **Augmentation lifecycle** — `gather` / `commit_refresh` / `append_mode` / `history_cap`
- **Archiving / Moving** — `artifact(action="update", patch={status:"archived"})` and `artifact(action="move")`
- **Common mistakes** — filter format inversion, forgetting `repo` on create, direct file edits

## Why pull it?

The system prompt references librarian tools but cannot include the full filter reference inline (token cost). When you encounter a complex artifact query, call `resources/read doc://librarian-guide` once to load the complete reference into context.

## Source

`src/prompts/guides/librarian.md` — embedded at compile time via `include_str!`, so the bundled content always matches the running binary.
