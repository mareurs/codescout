---
kind: bug
status: open
title: 'BUG: the pre-edit dirty-check advisory asserts "this session did not write" for edits the session made via edit_code''s multi-file rename'
tags:
- cluster/gate-keyed-on-unobservable-event
- companion-hook
- edit_code
- authorship
---

## Summary
`codescout-companion`'s pre-edit dirty-check advisory opens with *"`<path>` already has
uncommitted changes that this session did not write."* That headline asserts a negative
authorship fact the hook cannot observe. It fires on files the **current session wrote
moments earlier** through `edit_code(action="rename")`, whose LSP rename edits every file
containing a reference — not only the file named in the call. The hook sees a dirty file it
has no record of the session touching, and reports it as foreign.

The advisory's own fine print is accurate and self-limiting (*"This states only what `git
status --porcelain` proves. It does NOT establish a peer: an earlier session of your own
leaves the same trace."*). The defect is that the **headline over-claims relative to the
body**, and the headline is what a reader acts on.

## Symptom (Effect)
Renaming `is_subagent_capable_name` in `src/tools/core/types.rs` produced
`files_changed: 2, total_edits: 10` — the second file being
`src/tools/run_command/tests.rs`, which calls it seven times. The very next `edit_code`
call, targeting that second file, emitted:

```
[cs-hint] `src/tools/run_command/tests.rs` already has uncommitted changes that this
session did not write.
```

This session wrote all of them, two tool calls earlier.

## Reproduction
1. In a clean tree, `edit_code(action="rename", path=A, symbol=S, new_name=S2)` where `S`
   is referenced from file `B`. The response reports `files_changed: 2`.
2. Issue any `edit_code` against file `B`.
3. The advisory fires, naming `B` as carrying changes "this session did not write".

Tree at time of filing: `experiments`, HEAD `844d952b`.

## Environment
`codescout-companion` pre-edit dirty-check hook; codescout `edit_code` LSP rename path.
Any multi-file rename reproduces it — the wider the symbol's reference set, the more files
become falsely foreign for the rest of the session.

## Root cause
Inferred, not measured beyond the two tool calls above. The hook's predicate is `git status
--porcelain` on the target path, which answers *"is this file dirty?"*. The claim it makes
is *"this session did not write it"* — a different question, and one nothing in the hook's
inputs can answer: it holds no record of the session's own writes, and `edit_code`'s
multi-file edits are not announced to it.

That is `cluster/gate-keyed-on-unobservable-event` (IC-2): a gate keyed on an event it
cannot observe substitutes a proxy. Dirtiness is the proxy; authorship is the event.

**Why it is more than cosmetic.** The advisory's suggested next step is to run
`./scripts/file-provenance.py` and *"ask the holder if it is theirs and live"*. On this
false positive there is no holder to ask — the answer is the reader. A reader who follows
it spends a ~7s scan plus a peer round trip to be told nothing, and on a checkout with five
live sessions the more likely outcome is that the advisory is learned to be noise and then
ignored on the call where it is true. The cost is the guard's credibility, not the edit.

## Evidence
- `edit_code` rename response: `{"files_changed": 2, "total_edits": 10}` for
  `is_subagent_capable_name`.
- `git diff -- src/tools/run_command/tests.rs` immediately after: the entire diff is the
  rename — one `use` line and seven call sites — with no unrelated hunk.
- The attribution hook on the same tree gets it **right** in the other direction: a later
  `cargo test` red reported `written by THIS session (aa272bed) — your own uncommitted
  work`. So the session's own writes *are* derivable from the transcript scan; the
  dirty-check advisory simply does not consult it.


### Two further instances — 2026-09-14, and NEITHER is an `edit_code` rename

From session `6be73414-6293-4a4e-95a4-4bada8327f08`. They are worth adding not as a count but
because **neither involved a rename, a multi-file edit, or any file the call did not name** —
which is the trigger § *Summary* and § *Root cause* identify.

| file | how this session had written it, minutes earlier | advisory on the next `edit_file` |
|---|---|---|
| `docs/issues/archive/2026-09-03-il4-deny-hook-will-deadlock-markdown-reads-after-the-fold.md` | `doc(action="update")` — one file, one call | *"already has uncommitted changes that this session did not write"* |
| `docs/issues/2026-09-14-read-only-blocks-five-tool-names-not-the-writes-it-promises.md` | `doc(action="create")`, then `doc(action="update")` | same |

`scripts/file-provenance.py` was run at the time on both and returned
**`MINE … written by THIS session (6be73414)`**. So this is not a reader's belief against a
gate: it is two instruments disagreeing on one tree at one instant, and the dirty-check is the
wrong one.

**What this changes — the trigger is wider than the title.** § *Root cause* attributes the blind
spot to `edit_code`'s LSP rename touching files beyond the one named in the call. That is a real
route and it is not the only one. A **single-file write through the librarian** reaches the same
state: `doc(action="create" / "update")` mutates the file directly and the hook holds no record
of it either. So the predicate is not *"the session wrote a file it did not name"* — it is
*"the session wrote through a path the hook does not mediate"*, and on a docs-heavy session the
librarian is by far the larger of those paths by call volume.

The title and § *Summary* still scope this to `edit_code`'s rename. **Left unrewritten — that is
the owner's call, not a passing contributor's** — but flagged here because a fix scoped to the
rename path would close one route of at least two and the suite would go green.

**Consistent with this section's last bullet, on a third surface.** That bullet notes the
attribution hook derives authorship correctly from the transcript scan while the dirty-check
does not consult it. The same held here in the same hour: `attribute-red` named this session
correctly on a `cargo test` red (*"written by THIS session (6be73414) — your own uncommitted
work"*) while the dirty-check was calling that session's own writes foreign. The data the
advisory needs is not missing from the machine; it is missing from the advisory.

**Not claimed, and no fix attempted here** — these are observations handed to whoever picks the
record up. Offered via `attach-alias-advisory-anyhow`, who retagged this record into
`cluster/gate-keyed-on-unobservable-event` and correctly declined to write into a file they do
not hold.

## Hypotheses tried
1. **Hypothesis:** a peer genuinely edited the file in the interval. **Test:** read the
   full `git diff` for the path and compare against the rename's reported edit count and
   shape. **Verdict:** rejected — every hunk is the rename, and the file was absent from
   `git status` in a check taken minutes before the rename.

## Fix
*Not written.* Two candidates, in preference order:

1. **Soften the headline to its predicate.** *"`<path>` has uncommitted changes"* plus the
   existing fine print, dropping the authorship claim the hook cannot support. Cheapest,
   and loses nothing the hook actually knows.
2. **Consult the same source the red-attribution hook already uses.** That hook resolves
   authorship from Claude transcripts across profiles and names this session correctly. If
   the dirty-check called it, the advisory could say *"written by THIS session"* and
   suppress itself — at the cost of a ~7s scan on every structural edit, which is probably
   why it does not.

Option 1 is likely right: the guard's value is *"look before you commit by pathspec"*, and
that survives dropping the authorship assertion entirely.

## Tests added
None — fix not written. A regression test should assert the advisory string contains no
negative authorship claim, i.e. that it names dirtiness and not a writer. Per this repo's
own rule, assert the **shape** (does the message still name the provenance script as a
second step?) rather than pinning the prose.

## Workarounds
Read `git diff -- <path>` before believing the advisory. If the diff is entirely your own
recent edit, it is this false positive. `edit_code`'s rename response already tells you
`files_changed`, so the set of files about to become falsely foreign is knowable in advance.

## Resume
Edit the advisory text in `codescout-companion`'s pre-edit dirty-check hook to drop the
"this session did not write" clause. Cross-repo: the fix lands in `claude-plugins`, not in
codescout.

## References
- `docs/trackers/issue-clusters.md` — `IC-2`, slug `gate-keyed-on-unobservable-event`
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — the real
  hazard this advisory exists to prevent, which is unaffected by this defect
- `docs/trackers/context-injection-session-log.md` — the work stream this surfaced in
