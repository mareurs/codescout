---
status: draft
spec_version: 1
---

# Artifact CLI — Design

## Goal

Expose codescout's artifact / artifact-event / artifact-refresh / artifact-augment MCP tools as `codescout` binary subcommands so shell scripts and hooks (notably the goal-tracker Stop hook deferred from the goal-tracker plan's Phase 3) can read and mutate the librarian catalog without speaking MCP stdio.

## Audience

- Hook authors writing bash that needs to check goal-tracker state on every CC turn-end.
- Local-tooling and CI scripts that want to list, fetch, link, or refresh artifacts without a running MCP server.
- Humans at the terminal who occasionally want pretty output for `codescout artifact find` instead of opening the dashboard or attaching a client.

## Out of scope

- Shell completions (`codescout completion <shell>`). Defer.
- Streaming / `--watch` modes.
- Bulk operations (`codescout artifact update --where ...`).
- TUI / interactive picker.
- Replacing the MCP `artifact` tool family. The CLI is an additional surface, not a substitute.

## Approach (decision summary)

| Dimension | Choice | Why |
|---|---|---|
| Scope | Full artifact surface — find, get, create, update, move, link, graph, state-at + artifact-event (create/list) + artifact-refresh (gather/list-stale) + artifact-augment | Matches user direction; one PR delivers a complete surface so later hooks/scripts don't trickle in additions. |
| CLI shape | 1:1 with MCP tool names: `codescout artifact <verb>`, `codescout artifact-event <verb>`, `codescout artifact-refresh <verb>`, `codescout artifact-augment <id>` | Hook authors translate MCP docs verbatim. No naming clashes with future tool groups. |
| Filter surface | Shortcuts (`--kind`, `--tag` repeatable, `--status`, `--owner`, `--has-topic`) + raw `--filter '<json>'` escape hatch + `--semantic <query>` | Common 80% stays ergonomic; power users get the full FilterNode language; semantic search works standalone. |
| Output | Pretty default + `--json` flag + `--no-color`; auto no-color when stdout is not a TTY | Terminal UX stays human; hooks add `--json` and pipe through `jq`. |
| Bootstrap | Reuse existing `librarian_mcp::build_tool_context()` with optional `--project <path>` overriding `LIBRARIAN_CWD` env | Already env-driven, embedder is opt-in (`LIBRARIAN_EMBED_MODEL`), no new helper needed. Sub-100ms cold start typical for non-semantic verbs. |
| Embedder activation | Lazy: only build when a verb passes `--semantic <query>`. Other verbs run with `embedding: None` | Matches "lean activation"; non-semantic find/get/graph/state-at stay fast. |
| Project resolution | `--project <path>` flag (consistent with `Index`/`MigrateMemories`) defaults to cwd | Same convention as siblings; one flag overrides one env var. |
| Exit codes | 0 on success including 0 results; 1 on recoverable / catastrophic error; 2 on clap parse error | Standard Unix. Hook `if cmd fails OR stdout empty → continue=true` stays simple. |

## Architecture

```
src/main.rs                       -- adds 4 Commands variants, parses, dispatches
src/cli/
  mod.rs                          -- shared bootstrap wrapper, OutputOpts, error→exit mapping
  format.rs                       -- pretty / JSON formatters per Value shape
  artifact.rs                     -- find/get/create/update/move/link/graph/state_at parsers + dispatch
  artifact_event.rs               -- create/list
  artifact_refresh.rs             -- gather/list-stale
  artifact_augment.rs             -- single augment op
```

Each verb function:

1. Parses subcommand-specific clap args into a struct.
2. Sets `LIBRARIAN_CWD` env if `--project` is passed.
3. Calls `librarian_mcp::build_tool_context().await?`.
4. Builds the tool's input JSON (`serde_json::Value`) from the parsed args, applying shortcut compilation for the `find` filter.
5. Calls the underlying tool's `call(&ctx, args).await?`.
6. Routes the returned `Value` through `cli::format::print(&v, &output_opts)`.

`main.rs` does not gain logic; it just routes:

```rust
Commands::Artifact { verb } => codescout::cli::artifact::dispatch(verb).await?,
Commands::ArtifactEvent { verb } => codescout::cli::artifact_event::dispatch(verb).await?,
Commands::ArtifactRefresh { verb } => codescout::cli::artifact_refresh::dispatch(verb).await?,
Commands::ArtifactAugment(args) => codescout::cli::artifact_augment::run(args).await?,
```

