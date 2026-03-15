# Experimental Features

Experimental features live on the [`experiments` branch](https://github.com/mareurs/codescout/tree/experiments/docs/manual/src/experimental).
Browse that branch to see what's in the works — each feature has its own page with full documentation.

When a feature graduates to stable, its page moves into the main manual here.

## Current experimental features

- [Hard gate for structural edits in `edit_file`](./edit-file-structural-gate.md) — `edit_file` now refuses multi-line edits containing definition keywords on LSP-supported languages; the bypass has been removed.
- [Compact tool schemas & `activate_project` safety](./compact-schemas-and-activate-project-safety.md) — ~24% schema token reduction and a new Iron Law for safe cross-project navigation.
