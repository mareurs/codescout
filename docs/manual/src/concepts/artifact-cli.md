# Artifact CLI

> **Status:** experimental — see [Experimental Features](../experimental/index.md).

For shell scripts, git hooks, and CI jobs that cannot speak MCP, the
`codescout` binary exposes the artifact catalog as subcommands. Names mirror
the MCP tool names 1:1, so any MCP example translates trivially.

Each subcommand defaults to a **pretty table** for human reading; add
`--json` for machine consumers.

## Why a CLI surface

The MCP server is the primary interface, but two reader classes cannot use
it:

- **Pre-commit hooks** that need to inspect open trackers before allowing a
  commit have no MCP client at hand.
- **CI jobs and shell pipelines** that want to fail a build when an audit
  finds high-severity drift need a binary they can pipe through `jq`.

The CLI gives both groups direct catalog access without bootstrapping a
session, while keeping the MCP surface as the single source of truth for
behaviour — the CLI is a thin wrapper that calls the same code paths.

## Subcommands

### `codescout artifact ...` — CRUD and queries

| Subcommand | Mirrors MCP action | Purpose |
|---|---|---|
| `find` | `artifact(action="find")` | Filter + semantic search |
| `get <id>` | `artifact(action="get")` | Body / headings / line slice / links |
| `create` | `artifact(action="create")` | New artifact (`--kind`, `--title`, `--rel-path`, `--augment`) |
| `update <id>` | `artifact(action="update")` | Patch fields; `--commit-refresh` after a gather |
| `move <id>` | `artifact(action="move")` | Rename / relocate; updates path edges |
| `link` | `artifact(action="link")` | Create typed edge between two artifacts |
| `graph <id>` | `artifact(action="graph")` | BFS neighbourhood as ASCII tree |
| `state-at <id>` | `artifact(action="state_at")` | Time-travel snapshot at commit or timestamp |

### `codescout artifact-event ...` — append-only event log

| Subcommand | Mirrors MCP action |
|---|---|
| `create` | `artifact_event(action="create")` |
| `list` | `artifact_event(action="list")` |

### `codescout artifact-refresh ...` — augmentation lifecycle

| Subcommand | Mirrors MCP action |
|---|---|
| `gather <id>` | `artifact_refresh(action="gather")` |
| `list-stale` | `artifact_refresh(action="list_stale")` |

### `codescout artifact-augment <id>` — attach or patch augmentation

Mirrors `artifact_augment` MCP tool. Accepts `--prompt`, `--params`
(JSON), and `--merge` for params-only RFC 7396 merge-patch.

## Output modes

```bash
# Pretty table (default)
codescout artifact find --kind tracker --status active

# Machine-readable JSON
codescout artifact find --kind tracker --status active --json | jq '.[] | .id'

# Body of one artifact
codescout artifact get abc123 --full

# Just the "Findings" section
codescout artifact get abc123 --heading "## Findings"
```

## Stdin support

For subcommands that take a body or augmentation params, pass `-` to read
from stdin:

```bash
echo "# New spec\n\nDraft body." | codescout artifact create \
    --kind spec --title "Retrieval rewrite" --body -
```

This lets you compose bodies from `cat` chains, heredocs, or upstream
pipeline stages without writing a temp file.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Recoverable error (missing artifact, invalid filter, etc.) — message on stderr |
| `2` | Usage error (bad flag combination) |

Combine with `--json` for scriptable error handling — the JSON body
includes the same error message the human-readable mode prints to stderr.

## Relationship to the MCP tools

The CLI does not invent new behaviour. Anything you can do via the CLI you
can also do via the MCP tools, and vice versa. When in doubt about a
subcommand's semantics, the MCP tool description is authoritative —
the CLI inherits it.

## Further reading

- Design spec: `docs/superpowers/specs/2026-05-16-artifact-cli-design.md`
- Implementation plan: `docs/superpowers/plans/2026-05-16-artifact-cli.md`
- [Librarian Embedded](librarian-embedded.md) — the catalog the CLI reads
