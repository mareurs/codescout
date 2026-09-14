---
kind: bug
status: mitigated
title: 'BUG: the pre-edit dirty-check reports any write it did not mediate as another session''s — edit_code renames and librarian writes alike'
tags:
- cluster/gate-keyed-on-unobservable-event
- companion-hook
- edit_code
- authorship
---

## Summary

`codescout-companion`'s pre-edit dirty-check advisory opens with *"`<path>` already has
uncommitted changes that this session did not write."* That headline asserts a negative
authorship fact the hook cannot observe, and it fires on files the **current session wrote
moments earlier**.

**The predicate is "the session wrote through a path the hook does not mediate" — not
"`edit_code` renamed something".** This file was opened on the rename route and titled for
it; that scoping was too narrow, and the correction is owed to sessionId
`6be73414-6293-4a4e-95a4-4bada8327f08`, who measured two instances involving no rename, no
multi-file edit, and no file the call did not name (§ *Evidence*). At least two routes
reach the identical state:

| route | why the hook has no record |
|---|---|
| `edit_code(action="rename")` | the LSP rename edits every file holding a reference, not only the one named in the call |
| **`doc(action="create" / "update" / "append_entry")`** | the librarian writes the file directly, server-side; no `PreToolUse` payload ever describes the write |

**On a docs-heavy session the librarian route is by far the larger of the two by call
volume**, which inverts how this record originally read: the rename is the exotic case and
the one the title named.

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

Inferred, not measured beyond the tool calls recorded in § *Evidence*. The hook's predicate
is `git status --porcelain` on the target path, which answers *"is this file dirty?"*. The
claim it makes is *"this session did not write it"* — a different question, and one nothing
in the hook's inputs can answer: **it holds no record of the session's own writes**.

That is the whole of it, and it is why the route does not matter. Any write that does not
pass through a `PreToolUse` payload the hook sees is invisible to it — `edit_code`'s
multi-file rename because the extra files are never named, the librarian's `doc(create /
update / append_entry)` because the server writes the file itself. Enumerating routes is
useful for reproduction and misleading as a cause: the hook is not failing to track two
specific paths, it is tracking none.

That is `cluster/gate-keyed-on-unobservable-event` (IC-2): a gate keyed on an event it
cannot observe substitutes a proxy. Dirtiness is the proxy; authorship is the event.

**Why it is more than cosmetic.** The advisory's suggested next step is to run
`./scripts/file-provenance.py` and *"ask the holder if it is theirs and live"*. On this
false positive there is no holder to ask — the answer is the reader. A reader who follows
it spends a ~7s scan plus a peer round trip to be told nothing, and on a checkout with five
live sessions the more likely outcome is that the advisory is learned to be noise and then
ignored on the call where it is true. The cost is the guard's credibility, not the edit.

**The data is not missing from the machine, only from the advisory.** Measured the same
hour on the same tree: `scripts/file-provenance.py` returned `MINE … written by THIS
session`, and `attribute-red` named the session correctly on a `cargo test` red, while the
dirty-check was calling that same session's writes foreign. Two instruments disagreeing at
one instant, and the dirty-check is the wrong one.
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
| `docs/issues/archive/2026-09-14-read-only-blocks-five-tool-names-not-the-writes-it-promises.md` | `doc(action="create")`, then `doc(action="update")` | same |

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

**Option 1 shipped 2026-09-14** — `claude-plugins:9169527`, patch-id
`f909298548df5b1631c84a2ac281d1cca4468e3d`.

The headline now reads *"`<path>` has uncommitted changes that no edit through this hook
accounts for"*, followed by a line stating that this is a claim about **this hook's
records** and listing the routes that leave the same trace. The move is from a claim about
the WORLD, unobservable from a `PreToolUse` payload, to a claim about the hook's own marker
set, which it holds. The predicate is untouched and was correct throughout.

**What is NOT fixed, and why the status is `mitigated` rather than `fixed`.** The advisory
still FIRES on a session's own unmediated writes — it no longer mislabels them. The
acceptance criterion stated above (*"call `doc(action="update")` on a clean file, then
`edit_file` it, and the advisory must stay silent"*) is **not met, and is not reachable
from this hook**: `doc()` addresses artifacts by **id, not path** —
`doc(action="update", id="dd98…")` carries no path — so there is nothing to hash a marker
from without catalog access the hook does not have. That criterion was written before the
obstacle was known; it is left standing rather than quietly relaxed, because the gap between
what was specified and what shipped is the part a later reader needs.

Silence on the librarian route therefore remains open, and would need either a marker
written by something that can resolve an artifact id to a path, or the hook shelling into
`codescout`. Neither was attempted.

Option 2 (consult the transcript-scanning attribution source) remains available and
unattempted. It would buy a stronger claim — *"written by THIS session"* and suppression —
at a ~7s scan on every structural edit.
## Tests added

Two suites cover this hook, in different trees, and the first search found only one —
recorded as `context-injection-session-log:F-8`.

`claude-plugins:codescout-companion/hooks/pre-edit-dirty-check.test.sh` — four new
assertions:

| assertion | direction |
|---|---|
| `headline claims only what the hook observes` | **absence** of the authorship claim |
| `headline states the observable fact` | presence of the replacement |
| `body still names the instrument that can answer authorship` | remedy shape |
| `body still says whose records the claim is about` | remedy shape |

The absence assertion is the load-bearing one: every pre-existing test in that file is
about the **predicate**, which was correct throughout and stayed correct through the
defect. A suite of predicate tests cannot see a headline that over-claims, so nothing would
have reddened — which is why the defect survived a passing suite for as long as it did.

`claude-plugins:tests/test-pre-edit-dirty-check.sh` — one assertion **repaired rather than
added**. Its comment stated the intent (*"It does NOT establish a peer"*) while its code
pinned the very sentence carrying the defect, so the fix reddened it. It now tests the
intent in both directions, because each half alone is monotone the wrong way: absence
alone passes on an empty message, presence alone passes on a message that scopes its claim
and then asserts authorship anyway.

Mutation-verified, with each mutation asserted to have APPLIED before its result was read
(`context-injection-session-log:F-7`): restoring the authorship headline kills 2, dropping
`file-provenance.py` kills 1, dropping the records-scoping line kills 1.

**Not tested:** that the advisory goes silent after a librarian write. See § *Fix* — it is
not reachable, and was left unasserted rather than weakened into something passable.
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
