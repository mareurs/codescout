---
id: '79b2591f4ed05715'
kind: bug
status: open
title: memory(read) `sections` filter only matches `###`, but 15 of 21 memories use `##` — section targeting is unusable, including on gotchas
tags:
- memory
- progressive-disclosure
- tooling
- docs-vs-code
closed: ''
opened: 2026-07-28
owner: marius
related:
- docs/issues/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md
severity: medium
---

# BUG: memory(read) `sections` filter only matches `###`, but 15 of 21 memories use `##` — section targeting is unusable, including on gotchas

## Summary

`memory(action="read", sections=[...])` filters on `###` headings only. Exactly one
of this project's 21 memories (`language-patterns.md`) uses `###`. The other 15
structured memories use `##` — 87 headings in total — so the param cannot target any
of them and returns an error that reads as "this memory is unstructured" when it is
not. CLAUDE.md itself routes agents to specific *sections* of `gotchas` and
`conventions`, so the intended progressive-disclosure path is broken for the memories
it is most needed on.

## Symptom (Effect)

```
memory(action="read", topic="gotchas", sections=["MCP Binary Symlink"])
```

```json
{
  "ok": false,
  "error": "no sections matched",
  "hint": "this memory has no ### sections to filter"
}
```

`gotchas.md` has **14** `##` sections, one of which is literally
`## MCP Binary Symlink` at line 25. The hint's claim is true as written and
misleading in effect: the memory is richly sectioned, just not at the level the
filter inspects.

## Reproduction

```
git rev-parse HEAD    # 5875560a, branch experiments
```

1. `memory(action="read", topic="gotchas", sections=["MCP Binary Symlink"])` → the
   error above.
2. `grep -n '^## MCP Binary Symlink' .codescout/memories/gotchas.md` → `25:## MCP
   Binary Symlink`.
3. `memory(action="read", topic="language-patterns", sections=[<any of its 6 ###
   headings>])` → works, confirming the filter itself is functional and the mismatch
   is level-only.

## Environment

Linux, codescout `experiments` @ `5875560a`, MCP stdio transport, project codescout.
Corpus-dependent rather than host-dependent: any project whose memories use `##`
sees this.

## Root cause

Not yet read in the implementation — the level mismatch is established from the
tool's own error text plus the corpus, so the *effect* is confirmed while the
mechanism is inferred. The `sections` param is documented as *"For read. Return only
the listed ### headings (case-insensitive)"*, so the code and its docstring agree;
the defect is that both disagree with every memory the project actually writes.

Corpus census, `.codescout/memories/`:

| | count |
|---|---|
| `.md` memories total | 21 |
| memories with ≥1 `##` and zero `###` | **15** |
| memories with `###` | **1** (`language-patterns.md`, 6 headings) |
| total `##` headings across the corpus | **87** |
| memories with no headings at all | 5 |

So `sections` is usable on 1 of 21 memories and blocked on 15.

## Evidence

### CLAUDE.md routes to sections the filter cannot reach

Project CLAUDE.md sends agents to named sections by name:

- *"the binary symlink gotcha → memory `gotchas` (MCP Binary Symlink)"*
- *"See codescout memory `gotchas` (LSP section)"*
- *"Full command reference … → memory `development-commands`"*

`gotchas.md` is 135 lines / 8564 bytes with 14 sections; `conventions.md` is 70
lines / 5428 bytes with 10. Retrieving one named section therefore costs a full read
of the whole memory. That is the exact cost `sections` exists to avoid, and it is
paid on the most-referenced memories in the project.

### The one working case proves it is a level bug, not a filter bug

`language-patterns.md` (0 `##`, 6 `###`) is the sole memory whose headings the filter
can see. Nothing else about it differs.

## Hypotheses tried

1. **Hypothesis:** the corpus is mostly unstructured, so the param is simply rarely
   applicable.
   **Test:** count `^## ` and `^### ` per memory.
   **Verdict:** rejected — 15 memories carry 87 `##` headings between them. The
   corpus is heavily structured, just one level up from what the filter reads.
2. **Hypothesis:** this is the residue of
   `docs/issues/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md`
   (that bug's "only 2 of 16 topics visible" symptom).
   **Test:** `memory(action="list")` returned 21 topics; `find . -name '*.md'` in the
   memories dir returned 21 files.
   **Verdict:** rejected, and worth recording as a near-miss: an initial `ls | wc -l`
   read 33 and looked like a 12-file gap, but it was counting `.anchors.toml`
   sidecars and the `infra/` + `research/` subdirectories. Counting the wrong thing
   nearly produced a second false bug report. `list` is exactly correct.

## Fix

Not implemented. Three options, in preference order:

1. **Match any heading level** (`##`, `###`, `####`) and keep the existing
   case-insensitive comparison. Smallest change, matches how every author has
   actually written memories, and cannot break `language-patterns.md`.
2. **Match `##` and `###`, and fix the hint** to name the levels it searched plus the
   headings it did find — an error that lists the 14 available section names is
   actionable, where "has no ### sections" sends the reader to the wrong conclusion.
3. Leave the filter and normalise the corpus to `###`. Rejected: 87 headings across
   15 files, it fights every author's instinct (a memory's `#` title is the document,
   so sections are naturally `##`), and it would silently break any external consumer
   reading these files as markdown.

Whichever lands, the param's docstring must stop naming a single level.

## Tests added

None yet — no fix. When fixed, the regression test should assert section retrieval
against a fixture memory using `##` (not `###`), since a `###` fixture is exactly the
one case that already passes and would let the bug survive. Add a second case
asserting the miss-hint enumerates the available headings.

## Workarounds

- Read the whole memory and locate the section in-context: `memory(action="read",
  topic="gotchas")`.
- Or bypass the tool for a targeted grab: `grep -n -A6 '<Section Name>'
  .codescout/memories/<topic>.md` — this is what actually retrieved the MCP Binary
  Symlink content during this pass.

## Resume

Read the `sections` handling in the memory tool's read path (`grep -rn "sections"
src/tools/` and follow to the heading-split helper), confirm the level predicate, and
apply Fix option 1. Then re-run
`memory(action="read", topic="gotchas", sections=["MCP Binary Symlink"])` and expect
the 7-line section rather than an error.

## References

- `.codescout/memories/gotchas.md:25` — `## MCP Binary Symlink`, the section that
  cannot be targeted
- `.codescout/memories/language-patterns.md` — the sole `###` memory, where the param
  works
- `CLAUDE.md` — routes to `gotchas (MCP Binary Symlink)` and `gotchas (LSP section)`
  by name
- `docs/PROGRESSIVE_DISCLOSURE.md` — the budget rationale this param serves

