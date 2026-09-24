---
id: fa4a13f40ea82465
kind: convention
status: active
title: A committed sequence for shared-checkout git operations
owners:
- marius
tags:
- shared-checkout
- hooks
- conventions
topic: multi-session commit coordination
---

# A committed sequence for shared-checkout git operations

The rules for committing on a checkout several agent sessions share are all written down
already — in `CLAUDE.md`, in four pre-commit hooks, in a dozen bug files. **The sequence
is not.** That is the whole of this page.

**Measured cost of not having it, 2026-09-01:** one two-author commit took **nine
cross-session messages, two windows where the shared working tree was red, one refused
commit, and roughly two hours of two sessions' wall time.** Both sessions knew the rules.
Neither could predict the order, because each hook fires at its own collision and teaches
only its own rule — so the sequence gets learned one collision at a time, and every
collision costs a round-trip to another session.

## Where this text lives, and why in two places

| copy | what it is | who reads it |
|---|---|---|
| `scripts/commit-sequence-tail.txt` | the six steps, terse | anyone who just tripped a hook |
| this page | the same six, each with the measurement it was paid for | anyone deciding whether to change one |

**That is a summary and its source, not two copies** — the same split as `CLAUDE.md`'s
gate sentence against [`gate-ordering.md`](gate-ordering.md). The terse copy is emitted by
**every** refusing hook from one file, so those cannot drift apart; this page is
the only place a rationale lives, so there is nowhere for a rationale to drift *to*.
The emitting set is enumerated in `tests/hook_config.rs`'s
`every_refusing_hook_emits_the_shared_tail` and deliberately not counted here — it has
already grown once, when the cargo-fmt hook replaced a framework entry that could not emit
the tail at all.

**A summary can still drift from its source in ONE direction, and did.** Until 2026-09-14
the terse copy's step 4 read `git add <paths> && git diff --cached && git commit`, while
§ *4* below had the separate-line block throughout — so the copy actually PRINTED at the
moment of need carried the chained form that this page's own step 4 cites `1b40dabd` for.
The drift ran source-correct / summary-wrong, which is the dangerous direction: the
summary is what a session reads mid-refusal, and the source is what someone reads when
deciding whether to change a step. Now gated by
`the_tail_teaches_separate_calls_never_a_chained_commit`
(`docs/issues/archive/2026-09-13-the-commit-sequence-tail-teaches-a-read-step-that-cannot-fail.md`).

Deliberately **not** a `get_guide` topic. That route was designed, then refuted — see
§ *Why not guide injection* below. The short version: it cannot be built, and it fails
silently.

## The sequence

### 1. Enumerate before assuming you are alone

`scripts/peer-sessions.sh`, never `ListAgents`.

**Measured 2026-09-01:** `ListAgents` reported **2** peers. The real figure was **16 live
sessions across 3 profiles, 6 of them in this checkout.** `ListAgents` reads
`$CLAUDE_CONFIG_DIR/sessions/*.json` — per-profile — while `SendMessage` delivers over
`/run/user/<uid>/cc-socks/<pid>.sock`, which is per-user. Discovery is narrower than
delivery, and nothing marks the count as a subset.

### 2. Identify your own work positively

By the `Session-Id` trailer, or `scripts/file-provenance.py` — **never by a commit range**.

A range is a proxy for authorship and stops being one the moment anyone else commits. A
review package scoped by a correctly-recorded `BASE` collected **14 commits, 1 of them
mine**. And when `file-provenance.py` answers, report **the window it searched**: it is
scoped to writes after a timestamp, so an older edit is invisible to it and its silence is
not evidence of absence.

**Never close an authorship question by elimination.** Ask the session; the harness makes
its id a path component of its own scratchpad, so the id is *given* rather than inferred.

### 3. Decide the coupling before you stage

A count and its member. A citation and its target. These commit together or not at all,
because **every partial state is a red tree for every other session**.

