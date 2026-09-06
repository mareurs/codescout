---
id: '36ff17248b2c6ec7'
kind: bug
status: open
title: 'BUG: an orphaned manual page documents librarian as a separate MCP server, unreachable from SUMMARY.md and invisible to the gate that scans it'
tags:
- cluster/doc-contradicted-by-code
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

