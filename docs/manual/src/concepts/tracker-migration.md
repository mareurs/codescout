# Tracker Migration: `codescout trackers`

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

```text
codescout trackers inventory <paths>...   # read: what the sources hold
codescout trackers apply <mapping>        # write: the collection, and delete the sources
codescout trackers check <mapping>        # read: prove that nothing was lost
```

A flat tracker is one markdown file with many records. A lifecycle collection is a
folder with one file per entry. `codescout trackers` moves the first into the second.
An agent drafts a mapping file. The user approves it. `apply` then does the move.

The `migrating-trackers` skill will drive these commands. It is part of Plan L-E2, a
separate and later plan. Until then, you run the commands yourself.

The commands never commit. You commit the result.

## The three commands

### `inventory`

`inventory` prints what each source holds at `HEAD`. It writes nothing. It reports:

- the records, the sections that are not records, and the possible records;
- the status values, the index rows, and the dates that records state;
- every file that names a source, and what kind of file it is.

| Option | Meaning |
|---|---|
| `<paths>` | The sources, relative to the project directory. For `--kind files`, a folder. |
| `--kind` | What a record is: `file` (default), `sections`, `whole` or `files`. |
| `--level` | The heading level of a `sections` source. Allowed only with `--kind sections`. |
| `--status-column` | Read each status from this index column, not from the `**Status:**` line. |
| `--project` | The project directory. Default: the working directory. |
| `--json` | Print one JSON object instead of a summary. |

### `apply`

`apply <mapping>` reads every source at `HEAD`. It takes the project write lock. It
refuses before the first write when a precondition fails. Then it writes the
collection and deletes the sources.

A second run with the same mapping writes nothing and says `already applied`.

| Option | Meaning |
|---|---|
| `<mapping>` | The mapping file. A relative path starts at the working directory. It can contain `..`. |
| `--project` | The project directory. Default: the working directory. |
| `--no-old-catalog` | This repository never had an old catalog. Skips the catalog-facts check when this machine has none. |
| `--json` | Print the result as one JSON object. |

### `check`

`check <mapping>` proves that the migration lost nothing. It plans again from `HEAD`.
It reads the collection folder and the working tree. It compares each entry body with
its source lines at `HEAD`. It writes nothing and takes no lock.

| Option | Meaning |
|---|---|
| `<mapping>` | The mapping file. A relative path starts at the working directory. It can contain `..`. |
| `--project` | The project directory. Default: the working directory. |
| `--json` | Print the result as one JSON object. |

**Run `check` before the migration commit.** After the commit, the sources are no
longer at `HEAD`. Then `check` exits with code 2 and says that a source is not in
`HEAD`.

The usual order is:

1. `inventory` the sources.
2. Draft the mapping file. The user approves it.
3. `apply` the mapping.
4. `check` the mapping.
5. Commit.

## The mapping file

The mapping file is TOML. Every table refuses an unknown key. `apply` refuses while the
mapping leaves an item of the inventory unsettled. Each refusal names the item and the
key that settles it.

### Sources and target

- `sources`: a list of tables. Each has `path`, `kind` and, for `sections`, `level`.
  - `path`: the source, relative to the repository.
  - `kind`: `file`, `sections`, `whole` or `files`.
  - `level`: the heading level. Required for `sections`. Refused for any other kind.
- `target`: a table.
  - `lifecycle`: `ledgers`, `streams` or `temp`.
  - `key`: the collection key. Default: the stem of the first source.
  - `archived`: write the collection under the `archive` folder of the lifecycle. Default `false`.
    An archived collection holds only terminal entries. `apply` refuses a record whose status is
    not terminal. Map it to a terminal status in `status_map`, or set `overrides.<record>.status`.
  - `existing`: add entries to a collection that exists. Default `false`.
- `schema`: the collection schema, as a table. Required when `target.existing` is `false`. Refused when it is `true`.
- `hot_view_none`: a reason for a collection with no hot view. Without it, a new collection needs `hot_cap` and `stale_after` in `schema`.
- `readme_body`: the readme prose of the new collection.

### Source frontmatter

Each entry gets new frontmatter with its own `kind`, `status` and `title`. Every other key of
the source frontmatter is lost with the source. `columns` reads index rows only, so it maps no
frontmatter key.

The commands list each lost key with its value:

- `inventory` lists them per source in `dropped_frontmatter`.
- `apply` and `check` list them in `review.dropped`, as `<source> frontmatter <key>: <value>`.
- `check` reads them from the source at `HEAD`, not from the plan.

A lost key does not block. Read the list before you commit.

### Entries and status

- `entry_kind`: one kind for every entry (`entry_kind = "finding"`), or a table from token prefix to kind (`F = "friction"`). A table cannot class a record with no token.
- `status_source`: one of these:
  - `"body_label"`: the `**Status:**` line of the record;
  - `{ index_column = "<header>" }`: a column of the index table;
  - `{ constant = "<status>" }`: one status for every record.
