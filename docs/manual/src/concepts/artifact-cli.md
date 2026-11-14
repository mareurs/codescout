# Artifact CLI (`codescout doc`)

> **Status:** experimental — see [Experimental Features](../experimental/index.md).

For shell scripts, git hooks, and CI jobs that cannot speak MCP, the
`codescout` binary exposes the artifact catalog under `codescout doc`. Names mirror
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

### `codescout doc ...` — CRUD and queries

| Subcommand | Mirrors MCP action | Purpose |
|---|---|---|
| `find` | `doc(action="find")` | Filter + semantic search |
| `get <id>` | `doc(action="get")` | Body / headings / line slice / links |
| `create` | `doc(action="create")` | New artifact (`--kind`, `--title`, `--rel-path`, `--augment`) |
| `update <id>` | `doc(action="update")` | Patch fields; `--commit-refresh` after a gather |
| `move <id>` | `doc(action="move")` | Rename / relocate; updates path edges |
| `link` | `doc(action="link")` | Create typed edge between two artifacts |
| `graph <id>` | `doc(action="graph")` | BFS neighbourhood as ASCII tree |
| `state-at <id>` | `doc(action="state_at")` | Time-travel snapshot at commit or timestamp |

### `codescout doc event ...` — append-only event log

| Subcommand | Mirrors MCP action |
|---|---|
| `create` | `doc(action="event_create")` |
| `list` | `doc(action="event_list")` |

### `codescout doc refresh ...` — augmentation lifecycle

| Subcommand | Mirrors MCP action |
|---|---|
| `gather <id>` | `doc(action="gather")` |
| `list-stale` | `doc(action="list_stale")` |

### `codescout doc augment <id>` — attach or patch augmentation

Mirrors the `doc(action="augment")` MCP tool. Accepts `--prompt`, `--params`
(JSON), and `--merge` for params-only RFC 7396 merge-patch.

## Text in the document

`link` and `event create` write to the document as well as to the catalog.
The catalog row stays the source of reads. The text is written first, and a
failed text write is an error that records no row.

A link adds one relation line to the source document, under a `## Relations`
heading at the end of the body:

```markdown
## Relations

**Depends on:** [[retrieval-rewrite]]
```

The label is the rel with `_`, `-` and spaces read as word breaks: lowercase,
first letter capitalised. `depends_on` gives `Depends on`. The same link run
twice leaves one line and one row. A rel that cannot make a valid label keeps
the row and skips the line.

An event is written as text when the document has `single_writer: true` in its
frontmatter. The value must be a YAML boolean; the string `"true"` does not
count. The kind must be `note`, `verdict`, `intent`, `reviewed` or
`external_signal`. A ledger, or a document with `entry_kind`, ignores the flag.
Each event is one section under a `## Events` heading at the end of the body:

```markdown
## Events

### note — 2026-11-14T09:30:00Z — alice
<!-- event {"id":"…","kind":"note","created_at_ms":…,"author":"alice"} -->

> The first thought.
```

The heading reads `### <kind> — <UTC time> — <author>`; the author is
`unknown` when none is known. The comment line holds the event as JSON. A
`**Mutates:** [[key]]` line follows it when the event names a target. The body
is a blockquote for a `note` whose payload is only text, and a fenced
`event-payload` JSON block for every other event.

Both commands report a skipped text in `text_twin` as `skipped: <reason>`.
`link` returns the bare string `ok` when the line was written. `event create`
skips with `not a single-writer document`, `document unreadable: <why>` or
`kind has no text twin`. It also skips with `ledger or entry document` or
`target not nameable: <why>`. `link` skips with
`rel is not a label: <why>`, `target not nameable: <why>` or `already present`.

## Output modes

```bash
# Pretty table (default)
codescout doc find --kind tracker --status active

# Machine-readable JSON
codescout doc find --kind tracker --status active --json | jq '.[] | .id'

# Body of one artifact
codescout doc get abc123 --full

# Just the "Findings" section
codescout doc get abc123 --heading "## Findings"
```

## Stdin support

For subcommands that take a body or augmentation params, pass `-` to read
from stdin:

```bash
echo "# New spec\n\nDraft body." | codescout doc create \
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
