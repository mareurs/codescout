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
**all three** refusing hooks from one file, so those three cannot drift apart; this page is
the only place a rationale lives, so there is nowhere for a rationale to drift *to*.

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
  ran and changed nothing.
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

### 6. If a commit captures another session's file, stop

Do not `reset`, do not `amend`, do not `stash`. On a shared tree **the repair destroys work
that the defect only mislabels** — the capture is an attribution error, and a reset is data
loss. Report it, and let the other session decide.

`--no-verify` is never the answer either. The entangled case is exactly when these guards
are load-bearing.

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