## Components

### Bootstrap (`cli/mod.rs`)

```rust
pub(crate) struct CommonOpts {
    pub project: Option<PathBuf>,
    pub json: bool,
    pub no_color: bool,
}

pub(crate) async fn open_ctx(opts: &CommonOpts) -> Result<librarian_mcp::tools::ToolContext> {
    if let Some(ref p) = opts.project {
        std::env::set_var("LIBRARIAN_CWD", p);
    }
    librarian_mcp::build_tool_context().await
}
```

The CLI does **not** introduce a parallel project-id or catalog-path resolution. `build_tool_context()` is the single point of truth — same logic the MCP server uses on startup. If we later want sub-100ms hook latency that this cannot deliver (e.g. the workspace-config parse is itself slow), we can carve out a `build_tool_context_lean(skip_classifier_rules: bool)` overload, but that is **out of scope for the first cut** — the same defaults the MCP server runs with are the CLI's defaults.

**Thread-safety caveat:** `std::env::set_var` is racy in the presence of other threads; the codescout binary runs a single command per process, so the racy window does not exist in practice. If a future refactor moves CLI dispatch into a long-running context (e.g. a REPL), this approach must change.

### Verb surface — read verbs

```text
codescout artifact find
  [--kind <k>]                          # eq filter; e.g. "tracker"
  [--tag <t>]...                        # repeatable; each → {"tags":{"contains":t}}
  [--status <s>]                        # eq filter; disables archived-hide default
  [--owner <o>]                         # eq filter
  [--has-topic <t>]                     # topic contains substring
  [--filter '<json>']                   # raw FilterNode JSON; AND-merged with shortcuts
  [--semantic <query>]                  # natural-language search; lazy-builds embedder
  [--scope project|repo|umbrella|all]
  [--include-archived]
  [--augmented true|false]
  [--limit N] [--offset N]
  [--json] [--no-color] [--project <p>]

codescout artifact get <id>
  [--full] [--heading <h>] [--start-line N] [--end-line M]
  [--include-links] [--links-direction in|out|both] [--links-rel <r>]
  [--include-observations] [--include-events]
  [--json] [--no-color] [--project <p>]

codescout artifact graph <id>
  [--depth 1..3] [--rels <r>,<r>...] [--include-events]
  [--json] [--no-color] [--project <p>]

codescout artifact state-at <id>
  (--commit <sha> | --timestamp <ms>)
  [--json] [--no-color] [--project <p>]

codescout artifact-event list
  --artifact-id <id> [--kinds <k>,<k>...]
  [--since <ms>] [--until <ms>] [--limit N]
  [--json] [--no-color] [--project <p>]

codescout artifact-refresh list-stale
  [--threshold-hours N] [--scope ...] [--limit N]
  [--json] [--no-color] [--project <p>]
```

### Verb surface — write verbs

```text
codescout artifact create
  --kind <k> --title <t> --rel-path <p>
  [--repo <r>] [--status <s>]
  [--owners <o>,<o>...] [--tags <t>,<t>...] [--topic <t>]
  [--body @<file>|-]                    # file or stdin
  [--augment-prompt <p>|--augment-prompt-file <path>]
  [--augment-params @<file>]
  [--json] [--no-color] [--project <p>]

codescout artifact update <id>
  [--title <t>] [--status <s>]
  [--owners <o>,<o>...] [--tags <t>,<t>...] [--topic <t>]
  [--body @<file>|-] [--patch-params @<file>] [--commit-refresh]
  [--add-blocks <id>,<id>...] [--add-blocked-by <id>,<id>...]
  [--owner <o>] [--active-form <a>]
  [--json] [--no-color] [--project <p>]

codescout artifact move <id>
  --new-rel-path <p>
  [--json] [--no-color] [--project <p>]

codescout artifact link
  --src <id> --dst <id> --rel <r>
  [--json] [--no-color] [--project <p>]

codescout artifact-event create
  --artifact-id <id> --kind <k>
  [--payload @<file>|-]
  [--author <a>] [--anchor-commit <sha>] [--head-commit <sha>]
  [--parent-event-id <id>] [--resolves-intent-event-id <id>]
  [--also-mutates <id>,<id>...]
  [--source-uri <u>] [--source-kind <k>]
  [--json] [--no-color] [--project <p>]

codescout artifact-refresh gather <id>
  [--json] [--no-color] [--project <p>]

codescout artifact-augment <id>
  [--prompt <p>|--prompt-file <path>]
  [--params @<file>] [--params-schema @<file>]
  [--render-template @<file>]
  [--merge] [--append-mode] [--history-cap N]
  [--json] [--no-color] [--project <p>]
```

