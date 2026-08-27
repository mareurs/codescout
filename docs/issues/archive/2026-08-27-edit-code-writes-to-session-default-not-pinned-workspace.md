---
kind: bug
status: fixed
title: An unpinned write is silently routed to the session default in a multi-worktree repo — guard_worktree_write latches open on the session's first activate
tags:
- edit_code
- worktree
- workspace-pin
- silent-corruption
- regression
closed: 2026-08-27
unverified: By design, the `guard_worktree_write` session latch is unchanged — an unpinned write still routes to the session default, it now just says so. Live verification of the annotation itself is COMPLETE (2026-08-27, see § Live verification); nothing else is outstanding.
---

# edit_code writes to the session-default project, not the `workspace=` pin

> **Filed as** a regression of `docs/issues/archive/2026-07-09-edit-code-write-path-ignores-workspace-pin.md`
> (id `875e834b95286445`, archived `fixed`). **That framing was refuted by measurement on
> 2026-08-27** — see § *Reproduction — minimised*. The 2026-07-09 fix holds; `edit_code` honours
> the `workspace=` pin. The real defect is one layer up, in `guard_worktree_write`, and the title
> above has been corrected to name it. The original title is kept here for citation continuity:
> *"edit_code writes to the session-default project, not the `workspace=` pin"*.

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

## Evidence from Tasks 5-10

Task 5 reported zero silent no-lands on `workspace=`-pinned `edit_code` calls, plus one
*loud* failure (an error, not a silent `ok`) on a single call issued **without** the pin.
Tasks 6, 8, 9 and 10 each reported zero landing failures, silent or loud, across their
`edit_code` writes — all pinned per their dispatch instructions.

Taken together this partly contradicts the "resolves against the session-default project"
mechanism above: if that were the whole story, a pinned call landing correctly would be
lucky rather than expected, and five further tasks' worth of pinned calls landing clean
every time is not the distribution that predicts. The data instead points toward "the
defect is confined to unpinned (or pin-dropped-mid-call) `edit_code` calls" — a narrower
claim.

This narrows but does not confirm anything: n = one plan (six further tasks, not an
independent sample), and the original Task 4 leak's own two calls were dispatched *with*
the pin per instruction, which this evidence does not explain — if the pin were sufficient
on its own, Task 4's leak should not have happened either. The mechanism is still open.
## Reproduction — minimised 2026-08-27

Run live against the serving binary (main checkout = session default, linked worktree
`.worktrees/operator-rules-phase-1` present). Each write was followed by reading the file back
in **both** trees.

| tool | pinned to worktree | unpinned |
|---|---|---|
| `create_file` | lands in worktree ✓ | lands in main |
| `edit_file` | lands in worktree ✓ | — |
| `edit_markdown` | lands in worktree ✓ | — |
| `edit_code` (`replace`) | lands in worktree ✓ | — |
| `edit_code` (`insert`) | lands in worktree ✓ | **lands in main, returns bare `status: ok`** |

The pinned column was measured in both shapes that could differ: a target existing **only** in
the worktree, and a target existing in **both** trees under the same relative path (the incident's
shape). No misroute in either.

**So the filed mechanism is refuted.** `edit_code` does not ignore the pin. All four
`resolve_write_path_for` call sites still thread `ctx.workspace_override.as_deref()`
(`edit_code.rs:331, 683, 874, 1166`), and `resolve_write_path_for_honors_workspace_override`
(`src/fs/mod.rs`) still guards it. The 2026-07-09 fix did **not** regress.

### What actually happened

The leak is the **unpinned** row. An unpinned write resolves against the session default, which
was the main checkout — correct by design, and silent by design.

The thing that should have caught it is `guard_worktree_write`
(`src/tools/core/guards.rs:20-47`), whose stated job is refusing a write when "git worktrees exist
but the agent hasn't explicitly called `activate_project` to confirm which project to write to."
It opens with an unconditional session-global bypass:

```rust
if ctx.agent.is_project_chosen_this_session().await {
    return Ok(());
}
```

`project_chosen_this_session` is a **session-wide latch**. Once any `activate` has run — which an
SDD controller does at the start of essentially every multi-agent run — the guard returns `Ok`
for every subsequent write in the session, regardless of which root that write resolves to and
regardless of whether it carries a pin. Measured this session: before `activate`, an unpinned
`create_file` was refused loudly; after `activate(main)`, an unpinned `edit_code(insert)` landed
in main with `status: ok` and no mention of the tree it wrote to.

That is the whole incident. A subagent working in the worktree, pinning per call as instructed,
drops the pin on one or two calls; those calls land in main; nothing in the response distinguishes
them from the ones that landed correctly.

It also explains the distribution the *Evidence from Tasks 5-10* section could not: pinned calls
land correctly because the pin works, and Task 5's single **loud** failure was an unpinned call
issued *before* the session's latch was set.

