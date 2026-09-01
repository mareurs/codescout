---
id: 5e4d66560433f3e3
kind: bug
status: fixed
title: docs/templates/session-log.md's own example Index/Wins-Index row burns the id it displays, on the very first bootstrap
tags:
- session-log
- append_entry
- tracker-template
- measurement
closed: 2026-08-21
---

## Summary

`docs/templates/session-log.md`'s `## Index` and `## Wins Index` tables ship a
literal example row — `| F-1 | YYYY-MM-DD | ... |` and
`| W-1 | YYYY-MM-DD | ... |` — kept in every fresh copy as shown-shape
documentation. `append_entry`'s high-water scan cannot distinguish a real
claimed id from a `PREFIX-N`-shaped example in prose, so that documentation
row itself consumes `F-1` / `W-1` before any real entry exists. Every tracker
bootstrapped from this template starts its first real entry at `F-2` / `W-2`,
not `F-1` / `W-1`.

## Symptom (Effect)

Observed 2026-08-21 in a downstream project (`claude-plugins`), bootstrapping
`docs/trackers/repo-hygiene-session-log.md` fresh from this template, then
calling `append_entry` for the session's first real friction and first real
win:

- First `append_entry(id_prefix="W", ...)` call on the brand-new,
  never-before-appended tracker returned `W-2`.
- First `append_entry(id_prefix="F", ...)` call likewise returned `F-2`.
- Neither `F-1` nor `W-1` was ever written as a section; only the template's
  own example rows in the Index tables ever "claimed" those ids.

## Reproduction

1. Copy `docs/templates/session-log.md` verbatim to a fresh
   `docs/trackers/<topic>-session-log.md` (no edits).
2. Declare `entry_prefix: [F, W]` in frontmatter (or attach any augmentation)
   to guard the ledger, per `get_guide("tracker-conventions")`.
3. Call `artifact(action="append_entry", id_prefix="W", anchor_heading="## Template for new entries", title="...", body="...")`.
4. Observe the returned id is `W-2`, not `W-1`.

## Root cause

The allocator's high-water mark is computed from every `PREFIX-N`-shaped
token found in the body (per `get_guide("tracker-conventions")` § *Entry
ids*), with no way to distinguish a token that is a real claimed entry from
one that appears purely as documentation of the expected shape. The
template's own `## Index` / `## Wins Index` example rows are, structurally,
indistinguishable from a real pre-written index row — the exact anti-pattern
the template's own "How to use" prose warns against ("do not pre-write index
rows... the allocator counts an id claimed by an index row").

The template's prose already documents the *general* version of this
mechanism (citing `statement-validity-session-log` starting at `F-2`/`W-3`
from mid-use index-row drift), but frames it as something that happens to an
established ledger, not as something guaranteed to happen on the very first
bootstrap. The minimal case is simpler than the documented one: the template
ships the pre-written row itself.

## Evidence

Filed from `claude-plugins:docs/trackers/repo-hygiene-session-log.md` § F-2,
recorded the same session the defect was hit:

```
| F-1 | YYYY-MM-DD | low/med/high | <category> | open | <one-line title> |
```
```
| W-1 | YYYY-MM-DD | low/med/high | <pattern> | <what-would-have-happened> | open |
```

Both are literal `PREFIX-N` tokens per the resolver's `\b[A-Z]{1,3}-\d+\b`
grammar, present in every fresh copy of this template.

## Fix

Applied — exactly the fix this bug's own write-up prescribed. Both example rows in
`docs/templates/session-log.md` (`## Index`: `F-1` → `F-<n>`; `## Wins Index`: `W-1` →
`W-<n>`) now use a placeholder shape `body_claimed_indices`'s `(\d+)` capture cannot
match — verified directly against the actual regex
(`(?:#{1,6}[ \t]+|\|[ \t]*)[`*\[]*{prefix}-(\d+)\b` in
`src/librarian/catalog/augmentation.rs`), not assumed from the citation-resolver's
grammar (a different, if similar, pattern).

**Followed the "apply to any other template" instruction and found it was needed.**
Checked `codescout-companion`'s `reconnaissance-patterns-template.md` and
`tracker-hygiene-log-template.md`, which this bug flagged as "may already be safe."
`tracker-hygiene-log-template.md` genuinely is safe — by design, not luck: it has no
Index table at all (`## HY-N — <title>` headings ARE the index). But
`reconnaissance-patterns-template.md` had the **exact same defect** (`| R-1 | ... |` in
its `## Index` table) — the hedge was wrong there. Fixed in the same pass
(`claude-plugins:21b8776`). Swept every other `*template*.md` in that repo for the
same shape — clean, no further instances.

**A related but out-of-scope observation, not chased here:** `body_claimed_indices`
has no fence-awareness at all (unlike `headings::parse`, which is used elsewhere
specifically to skip fenced code blocks) — it's a bare multi-line regex scan. The
F-N/W-N/R-N entry-template blocks happen to be safe only because their examples use
non-digit placeholders (`F-N`, not `F-1`), not because they're fenced. A tracker whose
fenced *example* content used a real digit-shaped id would hit the same class of bug.
Not investigated further — no known instance, and it's a different code path than this
bug's own report.

- **SHA (experiments):** `2af9e5b76f4d1c8f55c559a62463e80924825189`
- **patch-id:** `1bf3b73fac600767bffd0be762b0d6fa1fa5cbc5`
- **claude-plugins fix:** `21b8776` (main branch — that repo has no documented
  protected-branch/experiments convention; verified before committing there).
## Tests added

`fresh_session_log_template_bootstrap_allocates_f1_and_w1` in
`src/librarian/catalog/augmentation.rs` — reads the real
`docs/templates/session-log.md` file (not a synthetic fixture), bootstraps it fresh
with a declared `entry_prefix`, and asserts the first `allocate_entry_id` call for
each of F and W returns `F-1` / `W-1`. Runs against the actual shipped template, so a
future edit reintroducing a digit-shaped example row fails this test directly rather
than needing a human to notice.

`cargo test --lib augmentation::` — 73 passed. Full `cargo test` + `cargo clippy
--all-targets -- -D warnings` clean on `experiments`.
## Tests added

None — not fixed. Worth a regression test: bootstrap the template fresh,
call `append_entry` once, assert the returned id is `F-1` / `W-1`.

## Workarounds

None needed functionally — the resulting `F-2`/`W-2` numbering is a valid,
citable ledger; this is a cosmetic surprise for anyone auditing "why does
this ledger skip F-1/W-1," not a broken state.

## Resume

Edit `docs/templates/session-log.md`'s `## Index` and `## Wins Index`
example rows to use a non-id-shaped placeholder token. Consider a
regression test per *Tests added*.

## References

- `claude-plugins:docs/trackers/repo-hygiene-session-log.md:F-2` — where this was found and reproduced
- `claude-plugins:docs/trackers/repo-hygiene-session-log.md:W-2` — the session's actual work (unrelated content, same session)
- `docs/templates/session-log.md` — the file to fix
- `get_guide("tracker-conventions")` § *Entry ids* — the general rule this is a bootstrap-time special case of
