---
id: '74dcfa57bda6b4cb'
kind: bug
status: superseded
title: 'BUG: an orphaned manual page documents librarian as a separate MCP server, unreachable from SUMMARY.md and invisible to the gate that scans it'
tags:
- cluster/doc-contradicted-by-code
closed: 2026-09-06
---

## Summary

`docs/manual/src/concepts/librarian-mcp.md` documents librarian as *"Codescout's sister MCP
server"* that *"runs as a separate stdio MCP server"*, with its own binary and build command. It
was dissolved into `src/librarian/` months ago. The page has no tombstone, is **not linked from
`SUMMARY.md`**, and carries **18** occurrences of retired tool names in a present-tense inventory
table.

The codebase states the correction itself, in the build file the page tells you to use:
`Cargo.toml:141` reads *"Librarian dependencies (formerly in `crates/librarian-mcp`, now dissolved
into `src/librarian/`)."*

## Symptom (Effect)

Anyone reaching the page by search or by an external link is told to build a binary that cannot be
built, connect a second MCP server that does not exist, and call eleven tools the server does not
register.

## Reproduction

At `0b017a63` on `experiments`:

```
grep -n 'librarian-mcp' docs/manual/src/SUMMARY.md   # → no match: unreachable from the book
ls -1 crates/librarian-mcp/                          # → empty; the directory is a husk
grep -n 'members' Cargo.toml                         # → members = [".", "crates/codescout-embed"]
```

