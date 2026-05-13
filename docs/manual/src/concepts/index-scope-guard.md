# Index Scope Guard
Before `index(action: build)` commits to walking and embedding a directory, codescout
checks whether the scope looks broad enough to be accidental, and requires
explicit human confirmation via an MCP elicitation dialog before proceeding.

## Triggers

Confirmation is required if either:

1. **The project root is a known-broad directory**, such as:
   - Your home directory (`~`)
   - The parent of home (e.g. `/home`)
   - A system root: `/`, `/usr`, `/etc`, `/var`, `/tmp`, `/root`, `/opt`, `/proc`, `/sys`
2. **The approximate raw source size exceeds the threshold** (default 500 MB of
   eligible content, respecting `.gitignore` and hidden-file rules — same
   filter `index(action: build)` itself uses).

When either trigger fires, the MCP client shows a dialog like:

```
⚠ Broad index scope detected

Root: /home/alice  (home directory)
Eligible files: ~3,200
Approx source content: ~2.4 GB
Estimated chunks: ~600,000

This will use significant RAM and CPU time.
Confirm indexing this directory?
```

You can accept to proceed or decline to abort. The check runs on **every** call
— it is not persisted. If your MCP client does not support elicitation, the
call is refused with a clear error rather than silently proceeding.

## Configuration

Adjust the size threshold in `.codescout/project.toml`:

```toml
[security]
max_index_bytes = 1073741824   # 1 GB
```

The default is `524288000` (500 MB). Set it higher to allow larger projects
without a prompt; lower to trigger the guard more aggressively.

Currently, the suspicious-path list is fixed — it is not configurable.

## Rationale

An agent that calls `workspace(action: activate, path: "~")` followed by `index(action: build)` would
otherwise walk the entire home directory, ingest every file, and cause severe
RAM spikes or OOM (see `docs/issues/memory-leak-x-session-freeze.md`). The
scope guard makes that path impossible without a human in the loop.
