# Experimental Features

> These features are available on the `experiments` branch and not yet released
> to `master` or crates.io. APIs and behaviour may change without notice. When
> a feature graduates to stable, its page moves into the main manual and this
> entry is removed.

The following clusters are in flight as of 2026-05-18:

## Ripgrep-style text output for locator tools

`grep`, `references`, `tree`, `symbols`, and `call_graph` now emit
ripgrep-faithful plain text instead of JSON for small results — a per-file
`(count)` header, `N:` for matches, `N-` for context, `--` between blocks.
JSON output is still available for buffered (large) responses and via
`detail_level="full"`. See [Output Modes](../concepts/output-modes.md#text-form-ripgrep-style).

Tracked in `docs/ROADMAP.md` *What's Next* — `semantic_search`, `memory`, and
`IndexStatus` are queued to opt into the same wire form.

## Goal-tracker archetype

A new tracker archetype (`kind=tracker`, `tags: ["goal"]`) that names a
completion criterion and aggregates child tracker state. At most one goal is
`status=active` per project at a time, and `librarian(action="context")`
surfaces it automatically with no anchor. See
[Goal-Tracker Archetype](../concepts/goal-tracker.md).

## `codescout artifact*` CLI subcommands

The artifact / event / refresh / augment surface is now exposed as binary
subcommands for shell scripts and hooks that cannot speak MCP. Names mirror
MCP tools 1:1; each subcommand defaults to a pretty table and accepts
`--json` for machine consumers. See [Artifact CLI](../concepts/artifact-cli.md).

## `librarian(action="audit_doc_refs")`

Scan markdown surfaces for stale code references — file paths, symbols, line
numbers, link targets, module paths — and emit findings as an `audit_issues`
tracker. Manual cadence; intended to run before doc-heavy PRs merge. See
[Audit Doc Refs](../concepts/audit-doc-refs.md).

## `edit_markdown` additions

Two new params landed on the existing tool:

- `frontmatter: { set, delete }` — atomic flat-YAML mutation alongside any
  body edits in the same call.
- `at: "end-of-section" | "after-heading-line"` — controls placement for
  `insert_after` when a wrapping H1 makes "end-of-section" mean EOF.

See [Markdown Tools](../tools/markdown-tools.md#frontmatter-mutation) for
examples.