**Guard granularity is the defect.** Under per-call pinning — which
`get_guide("workspace-state")` § *Per-call workspace pinning* explicitly recommends over
`activate` for parallel fan-out — a per-session confirmation is the wrong unit. A pin *is* the
per-call confirmation; its absence on one call is an unconfirmed write, and the guard cannot see
that because it stopped looking at the first `activate`.

Same structural shape as open bug `54a70b49f6f26681` (*the rendezvous gate latches open*): a
session-global boolean standing in for a per-event check.
## Measured 2026-08-27 — the obvious fix is refuted, and the pressure points the other way

Before building anything, the candidate fix was costed against real transcripts: 2,319 JSONL files,
195,126 tool-call records, 724 sessions, 2026-03-21 → 2026-08-27, across all three CC profiles.
Instrument validated on a known-positive session first (this one — pinned writes on all four write
tools, one unpinned `edit_code`, one live guard fire); it also had to be taught that subagent
transcripts live in `<session-id>/subagents/agent-*.jsonl`, 1,637 sidecar files against 675
top-level ones. Omitting those would have hidden the phenomenon entirely, since the leak happens
*inside* a subagent.

**Candidate rule costed:** refuse an unpinned write when linked worktrees exist AND some earlier
*write* in the session carried a `workspace=` pin.

**Result: 9,914 refusals — 30.3% of every codescout write in the corpus — against at most 18 true
positives (0.18%). ~551 false refusals per genuine catch.** One session arms on three pinned writes
and then refuses 2,663 ordinary ones.

Two structural errors, both the same shape — a repo-level or cross-repo fact misread as a statement
about the current session:

- **`list_git_worktrees` is a property of the REPO, not the session.** It answers "does this repo have
  worktrees on disk", not "is this session using one". codescout has `.worktrees/operator-rules-phase-1`
  on disk, so the rule arms in every codescout session — including shared-checkout fan-outs that never
  touch a worktree. Session `main|55515bc5`: 44 writing subagents, shared checkout, **302 would-be
  refusals**.
- **The arming pin is usually about a DIFFERENT repo.** 63.4% of refusals were armed by a pin at
  `claude-plugins` / `prompt-engineering` / `researcher`. One cross-repo write arms the guard, and every
  subsequent home-project write is refused.

False-positive decomposition of all 9,914 (classified mechanically, then 20 stratified + all 214
residuals hand-read): 63.4% arming pin at an unrelated repo; 18.4% active project *was* the worktree,
so the relative write lands correctly; 16.0% target was an absolute path or artifact id, nothing to
resolve ambiguously; 2.2% residual, of which **18** match the true leak shape.

**The true leak shape, now precisely characterised** (from those 18, spanning 4 sessions): shell cwd
inside a worktree + codescout active project = main + a bare *relative* write target. Even these are an
upper bound — several targets are cross-cutting docs (`docs/ROADMAP.md`, `.codescout/memories/*`,
`docs/issues/*`) plausibly meant for main while code work happened in the worktree.

### The harm is real; the current guard is the friction

- **Harm confirmed:** 131 wrong-checkout hits across 18 sessions, e.g. *"the `artifact` tool's writes
  have repeatedly landed in the wrong checkout this session, undetected by most prior uses."* This is a
  **lower bound** — it counts only misroutes someone noticed and wrote down, and the failure mode
  returns a bare `status: ok`. It is currently mitigated by hand-written prompt discipline, not by code.
- **Current guard fires 122 times across 27 sessions** (21 codescout), ~4.5 refusals per session before
  the agent works out it needs an `activate` — *including in sessions that never touch a worktree*,
  purely because a stale worktree exists on disk.
- **Dominant workflow is shared-checkout:** of 37 sessions with 2+ writing subagents, **24 (65%) use a
  single shared checkout**, largest fan-outs at 131 and 50 distinct writing subagents. Worktree sessions
  top out at 89. Only 7.47% of all writes carry a pin at all, and **72% of write-bearing sessions never
  pin a single write** — so any pin-armed rule is silent exactly where it is not needed and loud
  where it is.

### What survives

Not a blocking guard. The surviving candidate is **information, not refusal**: when a write resolves
*unpinned* and the repo has linked worktrees, name the tree in the response (`wrote_to: <root>`). Zero
false-positive cost — it refuses nothing — and it would have made every one of the 18 visible on the
first call rather than after a `git status` on another checkout. Pinned writes need no annotation; the
call already names its target.

A second, independent candidate the measurement surfaced: the **existing** guard's 122 fires are
largely friction, since it arms on a repo-level fact. Narrowing it is a separate decision from this bug.

**Limits of the measurement:** historical `git worktree list` state is unrecoverable, so 9,914 is a
*lower* bound on refusals; active project was inferred from the last observed `activate` (CLI-flag and
hook activations are invisible); no transcript records where a write actually landed, so the 18 are a
shape match, never a confirmed wrong-tree file; one user, one machine.
## Fix

**`22e60ee9724888a806dc89ba4cb50715d765bda1`** (`experiments`)
patch-id **`1bf09e75f9b07d0703af534a83e636a8e09ede69`**

