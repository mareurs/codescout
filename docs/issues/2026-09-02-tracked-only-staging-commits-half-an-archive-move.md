---
id: '2a9e782b58f6844c'
kind: bug
status: open
title: 'BUG: tracked-only staging commits half an artifact(move) archive, silently undoing it'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
- librarian
- artifact-move
- git
- archive-flow
topic: bug-file archive flow
closed: ''
opened: 2026-09-02
owner: marius
related: []
severity: high
---

## Summary

`artifact(action="move")` archives a bug file as a **tracked deletion plus an untracked
addition**. Every selector that stages "everything that changed" — `git add -u`,
`git commit -a` — has an index-entry precondition, so it sees only the tracked half: it
commits three deletions and none of the three archive twins. The archive is silently
undone, the bug files vanish from `docs/issues/` altogether, and the commit is green.

`get_guide("tracker-conventions")` § *Bug files* prescribes the move and the citation
sweep in detail and never mentions staging. It is the second member of `IC-18` to be
about this same recipe.

## Symptom (Effect)

After three `artifact(action="move")` calls, `git status --short`:

```
 D docs/issues/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md
 D docs/issues/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md
 D docs/issues/2026-09-02-lockfileex-refuses-an-append-only-handle-on-windows.md
?? docs/issues/archive/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md
?? docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md
?? docs/issues/archive/2026-09-02-lockfileex-refuses-an-append-only-handle-on-windows.md
```

The three deletions were **703 of the working tree's 953 deletions** at that moment, so
the diffstat of a tracked-only commit looks like a large, deliberate removal rather than
an accident.

**Second effect, and the one that actually surfaced it.** While the halves are split,
`cargo test --test issue_clusters` reds 2 of 18 — three class counts each one short:

```
gate-keyed-on-unobservable-event      23 -> 22
hint-composed-without-the-request      2 -> 1
repro-env-diverges-from-gate-env      13 -> 12
```

Staging both halves restored all three **without editing a number**, which is what
identifies the shortfall as the staging split rather than a real count change.

## Reproduction

At `7d2b3ee7^`, on `experiments`:

1. `artifact(action="move", id=<bug>, new_rel_path="docs/issues/archive/<same-name>.md")`
2. `git status --short` — observe the ` D` / `??` pair.
3. `git add -u && git diff --cached --stat` — the deletion is staged; the twin is not.

## Environment

Linux, codescout 0.15.0, branch `experiments`, shared checkout with three concurrent
agent sessions. Reproduces regardless of session count — concurrency is not a factor,
it only raises the odds that someone else runs the staging step.

## Root cause

Two mechanisms compose, each correct alone.

1. **`artifact(action="move")` re-keys by path.** `id = sha256(abs_path)`, so the move
   necessarily writes a new file and removes the old one. Git sees an unrelated
   deletion and creation; rename detection only applies once **both** sides are staged.
2. **`git add -u` is defined over paths that already have an index entry.** An
   untracked file has none, so it is not a skipped candidate — it is never enumerated.
   That is why nothing is reported: there is no count of what was missed.

*Measured 2026-09-02:* the ` D`/`??` status pair, the 703-of-953 diffstat
(`git diff --stat` over the three paths), and the 18/18 green after staging both halves
were all observed directly in this session. *Inferred, not measured today:* that
`git add -u` skips the untracked twins — that is git's documented `-u` semantics, and
the destructive half was never run.

The reason the guide omits it: at the catalog layer the move **is** atomic — one call,
one response, `moved: true`. The split only becomes visible one tool call later, at the
git layer, to a different observer. The author performing the move is structurally the
party least able to see it.

## Evidence

### The test knows, in a comment, and only there

`tests/issue_clusters.rs:1172-1176`:

> **The count gate sees TRACKED files only, so a local green defers rather than clears.**
> `tracked_all_bug_files` shells out to `git ls-files`, so a bug file that exists but has
> not been `git add`ed is invisible to the count while its ledger row may already have
> been updated — the pair agrees, the test passes, and the disagreement surfaces at CI
> once the file is committed.

That paragraph describes this exact state and is the only place in the repo that does.
It is in a test's doc comment, addressed to a reader of the test — not to the reader of
the archive procedure, who is the party who will hit it.

### The guide's own sweep recipe already failed this way once

`IC-18` member `docs/issues/archive/2026-08-26-archive-citation-sweep-grep-cannot-see-shell-or-yaml.md`
is the same guide, same procedure, one step earlier: the `--include` list it hands the
reader could not see six live surfaces. The guide has now produced two members of one
class from two consecutive steps of one recipe.

## Hypotheses tried

1. **Hypothesis:** the three short counts meant a class count genuinely needed
   re-deriving.
   **Test:** stage both halves of each move, re-run `cargo test --test issue_clusters`.
   **Verdict:** rejected — 18/18 green with no number edited.
   **Evidence:** § *Symptom*.