The cost of getting this wrong is not yours to pay: a ledger bumped without its member reds
the gate for all six sessions until you finish. If you cannot commit both yet, commit
*neither* — an untracked file is invisible to `git ls-files` and therefore to the count
gates, which is why leaving work uncommitted is safe and leaving it half-committed is not.

**A gate and the change that falsifies it are a third such pair — and a plan can split them before
anyone stages.** An SDD plan that deletes or rewrites a test in a *later* task than the change that
makes it fail has sanctioned a red spanning the task boundary. Operator ruling 2026-09-24: not on this
checkout — fold the gate's change into the falsifying commit. Measured 2026-09-10: one such sanctioned
red was investigated independently by two peer sessions inside ninety minutes, neither of whom had
touched what it covered and neither of whom made a mistake
(`docs/issues/archive/2026-09-10-a-deliberately-red-commit-exports-a-red-only-its-author-can-interpret.md`).
The explanatory assertion message tried there is kept as defence in depth, not as the remedy: it
reaches the reader who opens the failure, and none of the results a fail-fast lane never ran.
**Unlike the two pairs above, no hook can catch this one** — plan shape is invisible to every gate — so
the ruling is also stated in `CLAUDE.md` § *Git Workflow*, the surface a plan author actually reads.

### 4. Stage, read the diff, then commit by pathspec

```
git add <paths>
git diff --cached --name-only     # the index is SHARED — confirm these are all yours
git diff --cached                 # read the content; that is the whole point
git commit -m "..." -- <paths>
```

Three separate traps, each measured:

- **A bare `git commit` commits the whole index**, including whatever a peer staged in the
  interval. `1b40dabd` took a peer's entire `OB-6` entry that way — the session staged one
  file, printed `git diff --cached --name-only`, and chained `&& git commit`, so the check
  ran and changed nothing. **Again on 2026-09-17 at `7b5f3d5b`, which is why this bullet
  carries two dates: the trap is live, not historical.** Same shape — explicit paths on the
  `add`, a check, `&& git commit` — and it took a peer's 73-line bug-file close-out. The peer
  had staged it in the window and confirmed so from their own transcript. **The variation
  worth naming is the check: that session ran `git status --short` instead of `git diff
  --cached --name-only`.** It carries the same fact — staged paths are lettered in column 1 —
  but buries it among every peer's unstaged path, so it reads as a SURVEY of the tree rather
  than a question about the index, and the reader confirms "my paths are there" without
  noticing what else is. Run the `--cached` form: its output is the commit's contents and
  nothing else.
- **`-m` goes before `--`.** After `--` everything is a pathspec: `git commit -- <paths> -m
  "msg"` exits 1 with `pathspec '-m' did not match any file(s)`.
- **Staging is what satisfies the unreviewed-content check, not the pathspec.** A pathspec
  commit takes the *working tree* at those paths, so on a shared checkout it can carry a
  concurrent session's writes. **This also breaks the assumption this step rests on** — that
  the diff you read is the diff that lands. It is not, for any path a second session touches
  between your `git diff --cached` and your `git commit`. Re-check immediately before
  committing (`git diff --quiet -- <paths>`) rather than trusting the earlier read.
  **`docs/trackers/issue-clusters.md` is the known hot file for this** — 22 classes share it,
  every bug filing touches it, and it once took 16 sessions and 53 commits in a day. Measured
  2026-09-03: a pathspec commit over it (`964df77e`) carried six Index-row edits belonging to
  another session. They landed correctly but under the committer's `Session-Id`. Nothing was
  lost — the attribution was wrong — so step 6 applies rather than a repair.

### 5. Verify the index, not the exit code

`git add` can fail on a contended `.git/index.lock`, and a retry loop that exhausts looks
identical to one that succeeded. Check the post-condition — `git diff --name-only -- <paths>`
empty, `git show --stat HEAD` after — rather than the command's return.