`annotate_write_root` (`src/tools/core/types.rs`), the third member of the family with
`guard_worktree_write` and `worktree_read_notice`. Runs in `call_content`, so every write tool is
covered including `artifact`, whose misroutes this bug's own evidence quotes.

Fires only when the repo genuinely has linked worktrees **and** the call was unpinned — a pinned
call already named its target. A bare `"ok"` is promoted to `{"status": "ok", "wrote_to": "<root>"}`;
object results keep their fields and gain `wrote_to`. Single-checkout repos, including every test
tempdir, are byte-identical to before, which is what makes the promotion safe against the no-echo
write convention.

Exempt: `approve_write`, `register_library`, `library` — `is_write` is true for them but their write
does not land at a project-relative path, so naming a checkout would describe something that did not
happen.

**Deliberately NOT fixed: the latch.** `guard_worktree_write` still bypasses on
`is_project_chosen_this_session`, and an unpinned write still resolves against the session default.
That is the measured decision, not an oversight — see § *Measured 2026-08-27*. The silence is what
was closed.

### Tests

Five, in `src/tools/core/tests.rs`, all driven through the real `call_content` path so the **wiring**
is under test rather than the helper in isolation:

| test | mutation that breaks it, and only it |
|---|---|
| `an_unpinned_write_names_the_checkout_it_reached` | disable the `annotate_root` block |
| `annotating_an_object_result_preserves_its_fields` | disable the `annotate_root` block |
| `a_write_without_worktrees_keeps_the_bare_ok` | drop the `list_git_worktrees().is_empty()` return |
| `a_pinned_write_is_not_annotated` | drop the `workspace_override.is_none()` clause |
| `an_exempt_tool_is_not_annotated` | drop the `WRITE_ROOT_ANNOTATION_EXEMPT` clause |

All four mutations were run with the blast radius predicted in advance; each matched exactly (1, 1,
1, and 2 failures respectively). No test passes in a world where its clause is gone.

Gate: `cargo fmt` clean, `cargo clippy --workspace --all-targets --features local-embed -- -D
warnings` clean, `cargo test` 4676 passed / 0 failed / 46 ignored.
## Live verification (2026-08-27, post-`cargo rb` + `/mcp`)

Verified through a rebuilt, freshly-reconnected server. Freshness first, on all three R-89 axes:

| axis | evidence |
|---|---|
| build | binary `21:14:30` > last commit `295ce8c9` at `21:11:39` |
| distribution | `wrote_to` = 1 in the symlink-resolved binary; positive controls `"another codescout instance is writing"` = 4 and `"was given nothing to change"` = 4; negative control = 0 |
| process | serving pid `2117173`, PPID = this session's `claude`, started `21:16:25` > build |

Five probes, all as designed:

| probe | result |
|---|---|
| unpinned write, no worktrees present | bare `"ok"` — no-echo convention preserved |
| unpinned write, worktree present, before `activate` | refused by `guard_worktree_write` |
| unpinned `create_file`, after `activate` | `{"status": "ok", "wrote_to": "/home/marius/work/claude/codescout"}` |
| unpinned `edit_file` (bare-string result promoted) | `wrote_to` present |
| **pinned** write | bare `"ok"` — correctly not annotated |

### Two instrument failures worth recording, because both would have read as "the fix did not ship"

**1. `grep -c` on the binary returned 0 for every string, including a known-present control.**
The first distribution pass reported 0 for `wrote_to` *and* for `"was given nothing to change"` — a
string verified present in this same binary during an earlier recon. Four strings, four zeros.
Had only the new string been probed, that 0 would have been read as the change missing.
`grep -c` does not count matches inside a binary the way the probe assumed; `strings -a | grep -c`
does. **The known-present control is the entire reason the instrument was suspected rather than
the code** — R-3, in the form where the instrument answers confidently rather than erroring.

**2. The feature's precondition vanished between commit and probe.** The first functional probe
returned a bare `"ok"` with no annotation — indistinguishable from the annotation failing. It was
correct: a concurrent session had removed the `.worktrees/operator-rules-phase-1` worktree, so
`list_git_worktrees` was empty and `annotate_write_root` early-returned exactly as
`a_write_without_worktrees_keeps_the_bare_ok` specifies. Confirming the feature required
**manufacturing** the condition — `git worktree add --detach` to a scratch path, probe, then
`git worktree remove`. A feature gated on an environmental fact cannot be verified in an
environment where that fact is absent, and the absence looks identical to a defect.
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

All three original items are answered (see § *Reproduction — minimised*), and the obvious fix has been
costed and refuted (see § *Measured 2026-08-27*). What remains is a decision, not an investigation:

- **Decide between** (a) annotating unpinned writes with the tree they landed in, in repos that have
  linked worktrees; (b) narrowing the *existing* guard, which the same measurement shows firing 122
  times mostly as friction; (c) neither — the harm is real but is currently absorbed by prompt
  discipline, and both code changes carry cost.
- **Do NOT ship a pin-armed refusal.** Costed at 30.3% of all writes refused for a 0.18% true-positive
  rate. The number is recorded above so this is not re-proposed.