## Fix

**Implemented 2026-09-02 — and the mechanism candidate as originally written was aimed
one step off, which is the finding worth keeping.**

The proposal here asked for `move` to "report the pair explicitly". But `old_abs_path`
and `new_abs_path` were *already* returned unconditionally by the `json!` block in
`src/librarian/tools/mv.rs` on the day this file was written. The data was never
missing; the **action** was. Note the asymmetry that made it invisible: `previous_id`
carries four lines of comment stating what the caller must then do, while the two path
fields carried none.

Evidence the distinction is real rather than pedantic: one session ran six archive moves
in a row for `c71e97c7`, read both path fields in all six responses, and still had to
return to § *Workarounds* to learn that `git add -u` takes only the tracked deletion.
Six consecutive opportunities produced no inference.

Both candidates were taken:

- **Mechanism.** `move` now returns `stage_together: [<old>, <new>]`, added to
  `path_strip::PATH_KEYS` so it relativizes like its two sibling path fields, plus
  `stage_hint` — an imperative naming `git add -- <old> <new>`, the single `R` rename
  line that confirms both halves landed, and `git add -u` / `git commit -a` as the
  selectors with an index-entry precondition. `stage_hint` deliberately carries no path
  of its own: it is not a `PATH_KEYS` member, so an embedded path would render absolute
  beside a relativized sibling.
- **Documentation.** `get_guide("tracker-conventions")` § *Bug files* now carries the
  staging step beside the citation sweep — the omission this file identified.

The author-facing half of `IC-18` remains unreachable and is not claimed: nothing can
annotate a `git add -u` the operator types in their own shell.
## Tests added

`move_names_the_staging_action_not_only_the_two_paths`, in `src/librarian/tools/mv.rs`
beside `move_renames_file_and_updates_catalog`. Confirmed RED first — panicked at
*"stage_together must be an array naming both halves of the move"* — then green: 13/13
in that module, 14/14 in `path_strip`.

**The test this file originally specified would have passed against the defect, and
that is kept rather than quietly replaced.** It asked to "assert that a `move` response
names both paths", which was already true when written — a green assertion over the
broken shape. The assertion had to move to the *imperative*: `stage_together` present
and ordered deletion-first, and `stage_hint` naming `git add --`, `git add -u`, and
`R`. Asserting on the paths tests the half that was never broken.
## Workarounds

Stage the move explicitly, naming both sides:

```
git add -- docs/issues/<slug>.md docs/issues/archive/<slug>.md
```

`git status --short` then shows a single `R` rename line, which is the positive
confirmation that both halves are in the index. Anything still showing ` D` plus `??`
is half-staged.

## Resume

> **SUPERSEDED 2026-09-02 — the blocker described below cleared, and the fix has since
> landed.** This section instructed the reader that the file was "intentionally left
> **untracked**" pending an untangled `docs/trackers/issue-clusters.md`. It was in fact
> committed at `781633e4` (2026-09-02 05:31) as `IC-18`'s sixth member, ~11 hours before
> this note. The class has gained members since; derive the count with `python3
> scripts/probe-cluster-census.py` rather than reading a cell — the ledger stopped
> storing derived counts on 2026-09-02 (`no_class_field_states_a_bare_n`). A reader
> following the instruction below
> today would decline a commit that is already made. Read § *Fix* and § *Tests added*
> for the current state.

Historical record, accurate when written:

**Blocked on an unrelated file, deliberately — do not "fix" this by committing it.**
This bug file is intentionally left **untracked**. Staging it would add a sixth member
to `cluster/selector-narrower-than-its-population`, and the count gate
(`every_index_count_matches_the_corpus`, `every_bare_n_in_a_class_field_matches_the_corpus`)
would then require `docs/trackers/issue-clusters.md` to move `IC-18` from `n=5` to
`n=6` — in both the Index row and the `**Members:**` field. As of 2026-09-02 that file
carried three sessions' uncommitted edits and could not be committed by any one of
them.

Note the irony and do not let it pass as a joke — this record was itself sitting in the
half-state it describes, for a different reason.
## References

- `IC-18` in `docs/trackers/issue-clusters.md` — the defect class.
- `docs/issues/archive/2026-08-26-archive-citation-sweep-grep-cannot-see-shell-or-yaml.md`
  — the same guide's previous instance, one step earlier in the same recipe.
- `tests/issue_clusters.rs:1151-1176` — `tracked_open_bug_files` /
  `tracked_all_bug_files`, and the doc comment that names this state.
- `7d2b3ee7` (`experiments`, patch-id `3f9dd6822aeb24684307e4802ee3867dc667faff`) — the
  archive commit whose staging surfaced this.
