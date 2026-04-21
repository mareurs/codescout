# Rust LSP multiplexer
## What it does

When two `codescout` instances open the same Rust project, they now share a
single `rust-analyzer` process via the existing LSP multiplexer (first used
for `kotlin-lsp`). This eliminates the stale-hover / stale-goto bug that
appeared after a write in instance A was not reflected in instance B.

## Footprint

- One `rust-analyzer` per `(project-root)` across all `codescout` instances
  on the machine.
- Idle-shutdown after 180 seconds with no connected clients.
- Memory saved: one full `rust-analyzer` (2–4 GB on a medium Cargo
  workspace) per extra `codescout` instance.

## Opt out

Add to `.codescout/project.toml`:

```toml
[lsp.rust]
mux = false
```

Then `/mcp` restart. Codescout will fall back to spawning a dedicated
`rust-analyzer` per instance, as before.

## Known limits

- Unix only (the mux is `#[cfg(unix)]`).
- `rust-analyzer` must be on `PATH`.
- If two clients connect before `rust-analyzer` completes initialization,
  the second client waits on a 5-retry / 1-second backoff. No-op
  behaviourally; you may see a brief startup delay under heavy
  concurrency.