Convention: `@<file>` means "read content from file path"; `-` means "read from stdin". This avoids shell-escaping JSON literals on the command line for `--filter`, `--patch-params`, `--body`, `--payload`, `--params`, etc.

### Filter-shortcut compilation

`cli::artifact::find` compiles shortcut flags into a `FilterNode` AST identical to what the MCP tool receives:

```rust
fn compile_filter(args: &FindArgs) -> Option<Value> {
    let mut leaves: Vec<Value> = Vec::new();
    if let Some(k) = &args.kind         { leaves.push(json!({"kind": {"eq": k}})); }
    if let Some(s) = &args.status       { leaves.push(json!({"status": {"eq": s}})); }
    if let Some(o) = &args.owner        { leaves.push(json!({"owners": {"contains": o}})); }
    if let Some(t) = &args.has_topic    { leaves.push(json!({"topic": {"contains": t}})); }
    for tag in &args.tag                { leaves.push(json!({"tags": {"contains": tag}})); }
    if let Some(raw) = &args.filter {
        let parsed: Value = serde_json::from_str(raw)?;
        leaves.push(parsed);
    }
    match leaves.len() {
        0 => None,
        1 => Some(leaves.pop().unwrap()),
        _ => Some(json!({"and": leaves})),
    }
}
```

`--semantic` is passed separately as the tool's `semantic` arg, not folded into the FilterNode (semantic search is its own dimension in the tool's surface).

### Output formatter (`cli/format.rs`)

```rust
pub(crate) struct OutputOpts {
    pub json: bool,
    pub no_color: bool,
}

pub(crate) fn print(value: &Value, opts: &OutputOpts) -> Result<()> {
    let opts = resolve_no_color(opts);
    if opts.json {
        serde_json::to_writer_pretty(std::io::stdout(), value)?;
        println!();
        return Ok(());
    }
    match infer_shape(value) {
        Shape::FindResult     => print_find_table(value, &opts),
        Shape::GetResult      => print_get_summary(value, &opts),
        Shape::GraphResult    => print_graph_tree(value, &opts),
        Shape::StateAtResult  => print_state_summary(value, &opts),
        Shape::EventList      => print_event_list(value, &opts),
        Shape::StaleList      => print_stale_list(value, &opts),
        Shape::WriteAck       => print_ack(value, &opts),
        Shape::Unknown        => fallback_json(value),
    }
}
```

- `Shape::FindResult` recognises `{"items":[...],"total":N,...}` and renders a table with columns `id | kind | status | title | rel_path`.
- `Shape::GraphResult` renders a depth-indented ASCII tree.
- `Shape::WriteAck` recognises `"ok"` and similar acks → prints "ok" (or "ok: created <id>" when the value includes an id).
- `Shape::Unknown` is the safety net — uncaught shapes pretty-print as JSON.

`resolve_no_color` defaults to `--no-color`-on when `!std::io::stdout().is_terminal()`.

### Error handling and exit codes

`cli::mod` exposes one error sink:

```rust
pub(crate) fn exit_with(result: Result<()>, opts: &OutputOpts) -> ! {
    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            if opts.json {
                let _ = serde_json::to_writer(std::io::stdout(),
                    &json!({"ok": false, "error": format!("{e:#}")}));
                println!();
                std::process::exit(1);
            }
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
    }
}
```

| Condition | Exit | Output |
|---|---|---|
| Successful call (including 0 rows) | 0 | results |
| Recoverable / catastrophic error | 1 | error to stderr (pretty) or `{ok:false,error}` JSON to stdout |
| clap parse error | 2 | clap's stderr |

Phase 3 hook stays simple: `output=$(codescout artifact find ...) || output=""; ... fail-open on empty`.

## Data flow

### Read path (e.g. `codescout artifact find --tag goal --status active`)

