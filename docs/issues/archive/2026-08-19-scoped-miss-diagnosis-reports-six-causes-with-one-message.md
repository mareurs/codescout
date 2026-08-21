---
kind: bug
status: fixed
tags:
- edit-markdown
- artifact
- error-quality
- recoverable-error
closed: 2026-08-21
opened: 2026-08-19
owner: marius
related: []
severity: medium
---

# BUG: `diagnose_scoped_miss` has six bail-out causes and reports all six identically, so its closest-text recovery never reaches the caller

## Summary

`diagnose_scoped_miss` (`src/tools/markdown/edit_markdown.rs:1051`) exists to turn an
`old_string` miss into an actionable error: it finds the closest window in the section
and returns `want:` / `have:` snippets, with a `whitespace_invisible` tier that renders
invisible characters. That machinery works. It just never engages.

Six separate conditions short-circuit to `no_close("")` **before** the closest-window
search, or reject its result — and every one passes the *same empty string* to the one
parameter designed to tell them apart. The caller receives a single message,
`"The text must match exactly (whitespace-sensitive). "` (note the trailing space where
`extra_note` should be), for six different problems with six different fixes.

Measured on `.codescout/usage.db`, 30 hours to 2026-08-19T19:00Z: **7 of 7**
`edit_stale_match` failures on the `artifact` `body_edits` path returned the bare
`no_close` form. The tiered recovery had a **0/7 engagement rate**. Three of the seven
landed within one minute of each other — the blind-retry signature.

## Symptom (Effect)

The agent is told the text "must match exactly" but not what the section actually
contains, and cannot distinguish:

- "your `old_string` is empty"
- "your `old_string` is over 8 KB"
- "the section is over 400 lines, so I did not look"
- "the section is over 64 KB, so I did not look"
- "your `old_string` has more lines than the whole section"
- "I looked, and nothing scored above 0.5 similarity"

The first two are caller errors. The middle three mean *the tool declined to search* —
which is not a fact about the file at all. The last is the only one where the stock
hint ("re-read the current section text and retry") is the right advice. Because they
are indistinguishable, the agent re-reads and retries against causes a re-read cannot
fix, and the retry fails identically.

## Reproduction

```
artifact(action="update", id=<any large tracker>, patch={body_edits: [{
    heading: "## Index", action: "edit",
    old_string: "<any text not in the section>", new_string: "x"}]})
```

Observed messages, all seven from the 30-hour window (headings only, truncated):

```
body_edits[0]: old_string not found in section '## What this is, and what it is not'. The text must match exactly (whitespace-sensitive). 
body_edits[0]: old_string not found in section '## Local-trace evidence (FND-11..12)'. ...
body_edits[0]: old_string not found in section '## Scope & boundary'. ...
body_edits[0]: old_string not found in section '## Findings'. ...
body_edits[0]: old_string not found in section '## Index'. ...
body_edits[0]: old_string not found in section '## A-27 — `artifact_augment` states one rule seven times ...'. ...
body_edits[0]: old_string not found in section '## Index'. ...
```

Every one ends at `(whitespace-sensitive).` plus a space. No `want:`, no `have:`, no tier.

## Environment

Branch `experiments`, commit `c86c5a68` era. Platform-independent — no `cfg` gates.
Reached through `artifact(action="update", patch={body_edits})`, which routes to
`perform_scoped_edit` via `src/librarian/tools/update.rs:229`; `edit_markdown`'s own
`action="edit"` shares the same diagnosis path.

## Root cause

`src/tools/markdown/edit_markdown.rs:1057-1093`. The closure is *parameterised* for
this and every call site declines to use it:

```rust
let no_close = |extra_note: &str| { /* ... "{extra_note}" ... */ };

if old_string.is_empty()
    || old_string.len() > OLD_STRING_CAP      // 8192
    || lines.len() > SECTION_LINE_CAP         // 400
    || section.len() > SECTION_BYTE_CAP       // 65_536
{
    return no_close("");                      // four causes, one message
}
...
if n == 0 || n > lines.len() { return no_close(""); }   // fifth
...
if best_score < SIM_THRESHOLD { return no_close(""); }  // sixth (0.5)
```

Note the first four are collapsed into a single `if` with `||`, so even the *code*
cannot report which fired without being restructured.

The caps are defensible as performance guards — the window scan is O(lines × len) —
but a guard that silently declines to search, and then reports its silence in the same
words as a genuine no-match, converts a performance decision into a correctness-shaped
error message.

## Evidence

`.codescout/usage.db`, `tool_calls` where `err_family='edit_stale_match'` and
`tool_name='artifact'`, 30 hours to 2026-08-19T19:00Z: 7 rows, 7 bare `no_close`.
Same window, all tools: 175 errors in 5087 calls (3.44%), of which 20 are a repeat of
the same `(tool, err_family)` inside five minutes; `artifact`/`edit_stale_match`
contributes 3 of those 20.

