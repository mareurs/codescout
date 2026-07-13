---
id: '9bd3484eca701d4f'
kind: bug
status: open
title: 'BUG: artifact(update) rewrites bug-file frontmatter to canonical shape — emits id/title/topic/time_scope as explicit null, reflows inline tags'
owners: []
tags: []
topic: null
time_scope: null
---

# BUG: artifact(update) rewrites bug-file frontmatter to canonical shape, emitting explicit `null`s

## Severity
low — cosmetic / diff-noise. **Inert**: the catalog derives `id = sha256(abs_path)`, so a `null` id in frontmatter is never read back. Confirmed by `artifact(find, kind="bug")` resolving all affected files to their real ids (e.g. `33e4ae68188322ee`) while the on-disk frontmatter said `id: null`.

## Summary
Any `artifact(action="update", ...)` on a bug file rewrites the whole YAML
frontmatter into the librarian's canonical field set, rather than patching only
the fields requested. Two visible effects:

1. **Explicit nulls for unset fields.** Files that had no `id`/`title`/`topic`/
   `time_scope` key at all come back with `id: null`, `title: null`,
   `topic: null`, `time_scope: null`.
2. **Tag reflow.** Inline `tags: [read_markdown, error-handling]` is rewritten
   as a block sequence.
3. **Key reordering.** `severity`/`owner`/`related` migrate below the canonical
   keys.

## Symptom (Effect)
    -status: open
    -opened: 2026-07-09
    -closed:
    -severity: low
    +id: null
    +kind: bug
    +status: fixed
    +title: null
    +owners: []
    +tags:
    +- read_markdown
    ...

Every touched bug file shows ~10-20 lines of frontmatter churn on top of the
one or two fields actually changed, which buries the real diff during review.

## Discovered by
Committing the 2026-07-13 bug-file ledger drift (commit `61a800af`) — 9 files
whose only intended change was `status: fixed` + a `closed` date + the shipped
SHA carried 115 deletions of pure frontmatter reshuffle.

## Expected
`artifact(update)` should patch only the requested frontmatter keys and
round-trip the rest byte-for-byte, omitting keys that were absent rather than
serializing them as `null`. The `extra` field already documents round-trip-safe
behavior for custom keys; the canonical keys should be no worse.

## Fix idea
In the frontmatter serializer, skip `Option::None` fields on write (serde
`skip_serializing_if = "Option::is_none"`), and preserve the source document's
key order + scalar style where unchanged. Alternatively, do a true YAML merge
against the parsed source rather than re-serializing the struct.

## Workarounds
None needed — the churn is inert. Reviewers should read bug-file diffs with
`git diff -- <file>` and ignore the frontmatter block.

## References
- Commit `61a800af` — the drift commit that surfaced this
- get_guide("tracker-conventions") — frontmatter shape + status vocabulary

