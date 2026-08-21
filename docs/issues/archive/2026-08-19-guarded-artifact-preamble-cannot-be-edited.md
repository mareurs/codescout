---
id: 195f37475cc452e3
kind: bug
status: mitigated
title: 'BUG: a guarded artifact''s preamble cannot be edited — body_edits is section-scoped and the text before the first heading has no section'
owners:
- marius
tags:
- librarian
- artifact-update
- trackers
- append-only
topic: librarian-api
closed: 2026-08-21
opened: 2026-08-19
owner: marius
related: []
severity: medium
---

# BUG: a guarded artifact's preamble cannot be edited

## Summary

Text between an artifact's frontmatter and its first heading — the **preamble** — is
unreachable by every write surface a guarded artifact has:

- `edit_markdown` is refused outright on a librarian-managed file.
- `artifact(update, patch={body_edits})` requires a `heading` and searches **strictly
  within that section**; the preamble belongs to none.
- `artifact(update, patch={body})` reaches it, but is a whole-body overwrite of a file
  that is often four figures of lines, gated by the shrink guard.

The practical consequence: **a factual claim in a preamble can be appended to, never
corrected.** Session logs put exactly the wrong content there — compaction state, "what is
still open", resume pointers — because it is what a returning reader sees first. That is
also the content that goes stale fastest.

## Symptom (Effect)

`docs/trackers/prompt-surface-compaction-session-log.md` carries a compaction header in
its preamble listing three open bugs by 16-hex id. Two were fixed and archived the same
day, which re-keyed both ids. Re-pointing them is the discipline
`get_guide("tracker-conventions")` requires "in the same commit as the move".

```
artifact(action="update", id="03464a8808345846", patch={body_edits: [{
  heading: "## Index", action: "edit",
  old_string: "Still open: the rendezvous latch bug and `grep`'s silent zero …", …}]})
→ body_edits[0]: old_string not found in section '## Index'.
  scoped_miss_tier: "no_close"
```

The text is four lines *above* `## Index`. There is no other heading to name — the file
has no `#` H1, so `## Index` is the first heading in it.

## Reproduction

1. Take any guarded artifact whose body begins with prose before the first heading (a
   declared `entry_prefix` is enough to guard it).
2. `artifact(action="update", patch={body_edits: [{heading: <first heading>,
   action: "edit", old_string: <text from the preamble>, …}]})`.
3. `old_string not found in section`, with `scoped_miss_tier: "no_close"` — the search
   never leaves the section.

## Environment

2026-08-19, `experiments` at `c38bfd91`. Observed twice in one session on the same file.

## Root cause

Not read in the source. The **boundary** is measured: section-scoped resolution has no
section for pre-heading content, and the refusal is correct behaviour for what it was
asked. Recorded as an observed boundary, not a cited mechanism — the resolver in
`src/librarian/` has not been read for this.

The gap is that no write surface covers the region. `edit_markdown`'s guard and
`body_edits`' scoping are each individually right; together they leave a hole.

## Evidence

Two corrections to the same preamble in one session, both forced to append rather than
edit. The second had to say so in its own text — *"corrections stack here rather than
replacing the text above"* — which is the defect surfacing in the artifact itself.

## Hypotheses tried

1. **Hypothesis:** naming the first heading lets the edit reach text above it.
   **Test:** ran it against the real file.
   **Verdict:** rejected — `scoped_miss_tier: "no_close"`, the search is strictly inside
   the section.

## Fix

Not implemented. Candidates, roughly in increasing cost:

1. **A reserved heading token** for the pre-heading region, e.g.
   `heading: "^"` or `heading: null` with an explicit `region: "preamble"`, resolving to
   "start of body up to the first heading". Smallest change; keeps every existing call
   working.
2. **Fall back to whole-body scope** when `old_string` is not found in the named section
   *and* matches exactly once in the body. Convenient, but it makes the blast radius of a
   typo'd heading larger rather than smaller, and the current failure is at least loud.
3. **Frontmatter for the volatile part.** If what lives in preambles is mostly state
   ("open bugs", "resume here"), it may belong in `extra` where it is queryable, and the
   prose should stop carrying it. Largest change and the one that removes the class.

Option 1 is the direct fix; option 3 is the one that would stop session logs putting
decaying facts in an append-only region.

**Fixed 2026-08-21 (Option 1, reserved sentinel).** `plan_scoped_edit` (shared by
`edit_markdown` and `artifact(update, patch={body_edits})`) now recognizes
`heading: "^"` as the preamble region — content from byte 0 up to the first heading, or
the whole file if it has none. No real heading text can collide with it:
`HeadingInfo::text` always carries its `#` markers, so a bare `"^"` can never be a
heading's own text. Scoped to `action="edit"` only — `insert_before` on the first
heading already covers appending NEW preamble content, per this file's own Workarounds
section; the gap was specifically correcting EXISTING preamble text.

**Status is `mitigated`, not `fixed`: Option 3 (moving volatile state out of preambles
into queryable `extra` frontmatter) is not attempted.** That remains the fix that would
stop this class of staleness at the source, per this file's own framing ("the one that
would have prevented this file's header from carrying three ids that a same-day archive
re-keyed"). Option 1 only makes the region reachable; it does not discourage volatile
content from living there. Not filed as a separate bug — it's a design/policy question
(where should volatile state live), not a capability gap, and doesn't block on this fix.

**Also not done: schema documentation.** `patch.body_edits[].heading` (in
`artifact.rs`'s `input_schema`, ~177 lines) and `edit_markdown`'s own `heading` /
`edits[].heading` schema descriptions don't mention the sentinel yet. Skipped rather than
risking a manual transcription error reproducing that much escaped JSON by hand for a
discoverability nicety; the capability itself is tested end-to-end.

**experiments** SHA: `4dda3c45e4bbd335a689bc2e2ab2adc2fba0ad9f`
patch-id: `5c30bf0537d6e2636721543a86ee32e5573a591a`

## Tests added

None — no fix applied.

## Workarounds

- Append a dated correction before the first heading (`insert_before` works) and mark it
  as superseding what is above. This is what the affected file now does, twice.
- For an unguarded artifact, `edit_markdown` can reach the preamble; the hole is specific
  to guarded ones.
- A whole-body rewrite via `patch={body}` reaches it, at the cost of re-sending the file.

## Resume

Read the `body_edits` resolver in `src/librarian/` to convert the boundary above into a
cited mechanism: find where the section span is computed and whether a "before first
heading" span is representable at all. If it is, option 1 is a small addition; if the
resolver is heading-keyed by construction, option 1 needs a sentinel and the cost estimate
above is wrong.

Then decide between options 1 and 3 — they are not exclusive, and option 3 is the one that
would have prevented this file's header from carrying three ids that a same-day archive
re-keyed.

## References

- `docs/trackers/prompt-surface-compaction-session-log.md` — the affected preamble, and
  the two stacked corrections
- `get_guide("tracker-conventions")` § *Cross-linking* — the "re-point citations in the
  same commit as the move" rule this makes awkward to honour
- `get_guide("librarian")` § *Body Editing Surfaces* — the three surfaces, none of which
  covers the preamble
