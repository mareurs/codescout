# Entry Citations (`append_entry`, `entry_cite`)

Trackers whose entries carry monotonic ids — `F-1`, `W-7`, `T-21` — can allocate
the next id atomically and cite each other at entry grain rather than only at
file grain.

## The allocation problem

A tracker's ids live in two places at once: the structured rows in the
augmentation's `entry_collection` array, and the markdown body that renders
them (headings, index tables). Hand-picking "the next integer" means reading
one of those, and whichever you read can be behind the other — a body that ran
ahead of params reissues an id that a heading already claims.

```text
artifact(action="append_entry",
         id="<tracker-id>",
         id_prefix="F",
         entry_collection="frictions",
         entry={title: "...", status: "open"})
```

The server computes the next id from the live maximum across **both** sources
and assigns it, overwriting any `id` you pass in `entry`. When params lags the
body, the response carries a `warning` saying so — the id is still safe, but
the tracker needs a refresh.

Use this instead of a read-then-write for any monotonic-id tracker.

## Citing another entry

Pass `cites` alongside the entry:

```text
artifact(action="append_entry",
         id="<tracker-id>",
         id_prefix="W",
         entry_collection="wins",
         entry={title: "..."},
         cites=["F-12", "a1b2c3d4e5f60718", "docs/issues/2026-08-01-foo.md"])
```

Each ref is resolved as one of:

- a 16-hex artifact id;
- a `<slug>:<local>` entry id — `slug` being the artifact's lazily-assigned,
  deduped, immutable slug (schema v9);
- a unique `rel_path`.

Resolved refs become `entry_cite` edges, created in the same transaction as the
entry. **An unresolvable or ambiguous ref aborts the whole call** — the system
declines to guess which `F-12` you meant rather than recording a wrong edge,
because a wrong edge silently pollutes every context pack that walks the graph.

Not supported from a worktree checkout.

## Reading them back

```text
artifact(action="get", id="<tracker-id>", include_links=true)
```

surfaces `entry_cite` edges alongside artifact-level links.

## Relationship to `link_scan`

`cites` is the *write-time* path: you know the reference as you create the
entry. [`link_scan`](link-scan.md) is the *derive* path: it reads prose that
already cites tokens and materializes the same class of edge after the fact.
Both are appropriate; neither should be hand-created via
`artifact(action="link")`.

## Where this lives

`src/librarian/catalog/entry_cite.rs`, `src/librarian/tools/append_entry.rs`,
and the v9 migration (`artifact.slug` + the `entry_cite` table).

## Related

- [link_scan](link-scan.md)
- [tracker_design](tracker-design.md) — archetypes that declare an `entry_collection`
- [Augmentation: Templates &amp; Schemas](augmentation-render-template.md)
