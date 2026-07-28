---
status: mitigated
opened: 2026-07-17
closed:
severity: low
owner: marius
related: []
tags: [sql, like-escape, dry, tech-debt]
kind: bug
---

# BUG: SQL LIKE-escape idiom is copy-pasted in 5 places with no shared helper

## Summary
The wildcard-escaping needed before interpolating a user string into a SQL
`LIKE` pattern is duplicated inline in at least five call sites across two
different idioms, with no reusable helper. This is the direct enabler of the
unescaped-LIKE class of bug (a citation-resolution instance was just fixed in
`resolve_cite_ref`): a sixth LIKE written from scratch has nothing to route
through, so it defaults to unescaped.

## Symptom (Effect)
No runtime symptom by itself — this is a latent-defect / DRY hazard. The
concrete failure it enables: a `LIKE` pattern built from user input without
escaping treats `%`/`_` in the input as wildcards, causing over-broad or
ambiguous matches (e.g. `resolve_cite_ref("foo_bar.md")` also matching
`fooXbar.md`).

## Reproduction
Static — grep the escape idiom across the tree. Confirmed 2026-07-17 at
entry-graph Stage 2 head `70d16686`.

## Environment
codescout `experiments`, librarian catalog + tools.

## Root cause
Two distinct escaping idioms, neither extracted:

- **Rust-side pattern escaping** (`.replace('\\',"\\\\").replace('%',"\\%").replace('_',"\\_")`
  then a `LIKE ? ESCAPE '\\'` clause):
  - `src/librarian/filter.rs:230-236`
  - `src/librarian/catalog/augmentation.rs:330-337` (`resolve_cite_ref` — the just-fixed instance)
- **SQL-side column escaping** (`REPLACE(REPLACE(REPLACE(col, ...)))` inside the query):
  - `src/librarian/catalog/worktree.rs:86-88`
  - `src/librarian/tools/worktree.rs:181-183`
  - `src/librarian/tools/merge_worktree.rs:71-73, 83-85`

There is no `escape_like_pattern` (or equivalent) `pub(crate) fn`, so each new
LIKE call site re-derives the escaping (or forgets it). Note the two idioms do
different jobs — the Rust-side escapes the *needle* (user pattern) before
binding; the SQL-side escapes the *haystack* (stored column) for matching — so
they are not trivially unifiable into one function, but the Rust-side pattern
escaping (2 sites) clearly should be one shared helper.

## Evidence
Deep-exploration subagent sweep (2026-07-17 self-reflection task) grepping the
escape idiom; each call site cited above. Prior fix of this class:
commit `4b922ac4` ("escape LIKE wildcards in worktree_registration covering()").

## Hypotheses tried
N/A — static duplication confirmed by grep at filing time.

## Fix

**Mitigated 2026-07-17 (intervention I-2, commit `14bd8b55` on `experiments`).** Extracted
`pub(crate) fn escape_like_pattern` into `src/librarian/util.rs` and routed the two Rust-side
needle-escaping sites (`filter.rs`, `augmentation.rs::resolve_cite_ref`) through it. Added the
DRY gate `#[test] like_escape_idiom_is_not_inlined_outside_helper`, which fails CI if the
inline `.replace('%', "\\%")` idiom reappears anywhere except the helper — preventing a sixth
Rust-side copy.

Status is `mitigated`, not `fixed`, because the three SQL-side *haystack*-escaping chains
(`REPLACE(REPLACE(REPLACE(col, ...)))` in `catalog/worktree.rs`, `tools/worktree.rs`,
`merge_worktree.rs`) are a distinct idiom (escaping the stored column inside SQL, not the
Rust-side pattern) and were left untouched — the shared helper does not cover them. A future
pass could unify or gate those too; lower risk since they are column-escaping, not the
user-pattern escaping that caused the original `resolve_cite_ref` defect.
## Tests added

- `escape_like_pattern` unit tests (`src/librarian/util.rs`) — `%`, `_`, `\` each escaped,
  ordinary chars pass through, combined input correct.
- `like_escape_idiom_is_not_inlined_outside_helper` (DRY gate) — asserts the inline idiom
  appears only in the helper; mutation-verified (injecting it into a second file fails CI).
## Workarounds
Copy the idiom from `src/librarian/filter.rs:230-236` verbatim when writing a
new Rust-side LIKE pattern; always add the `ESCAPE '\\'` clause.

## Resume
Extract `escape_like_pattern` in `src/librarian/filter.rs` (or a shared
`catalog`/`util` module), migrate the two Rust-side sites, add the grep gate.
Cross-reference the escape-idiom memory entry (codescout `gotchas`).

## References
- Self-reflection deep-dive, entry-graph Stage 2 (2026-07-17).
- Prior fix: commit `4b922ac4`.
- Related: `docs/issues/2026-07-17-worktree-cites-refusal-materializes-shadow-fork.md`.
