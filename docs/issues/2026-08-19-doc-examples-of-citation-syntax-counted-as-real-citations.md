---
id: '6e2cafbb1dea1678'
kind: bug
status: open
title: 'BUG: documentation examples of citation syntax are indistinguishable from citations, inflating link_scan''s diagnostic counts'
tags:
- librarian
- link-scan
- citations
- diagnostics
closed: ''
opened: 2026-08-19
owner: marius
related:
- '10d7e46375cc3053'
severity: med
---

# BUG: documentation examples of citation syntax are indistinguishable from citations

> **Status: open, severity low.** Nothing breaks. What degrades is the trustworthiness of
> `link_scan`'s own diagnostic counts — which is exactly what a session triaging link
> health reads.

## Summary

`link_scan` derives citations from prose, and prose is the only write surface for them.
That is a deliberate and good design. The consequence nobody declared is that **there is
no "mention" mode**: a token written to *teach* the syntax is extracted identically to a
token written to *cite* something.

So every document that explains how citations work — the guides, the conventions doc, a
bug file about a citation defect — injects its own examples into the graph and into the
report.

## Symptom

Measured 2026-08-19, `librarian(action="link_scan", scope="project")`:

- `cross_repo` read **6** against a world containing **3**. Three of the six came from a
  bug file's evidence section that quoted the other three verbatim as proof they were
  genuine. Removing the quotes took it to 4.
- The residual fourth is a faithful quotation of `get_guide("tracker-conventions")`, which
  uses a `<repo>:<PREFIX>-<N>` example to explain what a cross-repo reference *is*. The
  guide teaches the syntax; the scanner counts the lesson.

## Why it matters more than it looks

`cross_repo` is inert by design — reported, never turned into an edge — so six-instead-of-
three costs nothing directly. The same mechanism, however, feeds the two populations that
are **not** inert as diagnostics:

- `ambiguous`: 423
- `dangling`: 534

Both are read as health metrics, and both are triage targets. An unknown fraction of each
is documentation explaining the very failure it is being counted as. Nobody can currently
say what that fraction is, which means neither number supports the inference people
naturally draw from it ("534 broken citations to fix").

That is the actual defect: not the count, but that **the count cannot be interpreted**.


## A second consumer, and a measurement that bounds it (2026-08-19)

**Severity raised low → med here.** Not because anything new broke — because the defect
acquired a consumer that *classifies* rather than one that merely counts.

`doctor`'s `entry_without_definition` now partitions its undefined ids on whether anything
cites them (`corpus_cited_tokens`, shipped `5a72304c`, patch-id
`e9f8df63b9113a5b4073deebc5501a2cb623287a`). It calls the same `link_scan::extract`, so it
inherits this defect exactly: a guide that writes an entry token as an *example* of citation
syntax makes that token look cited, and the check would then report a define-on-citation
entry as a real dangling reference — landing it in the actionable half of its own partition,
which is the half a reader is told to act on first.

**Bounded, not hypothetical, on the one population measured so far.** The finding that
prompted this — 33 cited / 9 uncited on `docs/trackers/provenance-subsystem.md` — was checked
by reading the occurrences rather than trusting the count: they are prose of the form
*"(PV-3)"*, *"PV-12's labelling population"*, *"the answer on PV-12"*. All genuine citations;
none is a syntax example, and none sits in inline code. **The 33 is not inflated.**

Two things narrow the exposure, and both are worth knowing before anyone re-measures:

- `extract` already excludes **fenced** code blocks via its `in_code_block` guard, so the
  exposure is confined to **inline** code and bare prose — and inline code is scanned
  deliberately, with the comment *"real citations live here"*. The fix cannot simply stop
  scanning it.
- Guide-like artifacts are where token-as-example text concentrates. The population to check
  before trusting a *cited* count is `get_guide` sources, `docs/TAXONOMY.md`, `CLAUDE.md`,
  and the conventions docs — not the ledgers themselves.
## Reproduction

1. Write a document explaining citation syntax, using a realistic token as the example.
2. `librarian(action="link_scan")`.
3. The example appears in `cross_repo` / `ambiguous` / `dangling` exactly as a real
   citation would, attributed to the explaining document.

Observed twice in one session, in both directions: a paragraph describing the
ambiguous-token problem contained a bare token and created the ambiguity it described; and
the evidence section of
`docs/issues/archive/2026-08-18-qualified-citation-silently-truncated-when-file-stem-exceeds-31-chars.md`
quoted three cross-repo rows as proof and thereby doubled them.

## Root cause

`src/librarian/tools/link_scan/extract.rs` — extraction is purely lexical. Fenced code
blocks are already skipped and inline code is deliberately scanned (there is a test pinning
exactly that: `fenced_blocks_are_skipped_inline_code_is_scanned`), so backticks are not an
escape and were never meant to be. There is no marker an author can use to say "this token
is an example".

## Fix ideas

Ordered by cost, none yet chosen:

1. **Do nothing; document the hazard.** Cheapest, and arguably correct for `cross_repo`.
   Does not repair the interpretability of `ambiguous` / `dangling`.
2. **An opt-out marker.** An HTML comment scoping the following block, or a per-file
   frontmatter key. Cheap to implement, needs authors to know it exists.
3. **Attribute-and-subtract.** Keep extracting, but report a per-source breakdown so a
   triager can see that N of the 534 originate in guides and conventions docs. Changes no
   semantics and makes the numbers interpretable, which is the actual complaint.
4. **A reserved example namespace** that extraction classifies and drops. Clean, but only
   helps documents written after it lands.

Option 3 is the one that addresses the stated defect; 2 is the one authors will ask for.

## Note on this file

Written deliberately without emitting a matchable token — every example above is spelled
`<repo>:<PREFIX>-<N>`, whose angle brackets break the qualifier and whose `<N>` supplies no
digits. A bug report about self-citing documentation that self-cited would be a poor
advertisement for the finding.

## References

- `src/librarian/tools/link_scan/extract.rs` — the extractor
- `get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified*
- `docs/issues/archive/2026-08-18-qualified-citation-silently-truncated-when-file-stem-exceeds-31-chars.md` — where this was noticed
