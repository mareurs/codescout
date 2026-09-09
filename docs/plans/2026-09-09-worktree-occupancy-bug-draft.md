<!--
DRAFT — create from the MAIN checkout via doc(action="create") so the catalog row is minted:

  doc(action="create",
      rel_path="docs/issues/2026-09-09-worktree-occupancy-is-checked-against-a-cwd-no-session-has.md",
      kind="bug", title="...", tags=["cluster/gate-keyed-on-unobservable-event"],
      status="open", owners=["marius"], body=<everything below the frontmatter>)

Writing it as a bare file leaves it invisible to find(kind="bug") until a reindex, and a direct
frontmatter edit never reaches the catalog at all (BL-48).

Drafted by tool-collapse-4a, sessionId bf6a6925-f207-4a2f-8135-95e7563e859f (.claude-sdd, pid 2841831),
which could not write it itself: 10 catalog rows were already pending merge from that worktree, so a
ledger write from there would have added an 11th.
-->
---
status: open
opened: 2026-09-09
closed:
severity: high
owner: marius
related:
  - docs/issues/archive/2026-09-02-comm-filter-misses-version-pinned-claude-processes.md
  - docs/issues/archive/2026-09-02-greedy-name-regex-reads-a-former-session-name-as-the-current-one.md
  - docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md
  - docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md
tags:
  - cluster/gate-keyed-on-unobservable-event
kind: bug
---

# BUG: worktree occupancy is checked against a process cwd, and a session does not have one

## Summary

Every instrument this repo uses to answer *"is anyone working in this worktree?"* — including
`/codescout-companion:reaching-peer-sessions` Step 1 — resolves occupancy by reading `/proc/<pid>/cwd` for
one socket-bound pid. A Claude Code session is **several processes with independently-set cwds**, and
`workspace(action="activate")` moves the active project without moving any of them. So a session can work in
a worktree all night while every cwd-based instrument reports that worktree empty. On 2026-09-09 this came
one refusal short of `git worktree remove` on two occupied worktrees, and did destroy 32.1 GiB of a live
session's build tree.

## Symptom (Effect)

A session self-reports working in a worktree; every external instrument says nobody is there.

```
$ readlink /proc/872862/cwd
/home/marius/work/claude/codescout

$ for s in /run/user/1000/cc-socks/*.sock; do p=${s##*/}; p=${p%.sock};
    readlink /proc/$p/cwd 2>/dev/null; done | grep -c doctor-per-project
0
```

Measured 2026-09-09T06:13:27Z over all 13 live sockets. Session `codescout-87` (pid 872862) states it is
working in `.worktrees/doctor-per-project-isolation`; that worktree holds **17G**; **zero** sessions have a
cwd inside it. Both readings are correct.

There is no error. The instrument returns a plausible number, and the number is *consistent* — re-running it
returns the same zero.

## Reproduction

```
git rev-parse HEAD          # 6f032dbd (tool-collapse); reproduces on experiments @ 456e217c
```

1. From session A, launched in the main checkout, call
   `workspace(action="activate", path="<repo>/.worktrees/<name>")` and do work there.
2. From session B, run `/codescout-companion:reaching-peer-sessions` Step 1, or any
   `readlink /proc/<pid>/cwd` walk over `/run/user/<uid>/cc-socks/*.sock`.
3. Session A's row shows `CWD = <main checkout>`. No row names the worktree. B concludes it is unoccupied.

The divergence needs no unusual setup: it is the default whenever a session activates a project other than
its launch directory.

## Environment

Linux 7.2.3-zen1-3-zen, Claude Code across `~/.claude` and `~/.claude-sdd`, codescout MCP over stdio,
`/home/marius/work/claude/codescout` with 6 worktrees under `.worktrees/`. 13 live sessions across 2
profiles at the time of measurement.

## Root cause

**A session is not a process, and none of its processes is authoritative for "where it is working."**

Measured 2026-09-09T06:13Z, walking my own parent chain from a `run_command` shell:

```
pid=3437713 cmd=sh         cwd=.worktrees/tool-collapse   <- tool-execution shell
pid=2842049 cmd=codescout  cwd=.worktrees/tool-collapse   <- MCP server process
pid=2841831 cmd=claude     cwd=.worktrees/tool-collapse   <- the socket-bound session process
pid=36650   cmd=bash       cwd=.worktrees/tool-collapse   <- launching terminal
```

Four processes, four independently-set cwds. In *this* session they coincide, because the harness launched
it inside the worktree. They coincide **for exactly the sessions from which the instrument looks correct**.

