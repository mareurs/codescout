> ⚠ Experimental — may change without notice.

# Auto-Reindex on Edit

Semantic search results stay current as files are edited, without requiring an explicit `index(action='build')` call.

## How it works

When a write tool (`edit_file`, `edit_code`, `create_file`) modifies a file, codescout checks whether the file's hash has changed since it was last indexed. If it has, the file is added to an in-memory dirty set.

The next call to `semantic_search` drains the dirty set and re-embeds all changed files before running the KNN query. The write tool still returns immediately — re-embedding is deferred until search time.

```
edit_code("src/foo.rs", ...)       → dirty_set: {"src/foo.rs"}
semantic_search("find all traits") → reindex src/foo.rs → knn on fresh index
```

## Properties

- **Zero write latency** — dirty set insertion is synchronous and sub-millisecond (one SHA-256 hash check).
- **Idempotent** — multiple writes to the same file before a search collapse to a single re-embed.
- **Non-blocking on failure** — if re-embedding fails (e.g. embedder unavailable), a warning is logged and the search continues with stale data.
- **Scope** — only files written through codescout tools. External editor edits are not tracked (filesystem watcher is a separate future feature).
