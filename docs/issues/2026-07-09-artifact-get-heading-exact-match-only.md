---
status: fixed
opened: 2026-07-09
closed: 2026-07-09
severity: medium
owner: marius
related: [2026-07-09-artifact-get-line-slice-blank-separator-offset]
tags: [librarian, artifact, silent-failure, prompt-surface-consistency]
kind: bug
---

# BUG: `artifact(get, heading=)` requires the full heading text verbatim — bare IDs silently miss

## Summary
`artifact(action="get", id=X, heading="<query>")` and its `headings=[...]` plural form
required an EXACT match (after stripping `#`/whitespace/case) against the full heading
line. For numbered-section trackers whose headings are written
`## SI-23 — <long descriptive title>`, addressing a section by its short id (`"SI-23"`,
`"## SI-23"`) always missed — no error, just `body: ""` with `body_meta.heading_missing:
true` buried in a buffered response the caller has no reason to inspect. This is
inconsistent with `read_markdown`/`edit_markdown`'s documented *fuzzy* heading matching.

## Symptom (Effect)
`artifact(get, id="690f130645515497", heading="SI-23")` on backend-kotlin's
solver-invariants tracker (real heading: `## SI-23 — Even the per-(teacher,group,week)
joint-slot COUNT...`) returned:
```json
{ "body": "", "body_meta": { "heading": "SI-23", "heading_missing": true } }
```
No error — a normal `success` envelope, buffered at 33KB+ (frontmatter + the tracker's
own large `augmentation.params` array), masking that the requested section came back empty.

## Reproduction
Git commit: `a7b2f839ec86d71ef31921ed649ce08df8979fb6` (branch `experiments`).

1. Any artifact with a heading shaped `## <ID> — <title>`.
2. `artifact(action="get", id=<id>, heading="<ID>")` (bare id, no `##`, no title).
3. Pre-fix: `body_meta.heading_missing: true`. Expected: the section's content.

Confirmed against 6 independent historical occurrences in
`/home/marius/work/mirela/backend-kotlin/.codescout/usage.db` spanning 2026-07-01 to
2026-07-09 (`heading="SI-20"`, `headings=["SI-19","SI-20"]`,
`headings=["## SI-22","## SI-1"]`, `heading="SI-29"`, `heading="## SI-33"`) plus 1 in
codescout's own `usage.db` (`heading="## WIN-27"` against a different tracker) — every
single query style (with/without `##` prefix, with/without heading list) missed.

## Environment
codescout MCP server, Rust, `src/librarian/tools/get.rs`. Reproduced against
backend-kotlin's `docs/trackers/solver-invariants.md` and codescout's own tracker corpus.

## Root cause
`get.rs`'s `find_heading_section`/`normalize_heading` (pre-fix, `src/librarian/tools/
get.rs:25-44`) matched via `hs.iter().position(|h| normalize_heading(&h.text) ==
normalized_query)` — **exact equality only** after `#`-stripping and lowercasing. No
prefix, substring, or fuzzy fallback. Meanwhile `src/tools/file_summary/
file_summary.rs::resolve_section_range` already implements a documented "4-tier
matching cascade: exact raw → exact stripped → prefix stripped → substring stripped"
and is the function backing `read_markdown`/`edit_markdown`'s fuzzy heading param —
`get.rs` used a separate, weaker, hand-rolled matcher instead of delegating to it. This
is a prompt-surface consistency gap: the SAME parameter shape (`heading=`, documented
"fuzzy matched" for markdown files) behaves differently depending on whether it's routed
through `artifact(get)` or `read_markdown`/`edit_markdown`.

