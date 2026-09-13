---
id: '6228f4892066b689'
kind: bug
status: open
title: 'BUG: released_history_boundary scans lines without fence awareness, so a changelog''s own example heading can become the boundary'
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
opened: 2026-09-13
owner: marius
related: []
severity: low
---

## Summary

`released_history_boundary` (`src/librarian/tools/audit_doc_refs/severity.rs`) finds the
line dividing a changelog's live claims from its shipped history with a raw
`text.lines()` scan for `## [version]`. It is **not fence-aware**, so a line reading
`## [1.2.0] - 2026-01-01` *inside* a fenced code block — a changelog showing what its own
entries look like — is taken as the boundary itself.

The boundary gates a severity cap: `cap_released_history` drops a `High` missing-ref
finding to `Med` when `md_line >= boundary`, because a released section naming a
since-moved path is a true statement about the past and unfixable by design. A boundary
read off a fenced example moves that gate to the wrong line, in whichever direction the
example sits.

**Undemonstrated.** Found by reading, while fixing the sibling bug below. No corpus
instance is known; this repo's `CHANGELOG.md` has not been checked for a fenced version
heading. Filed because the failure is silent and returns a plausible number rather than an
error.

## Symptom (Effect)

A missing-ref finding in `CHANGELOG.md` reports the wrong severity band:

- Boundary read **too early** (a fenced example above the first real release heading) —
  refs in `[Unreleased]` below that line are capped `Med` when they should gate at `High`.
  `[Unreleased]` is the half that describes the current tree, and `cap_released_history`'s
  own doc comment records that excluding it earned its keep by catching a genuinely broken
  ref.
- Boundary read **too late** — refs in released sections above it keep `High` and red CI on
  history that must not be rewritten to satisfy a linter.

Both directions return a well-formed finding. Nothing errors.

## Reproduction

Not reproduced. The shape a reproduction needs:

```
## [Unreleased]

Example of an entry:

​```
## [1.2.0] - 2026-01-01
### Fixed
- something in src/gone.rs
​```

- a real unreleased change citing `src/also-gone.rs`
```

`released_history_boundary` returns the fenced line's number; the ref below it is then
treated as released history.

## Environment

- `experiments`, verified by reading `severity.rs` on 2026-09-13.
- Applies to any file named `CHANGELOG.md` (case-insensitive); `None` for every other file,
  so the blast radius is one file per repo.

## Root cause

```rust
text.lines().enumerate().find_map(|(i, line)| {
    let rest = line.strip_prefix("## ")?;
    ...
})
```

A line scan over a markdown document, with no fence tracking. The sibling parser in the
same subsystem (`parse_refs`) uses pulldown-cmark and gets fence state for free; this
function predates or sidesteps that and re-derives heading detection by prefix match.

**This is `IC-6` proper, unlike the sibling that pointed here.** The namespace is markdown
headings; the escape a reader would reach for is the fenced block — CLAUDE.md
§ *Parsers Over a Namespace* names the fence as the escape `audit_doc_refs` honours
elsewhere — and this scanner does not implement it. A changelog cannot show its own entry
format without the scanner reading the demonstration as the thing demonstrated.

## Fix

Not implemented. Two candidates, and the choice is not obvious:

1. **Track fences in the line scan.** Cheap, local, and duplicates state pulldown-cmark
   already computes correctly three functions away — a second implementation of fence
   parsing is how the two drift.
2. **Derive the boundary from the pulldown-cmark event stream** that `parse_refs` already
   walks, so heading detection has exactly one implementation. Costs a shared pass or a
   second parse.

(2) is the one that cannot rot, and it is more work than this undemonstrated bug justifies
on its own. Worth doing when something else opens that file.

## Tests added

None — nothing is fixed. A test written now would assert current behaviour and pin the
defect.

## Resume

Noticed while fixing `2026-09-12-every-ref-in-a-fenced-block-is-attributed-to-the-blocks-first-line.md`,
whose defect makes `md_line` drift **downward** — and `md_line` is this function's other
input. Filed separately at that bug's author's suggestion, so it survives their file's
archival: a residual recorded only inside an archived file is one that
`doc(find, status="open")` can never reach.

**The interaction is worth stating even though each half is separately survivable.** Before
the sibling fix, a fence straddling the boundary produced drifted line numbers *and* a
possibly-misplaced boundary, compared against each other. The sibling fix removes one of the
two, which makes this one's failure cleaner rather than rarer.
