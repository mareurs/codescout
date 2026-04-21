# `list_symbols` progressive directory overview

> ⚠ Experimental — may change without notice.

## What it does

When `list_symbols` is called on a directory (rather than a single file), it
now selects one of three output modes based on how many files are in the tree,
rather than always attempting a full symbol dump that overflows for large
directories.

## The three modes

| Mode | Triggered when | Shows |
|---|---|---|
| `full_tree` | ≤ 15 files | All symbols in all files — same as before |
| `class_overview` | 16–80 files | Class / struct / type names only, one line per file |
| `directory_map` | > 80 files | Subdirectory listing with file counts |

The response includes a `mode` field so agents know which level of detail they
received, and a `hint` with the recommended next step (e.g. drill down with
`list_symbols('<subdir>')`).

## Forcing a mode

Use `force_mode` to override the adaptive selection:

```json
{ "path": "src/", "force_mode": "class_overview" }
```

Accepted values: `"full_tree"`, `"class_overview"`, `"directory_map"`.

## Why this matters

Previously, calling `list_symbols("src/")` on a large project returned a
truncated dump with no structure. The new modes give agents a useful
coarse-to-fine navigation path: start at `directory_map`, pick a subdirectory,
drill down to `class_overview`, then open specific files with `find_symbol`.

## Known limits

- `class_overview` requires tree-sitter support for the language; files in
  detection-only languages are listed by path only.
- Thresholds (15 / 80) are fixed constants — no per-project override yet.
