---
id: '2cc00cdc9dac8462'
kind: bug
status: fixed
title: 'RESIDUAL: Exclude doc-example citation syntax from corpus_cited_tokens (or add a recurring re-measure of its exposure)'
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-10-16
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-21-doctor-cited-uncited-partition-inherits-doc-example-defect.md
severity: low
unverified: STANDING — extraction still cannot tell a documentation example of citation syntax from a reference, and nothing is excluded; entry_without_definition now names the citing artifacts of each sampled broken token on every run, so the exposure is read, not inferred.
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-21-doctor-cited-uncited-partition-inherits-doc-example-defect.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Exclude doc-example citation syntax from corpus_cited_tokens (or add a recurring re-measure of its exposure).

## Parent caveat, verbatim

`docs/issues/archive/2026-08-21-doctor-cited-uncited-partition-inherits-doc-example-defect.md` (status `fixed`):

> The counting defect in `corpus_cited_tokens` is DOCUMENTED, not fixed — doc examples of citation syntax still enter its token set. Option 1 was chosen because measured exposure was zero on 2026-08-25, which is a fact about that date's corpus rather than a property of the parser: a future ledger written in the guide-example style re-exposes it, and nothing re-measures.

## Fix

Re-checked at HEAD first: the caveat still held. `extract` skips fenced blocks only (`fenced_blocks_are_skipped_inline_code_is_scanned` pins that inline code IS scanned), and `corpus_cited_tokens` returned a flat set with no source.

Chosen: the second branch of the title, a re-measure on every run, and NOT an exclusion. An exclusion needs a rule for what a teaching source is, and the literal marker that would supply one was declined on 2026-10-06 (`docs/issues/archive/2026-08-31-an-entry-id-cannot-be-mentioned-without-citing-it.md`, option 1 chosen). Contradicting that here would reopen it for a population still measured at zero.

`CorpusTokens.cited` is now token to the set of citing artifact paths (`src/librarian/tools/doctor.rs`), and `citing_sources_clause` writes into the `entry_without_definition` finding which artifacts cite each sampled broken token (3 tokens, 2 sources each, the rest counted), with the sentence that extraction cannot tell an example from a reference. That is the refusal site the convention in `docs/conventions/parsers-over-a-namespace.md` asks for, and it replaces a one-time measurement of one date's corpus with one that runs whenever `doctor` runs.

Left: nothing is excluded. A guide that cites a ledger-claimed token as an example still lands in the broken half; the finding now shows the guide as the only citer so a reader can see that.

## Tests added

`a_broken_citation_names_the_artifact_that_supplies_it` and `a_broken_citation_caps_the_sources_it_names` in `src/librarian/tools/doctor.rs`. Both were red first (the detail named no source), and green after. The 320 `doctor::tests::` tests pass; clippy with `-D warnings` is clean.

## Resolution

Fixed 2026-10-16 by attribution, not exclusion. See Fix.

## References

- `docs/issues/archive/2026-08-21-doctor-cited-uncited-partition-inherits-doc-example-defect.md` — parent