1. clap parses `Commands::Artifact { verb: ArtifactVerb::Find(args) }`.
2. `cli::artifact::dispatch` routes to `cli::artifact::run_find(args)`.
3. `run_find` sets `LIBRARIAN_CWD` if `--project`, then awaits `build_tool_context()`.
4. Compiles shortcuts into a FilterNode AST.
5. Constructs `serde_json::Value` tool args:
   ```json
   {
     "filter": {"and": [...]},
     "scope": "project",
     "limit": 50,
     "include_archived": false
   }
   ```
6. Calls `librarian_mcp::tools::find::call(&ctx, args).await?`.
7. Routes the returned `Value` through `cli::format::print`.

### Write path (e.g. `codescout artifact create --kind spec --title ... --body @design.md`)

1. clap parses `ArtifactVerb::Create(args)`.
2. `run_create` resolves `@design.md` → reads file content as string (`-` reads stdin).
3. Builds args JSON; `body` is the string content, `tags` becomes a JSON array.
4. Calls `librarian_mcp::tools::create::call(&ctx, args).await?`.
5. Pretty-prints `ok: created <id>` or returns `"ok"` JSON.

### Semantic path (`codescout artifact find --semantic "goal tracker"`)

1. Same as read path through step 4.
2. Before calling `find::call`, if `args.semantic.is_some()` AND `ctx.embedding.is_none()`, return a `RecoverableError`-equivalent that explains: "semantic search requires the embedding service. Set `LIBRARIAN_EMBED_MODEL` (and optionally `LIBRARIAN_EMBED_URL`, `LIBRARIAN_EMBED_API_KEY`) and re-run."
3. If embedding is present, the tool internally embeds the query and runs the vector search — no CLI work needed.

## Error handling — named failure modes

### `4a` — `LIBRARIAN_DB` does not exist

`Catalog::open_with_workspace` returns an `IO` error with the missing path. The CLI exits 1 with `error: opening /home/u/.local/share/librarian/catalog.db: No such file or directory`. Suggestion in the message: "Start the codescout MCP server once to initialise the catalog, or set `LIBRARIAN_DB` to an existing catalog path."

### `4b` — `--filter '<bad-json>'` does not parse

Caught at the verb's arg-compilation step before any tool call. Exit 2 with `error: --filter is not valid JSON: <serde error>`. No catalog open performed (fail fast).

### `4c` — `--semantic` without `LIBRARIAN_EMBED_MODEL`

Caught before invoking the tool (no surprise mid-query). Exit 1 with the suggestion above. JSON mode emits `{"ok":false,"error":"semantic search requires...","hint":"set LIBRARIAN_EMBED_MODEL"}`.

### `4d` — `@<file>` for `--body` / `--payload` / `--params` cannot be read

The verb's arg resolver returns a wrapped error: `error: --body @foo.md: <IO error>`. Exit 1.

### `4e` — Stdin (`-`) requested in non-TTY context where stdin is empty

Read returns "". The tool receives an empty string body / payload; whether that's accepted is the tool's call. Document: "Pass `-` only when piping content; an empty pipe yields an empty value."

### `4f` — Project root cannot be resolved (no git toplevel, no `--project`)

`build_tool_context()` returns `current_project: None` and operates with `workspace-wide` scope. The verb continues — same behaviour as the MCP server's first request before a project is selected.

### `4g` — Concurrent writers

`Catalog` uses SQLite WAL mode; SQLite serialises writes. A concurrent `codescout artifact update` from two terminals will see one succeed and the other block briefly. Document this; do not add CLI-level locking.

### `4h` — Tool returns `RecoverableError`

Today `librarian-mcp` tools that hit known input-driven failures return a JSON value with `ok: false` (recoverable) rather than `Err`. The CLI prints those as-is in JSON mode and as `error: <message>` with exit 1 in pretty mode.

## Testing strategy

### Tier 1 — Unit (`src/cli/<verb>.rs::tests`)

- One test per shortcut → FilterNode mapping (`--tag goal --tag p1` produces the expected `{"and":[...]}`).
- `--filter '<bad-json>'` → `Err` with a clear message.
- `@<file>` resolution: temp file → expected string.
- Stdin (`-`) reading exercised with a faked stdin where practical.
- `infer_shape` round-trips for each known Value shape.

### Tier 2 — Integration (`tests/cli_artifact.rs`)

Uses `assert_cmd` + `predicates` + `tempfile`. Each test:

1. Sets `LIBRARIAN_DB=<tmpdir>/cat.db` and `LIBRARIAN_WORKSPACE=<tmpdir>/ws.toml`.
2. Seeds the catalog by running `codescout artifact create ...` (or by pre-inserting via SQL fixture).
3. Runs the verb under test; asserts stdout / exit code.

Smoke list (10 tests):
- `find` with no args returns `0` exit and either empty body or seeded rows.
- `find --tag goal --status active --json` produces parseable JSON with the seeded goal-tracker.
- `find --filter '{"kind":{"eq":"tracker"}}'` returns trackers.
- `get <id> --json` returns the row.
- `graph <id> --depth 1 --json` returns the seeded neighbourhood.
- `create --kind spec --title "..." --rel-path ...` writes a new artifact; subsequent `get <id>` shows it.
- `update <id> --status archived` succeeds; subsequent `find` (no `--include-archived`) omits it.
- `link --src A --dst B --rel implements` writes an edge; subsequent `graph A` shows B.
- `artifact-event create --artifact-id <id> --kind note --payload @msg.txt` succeeds.
- `--filter '<malformed-json>'` exits 2 with a useful stderr message.

### Tier 3 — Eval

Not applicable — CLI is a deterministic translation layer over MCP tools. No model in the loop.

### Not tested

- Concurrent-writer ordering (SQLite's responsibility).
- Latency / cold-start microbenchmarks (out of scope for this spec; the lean activation is the design choice, not a contractual target).
- Shell-completion correctness (out of scope).

## Dependencies

- **Re-uses existing crates**: `clap`, `serde_json`, `anyhow`, `tokio`, `librarian-mcp`.
- **New dev-deps for the codescout crate** (add to root `Cargo.toml` `[dev-dependencies]`):
  - `assert_cmd = "2"` (already in librarian-mcp; bumped to root for shared CLI tests).
  - `predicates = "3"` (same).
  - `tempfile` (already at root).

## Implementation order

### Phase 1 — Bootstrap + find (read-only minimum viable surface)

Lands `cli/mod.rs`, `cli/format.rs`, `cli/artifact.rs::run_find`, and the `Artifact` clap variant. After this phase: `codescout artifact find ...` works; Phase 3 hook becomes implementable. One commit.

### Phase 2 — Read verbs

Adds `get`, `graph`, `state-at`, `artifact-event list`, `artifact-refresh list-stale`. Each lands as one commit with a Tier-2 integration test smoke.

### Phase 3 — Write verbs

Adds `create`, `update`, `move`, `link`, `artifact-event create`, `artifact-refresh gather`, `artifact-augment`. One commit per verb where reasonable; verbs sharing a code path may be batched.

### Phase 4 — Goal-tracker Stop hook (revisit)

With the CLI in place, the deferred Phase 3 work from the goal-tracker plan becomes implementable. That work continues in the `codescout-companion` repo against this CLI surface and is not part of this spec.

## Open questions

1. **Pretty-print of `find` body excerpts** — should the table include the first line of each artifact's body, or stop at `title`? Current proposal: title only. Body excerpt is one `--full`-equivalent flag away.
2. **`codescout artifact` vs `codescout artifact help`** — clap defaults `codescout artifact` to listing subcommands. Confirm that's fine vs printing a one-line summary.
3. **JSON field naming** — when the CLI builds tool args, fields are snake_case (matching MCP). Confirm no fields need renaming for ergonomic flags (e.g. should `--anchor-commit` map to `anchor_commit`? Yes — clap's default-renaming flips the case).

## Validation after implementation

- `cargo fmt && cargo clippy -- -D warnings && cargo test` clean across the workspace.
- `cargo build --release` builds the new CLI surface; `target/release/codescout artifact find --help` shows the documented shape.
- Manual smoke: in the current project, run `codescout artifact find --kind tracker --tag goal --json` and confirm output matches `mcp__codescout__artifact(action="find", ...)` on the live MCP.
- Snapshot tests for `--help` output if they prove stable; otherwise rely on integration tests.
- Add a one-line summary of the new surface to `src/prompts/source.md` under `### Artifact & Tracker Routing` (so connecting LLMs learn the CLI exists).
- Once the CLI ships, unblock Phase 3 of the goal-tracker plan and write the Stop hook against `codescout artifact find` per the original plan text.
