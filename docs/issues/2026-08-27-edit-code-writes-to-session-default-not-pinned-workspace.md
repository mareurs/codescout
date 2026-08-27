---
kind: bug
status: open
title: "edit_code writes to the session-default project, not the workspace= pin — a subagent's structural edits silently land in another checkout"
tags:
  - edit_code
  - worktree
  - workspace-pin
  - silent-corruption
  - regression
closed:
---

# edit_code writes to the session-default project, not the `workspace=` pin

**Regression of** `docs/issues/archive/2026-07-09-edit-code-write-path-ignores-workspace-pin.md`
(id `875e834b95286445`, archived `fixed`). Same mechanism, observed again 2026-08-27.

## Symptom

A subagent implementing Task 4 of the section-grain `get_guide` plan reported that
**two `edit_code` insert calls returned `ok` but did not land** — it noticed only because
it re-read the file with `symbols()` afterwards, and re-ran them.

They were not no-ops. They were writes to the **wrong checkout**.

The subagent was working in the linked worktree
`.worktrees/get-guide-section-grain` and passing
`workspace="/home/marius/work/claude/codescout/.worktrees/get-guide-section-grain"`
on every call. After the task committed, the **main checkout** showed:

```
 M src/librarian/adapter.rs
 M src/tools/core/types.rs
```

Neither file was modified in the main checkout before the task ran (verified: the main
checkout's dirty set beforehand was `doctor.rs`, `output_buffer.rs`, `read_file.rs`,
`run_command/{mod,output,tests}.rs`, all belonging to a concurrent session, and all since
committed). The leaked diff is 62 insertions / 43 deletions and contains `selector_key`
(2 occurrences) and `names_path_containing` (3) — it is unambiguously Task 4's production
code, applied to a tree that never asked for it.

The worktree's own commit `fa49b695` is complete and correct (138 insertions / 43
deletions — the leak is a strict subset, missing the tests). So the edits were applied
**twice, to two different trees**, and only the second landing was visible to the caller.

## Why this is worse than a lost edit

1. **It reports success.** `ok` is returned for a write the caller cannot see. The only
   reason it was caught is that this particular subagent re-read the symbol afterwards.
   Nothing in the tool's contract suggests that is necessary.
2. **It writes into someone else's working tree.** The main checkout is where a concurrent
   session is working. This corrupted its `git status` with changes it did not make. Had
   that session run `git commit -a`, it would have committed a stranger's half-finished
   refactor.
3. **It is invisible to every gate.** `cargo test` in the worktree passes, because the
   worktree copy is correct. Nothing looks at the other tree.


**Contamination cleaned 2026-08-27, on the user's explicit authorisation.** Before reverting,
the leak was proven inert: the other session's in-flight `grep.rs` work (150 insertions)
referenced neither leaked symbol; the main checkout's committed code had **zero** references
to `selector_key` and **zero** to `names_path_containing`; and `names_tracker_path`'s one
production caller (`adapter.rs:210`) plus its ten test assertions (`814-862`) all predate the
leak and survive its removal untouched. `git checkout -- src/librarian/adapter.rs
src/tools/core/types.rs` restored HEAD exactly, leaving only that session's own two dirty
files. A copy of the leaked diff was retained before reverting.

Worth recording precisely because of how quiet it was: the leak was **semantically neutral**.
It added a trait method with a `None` default (purely additive) and refactored
`names_tracker_path` into a thin wrapper preserving its exact semantics. So the main checkout
**still compiled and its ten assertions still passed** with the contamination in place. This
was not broken code in the wrong tree — it was *correct* code in the wrong tree, which no
compiler, test, or lint can distinguish from code you meant to write. That is why the only
viable detector is a `git status` leak-check after each task, not a gate.
## Suspected mechanism

The archived bug's own title states it: the write path resolves against the
**session-default project** rather than the per-call `workspace=` pin. The pin is honoured
by read paths (`symbols`, `read_file`), which is why the subagent's verification reads were
correct and only the writes went astray.

What likely made the session-default wrong here: codescout's active project is a single
server-side slot. This session activated the worktree, but the slot was observed flipping
to read-only mid-task (a foreign activation defaults `read_only=true`), which implies
something re-activated it. If the active project was the **main checkout** at the moment
those two `edit_code` calls ran, a pin-ignoring write path lands exactly where these landed.

## Reproduction sketch (not yet minimised)

1. Open a linked worktree of this repo.
2. Ensure the server's active project is the MAIN checkout.
3. From a subagent, call `edit_code(action="insert", …, workspace="<worktree path>")`.
4. Expect: the worktree file changes. Observe: the main checkout's file changes.

Not reproduced in isolation yet — the original subagent judged it non-reproducible and
declined to file. That judgement was wrong under this project's rules (tool quirks and
misbehaviours are explicit trigger cases, and non-reproducibility is a detail to record,
not an exemption), but the minimisation work is genuinely outstanding.

## Impact on the work in flight

Tasks 5, 6, 8, 9 and 10 of the same plan all edit source. Until this is understood, every
one of them can leak into the main checkout. Mitigations in force:

- The controller keeps the active project pinned to the worktree.
- Every dispatch instructs the implementer never to call `workspace(action="activate")`.
- The controller runs `git status --short` on the **main checkout** after each task as a
  leak check.

## Not yet done

- Minimise the reproduction.
- Determine whether `edit_file`, `create_file` and `edit_markdown` share the defect, or
  only `edit_code`'s structural write path.
- Check whether the 2026-07-09 fix regressed, or only ever covered a narrower path than
  its title claims.
