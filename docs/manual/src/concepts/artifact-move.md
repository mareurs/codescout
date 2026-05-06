# `artifact(action="move")` — Atomic File Rename

Atomically renames a librarian-managed artifact file and updates the catalog's `rel_path` in a single operation. Replaces the previous `git mv` + `artifact(update, patch={rel_path:...})` + `librarian(action="reindex")` three-step sequence.

## Usage

```json
artifact(action="move", id="<16-hex>", new_rel_path="docs/archive/my-tracker.md")
```

### Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `id` | yes | 16-hex artifact ID |
| `new_rel_path` | yes | Destination path relative to the repo root |

### Response

```json
{
  "id": "abc123def456abcd",
  "old_rel_path": "docs/trackers/my-tracker.md",
  "new_rel_path": "docs/archive/my-tracker.md",
  "moved": true
}
```

## What it does

1. Resolves the artifact's current file path from the catalog
2. Calls `std::fs::rename` (atomic on same filesystem)
3. Creates any missing parent directories at the destination
4. Updates `rel_path`, `updated_at`, `file_mtime`, and `file_sha256` in the catalog

Git sees the rename automatically in `git status` — no extra `git add` needed.

## Error cases

- Destination already exists → `RecoverableError` (no filesystem change)
- Unknown `id` → `RecoverableError`
- Destination on a different filesystem → OS-level rename error propagated

## When to use

Use `artifact(action="move")` whenever you need to reorganize a tracker or archive it to a different directory. Do **not** use `git mv` or `fs::rename` directly — those leave the catalog stale until the next reindex.