- `status_map`: raw status value to declared status. It must cover each value that the inventory reports. A `constant` source needs no `status_map`.
- `status_none`: the status of a record with no raw value. Required when the inventory reports such a record. A `constant` source needs none.
- `overrides`: a table from record name to values for one record. The keys are `status`, `created`, `closed`, `revisit_by` and `slug`. A date is a TOML date or a quoted `YYYY-MM-DD`.
  - The record name is the token of the record.
  - A record with no token uses its fixed stem. A `files` source gives each file a fixed stem.
  - A record with no token and no fixed stem uses its title.
  - A `files` entry keeps its stem as its key. An override `slug` changes that key. `apply` and `check` list the change in `review.renamed_keys`. A `[[<stem>]]` citation then stops resolving, and `check` does not look for one.
- `shorten`: a table from record name to replacement body. `check` reports each shortened record.

### Index tables

- `columns`: index header to a field name, to `created`, or to `title_alias`. A `title_alias` cell stays in the `**Index row:**` line and writes no field.
- `row_title_column`: the index header whose cell titles a record that has a row and no heading. Required when the inventory reports such a row.
- `index_table`: the section whose index table gives the row of a token. Required when a token has rows in two index tables. It has two forms:
  - `index_table = <line>`: the first line of the section. It names that line in every source.
  - `index_table = { source = "<path>", start = <line> }`: the first line of the section in one source.
  - Use the second form when two sources each have an index table at the same line. A bare line then names two tables, and `apply` refuses the token.

### Possible records and sections

- `records_extra`: a list of `{ source, line, token }`. It makes the heading at `line` a record.
- `possibles_inside`: a list of `{ source, line }`. It keeps the possible record at `line` inside its enclosing record.
- `sections`: a list of `{ source, start, to, confirm }`. It places a section that is not a record.
  - `start`: the first line of the section.
  - `to`: `readme` or `drop`.
  - `confirm`: set it to `true` to state that a possible record in the section is not a record.

### Files that name a source

- `code_mentions`: a table from file to class: `reader`, `comment` or `fixture`. Each non-markdown file that names a source needs a class.
- `sidecar`: `"retire"` (delete it) or `{ keep = "<reason>" }`. Required when the source has a sidecar.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Done (`inventory`, `apply`, including `already applied`) or clean (`check`). |
| 1 | `apply` refused, or `check` is not clean. |
| 2 | The command could not run. It prints one line with the reason. |

With `--json`, a refusal prints `{"refused": [...]}`, one string per reason. A command
that could not run prints `{"ran": false, "reason": "..."}`. A `check` that ran prints
`{"clean": ..., "blocking": [...], "shortened": [...], "review": {...}}`.

`check` gives the refusal shape in three cases only:

- the mapping file does not parse;
- a source path is not a normalised repository path;
- `target.existing` is `true`, and no collection loads at the target folder.

Any other mapping or plan problem that `apply` refuses is a blocking line of `check`.
`check` then prints `"clean": false` and exits with code 1.

`check` does not test four preconditions of `apply`: readers in `code_mentions`, catalog
facts, uncommitted changes, and a shallow clone. A clean `check` does not show that
`apply` would run.

## Late records

A late record is a record that you add to a collection after its first migration. Set
`target.existing = true` and leave `schema` out.

The merge must be committed first. Then the flat file is at `HEAD` when `apply` reads
it. An uncommitted merge is invisible to `apply`.

In this mode `apply` writes no readme. So every `sections` entry must have
`to = "drop"`. `apply` refuses a section with `to = "readme"`.

## Differences from the design spec

The design spec is `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`. The
code differs from it in these places. The code is right.

- **The source key is `kind`.** The spec says `source_kind`.
- **`hot_view_none` is a top-level key.** The spec lists it as an alternative inside `schema`. The code reads it from the top level of the mapping.
- **A `constant` status source needs no `status_map` and no `status_none`.** The spec says `status_map` must cover every raw value. A constant gives every record one status, so `apply` does not read the raw values.
- **`overrides` also takes `slug`.** The spec lists `status`, `created`, `closed` and `revisit_by` for the key. An override `slug` comes first in the entry-name order, so it resolves a name collision.
- **`title_alias` writes no field.** The spec does not say what it does. The cell stays in the `**Index row:**` line.
- **`index_table` can name a source.** The spec has one line number. The table form names the source too, so it settles two sources with a table at one line.
- **No frontmatter key maps through `columns`.** The spec says that old keys map through `columns` or are listed as dropped. `columns` reads index rows only, so the commands list every source key but `kind`, `status` and `title` as dropped.
