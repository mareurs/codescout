---
id: '8784e5739e90fa37'
kind: bug
status: open
title: docs/templates/session-log.md's own example Index/Wins-Index row burns the id it displays, on the very first bootstrap
tags:
- session-log
- append_entry
- tracker-template
- measurement
---

---
kind: bug
status: open
owners: []
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

Not implemented. Replace the example rows' `F-1` / `W-1` tokens with a
non-matching placeholder shape — e.g. `F-<n>` / `W-<n>`, which the
`\b[A-Z]{1,3}-\d+\b` grammar does not match (digit position holds a literal
`<n>`, not `\d+`) — so the documentation no longer doubles as a claim. Apply
the same fix to any other template/archetype shipping a literal `PREFIX-1`
example row (check `reconnaissance-patterns-template.md` and
`tracker-hygiene-log-template.md` in `codescout-companion`, which ship
`R-N` / `HY-N` entry-template blocks with concrete-looking examples — those
use `R-N`/`HY-N` literally rather than `R-1`/`HY-1`, so they may already be
safe, but worth confirming against the same grammar).

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

