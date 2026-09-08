---
kind: bug
status: open
tags:
- cluster/unclassified
closed: null
opened: 2026-09-08
owner: marius
related: []
severity: low
---

# BUG: the memory concept page's workflow never mentions staleness or anchors, so the correct refresh procedure is written down in exactly one place

## Summary

`docs/manual/src/concepts/memory.md` is the page a reader reaches for "how does memory
work". It covers layout and a "Typical Workflow" and never mentions that memories go
stale, that anchors exist, or that `workspace(action="status")` reports staleness. The
whole procedure lives only in the tool reference page,
`docs/manual/src/tools/memory.md`. A reader who follows the concept page to completion
has a correct mental model with one mechanism missing — and it is the mechanism that
tells them their memory has decayed.

## Symptom (Effect)

Follow `docs/manual/src/concepts/memory.md` end to end and you learn to write, read,
list and delete memories. You do not learn that:

- memories carry a `<topic>.anchors.toml` sidecar of `{path, hash}` pairs derived from
  their own prose,
- `workspace(action="status")` buckets every topic `fresh` / `stale` / `untracked`,
- `write` re-anchors automatically while `refresh_anchors` only re-hashes,
- reviewing before `refresh_anchors` is the difference between recording a check and
  faking one.

None of that is discoverable from the page a reader lands on first.

## Reproduction

Read `docs/manual/src/concepts/memory.md`. Search it for "stale", "anchor",
"refresh_anchors" — no hits.

## Environment

codescout 0.15.0, branch `experiments`, at `09ecb58d`.

## Root cause

Not a code defect: the concept page and the tool reference page were written to
different scopes, and staleness landed only in the latter. The consequence is a
single-point-of-failure documentation surface for a mechanism with a silent failure
mode — `refresh_anchors` used as a substitute for review reports every topic fresh while
the content stays stale, and nothing anywhere errors.

Measured 2026-09-08 while establishing the refresh procedure: the correct sequence
(review; if accurate `refresh_anchors`; if outdated `write`, which re-anchors) was found
in `docs/manual/src/tools/memory.md` and nowhere else.

## Evidence

The three surfaces that describe the memory system, and what each covers:

```
docs/manual/src/concepts/memory.md   layout, typical workflow      -- no staleness, no anchors
docs/manual/src/tools/memory.md      per-action reference          -- the full correct procedure
src/tools/memory/mod.rs long_docs    two memory systems, topics    -- actions, not staleness
```

## Hypotheses tried

1. **Hypothesis:** the concept page deliberately excludes operational detail and the
   split is intentional.
   **Test:** the page does document the dashboard editor, which is operational.
   **Verdict:** deferred — the boundary may still be intentional, but it is not "no
   operational detail". Worth a moment's thought before adding, rather than assuming
   the omission is an oversight.

## Fix

Not started. Smallest useful change is a short section on the concept page naming
staleness and pointing at the tool page for the procedure — a pointer, not a second
copy, since two copies of a procedure drift.

Note this interacts with the two sibling bugs filed the same day: the dashboard editor
bypasses the anchor update entirely, and the tool schema misdescribes
`refresh_anchors`. Fixing the page without fixing those documents a procedure the UI
does not follow.

## Cluster — why `unclassified` rather than a forced fit

`IC-11` (`doc-contradicted-by-code`) is the obvious reach and is wrong: nothing here is
contradicted. The page is **silent**, and silence and contradiction have different
remedies — a contradiction is repaired by correcting a sentence, an omission by deciding
whether the sentence belongs on that page at all (see the deferred hypothesis above,
which is not yet resolved). `IC-18` (`selector-narrower-than-its-population`) is the
other near miss and needs a selector returning a well-formed answer over a subset; a
prose page is not one. Filed under the escape hatch rather than corrupting either
class's count.

## Tests added

None. `audit_doc_refs` and the retired-call-form gates check that documentation does not
say WRONG things; nothing checks that a page covers its subject. That is the harder
class and probably not worth a mechanism for one instance.

## Workarounds

Read `docs/manual/src/tools/memory.md` § `action: "refresh_anchors"` for the correct
procedure. `get_guide` has no memory topic.

## Resume

Decide whether the concept/reference split is deliberate (see the deferred hypothesis)
before adding anything. If adding: one paragraph on the concept page naming staleness
and linking the tool page, and no restatement of the procedure itself.

## References

- `docs/manual/src/concepts/memory.md`, `docs/manual/src/tools/memory.md`
- Siblings filed the same day: the dashboard bypass, and the `refresh_anchors` schema drift
- Found while re-deriving all 16 stale memories, `09ecb58d`
