# Diagnostic Logging


The `--debug` flag on the `start` command enables two things simultaneously:

1. **Structured lifecycle logs** — INFO-level events written to `.codescout/diagnostic-*.log`,
   covering server startup, heartbeats, tool calls, and shutdown. Useful for diagnosing
   MCP disconnect and silence issues.

2. **Usage traceability** — full `input_json` and `output_json` columns written to `usage.db`
   for every tool call, enabling post-mortem replay of failures.

> `--diagnostic` is a deprecated alias for `--debug` and will be removed in a future release.

## Enabling

Pass `--debug` when starting the server:

```bash
codescout start --project /path/to/project --debug
```

Or, if using `cargo run`:

```bash
cargo run -- start --project . --debug
```
## Log file

Each server instance writes to its own file:

```
.codescout/diagnostic-<4hex>.log
```

The `<4hex>` suffix is a random 4-character hex instance ID unique to that
process. Old files are rotated automatically: the 6 most recent
`diagnostic-*.log` files are kept by modification time; older ones are deleted
at startup.

## What is logged

| Event | When | Fields |
|-------|------|--------|
| `codescout_start` | Server boot | `pid`, `version`, `project`, `transport`, `instance` |
| `heartbeat` | Every 30 s | `uptime_secs`, `active_projects`, `lsp_servers` |
| `tool_call` | Tool invoked | `tool`, `arg_keys` |
| `tool_done` | Tool returned | `tool`, `duration_ms`, `ok` |
| `service_exit` | Shutdown | `reason` (signal name or quit reason) |

All events are INFO level and written in a structured format alongside the
existing stderr INFO layer.

## Reading the log

```bash
cat .codescout/diagnostic-*.log

## When to use it

- **MCP client disconnects silently** — `service_exit` captures the shutdown
  reason (SIGHUP, SIGTERM, pipe close, etc.).
- **Tool call hangs** — compare `tool_call` and `tool_done` timestamps to find
  which tool never returned.
- **Heartbeat gaps** — a missing heartbeat indicates the server process was
  suspended or killed.
- **Reproduce a failure** — `input_json` + `output_json` in `usage.db` let you
  replay a broken tool call at the exact codescout and project version that produced it.

## Debug Mode Coverage

`--debug` is the single flag for all verbose/debug behavior:

| What | Output |
|------|--------|
| Lifecycle events (start, heartbeat, tool call/done, exit) | `.codescout/diagnostic-*.log` (INFO, 6-file rotation) |
| Verbose internal state, LSP protocol traces | `.codescout/debug.log` (DEBUG level) |
| Full tool input/output JSON | `usage.db` `input_json` / `output_json` columns |

## Usage Traceability

Every tool call in `usage.db` records these columns (always, regardless of debug mode):

| Column | Description |
|--------|-------------|
| `codescout_sha` | Git SHA of the codescout binary (baked at compile time) |
| `project_sha` | Git HEAD of the active project at activation time |
| `session_id` | UUID identifying this server session |

In debug mode, two additional columns are populated:

| Column | Populated when |
|--------|---------------|
| `input_json` | Always (in debug mode) |
| `output_json` | Only on errors and recoverable errors |

### Replay Workflow

To reproduce a failed tool call:

1. Query `usage.db` for the failure (see [Reading the log](#reading-the-log) above)
2. `git checkout <project_sha>` — restore the project to its exact state
3. In the codescout repo: `git checkout <codescout_sha>` + `cargo build --release`
4. Start codescout, activate the project, re-invoke the tool with the stored `input_json`

Records are pruned after 30 days alongside normal usage records.

## Limitations

- The instance ID (`<4hex>`) is derived from `RandomState` seeded at process
  start — it is not a cryptographic or globally unique ID.
- Log rotation is by mtime, not sequence number. If the filesystem does not
  update mtime reliably (some network filesystems), rotation order may be
  incorrect.
- `arg_keys` logs parameter names only, not values, to avoid capturing
  sensitive content in log files.
- `project_sha` is captured once at `workspace(action: activate)`. If HEAD moves mid-session
  (e.g. a commit lands while the server is running), the stored SHA reflects the
  state at activation, not at call time — still a valid reproduction point.
# or tail the most recent:
ls -t .codescout/diagnostic-*.log | head -1 | xargs tail -f
```

To query failed calls with captured inputs from `usage.db`:

```bash
sqlite3 .codescout/usage.db \
  "SELECT tool_name, input_json, codescout_sha, project_sha, called_at
   FROM tool_calls
   WHERE outcome != 'success' AND input_json IS NOT NULL
   ORDER BY called_at DESC LIMIT 10;"
```
