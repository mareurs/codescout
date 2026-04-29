# Experimental Features

> These features are available on `master` and the `experiments` branch.
> APIs and behaviour may change without notice. When a feature graduates to
> stable, its page moves into the main manual.

## Available Features

- [Librarian Companion Hint](./librarian-companion-hint.md) — `librarian-mcp print-companion-hint` subcommand + `codescout-companion` SessionStart wiring so LLMs discover Librarian alongside codescout.
- [workspace_state_at](./workspace-state-at.md) — Time-travel snapshot: all artifacts in scope at a given commit/timestamp, with `freshness_at_as_of` vs `freshness_now` diff.
