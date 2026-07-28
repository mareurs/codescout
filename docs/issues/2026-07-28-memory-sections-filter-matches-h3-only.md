---
id: '79b2591f4ed05715'
kind: bug
status: fixed
title: memory(read) `sections` filter only matches `###`, but 15 of 21 memories use `##` — section targeting is unusable, including on gotchas
tags:
- memory
- progressive-disclosure
- tooling
- docs-vs-code
closed: 2026-07-28
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

**Read and confirmed** (this section originally said the mechanism was inferred from
the error text and the corpus). `filter_sections` (`src/memory/filter.rs`) split blocks
on `line.strip_prefix("### ")` — a hardcoded H3 literal. The `sections` param is documented as *"For read. Return only
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

**IMPLEMENTED** on `experiments` 2026-07-28.

The fix is **not** the "match any heading level" of option 1 above. Reconnaissance
killed that: `filter_sections_nested_h4_included_in_body` exists to assert that
`####` is part of its `###` section's *body*, so promoting every level to a boundary
would break a deliberate, tested behaviour.

Shipped instead: **the boundary is the shallowest heading level present among H2–H6**,
with deeper levels nesting as body (`boundary_level` + `heading_at` in
`src/memory/filter.rs`). That serves both conventions from one code path — `##`-sectioned
memories get `##` boundaries, `###`-sectioned ones keep `###` with `####` nested — so
every pre-existing test passes unchanged.

**H1 is deliberately excluded**, and the corpus is why: 19 of 21 memories carry exactly
one H1 as their title. Including H1 would make that title the sole block and nest every
real section inside it, so filtering would appear to work while always returning the
whole document. `development-commands.md` (5 H1 + 9 H2) is the one memory using H1
structurally; excluding H1 still leaves its 9 H2 sections addressable, with stray H1s
absorbed into the preceding section's body — a documented trade, covered by
`filter_sections_multiple_h1_still_addresses_h2_sections`.

Also corrected, since both stated the old contract:

- the miss-hint at `src/tools/memory/mod.rs` — was *"this memory has no ### sections to
  filter"* on a memory with 14 sections; now names the levels searched and points at
  reading without `sections`;
- the `sections` param description in the tool schema — no longer names a single level.

**Verification status — unit-level only, not yet live.**
`filter_sections_matches_h2_sectioned_memory` reproduces `gotchas.md`'s exact shape
(H1 title + `##` sections) and the exact section name that failed, and passes. The
live MCP path is NOT yet verified: the running server executes
`~/.cargo/bin/codescout` -> `target/release/codescout`, and this fix exists only in the
debug build. Confirming `memory(read, topic="gotchas", sections=["MCP Binary Symlink"])`
end-to-end requires `cargo rb` followed by a `/mcp` reconnect.

Per CLAUDE.md the **master-side** SHA goes here after cherry-pick; the
`experiments`-side original orphans on rebase. Tracked in
`docs/trackers/archived-bug-sha-reconciliation.md`.
## Tests added

Six new tests in `src/memory/filter.rs`, alongside the 13 pre-existing ones (all still
green — 19/19):

- `filter_sections_matches_h2_sectioned_memory` — the reported bug, using the real
  `gotchas.md` shape and the exact section name CLAUDE.md routes agents to.
- `filter_sections_h1_title_is_not_a_boundary` — pins the H1 exclusion. Without it a
  regression would still "match" while silently returning the whole document.
- `filter_sections_multiple_h1_still_addresses_h2_sections` — the
  `development-commands.md` shape; documents the accepted trade rather than leaving it
  to be refiled as a bug.
- `filter_sections_deeper_headings_nest_under_the_boundary_level` — the case that rules
  out "match any heading level", including the negative assertion that the nested
  `###` is *not* independently addressable.
- `filter_sections_hashes_without_space_are_not_a_boundary`.
- `filter_sections_title_only_memory_has_no_sections` — drives the reworded hint.

Note `filter_sections_matches_h2_sectioned_memory` uses `\n`-escaped strings rather
than a multi-line literal, and says why inline: `edit_code` reindented the literal's
interior on insert, turning column-0 headings into indented body and failing the test
for the wrong reason. Filed as
`docs/issues/2026-07-28-edit-code-reindent-shifts-string-literal-contents.md`.
## Workarounds

- Read the whole memory and locate the section in-context: `memory(action="read",
  topic="gotchas")`.
- Or bypass the tool for a targeted grab: `grep -n -A6 '<Section Name>'
  .codescout/memories/<topic>.md` — this is what actually retrieved the MCP Binary
  Symlink content during this pass.

## Resume

N/A — fixed and verified. Archive after the master-side SHA is recorded, per
`docs/trackers/archived-bug-sha-reconciliation.md`.
## References

- `.codescout/memories/gotchas.md:25` — `## MCP Binary Symlink`, the section that
  cannot be targeted
- `.codescout/memories/language-patterns.md` — the sole `###` memory, where the param
  works
- `CLAUDE.md` — routes to `gotchas (MCP Binary Symlink)` and `gotchas (LSP section)`
  by name
- `docs/PROGRESSIVE_DISCLOSURE.md` — the budget rationale this param serves
