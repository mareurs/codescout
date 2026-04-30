# Experimental Features

> These features are available on `master` and the `experiments` branch.
> APIs and behaviour may change without notice. When a feature graduates to
> stable, its page moves into the main manual.

## Available Features

- [Librarian (embedded in codescout)](./librarian-embedded.md) — workspace doc/spec/plan index served as part of codescout when built with the `librarian` cargo feature; **disabled by default** — opt in via `LIBRARIAN_ENABLED=1` env or `[librarian] enabled = true` in `.codescout/project.toml`.
- [workspace_state_at](./workspace-state-at.md) — Time-travel snapshot: all artifacts in scope at a given commit/timestamp, with `freshness_at_as_of` vs `freshness_now` diff.
- [Heartbeat memory fields](./heartbeat-memory-fields.md) — debug-mode heartbeat now logs `vm_size_kb` / `vm_rss_kb` / `vm_data_kb` / `vm_peak_kb` from `/proc/self/status`; gives per-instance memory time-series for OOM forensics.
