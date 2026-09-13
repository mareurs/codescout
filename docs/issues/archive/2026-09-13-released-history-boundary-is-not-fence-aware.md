---
id: daca7b2c8b7bf4da
kind: bug
status: fixed
title: 'BUG: released_history_boundary scans lines without fence awareness, so a changelog''s own example heading can become the boundary'
tags:
- cluster/addressing-without-an-escape-hatch
claimed_at: 2026-09-13
claimed_by: eba3d2c6-c1fe-4506-8e2b-a36417e4d9e4
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

**Fixed 2026-09-13 — candidate 1 (track fences in the line scan), reusing the codebase's
existing `FenceState` utility rather than hand-rolling a toggle.**

Shipped `82dc327c` on `experiments`, patch-id
`8f3275afdeb8c05c5e9046371b946803b8372937`.

Mirrors `src/librarian/statements.rs`'s `first_declaration_line` exactly: feed each
trimmed line to a `FenceState`, and treat a line as invisible to the heading match
whenever `feed()` reports it as a delimiter itself or `in_fence()` is still true. No new
fence-parsing logic was written — `FenceState` already had 10 other call sites in this
codebase (`doctor.rs`, `edit_markdown.rs`, `entry_token.rs`, `rule.rs`, the `preview/`
modules, `file_summary.rs`), so this was a matter of reaching for the established tool
rather than inventing a new one.

**Candidate 2 (derive the boundary from the pulldown-cmark event stream `parse_refs`
already walks) was NOT taken.** The bug file itself judged it "more work than this
undemonstrated bug justifies on its own" — still true; candidate 1 fully closes the
failure mode this file describes.

Regression test added mirroring the bug's own Reproduction section almost verbatim: a
changelog whose only real heading is `[Unreleased]`, containing a fenced worked example
of a `## [1.2.0]` entry. Confirmed RED before the fix (`Some(8)`, the fenced line),
GREEN after (`None`). Sibling test
`released_history_boundary_only_applies_to_changelogs` still passes unchanged.

Gate: `fmt-mine.sh` clean; `clippy --workspace --all-targets --features local-embed -D
warnings` clean; lean lane **3529 passed, 0 failed**; default lane **5487 passed, 0
failed**.

Not implemented, and not scheduled — the alternative for the record:

**Derive the boundary from the pulldown-cmark event stream** that `parse_refs` already
walks, so heading detection has exactly one implementation instead of two independently
correct ones. Costs a shared pass or a second parse; the shipped fix's own regression
test is what would have to keep passing if this is ever taken instead.

Worth doing when something else opens this file for a reason unrelated to this bug.

## Tests added

`released_history_boundary_is_not_fooled_by_a_fenced_example`, in
`src/librarian/tools/audit_doc_refs/mod.rs` beside the existing
`released_history_boundary_only_applies_to_changelogs`. Confirmed RED against the pre-fix
scanner, GREEN after.

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
