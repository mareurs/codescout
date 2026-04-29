# Experimental Features

> These features are available on `master` and the `experiments` branch.
> APIs and behaviour may change without notice. When a feature graduates to
> stable, its page moves into the main manual.

## Available Features

- [Librarian (embedded in codescout)](./librarian-embedded.md) — workspace doc/spec/plan index served as part of codescout when built with the `librarian` cargo feature; runtime-disable via `LIBRARIAN_ENABLED=0` or `[librarian] enabled = false` in `.codescout/project.toml`.
- [workspace_state_at](./workspace-state-at.md) — Time-travel snapshot: all artifacts in scope at a given commit/timestamp, with `freshness_at_as_of` vs `freshness_now` diff.
