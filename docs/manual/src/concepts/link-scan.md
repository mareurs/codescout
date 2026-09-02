# `link_scan` — Citation Edges Derived From Prose

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

```text
librarian(action="link_scan")              # report only (default)
librarian(action="link_scan", write=true)  # materialize and prune edges
```

`link_scan` reads artifact bodies, resolves the citation tokens it finds, and
maintains scanner-owned `rel="cites"` edges to match.

## Why derive rather than declare

The link graph pays off in three places: `doc(action="get",
include_links=true)`, `doc(action="graph")`, and
`librarian(action="context", anchor_id=…)`, which packs an artifact's
neighbourhood into a context bundle. All three are only as good as the edges.

Asking authors to hand-create an edge for every reference does not survive
contact with reality — people cite `F-3` in a sentence and move on. So prose is
treated as the **only** write surface for citations, and the scanner keeps the
graph in step. First live run on the codescout repo: 755 artifacts, 430 edges
where there had been none.

## What counts as a citation

Stable ids, in their own namespace:

| Form | Resolves to |
|---|---|
| `A-11`, `F-3`, `BUG-40` | The entry whose defining heading claims that token |
| 16-hex id | That artifact |
| `rel_path` | That artifact |
| `<repo>:<ID>` | Cross-repo reference |

Resolution rules that matter:

- An **archived** definer loses ties to an active one, so citing `F-3` resolves
  to the live tracker rather than the retired copy of it.
- **Ambiguous** tokens are reported, never guessed. `F-N` ids are locally scoped
  per tracker, so the same token legitimately exists in several — the scan
  declines to pick one.
- Tokens that exist only as augmented-artifact `params` rows (a `T-N` with no
  heading of its own) are invisible to a heading-based detector by design, and
  show up as dangling.

## Idempotent, and the repair path

Re-running is safe: the scan converges to a fixpoint, materializing missing
edges and pruning ones whose prose no longer supports them.

That property makes it the repair path after moves and reindexes. The catalog's
`abs_path` pre-clean cascade-drops a moved artifact's links; a re-scan heals
them. Run it after any bulk move, and before a docs-heavy merge.

## Frontmatter is not a metadata block

The extractor originally enabled pulldown_cmark's
`ENABLE_YAML_STYLE_METADATA_BLOCKS`, which pairs *any* two bare `---` lines in a
document as a YAML metadata block. Session-log trackers separate every entry
with a bare `---`, so roughly every other entry heading was silently swallowed
and never registered as a definition. Frontmatter is now skipped by an explicit
byte-offset guard instead. If you write a custom extractor against these
documents, this is the trap.

## Where this lives

`src/librarian/tools/link_scan/` — `extract.rs` for token and definition
detection, `diff.rs` for the materialize/prune decision.

## Related

- [Entry Citations](entry-citations.md) — the write-time counterpart
- [Audit Doc Refs](audit-doc-refs.md) — the lint for *stale* references, a
  different question from *which edges exist*
