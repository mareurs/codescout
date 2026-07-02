---
status: open
opened: 2026-07-02
closed:
severity: high
owner: marius
related: ["2026-05-26-edit-markdown-insert-after-fuses-heading", "2026-05-26-edit-markdown-scoped-edit-fuses-heading"]
tags: [edit_markdown, markdown, replace, heading-loss, silent-corruption]
kind: bug
---

# BUG: `edit_markdown(action="replace")` deletes the target heading line whenever the new content's first line happens to look like a heading

## Summary
`edit_markdown(action="replace", heading=H, include_subsections=true, content=C)`
silently drops the heading line `H` itself — not just its body — whenever `C`'s
first line parses as *any* markdown heading, even a deeper-level subsection
heading that was never meant to replace `H`. The tool's own documented
contract ("Heading preserved") is violated, and the call returns `"ok"` with
no warning.

## Symptom (Effect)
Called `edit_markdown(action="replace", heading="## [Unreleased]",
include_subsections=true, content="### Added\n\n- ...")` on `CHANGELOG.md`.
Tool returned `"ok"`. Re-reading the file afterward:

```
# Changelog

All notable changes to codescout are documented here.

### Added
...
## [0.14.0] — 2026-05-25
```

The `## [Unreleased]` heading line is gone entirely. `### Added` (my new
content's first line) now sits directly under the `# Changelog` H1, and
`read_markdown`'s heading map no longer lists `## [Unreleased]` at all.

## Reproduction
On `experiments`, HEAD `d7b65de5` (uncommitted change on top):

```
content = "# Changelog\n\nAll notable changes here.\n\n## [Unreleased]\n\n### Added\n\n- old entry\n\n## [0.14.0]\n\n### Added\n\n- v0.14 entry\n"

edit_markdown(action="replace", heading="## [Unreleased]",
              include_subsections=true, content="### Added\n\n- new entry")
```//
Result: `## [Unreleased]` heading is deleted; `### Added` (from the caller's
`content`) lands where `## [Unreleased]` used to be, one level of nesting
shallower than intended.

## Environment
codescout (running MCP binary, sha `d7b65de5` at time of observation).
Pure markdown string manipulation in `perform_section_edit_ext`, no LSP
involved. Any `.md` file, any heading level, triggers as long as
`include_subsections=true` and the replacement content's first line is a
heading.

## Root cause
`src/tools/markdown/edit_markdown.rs:128-131`, inside the `"replace"` arm of
`perform_section_edit_ext`:

```rust
let replace_heading = new
    .lines()
    .next()
    .map(|l| heading_level(l.trim_end()).is_some())
    .unwrap_or(false);
```

This heuristic checks only "does the new content's first line look like *a*
heading of *any* level" — it does not check whether that heading is meant to
*replace* the target heading `H` (same level, same slot) versus simply being
the first *child* heading of `H`'s body (which is exactly what
`include_subsections=true` is for: rewriting a section's subsections). When
`replace_heading` is `true`, the branch a few lines down splices `new` in
starting from `lines[..heading_idx]` (everything *before* the original
heading) — the original heading line itself is never re-included and nothing
in `new` replaces its role, since `new`'s first line is a *different*
(deeper) heading. Net effect: the H2 vanishes, the H3 slides up to fill the
gap in the heading hierarchy.

The two sibling bugs (`2026-05-26-edit-markdown-*-fuses-heading`) are a
related but distinct failure mode in the same file: those silently *fuse*
new content onto the *following* heading via a missing separator newline.
This bug silently *deletes* the *target* heading via a mis-scoped "does this
look like a heading" check. Same file, same general hazard class (heading
integrity), different mechanism.

## Evidence
- Direct reproduction this session against `CHANGELOG.md` (see Symptom).
- Source read of `perform_section_edit_ext`, `src/tools/markdown/edit_markdown.rs:78-260`
  — confirms the `replace_heading` branch never re-emits the original heading
  line when it takes the `true` path.
- The tool's own JSON schema and README-visible docs (`edit_markdown.rs:545`)
  state: *"`replace` | OVERWRITES the entire body... **Heading preserved**"* —
  this reproduction directly contradicts that contract.

## Hypotheses tried
1. **Hypothesis:** this is the same Class-A fusion bug as the two 2026-05-26
   sibling files (missing trailing-newline separator).
   **Test:** read `perform_section_edit_ext`'s `"replace"` arm in full.
   **Verdict:** rejected — the heading isn't fused onto adjacent content, it's
   omitted from the splice entirely; the mechanism is the `replace_heading`
   heuristic misfiring, not a missing `ensure_trailing_newline` call.

## Fix
*(Proposed — not yet implemented.)* `replace_heading` should only fire when
the new content's first heading is at the **same level** as the target
heading (i.e. genuinely replacing `H`'s own line), not merely *a* heading of
*any* level. Something like:

```rust
let replace_heading = new
    .lines()
    .next()
    .and_then(|l| heading_level(l.trim_end()))
    .map(|lvl| lvl == range.level)
    .unwrap_or(false);
```

When `include_subsections=true` and the new content's first line is a
*deeper* heading than `H`, the original heading line must be preserved and
`new` appended as `H`'s new body (the non-`replace_heading` branch already
does exactly this — the fix is just gating entry into the wrong branch).

## Tests added
N/A — bug not yet fixed. Regression test should cover: `replace` with
`include_subsections=true` where `content`'s first line is (a) a
same-level heading (should still replace `H`'s line — today's correct
behavior), (b) a deeper-level heading (should preserve `H`'s line and nest
the new content under it — today's bug), (c) plain body text (unaffected,
already correct).

## Workarounds
When calling `edit_markdown(action="replace", include_subsections=true, ...)`,
prefix `content` with the target heading's own line verbatim if its first
real line would otherwise parse as a heading, OR avoid `include_subsections`
entirely and use `body_edits`-style per-subsection `insert_after`/`edit`
calls instead. Always re-read the heading map after a `replace` with
`include_subsections=true` to confirm the target heading survived.

## Resume
Implement the same-level gate above in `src/tools/markdown/edit_markdown.rs`
(`perform_section_edit_ext`, `"replace"` arm, ~line 128). Add the three-case
regression test described in Tests added. Verify against the two sibling
fusion bugs' existing tests to confirm no regression.

## References
- `src/tools/markdown/edit_markdown.rs:78-260` (`perform_section_edit_ext`), `:128-131` (`replace_heading` heuristic)
- `docs/issues/2026-05-26-edit-markdown-insert-after-fuses-heading.md` — related, same file, different mechanism
- `docs/issues/2026-05-26-edit-markdown-scoped-edit-fuses-heading.md` — related, same file, different mechanism
- Discovered while editing `CHANGELOG.md`'s `[Unreleased]` section ahead of the `experiments`->`master` promotion this session.