So `cargo build --release -p librarian-mcp` (the page's § Installation) fails: not a workspace
member, and nothing to compile if it were. Last commit touching that path is `860d7bc7`, 4 months
ago.

## Environment

`experiments`. Not branch-scoped — the page and `Cargo.toml` are both shared with `master`.

## Root cause

**Two independent misses, and each alone would have been caught.**

*Not in the TOC* — `SUMMARY.md` is what mdBook renders, so an unlisted page is not built into the
book and nobody reviewing the manual's structure meets it. That is what let it sit through the
dissolve.

*Not visible to the gate* — `present_tense_surfaces()` (`tests/doc_tool_refs.rs`) **does** walk
`docs/manual/**`, so the page is scanned on every run. It is green because the two tests match
**anchored call forms** (`tool(param=…)`), and this page's 18 stale names live in a markdown table
and in prose that never writes a call. Same grammatical narrowing as
`docs/issues/2026-09-02-four-manual-surfaces-still-describe-read-markdown-in-the-present-tense.md`,
on a different retirement wave: this is the 2026-05-02 librarian-tools-collapse, not the 2026-09-02
one.

The combination is the interesting part: **the page is invisible to the human review path AND
inside the automated one that cannot see it.** Either miss alone is recoverable.

## Hypotheses tried

1. **Hypothesis:** the page is deliberate history and needs only a tombstone.
   **Verdict:** partly — it has none, and its § Installation is not history but an instruction.
   Whichever way it is resolved, the build command has to go.
2. **Hypothesis:** the crate still exists, so the page is merely behind.
   **Verdict:** rejected. `crates/librarian-mcp/` is an **empty directory** and is not a workspace
   member. `Cargo.toml:141` names the dissolve.

## Fix

**SUPERSEDED, not fixed here.** This record is a duplicate of
`docs/issues/archive/2026-09-01-librarian-mcp-page-describes-a-separate-server-that-was-collapsed.md`,
and was marked so on 2026-09-06 by `doc(action="link", src_id=876d7282ddc61f06,
dst_id=36ff17248b2c6ec7, rel="supersedes")` — that call is quoted verbatim in the
reproduction of
`docs/issues/archive/2026-09-06-supersedes-link-moves-catalog-status-without-writing-the-file.md`,
whose whole subject is that the link moved the status in the catalog and never wrote the
file. `36ff17248b2c6ec7` was this record's id before archiving.

The underlying defect did ship: `a6743089` (patch-id
`d95d41e70bbab9d031847601632d02f09c4480fb`) deleted the page and added
`tests/manual_toc.rs` — `every_manual_page_is_reachable_from_summary`, with
`the_toc_scan_is_reading_both_sides` as its non-vacuity control; both green 2026-09-15. It is
credited to the surviving record, not to this one.

**A WRONG NOTE STOOD HERE BRIEFLY AND ITS ERROR IS THE REASON THIS PARAGRAPH EXISTS.** It
read that no `supersedes` edge exists on this artifact — true of the catalog today,
`links.incoming` holds two `cites` and nothing else — and inferred from that absence that the
status had been *written by hand*, out of vocabulary. The inference is false, and the
falsifying evidence was one archived bug file away: the edge existed, was exercised, and is
recorded. **An edge that is absent now was not necessarily never there** — the source of this
one (`876d7282ddc61f06`) no longer resolves to any file, having been re-keyed or deleted in
the merge that created the edge, and an orphaned edge leaves no trace in `links`. Absence in
a link table is monotone under every way an edge can be removed.

**What survives as a real finding, narrower than the retracted claim.** `superseded` is not
in the bug-file status vocabulary that `get_guide("tracker-conventions")` § *Bug files*
publishes (`open | taken | investigating | fixed | mitigated | wontfix | zombie`), yet
`doc(action="link", rel="supersedes")` sets exactly that on a bug. The librarian then treats
it as archived: `doc(action="find")` hides the row and reports only
`hints.hidden_archived: 1`, and the four-status open filter never names it. So a superseded
bug that is never MOVED sits in the live `docs/issues/` directory reachable by no routine
query — not `open`, not the triage filter, not a default find. This one sat there 9 days and
was found by a filesystem scan of `docs/issues/*.md`. Supersession has a mechanism; the
archive step after it has no gate.

**SHIPPED `a6743089`**, patch-id `d95d41e70bbab9d031847601632d02f09c4480fb` — the page was
deleted and the TOC-reachability gate this file's § *Resume* asked for shipped in the same
commit as `tests/manual_toc.rs`. Verified 2026-09-15: the file is gone, `SUMMARY.md` holds no
reference to it, and the gate is green — `every_manual_page_is_reachable_from_summary` plus
`the_toc_scan_is_reading_both_sides`, the second being the non-vacuity control that makes the
first a measurement rather than a scan of an empty population.

**This record carried `status: superseded` until 2026-09-15, and that is the finding worth
keeping.** `superseded` is not in the bug-file status vocabulary
(`open | taken | investigating | fixed | mitigated | wontfix | zombie`,
`get_guide("tracker-conventions")` § *Bug files*), and the librarian treats it as an archived
state: `doc(action="find")` hides it by default and reports only `hints.hidden_archived: 1`.
So a fixed-and-gated bug sat in the LIVE directory, invisible to every routine triage query
— `status="open"` misses it, the four-status open filter misses it, and the default find
misses it. Not archived, not visible. It was reachable only by a filesystem scan of
`docs/issues/*.md`, which is how this was found.

**No `supersedes` edge exists on this artifact** — checked, `links.incoming` holds two `cites`
and nothing else — so the status was written by hand rather than set by
`doc(action="link", rel="supersedes")`, which is the tracker-shaped operation that would flip
it legitimately. Two independent sessions reached for the same out-of-vocabulary token on the
same day, which reads as a vocabulary gap rather than two typos.

Needs a decision rather than an edit, which is why this is filed and not fixed in the pass that
found it. Three options, cost ascending:

- **Delete the page.** It is unreachable, superseded, and every live fact in it is documented
  elsewhere (`docs/manual/src/concepts/librarian-tools-collapse.md` holds the old→new mapping).
- **Tombstone and keep**, in the style of `document-section-editing.md:41` — a `>` blockquote in
  past tense at the top, § Installation removed outright.
- **Rewrite as the current architecture** and add it to `SUMMARY.md`. Most work, and duplicates
  pages that already exist.

Recommendation: delete. A page no reader can reach and no gate can check is not documentation.

**Do not "fix" it by substituting live names into the table** — that manufactures a present-tense
description of an architecture that no longer exists, which is a worse defect than the stale one.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None. Acceptance for the delete option is that `present_tense_surfaces()` still returns a non-empty
corpus (`the_scan_is_not_reading_an_empty_corpus` covers this) and the three doc-ref tests stay
green. Note that green is **not** evidence here either way — the gate could not see this page's
defect while it was present, so it will not notice its removal.

## Workarounds

None needed; nothing reaches the page.

## Resume

Put the three options to the user, apply one, then sweep `docs/manual/src/` for other pages absent
from `SUMMARY.md` — this one was found incidentally and the absence-from-TOC check has never been
run as a population.

## References

- Sibling on the same surfaces, different retirement wave and same grammatical narrowing:
  `docs/issues/2026-09-02-four-manual-surfaces-still-describe-read-markdown-in-the-present-tense.md`.
- `Cargo.toml:141` and `:269` — the dissolve, stated in the build file.
- Found 2026-09-06 while verifying that bug's surface was clean; it was not in that bug's scope.
