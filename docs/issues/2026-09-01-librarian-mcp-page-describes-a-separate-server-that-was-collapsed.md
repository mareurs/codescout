---
id: '876d7282ddc61f06'
kind: bug
status: open
title: 'BUG: the librarian-mcp manual page describes a separate sister server; the tools collapsed into codescout'
tags:
- cluster/doc-contradicted-by-code
- docs
- manual
- librarian
- doc-drift
opened: 2026-09-01
owner: marius
severity: low
---

## Summary

`docs/manual/src/concepts/librarian-mcp.md` opens by describing librarian as
*"Codescout's sister MCP server … Runs as a separate stdio MCP server"*. It is not one.
The librarian tools were collapsed into codescout — `artifact`, `librarian`,
`artifact_augment`, `artifact_event`, `artifact_refresh` are codescout tools reached
through codescout's own MCP endpoint. The manual documents that collapse on a
neighbouring page (`librarian-tools-collapse.md`), which is what makes this one's
survival a drift rather than an open question.

The prose was **true when written**. Nothing checks a page's framing against the code.

## Symptom (Effect)

The page's whole frame is a separate server, so its tool table is pre-collapse:

- `:72` — `| artifact_update | Patch frontmatter or body; round-trips through the file |`
- `:137` — ``artifact_update`'s body patch replaces the entire body — no diff semantics.`

`artifact_update` has not existed as a tool since the collapse; it is
`artifact(action="update")`. Both lines read as present-tense reference.

**Fixing the two lines would be worse than leaving them.** They are consistent with the
page's stated premise, and correcting them in place would leave a page that describes a
separate server while listing codescout's collapsed API — internally inconsistent where
it is currently just out of date.

## Reproduction

```
head -8 docs/manual/src/concepts/librarian-mcp.md      # "separate stdio MCP server"
grep -n 'artifact_update' docs/manual/src/concepts/librarian-mcp.md
```

Then call `artifact(action="update", …)` against the running codescout server and observe
it work, with no second server anywhere.

## Root cause

The librarian tool collapse renamed the surface and merged the process; the concept page
describing the pre-collapse architecture was not revisited. `librarian-tools-collapse.md`
records the mapping (`:43`, `:50`), so the information exists — it just never propagated
backwards into the page it supersedes.

## Evidence

Measured 2026-09-01 while checking whether a denylist guard over `docs/manual/` was
viable. It is not, and that measurement is the other half of this file:

- **25 occurrences of retired tool names across `docs/manual/`; roughly 20 are
  legitimately historical** — migration tables (`api-redesign.md`,
  `librarian-tools-collapse.md`), a changelog (`history.md`), a `> Removed 2026-09-01.`
  banner (`ast.md`), and a rename note (`document-section-editing.md:40`). A bare
  `!contains` denylist — the shape `claude_md_contains_no_deprecated_tool_names` uses —
  would force deleting correct history. That is `IC-6`: a parser over a namespace with no
  escape hatch.
- **13 of those are names already on `DEPRECATED_TOOL_NAMES`** (`replace_symbol` ×4,
  `insert_code` ×4, `rename_symbol` ×4, `search_pattern` ×1), which none of that
  constant's three consumers scans — they cover the rendered instructions, `CLAUDE.md`
  and the prompt surfaces, never the manual.
- **A narrower heading-level guard fails too.** Of 4 manual headings naming a
  snake_case non-tool, only 1 was a real defect: `## render_template` and
  `## params_schema` are augmentation *field* names and `# tracker_design` is a
  `librarian` action. A 75% false-positive rate.

`tests/doc_tool_refs.rs` catches the *anchored call* form (`tool(param=`) precisely
because that form distinguishes "calls it" from "mentions it" by construction. Prose
mention has no such discriminator, which is why this is filed rather than gated.

## Hypotheses tried

1. **Hypothesis:** extend `DEPRECATED_TOOL_NAMES`' denylist check to `docs/manual/`.
   **Test:** count occurrences and classify each by whether the mention is historical.
   **Verdict:** rejected — ~20 of 25 are legitimately historical, and the check has no
   escape to express that.
2. **Hypothesis:** guard at heading level — no page may be *titled* after a dead tool.
   **Test:** enumerate manual headings matching a snake_case name absent from the live
   tool set.
   **Verdict:** rejected at 1 real defect in 4 hits; field names and action names are
   indistinguishable from tool names by shape.

## Fix

Not applied. This is a page rewrite, not a line edit: reframe `librarian-mcp.md` from
"sister MCP server" to "the librarian surface inside codescout", and bring its tool table
to the collapsed API — or supersede the page outright and redirect to
`librarian-tools-collapse.md` plus the live tool reference.

Decide which before editing. If the page has no reason to exist post-collapse, deleting
it beats maintaining a second description of the same surface.

## Tests added

None, and the reason is measured rather than skipped — see § Evidence. The two guards that
*could* have covered this were both tried and rejected on false-positive grounds.

## Workarounds

Read `librarian-tools-collapse.md` and `get_guide("librarian")` instead; both describe the
live surface.

## References

- `docs/manual/src/concepts/librarian-tools-collapse.md:43,50` — the collapse mapping.
- `tests/doc_tool_refs.rs` — the anchored-call guard, and why prose is out of its reach.
- `docs/trackers/issue-clusters.md` — `IC-11` (this class) and `IC-6` (why no denylist).