Check **after**, not only before: the index moves between your check and your commit.

**And when it has already moved, an empty `git diff --cached` pins the ORDERING after the
fact, without a clock.** Two sessions reconstructing a capture disagree about sequence, and
the instrument they reach for is timestamps — the one thing this checkout has already lost
time to, since a pair of stamps in mixed zones once read as a two-hour gap that was really
2 min 22 s. The git-native form needs no shared clock: a session that staged a path and then
finds `git diff --cached -- <path>` **empty** has learned that HEAD already matches, so
someone's commit landed between their `add` and that read. Combined with the committer's own
pre-commit index read, the window closes from both ends and every observation is a fact
rather than an inference.

**Its limit, which is the reason to state it rather than just use it:** it orders events
against *your own* commit and says nothing about WHO acted. Identity still comes from the
actor's own transcript, or from the socket route in `CLAUDE.md` § *Reaching a Peer Session* —
never from the commit's contents, which is exactly the inference that produced a wrong
mechanism claim in the record this technique then corrected.

(Technique from sessionId `9403d62d-116b-46ea-ac9b-004acff2b1cb`, 2026-09-17, resolving the
`7b5f3d5b` capture above.)

### 6. If a commit captures another session's file, stop

Do not `reset`, do not `amend`, do not `stash`. On a shared tree **the repair destroys work
that the defect only mislabels** — the capture is an attribution error, and a reset is data
loss. Report it, and let the other session decide.

`--no-verify` is never the answer either. The entangled case is exactly when these guards
are load-bearing.

## The entangled single file, where steps 4 and 6 both fail

Every step above takes the **file** as the unit of ownership. When two sessions' changes sit
inside one file, none of them reaches:

- **Pathspec commit** takes the working tree at that path, so it carries their hunks under your
  `Session-Id` — step 4's own measured trap, and here it is unavoidable rather than accidental.
- **Bare `git commit`** takes the index, which has the same problem the moment you `git add` the
  whole file.
- **Step 6** says stop and report, which is right *after* a capture. Here nothing has been
  captured yet and both sessions are blocked on the same file.

The construction that works is to **stage a version of the file that never has to exist on disk
afterwards.** `git add` snapshots *content*, not a path, so the index and the working tree can
deliberately disagree — and that disagreement **is** the split.

```
# 1. build a version carrying only YOUR changes
#    (their region reverted to HEAD, or to whatever base they wrote against)
# 2. git add <file>              -> the index now holds yours alone
# 3. restore THEIR region in the working tree
# 4. git diff --cached           -> yours. read it.
#    git diff                    -> theirs. read it too, as a separate call.
# 5. git commit                  -> commits the INDEX, not the tree
```

**Step 5 is a bare `git commit`, which step 4 above calls a trap, and here it is the precise
form.** The trap is that a bare commit takes the *whole index*; that is exactly what you want
once you have verified the index holds only your content. `git diff --cached --name-only`, run
as its own call per step 5, is what converts the trap into the tool. The two forms have not
swapped places — what changed is that the index is now a snapshot you constructed rather than
one that accumulated.

Measured 2026-09-15 on `docs/trackers/bug-fix-session-log.md`, the second known hot file after
`issue-clusters.md`. Two sessions held one file: an 11-row index backfill plus four rewritten
prescriptive blocks, and a `## W-140` `REFUSED` block plus its reconciled index row. Split this
way they landed as `84efa618` (68 insertions / 13 deletions) and `9b251426` (38 / 3). Neither
commit carried a byte of the other's work, and the second session re-verified the first at the
bytes before committing.

**The precondition, and it is not always met:** you must be able to reconstruct *their* version
of the region. HEAD serves when their change is uncommitted on top of it. **If their edits
overlap yours in the same lines, this is a merge and not a split** — wait, or ask them, and do
not reach for this.