For a session launched in the main checkout that later activates a worktree, they diverge: the `claude`
process keeps its launch cwd forever, while the tool-execution shell resolves to the active project
(`run_command` sandboxes cwd to the project — its own tool description says so). The socket filename is the
`claude` pid, so an instrument keyed on the socket reads the one process guaranteed **not** to have moved.

**The active project is session-local state with no external read surface.** No `/proc` field, no socket
attribute, and no file outside the session's own config exposes it. So this is not a selector that can be
widened — the population it names is not observable from outside the session at all. (Diagnosis refined by
`codescout-87` / sessionId `b80a27d4-9729-40ef-8c28-ad8982df6d13`, whose own case is the evidence.)

Classified `cluster/gate-keyed-on-unobservable-event` (`IC-2`) — **not** `IC-18`
(`selector-narrower-than-its-population`), which is where this file was first drafted. `IC-18` is a selector
that under-selects from the **right** population, and its implied remedy is to widen it. cwd is not a narrow
view of "sessions working here"; it is a **different set** — processes, of which a session has several — and
no cwd predicate reaches this at any strictness. Measured 2026-09-09T12:4x+03:00, over the strongest form of
the predicate (*any process in the session's tree has cwd under X*), against a session known by git object to
be working in a worktree:

```
codescout-87 (872862), active project = .worktrees/doctor-per-project-isolation
  pid=872862  cmd=claude          cwd=/home/marius/work/claude/codescout
  pid=873318  cmd=researcher-mcp  cwd=/home/marius/work/claude/codescout
  pid=873319  cmd=codescout       cwd=/home/marius/work/claude/codescout
```

Not one persistent process has a cwd in the worktree it is committing to — so `workspace(activate)` does not
move the MCP server's cwd either. The only process that ever returns true is the per-call tool shell, and
that is transient: this session's shell `3437713` was gone by the next call (`1550299`). **Between tool
calls the property is unobservable by any cwd at any strictness.** That is a proxy substituted across an
observation boundary, which is `IC-2`, and the remedy it implies — find an authoritative signal — is the one
that actually worked (the `Session-Id` trailer).

**Why the first classification was wrong, and it is a gap in `IC-18`'s stated discriminator rather than a
misreading of it.** `IC-18`'s test is *could the mechanism, unchanged, report how many it missed?* — and the
honest answer here is no, so the test fires. But the test presupposes what it cannot check: that the
selector ranges over the population named. It is a test for **within-population** under-selection, and
applied to a **wrong-population** selector it returns the same answer. So a firing diagnostic does not
distinguish *narrower* from *different*. The discriminator that does: **a class whose remedy provably cannot
apply is the wrong class, even when its diagnostic test fires** — routing a reader to `IC-18` here would send
them to widen a cwd predicate, which is unreachable. Adjudicated by `ad379a7c-a0cf-4c61-bcdb-f0696fea8c30`,
independently checked by `codescout-87`, confirmed on the measurement above.

Kept apart from `IC-14` on the existing test too: this is a selector producing a partial **result**, not a
guard producing a **refusal**.

## Evidence

### The near-miss, and the one real loss

`author-side-build-check-notification` (sessionId `5399543d-22d6-4ed9-9ebb-876be459989f`, `.claude`, pid
1849060) was instructed to reclaim ~49G by removing `.worktrees/tool-collapse` and
`.worktrees/result-cap-marker-gate` as merged and unoccupied. Both had live occupants. Its own account of the
predicate:

> "I enumerated live sessions with an exact match on `cwd == /home/marius/work/claude/codescout`. That
> excludes every worktree occupant **by construction** — you were never in my population, and my check
> returned 'nobody is there' as a true statement about a set I had defined to exclude you."

`git worktree remove` was refused and `--force` explicitly declined. What did happen: `cargo clean` in
`.worktrees/tool-collapse` at ~05:45Z while a session was live in it — 30617 files, 32.1 GiB, verified absent
afterwards (`ls -d target` → nothing; worktree 30G → 71M). No source was lost; the untracked
`.superpowers/` SDD ledger survived only because `--force` was refused.

### The fix that kept the predicate

After correction, the same author widened the check from an exact cwd match to a **prefix** cwd match and
reported six sessions, two in worktrees. That is a better instrument and it still cannot see this bug: a
prefix match on cwd is still a match on cwd. It then concluded `doctor-per-project-isolation` "turns out to
have nobody in it at all" — which is precisely the false negative, on the 17G tree, from the corrected tool.

### The only non-proxy in the set

`codescout-87` established its position with a git object rather than a self-report:

```
$ git show -s --format='%H %s%n%(trailers)' fe7b6658
fe7b6658a346a218e85c1f166478bf32d5c8cf83 feat(librarian): doctor scoping unit -- admit-based narrowing, not SQL-splice
Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Session-Id: b80a27d4-9729-40ef-8c28-ad8982df6d13

$ git branch -a --contains fe7b6658
+ doctor-per-project-isolation
```

Verified independently 2026-09-09T06:15Z. The `Session-Id` trailer (`IC-10`'s shipped mechanism) needs no
cooperation, survives the session exiting, cannot be forged by a session that was not there, and answers the
question a removal actually turns on — *has this session done work here?* **Its gap is a worktree with no
commits yet**, where it degrades to nothing, which is exactly `IC-10`'s recorded *"uncommitted half still
none"*.

## Hypotheses tried

1. **Hypothesis:** the divergence is `workspace(action="activate")` changing the active project without
   changing the process cwd.
   **Test:** read `run_command`'s contract (*"sandboxes cwd to the project"*) and compare a session's
   socket-bound `claude` pid cwd against its tool shell's `pwd`.
   **Verdict:** confirmed but incomplete — true, and it still implies a single "session cwd" exists to be
   read.
2. **Hypothesis (refinement):** a session has no cwd at all; server process and tool shell are distinct
   processes with independent cwds, and active-project is session-local state with no external read surface.
   **Test:** walked the parent chain from a `run_command` shell — four processes (`sh`, `codescout`,
   `claude`, `bash`), each with its own cwd.
   **Verdict:** confirmed. This is why no widening of a cwd predicate can fix it.
3. **Hypothesis:** widening the cwd match (exact → prefix) repairs the enumeration.
   **Verdict:** rejected, empirically, on the same evening — see *The fix that kept the predicate*. It
   returned a false negative on a 17G occupied worktree.

## Fix

**Plan.** Stop asking "whose cwd is here?" and ask "who has done work here?", which has a durable answer.

1. **Short term, in the skill — and note it is a DIFFERENT REPO.** The emitter is
   `/home/marius/work/claude/claude-plugins/codescout-companion/skills/reaching-peer-sessions/SKILL.md:49`:
   `printf 'PID\tPROFILE\tNAME\tSTATUS\tCWD\n%s\n' "$rows"`. It must stop presenting `CWD` as occupancy — it
   is sound for *"who is alive"* and unsound for *"who is working where"*. Rename the column to `LAUNCH CWD`
   and add the standing note: **a session whose row shows the main checkout may still be working in a
   worktree; to attribute work to a worktree, ask the session or read `Session-Id` trailers on that
   worktree's branch.** The skill already teaches "enumerating a complete set only bounds who was present —
   to attribute a write, ask"; this is the same law one layer down, and the `CWD` column currently
   contradicts it.
   **Two cross-repo hazards on this step.** `claude-plugins` is a separate checkout with its own live
   session (`claude-plugins-bc`, pid 256394) and its own uncommitted file
   (`docs/trackers/version-bump-checklist.md`) as of 2026-09-09T06:20Z — coordinate before editing. And per
   the operator's global `CLAUDE.md`, three profiles (`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`) share
   plugin **source** repos but keep separate **caches** and install records, so confirm whether a source edit
   reaches a running session or whether each profile's cache needs refreshing — otherwise the fix ships to
   one profile and the bug keeps firing in the other two, which is `IC-4` (`config-propagation-is-additive`).

   **And a third layer under both: the delivered body can differ from the cache.** Measured 2026-09-09,
   two sessions on the **same profile** (`.claude-sdd`) reading the **same file**:

   | session | Skill call | `SKILL.md:32` as delivered | outcome |
   |---|---|---|---|
   | `codescout-87` (872862) | with arguments | `{print live}` — `$2` substituted by word 2 of the invocation args | hand-repaired before running |
   | `tool-collapse-4a` (2841831) | **no arguments** | `{print $2}` — byte-identical to disk | ran correctly; `<-- you` row present |

   `$2` at line 32 is the **only** positional in the entire file, so it is the whole blast radius — and it
   sits in the `PPid` walk, i.e. in the self-identification step whose failure the skill itself documents as
   silent. **Consequence for this fix: a source edit at `:49` is verifiable by reading the file and NOT
   verifiable by reading what any session received.** So the confirmation step must be *a session quoting the
   line it was served*, never anyone reading the file or the cache. Not a codescout defect — a harness
   argument-substitution bug, reported through the harness channel by `codescout-87`; recorded here only
   because it changes what "verified" means for this remedy.
2. **Positive identification.** For any worktree with commits, `git log --format='%(trailers:key=Session-Id)'
   <branch>` names its authors durably. Pair it with the ask-the-session route for the uncommitted case.
3. **The check that runs when nobody is worried** (§ *Observer Blindness*, third position). A pre-flight on
   worktree removal, not a rule anyone must remember: refuse `worktree remove` while the branch carries
   `Session-Id` trailers from a currently-live sessionId, **or** while `doctor` reports a
   `worktree_scoped_row` for that root. See the candidate sibling bug in `Resume`.

**Do NOT** fix this by widening the cwd predicate. That was tried and refuted the same evening.

SHA / patch-id: not yet fixed.

## Tests added

None yet. A regression test must **mutate the production path, not the test's inputs** — a fixture that
re-implements the enumeration is indistinguishable from coverage. The discriminating fixture: a session row
whose launch cwd is the repo root and whose active project is a worktree, asserting the instrument does
**not** report that worktree unoccupied. Note the monotonicity: an assertion of the form *"a session in the
worktree is found"* is monotone under widening and will pass for an instrument that reports everyone
everywhere, so the test must also assert the negative case discriminates.

## Workarounds

- Treat a cwd-derived occupancy zero as **unknown**, never as empty.
- Before any `git worktree remove` or `cargo clean` in a shared tree: check `Session-Id` trailers on that
  branch, run `librarian(action="doctor")` for `worktree_scoped_row`, and message the socket set directly.
- `du -sh` the tree first. On 2026-09-09 the removal's whole justification was 30G that a prior `cargo clean`
  had already reclaimed; the actual saving was 71M.

## Resume

Draft the skill edit first, since it is the surface being served: change
`codescout-companion/skills/reaching-peer-sessions/SKILL.md` Step 1's `CWD` header to `LAUNCH CWD` and add
the attribution note above it, then re-score `docs/evals/reconnaissance-trigger.md` only if the description
changes (it should not). Then decide whether the removal pre-flight belongs in `doctor` — coordinate with
`codescout-87` at `uds:/run/user/1000/cc-socks/872862.sock`, which is working in `doctor`'s scoping subsystem
and has taken the catalog half as a live input to its own plan.

## References

- Instance of `OB-22` — *a self-written filter makes an enumeration a fixed point of its author's beliefs*
  (`docs/trackers/observer-blindness.md:2310`). **Not a new class.** The addendum it earns: *the remedy that
  widens the filter while keeping the predicate leaves the fixed point intact* — the author who wrote OB-22's
  lesson applied it an hour later and the blind spot survived the fix.
- **Separate hygiene defect, two rows not one, and its mechanism is worse than it first reads.**
  `observer-blindness.md` carries body sections for OB-1…OB-23 complete, but its index table covers
  OB-1…OB-20 plus OB-23 — **both `OB-21` and `OB-22` are unindexed** (`## OB-21` line 2222, `## OB-22`
  line 2310; index rows 141–161 skip both). Verified 2026-09-09 by two sessions independently.
  **A fix that adds only OB-22 leaves the index still jumping OB-20 → OB-23 and reads as closed** — this
  bug's own class applied to its own filing, since the repair would be scoped to the row its author was
  looking at.
  **The gap is at the TOP of the table, not buried in it.** The index sorts by **descending OB id, not by
  date** — `OB-18` (2026-08-31) sits between `OB-19` (2026-09-06) and `OB-17` (2026-09-04), which rules out
  a date key. So `OB-22` and `OB-21` belong at rows 142–143, immediately below `OB-23` at row 141. Whoever
  added `OB-23` inserted a row **directly above a two-row hole** and did not see it. The available
  explanation — *"the index is newest-first, so a mid-list gap is adjacent to nothing an author looks
  at"* — is therefore **refuted**: proximity was maximal and the gap still accumulated. That removes "look
  nearby when appending" as a remedy and leaves only a mechanical check (`doctor`'s `entry_without_definition`
  has the inverse shape — a cited id with no body; this is a body with no index row, which nothing checks).
  `OB-21` is *a session cannot audit its own successive claims*, which is too apt to omit and too neat to
  build on.
- `IC-18` `selector-narrower-than-its-population` — classified by that entry's own discriminator.
- `IC-10` `authorship-unrecoverable-after-the-fact` — the `Session-Id` trailer is its shipped mechanism, and
  this bug is a second consumer of it.
- `CLAUDE.md` § *Reaching a Peer Session*, § *Observer Blindness* — "never close an authorship question by
  elimination", "a windowed instrument's zero is scoped to its window".
- Peer accounts, 2026-09-09 ~06:10Z: `5399543d` (`author-side-build-check-notification`),
  `ad379a7c` (`fix-file-provenance-tool-blindness`), `b80a27d4` (`codescout-87`).
