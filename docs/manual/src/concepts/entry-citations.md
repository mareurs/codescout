# Entry Citations (`append_entry`, `entry_cite`)

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

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
doc(action="append_entry",
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

## Ledgers, and entries that live in prose

The call above assumes the entries are **params rows**. Most numbered surfaces in a
codescout repo are not: their entries are `## R-42 — <title>` body sections, with no
`entry_collection` at all. Those are **ledgers**, and a ledger is a much narrower thing
than a tracker — an artifact that owns an id namespace.

Declare one in **frontmatter**, not in the catalog:

```yaml
entry_prefix: R          # or a sequence, for a ledger owning two namespaces:
entry_prefix:            #   a session log carrying both F-N frictions and W-N wins
  - F
  - W
```

Frontmatter because the catalog is machine-local and git-ignored: a declaration stored
only in the augmentation is absent in a fresh clone, and every `append_entry` there
fails.

Then **omit `entry_collection`** to reserve an id without writing anything:

```text
doc(action="append_entry", id="<ledger-id>", id_prefix="R")
→ { "id": "R-42", "reserved": true,
    "body_max": 41, "reserved_max": 41, "frontmatter_max": 41,
    "next_step": "… Add the section as `## R-42 — <title>` …" }
```

The reservation is recorded inside the same transaction that reads the maximum, so a
concurrent session observes it and gets `R-43`. That is what makes it safe to write the
body in a **separate** call — and it is the property a bare "next free index" lookup does
not have. The response reports all three inputs the counter is derived from, and the hint
names the heading level **this** ledger's existing entries use rather than assuming one.

### The counter travels with the repo

A ledger carries its own high-water mark in committed frontmatter, one key per namespace,
written by the allocator:

```yaml
entry_prefix: HY
entry_high_water_HY: 11
```

Do not hand-edit it downward, and do not drop it when compacting. It is the only one of
the three inputs that survives a fresh clone, an `doc(action="move")`, and compaction
— the live body's maximum *falls* when entries move to an archive companion, and the
machine-local reservation does not travel at all. Without the committed mark a
compacted-then-archived ledger reissues `HY-1`, and because the resolver binds a token to
its sole **active** definer, every historical citation silently re-points to the new entry
with no dangling or ambiguous count moving.

When the committed mark leads both other inputs, the reservation says so in words: that
state means the ledger was compacted, or the checkout is fresh, and it is expected rather
than drift.

### From the main checkout only

`append_entry` refuses id allocation from a worktree session, on the same grounds it
refuses `cites`. An entry id is ledger-wide state and must key to the main tracker; left
unguarded, the worktree's shadow row is a different `artifact_id`, so both trees issue the
same number — and nothing repairs it at merge, because the renumber only covers params
rows. Record it in a worktree-local file and fold it in from the main checkout after the
merge.

## Citing another entry

Pass `cites` alongside the entry:

```text
doc(action="append_entry",
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
doc(action="get", id="<tracker-id>", include_links=true)
```

surfaces `entry_cite` edges alongside artifact-level links.

## Relationship to `link_scan`

`cites` is the *write-time* path: you know the reference as you create the
entry. [`link_scan`](link-scan.md) is the *derive* path: it reads prose that
already cites tokens and materializes the same class of edge after the fact.
Both are appropriate; neither should be hand-created via
`doc(action="link")`.

## Where this lives

`src/librarian/catalog/entry_cite.rs`, `src/librarian/tools/append_entry.rs`,
and the v9 migration (`artifact.slug` + the `entry_cite` table).

## Related

- [link_scan](link-scan.md)
- [tracker_design](tracker-design.md) — archetypes that declare an `entry_collection`
- [Augmentation: Templates &amp; Schemas](augmentation-render-template.md)