**One guard will decline, and that is correct behaviour rather than a failure.**
`dead-artifact-ids` prints `NOT CHECKED — staged bytes differ from the worktree … It declines
rather than guessing. Passing open.` It is the only guard in the chain that can see the split at
all. Note what you are accepting: that check does not run for that commit.

**Tell the other session.** Their work is still uncommitted in a tree you have been writing to,
and they may not know you touched the file.

**This case is NOT in the served tail, and the reader who needs it is the one who cannot see
it.** § *Where this text lives* splits the tail (the six steps, terse, for anyone who just
tripped a hook) from this page (the rationale, for anyone deciding whether to change one). This
section is not a seventh step — it is the case where two of the six stop applying — so it does
not belong in that list. But it is reached by tripping `refuse a pathspec commit carrying
unstaged content` on a file a peer is also editing, which puts its reader squarely in the tail's
audience and not in this page's. Left unresolved deliberately rather than fixed by appending to
a surface every refusing hook emits: one instance is thin evidence for changing what six guards
print. Whoever meets it a second time should decide, and the cheapest form is probably one line
in the tail pointing here rather than the construction itself.

**If you get the reconstruction wrong, their content may still be recoverable.** `git add`
leaves a complete tree in the object store and `git restore --staged` only un-references it, so
`git fsck --unreachable` recovers it byte-exact — which is how 35 lines deleted during exactly
this procedure were restored. `bug-fix-session-log:W-142`.

## The empty intersection, which no sequence fixes

`foreign-index` accepts **only** a pathspec commit when the index holds a peer's paths.
`ledger-counts` accepts only a commit carrying every count *with* its member — which, on an
entangled index, is only the **bare** one. **The intersection is empty**, and no ordering of
correct steps escapes it.

That is a defect, filed separately at
`docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`, with
the mechanism re-derived from the hooks' source. This page cannot solve it; a sequence
teaches the choreography and does not remove the need for one.

## Why not guide injection

The first draft of this proposed a `shared-checkout-commits` guide topic injected on the
first git write. It is **unbuildable**, for three independent reasons, each read in the
bytes at `30b6fc41`:

1. **`relevant_guide_topic` never sees the input.** Its signature is
   `fn relevant_guide_topic(&self, _result: &Value) -> Option<&str>`
   (`src/tools/core/types.rs`). It is handed the *result*; no command string is in scope.
2. **The selector path cannot introduce a topic on its own.** `topic_declaring` sits
   *inside* `} else if let Some(content_topic) = self.relevant_guide_topic(&val) {` and only
   appends a candidate to a list that call must first open. It also selects only topics
   declaring `serves:` sections.
3. **`run_command`'s selector carries no command string.** It is
   `action_selector_key("run_command", input)`, and `run_command` has no `action` param —
   so the selector is the constant `"run_command"`, identical for `git commit` and `ls`.
   Since `30b6fc41` the default sits on the hot path for all 21 registered tools, so adding
   a command-string scan would be paid by every `symbols` call.

**The failure mode is silence** — the topic simply never arrives, and nothing reports that
it did not. Refutation credited to `compact-root-claude-md`.

Hook refusal texts have none of these problems: the trigger already exists, it costs
nothing when it does not fire, and it reaches every session on the checkout rather than
one MCP session.

**The honest bound:** a refusal still fires *after* the work it invalidates. This shortens
recovery from N collisions to one. It does not move the teaching before the work — only a
per-session worktree does that, which is what
`scripts/pre-commit-unreviewed-content.sh`'s own header already says.

## References

- `scripts/commit-sequence-tail.txt` — the emitted copy
- `scripts/pre-commit-foreign-index.sh`, `scripts/pre-commit-unreviewed-content.sh`,
  `scripts/pre-commit-ledger-counts.py` — the three that print it
- `docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`
- `docs/trackers/response-envelope-session-log.md` — `F-2`, `F-3`, `F-4`, `F-5`
- `CLAUDE.md` § *Reaching a Peer Session*
