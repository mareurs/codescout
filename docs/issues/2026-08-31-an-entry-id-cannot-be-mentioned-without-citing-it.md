---
kind: bug
status: open
tags:
- cluster/addressing-without-an-escape-hatch
- librarian
- link_scan
- citations
- graph-hygiene
- no-escape-syntax
closed: null
opened: 2026-08-31
owner: marius
related: []
severity: low
---

# BUG: an entry id cannot be MENTIONED without citing it — the only escape is a fenced block, and inline backticks are explicitly scanned

## Summary

`link_scan` derives a `cites` edge from any `\b[A-Z]{1,3}-\d+\b` token in a body. There
is no way to write an id as a **literal** — to discuss the identifier itself rather than
reference the entry it names — anywhere a sentence can go. The one escape is a fenced
code block; inline backticks, the notation an author reaches for to mean "this is a
literal token", are deliberately **scanned**.

The consequence is narrow but real: a document about **id allocation** cannot name the
colliding id without emitting a false edge to whatever ledger happens to define that
number, whose content is unrelated by construction.

## Symptom (Effect)

Measured 2026-08-31 at `83125c1f` (patch-id `92ca62eb7c9800ef3f48d4b0be378813d775a300`;
the pre-rebase SHA was `831d2496`). A spec section discussing an id-allocation collision
across two hosts produced a `cites` edge to a tracker it does not cite:

```
librarian(action="link_scan", findings_limit=800, write=false)
  edges_missing: 1
    src: docs/superpowers/specs/2026-08-31-cross-machine-catalog-integration-design.md
    dst: docs/trackers/reconnaissance-patterns.md
```

The spec's prose names the id twice in the sense "the number both allocators resolved
to". The entry actually defined at that number in `reconnaissance-patterns.md` is titled
*"A quotation that asserts its own fidelity does not check it, and the assertion is what
stops the reader looking"* — no relation to cross-machine allocation. The edge is
syntactically derived and semantically false.

Same document set, same pattern, three further instances at
`docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md:29,31,89`
— the bug file describing the collision, which cannot describe it otherwise.

## Root cause

`src/librarian/tools/link_scan/extract.rs` — `extract` skips fenced regions and scans
everything else, inline code included. Its own pinned test states the contract:

```rust
// extract.rs:644-652
fn fenced_blocks_are_skipped_inline_code_is_scanned() {
    let text = "See `f2ecdd76a6189efb` for the exemplar.\n\n```\nF-3 and 59ebeebb6ed05c89 inside a fence\n```\n";
    let ex = extract(text);
    assert_eq!(tokens(&ex, CitationKind::ArtifactId), vec!["f2ecdd76a6189efb"]);
    assert!(tokens(&ex, CitationKind::EntryToken).is_empty());
}
```

Scanning inline code is correct and load-bearing — it is how the overwhelming majority
of real citations are written, and narrowing it would silently drop most of the graph.
The gap is that no third state exists between "scanned" and "inside a fence".

## Why this is easy to miss

The author who most needs mention-without-citation is the one writing about identifiers,
and the notation they will reach for — wrapping the token in backticks to mark it as a
literal — is precisely the one that does not escape. Nothing surfaces the mistake: the
derived edge is reported as `edges_missing`, i.e. as work to be done, not as a suspect
resolution. Running the prescribed `write=true` then **materialises** it and the report
goes clean.

Note the shape: the tool's own remedy converts the defect into a silent success.

## Fix options

1. **Do nothing; document the fenced-block escape** in `get_guide("tracker-conventions")`
   § *Citing an entry*, which currently describes how to cite and never how not to.
   Cheapest, and honest.
2. **A literal marker.** A doubled backtick, a `!` prefix (`!PFX-<n>`), or an
   HTML-comment pragma suppressing extraction for one line. Small, but it is new syntax
   every author must learn to avoid a rare problem.
3. **Confidence-rank the edge rather than suppress it.** An entry token whose citing
   sentence contains allocation vocabulary is a weak citation. Heuristic, and heuristics
   in a resolver are how "resolves to nothing right now" claims get made.

Recommend (1). The population is small — four instances in one run, all in documents
*about* id allocation — and options 2 and 3 both add machinery to a path where the
existing behaviour is right for every other caller.

## Consequence accepted for now

The edge was materialised at `83125c1f` (`link_scan(write=true)`, `edges_added: 1`),
deliberately, rather than dodged by rewording the spec — rewording to satisfy a graph
trades a legible sentence for one edge.

**That is no longer the state, for an unrelated reason.** The fix wave's re-review found
the same annotation overstated its claim, and rewriting the sentence for accuracy
happened to drop the token, so `link_scan` will prune the edge as stale on its next
`write=true`. The rewrite was not motivated by this bug and the reasoning above still
stands on its merits; the surviving demonstration is the three instances in
`docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md`
(`:29`, `:31`, `:89`), which genuinely cannot be reworded away — that file's subject *is*
the colliding id.

## Resume

Start at `src/librarian/tools/link_scan/extract.rs` — `extract` (`:324-447`) and
`scan_tokens` (`:454-505`); the fence-skipping is what a literal marker would extend.
The contract test to amend is `fenced_blocks_are_skipped_inline_code_is_scanned`
(`:644-652`). If taking option (1) instead, the text belongs in
`get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified*, next to the
existing note that a qualifier naming no file is reported and never turned into an edge.