## Evidence
Live repro against backend-kotlin's real tracker (post-fix, for confirmation):
`artifact(get, id="690f130645515497", heading="SI-23")` → `body_meta` has no
`heading_missing` key; `body` is the real 11-line `## SI-23 — ...` section (see the
sibling investigation's Evidence). Pre-fix behavior reproduced identically via
`heading="SI-23"` on a scratch fixture with headings `## SI-23 — <title>` and
`## SI-2 — <title>` (chosen to also catch a naive substring-match false positive between
"SI-2" and "SI-23" — the file_summary cascade's prefix/substring tiers correctly
distinguish them via tier ordering).

## Hypotheses tried
1. **Hypothesis:** `start_line`/`end_line` (a declared, undocumented-precondition param)
   were silently dropped unless `full=true` was also set — same class of bug, different
   param.
   **Test:** live-reproduced `heading=` and `start_line`/`end_line` separately on the
   same artifact; inspected `body_meta` for both.
   **Verdict:** related but distinct — see the sibling bug file
   (`2026-07-09-artifact-get-line-slice-blank-separator-offset.md`). This file covers
   `heading=`/`headings=` specifically.
2. **Hypothesis (confirmed):** `heading=` matching is exact-text-only, no fuzzy fallback.
   **Test:** traced `normalize_heading`/`find_heading_section` in `get.rs`; compared
   against `file_summary::resolve_section_range`'s documented cascade; confirmed the two
   are separate implementations.
   **Verdict:** confirmed. **Evidence link:** Evidence section above; Root cause section.

## Fix
Deleted the bespoke `normalize_heading`/`find_heading_section` exact-match pair and
replaced `find_heading_section` with a thin delegate to the shared, already-tested
`crate::tools::file_summary::extract_markdown_section` (which itself calls
`resolve_section_range`'s 4-tier cascade). `src/librarian/tools/get.rs` — removed lines
25-27 (`normalize_heading`), replaced lines 26-41 (old `find_heading_section`) with:
```rust
fn find_heading_section(body: &str, query: &str) -> Option<String> {
    crate::tools::file_summary::extract_markdown_section(body, query)
        .ok()
        .map(|r| r.content)
}
```
Both call sites (`a.heading` and the `a.headings` loop) updated to drop the now-removed
`parsed_headings` argument. An ambiguous multi-match (2+ headings satisfying the same
tier) now collapses to `heading_missing: true` via `.ok()?` rather than the old code's
undetected silent "first match wins" — a strict improvement, not a behavior this fix
set out to specifically test (no known tracker has duplicate SI-N-style headings).
Reused code, zero blast radius on `read_markdown`/`edit_markdown` (unchanged).
Uncommitted on `experiments` as of this filing — pending user decision to commit.

## Tests added
- `librarian::tools::get::tests::heading_matches_by_short_id_prefix`
  (`src/librarian/tools/get.rs`) — asserts `heading="SI-23"` against a fixture with
  `## SI-23 — <title>` and `## SI-2 — <title>` returns the SI-23 section only (guards
  against a naive substring match false-positiving on the shared "SI-2" prefix).
- Full existing suite re-run clean: `cargo test --lib` → 2959 passed, 0 failed (includes
  the 4 pre-existing heading tests: `heading_targeted_read_returns_single_section`,
  `heading_missing_sets_meta_flag`, `multi_heading_selector_finds_all_sections`,
  `body_meta_line_count_reflects_returned_body_for_heading` — all still pass unchanged,
  since exact matches are a strict subset of the new fuzzy cascade's Tier 1/2).
- Live-verified post-`cargo rb` + `/mcp` reconnect against the exact original failing
  case (backend-kotlin's `690f130645515497`, `heading="SI-23"`) — now returns the real
  section content, no `heading_missing`.

## Workarounds
Pass the complete heading text (including the `— <title>` suffix) verbatim until the fix
is deployed.

## Resume
N/A — fixed and verified this session. If reopened: check whether a tracker's headings
have duplicate SI-N-style prefixes across sections — the new cascade's ambiguity
handling (silent `heading_missing` on 2+ matches) hasn't been exercised against a real
duplicate-heading fixture, only reasoned about.

## References
- Sibling bug: `docs/issues/2026-07-09-artifact-get-line-slice-blank-separator-offset.md`.
- Reference implementation reused: `src/tools/file_summary/file_summary.rs::resolve_section_range`
  (lines ~218-317), already backing `read_markdown`/`edit_markdown`'s `heading=` param.
- `docs/trackers/tool-usage-patterns.md` (T-N entry pending) — cross-repo usage.db evidence.
