---
id: '9bd3484eca701d4f'
kind: bug
status: fixed
title: 'BUG: artifact(update) rewrites bug-file frontmatter to canonical shape — emits id/title/topic/time_scope as explicit null, reflows inline tags'
owners: []
tags: []
topic: null
time_scope: null
---

# BUG: artifact(update) rewrites bug-file frontmatter to canonical shape, emitting explicit `null`s

## Severity

low — cosmetic / diff-noise. **Inert**: the catalog derives `id = sha256(abs_path)`, so a `null` id in frontmatter is never read back. Confirmed by `artifact(find, kind="bug")` resolving all affected files to their real ids (e.g. `33e4ae68188322ee`) while the on-disk frontmatter said `id: null`.

**Self-healing:** an `id: null` already on disk parses back to `None`, so the fixed serializer simply omits the key on the next update. Files churned before the fix clean themselves up the next time they are touched — no migration needed.
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

**Implemented (2026-07-13).** Root cause was a serde asymmetry in `src/librarian/frontmatter.rs`: every `Frontmatter` field carried `#[serde(default)]`, which governs *deserialization* only (absent key → `None`/empty). Nothing governed the write direction, so an unset `Option` was faithfully re-emitted as an explicit `null` and an empty `Vec` as `[]`.

Fix: pair each `default` with a `skip_serializing_if` — `Option::is_none` for the six `Option` fields (`id`, `kind`, `status`, `title`, `topic`, `time_scope`) and `Vec::is_empty` for `owners`/`tags`. `write()` is the single choke point for `create.rs`, `update.rs`, and `update_in_place`, so one change covers every call site.

**Scope note — effect (1) fixed, plus one the report missed; effects (2) and (3) are inherent and now proven harmless.** The failing test surfaced churn the original report didn't mention: `owners: []` and `tags: []` were *also* being injected into files that never had those keys (same defect class), and are now skipped too.

Effects (2) tag reflow and (3) key reordering are NOT fixed and cannot be by this approach: serde re-serializes the struct, so it cannot preserve the source document's key order or inline-vs-block scalar style. Preserving those would need a format-preserving YAML layer (no good Rust equivalent of ruamel.yaml exists). What makes this acceptable is that the remaining normalization is **one-time, not recurring** — a file is reshaped on its first update and is byte-stable from then on. That property is now pinned by a regression test (`update_in_place_is_idempotent_after_first_normalization`), so the churn cannot start recurring silently.
## Workarounds
None needed — the churn is inert. Reviewers should read bug-file diffs with
`git diff -- <file>` and ignore the frontmatter block.

## References
- Commit `61a800af` — the drift commit that surfaced this
- get_guide("tracker-conventions") — frontmatter shape + status vocabulary