The capability is not hypothetical — `whitespace_invisible` and the closest-text tier
are implemented at `:1114-1131` and covered by tests at
`src/librarian/tools/update.rs:2197`. They are simply unreachable for these inputs.

## Hypotheses tried

1. **Hypothesis:** the closest-text recovery was never built for the `artifact` path,
   only for `edit_file` (whose equivalent shipped in `857f9fc5`, archived as
   `docs/issues/archive/2026-06-04-edit-file-old-string-miss-no-closest-match.md`,
   which explicitly deferred "batch (`edit[]`) path").
   **Test:** read `perform_scoped_edit` -> `plan_scoped_edit` -> `diagnose_scoped_miss`.
   **Verdict:** rejected. The recovery exists on this path and is tier-aware. The defect
   is that it never engages, which is a strictly worse failure than absence — a reader
   of the code concludes the problem is solved.

2. **Hypothesis:** the seven misses are `SECTION_LINE_CAP` (400) hits on large trackers.
   **Test:** not run. Cannot be determined from the emitted message — which is the bug.
   **Verdict:** undecidable with current instrumentation. Recording the tier would settle
   it retroactively, since `err_family` is already persisted per call.

## Fix

Give each bail-out its own note, and split the collapsed `if`. The closure already
takes the argument:

```rust
if old_string.is_empty() { return no_close("old_string is empty."); }
if old_string.len() > OLD_STRING_CAP {
    return no_close("old_string exceeds 8 KB, so no closest-match search was run — \
                     target a smaller, unique anchor.");
}
if lines.len() > SECTION_LINE_CAP || section.len() > SECTION_BYTE_CAP {
    return no_close("this section is too large to scan for a closest match \
                     (>400 lines or >64 KB), so none was attempted — re-read the \
                     section and copy an exact anchor.");
}
```

and give the genuine-no-match case a distinct note from the declined-to-search cases.
Also set `scoped_miss_tier` to a distinguishing value per cause rather than the single
`"no_close"` — the field is already plumbed to callers and persisted, so the next
30-hour window would report which cause dominates instead of leaving it undecidable.

Deliberately NOT proposed: raising the caps. The caps are a performance guard, and the
defect is that their effect is unreportable, not that they are wrong. Raising them would
mask the ambiguity rather than remove it.

**Fixed 2026-08-21.** Gave each of the six causes its own `scoped_miss_tier` value and
message — `old_string_empty`, `old_string_too_large`, `section_too_many_lines`,
`section_too_many_bytes`, `old_string_longer_than_section`, `no_similar_match` (renamed
from the ambiguous `no_close`, which stays gone — no caller matched on that literal string,
only on `"visible_drift"` in `update.rs:450`, which is unchanged). Split the collapsed
four-way `||` and dropped the dead `n == 0` branch (unreachable once `old_string.is_empty()`
already returns). Bail-out precedence order preserved from the original code. Gate green:
`cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (4426 passed, 46
ignored, 0 failed).

**experiments** SHA: `8814c0afb7ef974fb41333bd450706e69d31dc2c`
patch-id: `e25c6910eb007c35426379041b2750023975390b`

## Tests added

**Fixed 2026-08-21.** Five new tests in `src/tools/markdown/tests.rs`: one per
previously-undistinguished cause (`diagnose_empty_old_string_gets_own_tier`,
`diagnose_section_too_many_lines_gets_own_tier`, `diagnose_section_too_many_bytes_gets_own_tier`,
`diagnose_old_string_longer_than_section_gets_own_tier`), plus
`diagnose_causes_produce_pairwise_distinguishable_messages` as the mutation guard
against the collapse creeping back — it asserts all six messages are pairwise `!=`.
Two existing tests updated to expect the new tier names instead of the shared `no_close`
(`diagnose_giant_old_string_bails_to_no_close_cheaply` → `old_string_too_large`;
`diagnose_no_close_nudges_heading` → `no_similar_match`).

## Workarounds

Re-read the section with `artifact(action="get", id=..., heading=...)` and copy the
anchor verbatim. This works for the genuine-no-match cause and is useless for the three
declined-to-search causes — but the message does not say which one you have, so the
workaround is applied blind. That is the friction being reported.

## Resume

Start at `src/tools/markdown/edit_markdown.rs:1051` (`diagnose_scoped_miss`). Split the
four-way `||` at :1068, give each `no_close` call a note, and widen `scoped_miss_tier`.
Then re-run the usage.db query in § Evidence over a fresh window to see which cause
actually dominates — it is currently unknowable, and that is the point of the fix.

Do not re-derive whether the recovery exists: it does, and works. The archived
`edit_file` bug is a sibling, not a duplicate — it fixed a different tool's path and its
own § Fix records the batch path as deferred.

## References

- `src/tools/markdown/edit_markdown.rs:1051-1131` — `diagnose_scoped_miss`
- `src/librarian/tools/update.rs:229` — the `artifact` body_edits call site
- `docs/issues/archive/2026-06-04-edit-file-old-string-miss-no-closest-match.md` — the
  sibling fix for `edit_file`, which deferred the batch path
