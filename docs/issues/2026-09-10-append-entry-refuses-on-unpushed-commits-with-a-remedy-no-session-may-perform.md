---
kind: bug
status: investigating
tags:
- cluster/shared-resource-carries-no-owner
- librarian
- append-entry
- guard-remedy
claimed_at: 2026-09-10
claimed_by: 26cb9b5b-2c9c-489e-97d9-3a907c8b2941
closed: null
opened: 2026-09-10
owner: marius
related:
- docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md
severity: high
---

# BUG: `append_entry` refuses on unpushed ledger commits, and the only remedy it names is an action no session is allowed to take

## Summary

`append_entry` — the *only* write path into a declared ledger — refuses whenever that ledger's
own file has commits not on `@{upstream}`. The single remedy it offers is "push this ledger's
commits". Every session in this project operates under a standing instruction that pushing
happens only when the user asks, so the refused party structurally cannot perform the remedy.
Meanwhile the guard's own source says it **does not prevent the collision it names**. The result
is a hard block on a mandated workflow step, bought with a mitigation its author labelled
partial.

## Symptom (Effect)

Reproduced live 2026-09-10 while the `codescout-companion:reconnaissance` skill's Phase 3 —
which mandates `append_entry` — was executing:

```
doc(action="append_entry", id="03464a8808345846", id_prefix="F",
    anchor_heading="## Template for new entries", title="…", body="…")

→ {"ok": false,
   "error": "append_entry: this ledger has commits that are not on its upstream branch,
             so its `entry_high_water_` mark is ahead of what any other host can see",
   "hint": "Push this ledger's commits, then allocate. … If you cannot push right now
            (no network, no push access), do not write the entry by hand instead — a
            declared `entry_prefix` puts this file off-limits to direct `edit_file`.
            Note the entry somewhere worktree-local instead, and fold it into the
            ledger once these commits are pushed."}
```

The entry was written up in full and parked at
`.codescout/tmp/pending-ledger-entry-F-alias-adr-collision.md`. It is still not in the ledger.

## Reproduction

`git rev-parse HEAD` at filing: `2c8ee904`. Branch `experiments`.

1. On a branch with a configured upstream, commit any change touching a ledger file — one
   that declares `entry_prefix` in frontmatter. (Here: `47b818b8`, a peer's `F-12` entry in
   `docs/trackers/prompt-surface-compaction-session-log.md`.)
2. Do not push.
3. Call `append_entry` against that ledger.

Refuses every time. State at filing: **23 commits unpushed on `experiments`**, 4 from this
session and 19 from peers — the ordinary state of a shared branch that is pushed at ship time.

## Environment

codescout `0.15.0`, `doc(action="append_entry")`, `src/librarian/tools/append_entry.rs`.
Branch `experiments`, upstream `origin/experiments`. Seven Claude sessions share this checkout.

## Root cause

Two mechanisms compound, and only the second is a defect.

**1 — the guard, which is correctly built.** `ledger_unpushed_commits`
(`src/librarian/tools/append_entry.rs:423`) walks `@{upstream}..HEAD` and returns true if any
commit's diff touches this ledger's own path. It is **per-file, not per-branch**, and its doc
comment records why: *"Measured on codescout 2026-09-02: HEAD was 34 commits ahead of
`origin/experiments` … while 2 of 3 ledgers had zero unpushed commits touching them. A
branch-wide check refuses every ledger permanently and gets disabled within a day."* Every
failure path allows. This narrowing is right, and it is not the problem — the problem is what
happens on the ledgers that *are* being actively appended to, which is exactly the population
that matters.

**2 — the remedy is unperformable by the party refused.** The hint's primary instruction is
"push this ledger's commits". `CLAUDE.md` § *Git Workflow* and every session's standing
instruction make pushing user-initiated. So the guard refuses a session and directs it to do
the one thing it may not do unasked. Its fallback — park the entry worktree-local and fold it
in later — is performable, and it converts a completed piece of work into an obligation nobody
tracks and no query reports. The ledger's own `Status: open` population cannot see a parked
entry, because it is not in the ledger.

**And the guard states that it does not prevent the collision.** Verbatim, from
`append_entry.rs:143-147`:

> *"PARTIAL BY CONSTRUCTION, and labelled so. This does not prevent the collision — a peer at
> origin allocates from origin's mark and collides with these unpushed entries whether or not
> this caller is refused. What it converts is an invisible divergence into a pushed one, which
> is why the hint names pushing rather than the refusal."*

Its own regression test agrees, asserting on the hint text rather than on any prevented
collision: *"the hint is the entire value and the assertion is on the hint"*
(`append_entry.rs:1651`).

So the trade is: a **certain** block on the only write path into a ledger, in exchange for a
**partial** mitigation that leaves the collision reachable from the other side.

Measured 2026-09-10 by the reproduction above; guard read at `2c8ee904`.

**Why this is the same shape the project has already paid for once.** `CLAUDE.md` § *Testing
Discipline* records the `pre-push` foreign-session guard whose text said *"ASK THE AUTHOR"* — a
party who can report but cannot grant. Four sessions followed it and held for eight hours, each
correctly refusing to decide what none had authority over, and the pile grew 2 → 14 commits. It
survived a 54-assertion suite because every assertion was about the predicate. **This guard is
one rung further out: its addressee is not merely unable to grant, it is instructed not to act.**
And the analogous pile is already visible — 23 commits.

## Evidence

### The obvious verification command disagrees with the guard, and returns the reassuring answer

Measured 2026-09-10 by sessionId `26cb9b5b-2c9c-489e-97d9-3a907c8b2941` while trying to land two
entries parked by this very bug. A session that wants to know *"will `append_entry` refuse on this
ledger?"* reaches for the range the refusal names:

```
$ git rev-list '@{upstream}'..HEAD -- docs/trackers/bug-fix-session-log.md | wc -l
0
$ git rev-list --full-history '@{upstream}'..HEAD -- docs/trackers/bug-fix-session-log.md | wc -l
1
$ git log --full-history --format='%h %p %s' '@{upstream}'..HEAD -- docs/trackers/bug-fix-session-log.md
8cf67de0 2bfb2e15 36bfccd3 Merge branch 'experiments' into doctor-per-project-isolation
```

**`git log`/`git rev-list` apply history simplification by default and omit a MERGE commit that
touched the path**; `ledger_unpushed_commits` walks each commit in the range and reads its
diff, so it sees `8cf67de0` and refuses. Both are correct about their own question. The guard
is right; the diagnostic is the thing that lies, and it lies in the **allowing** direction, so
the reader concludes the ledger is clear and is then refused with nothing to reconcile.

That cost one refused call and a confident wrong prediction here. It is a separate defect from
the titular one and needs no code change to `append_entry` — what it needs is for the refusal to
name the blocking commits itself, since it has already computed them. **The hint says "push this
ledger's commits" and never says WHICH**, so the reader cannot check the claim, cannot tell how
many there are, and cannot discover that the answer is a merge rather than an ordinary commit.

**And it sharpens the titular defect rather than merely sitting beside it.** The blocking commit
here is `8cf67de0` — a merge a PEER created, folding a branch that had touched this ledger. This
session authored none of it. So the remedy is not merely unperformable because pushing is
user-initiated; the commits the caller is told to push are **not theirs to push** under the
pre-push foreign-session guard either, which would refuse that push in turn and route them to
the rung's author. After any merge on a shared branch this is the normal case, not the edge one:
two guards in the same repo, each correctly refusing, composing into a state with no legal move.

**Not fixed here.** Recorded so the next session does not re-derive it, and so the reproduction
in this file is read with the right instrument: use `--full-history`, or do not predict the
guard at all.

### The detection half of a reconciliation already exists

`doctor`'s `entry_defined_twice` (`src/librarian/tools/doctor.rs:3172`) detects precisely the
post-merge state this guard is trying to prevent, and names the cause: *"two clones each
allocated `{token}` from their own committed `entry_high_water_` mark before either saw the
other's commit."*

It is deliberately read-only. Verbatim from its doc comment:

> *"Read-only; there is no `fix=`. Renumbering rewrites a citable token, which silently
> re-points every existing citation — a separate decision."*

That conservatism is right in general and **over-broad for the case that matters here**: an
entry allocated minutes ago in an unpushed commit has, by construction, accumulated no
citations yet. The risk the deferral protects against does not exist for a freshly-allocated
duplicate, and codescout already holds the citation graph (`cites` edges, `link_scan`) needed
to tell the two apart.

### The check is sited where the answer is unknowable

At allocate time the local host cannot know whether a collision will occur: the competing
allocation is, by definition, also unpushed and therefore invisible. At **push** time the host
has network access and can fetch the remote ledger's `entry_high_water_` mark and compare. The
guard fires at the one moment the question cannot be answered, and is silent at the moment it
can.

## Hypotheses tried

1. **Hypothesis** — the guard is branch-wide and therefore trivially over-broad. **Test** —
   read `ledger_unpushed_commits`. **Verdict** — rejected. It is per-file, deliberately,
   with a measurement behind the choice. The over-blocking is real but narrower than it looks:
   it hits exactly the ledgers under active use.
2. **Hypothesis** — a direct `edit_file` write is an acceptable workaround. **Verdict** —
   rejected, and the hint says so: a declared `entry_prefix` puts the file off-limits to
   `edit_file` (`src/util/librarian_guard.rs`). Confirmed by the refusal text.
3. **Hypothesis** — this is the archived bug recurring. **Test** — read
   `docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md`.
   **Verdict** — rejected. That bug is the *collision*; this guard was its fix. This is a
   **residue of the fix**, not a recurrence, which is why it needs its own file.

## Fix

Not implemented. Three parts, in dependency order; the first is the one that unblocks work and
is cheap.

**1 — Site the check where the answer exists: `pre-push`, on a DIVERGENT push.** Allocate
optimistically. At allocate time the answer is unknowable — the competing allocation is, by
definition, also unpushed and therefore invisible.

**Corrected 2026-09-10 (peer sessionId `26cb9b5b-2c9c-489e-97d9-3a907c8b2941`), verified at the
bytes.** This section first said the check needs a `git fetch`. It does not: the `pre-push` hook
already receives the remote's real tip on stdin and the existing guard already trusts it —
`scripts/pre-push-foreign-session-guard.sh:129` reads
`local_ref local_sha remote_ref remote_sha`, and `:153` builds `range=("$remote_sha..$local_sha")`.
So the remote's committed `entry_high_water_<PREFIX>` is reachable without a network call.

**And the subtlety this section originally missed, which changes what "at push time" means.** On
a **fast-forward** push the remote's ledger is an ancestor of yours, so no collision exists and
none is detectable — there is nothing to check. The collision is *born in the MERGE* after a
rejected push. The knowable moment is therefore a **divergent** push, where `remote_sha` is not
an ancestor and `pre-push` runs before git rejects it. So part 1 is not "move the check later";
it is **site the check at the one moment the answer exists**, and that moment is narrower than
"push".

This also puts the refusal in front of a party who *can* act: a push is already a user-initiated
action, so refusing there interrupts something the user chose to do rather than something a
session was told to do.

**2 — `doctor --fix=renumber_uncited_duplicate`.** For an `entry_defined_twice` violation where
the later definition has **zero inbound `cites` edges**, allocate a fresh id, rewrite the
`## PREFIX-N — <title>` heading and any index row, and report the mapping. Uncited is the
precondition that makes it mechanical, and it is checkable from the catalog rather than assumed.
Where the duplicate *is* cited, stay read-only and report — the existing conservatism, kept for
the case it was written for.

**3 — Stamp allocation provenance on the entry.** Recording the allocating host/session and the
upstream mark it allocated from makes post-merge ordering deterministic instead of a judgement
call, and turns "which of these two is the later one?" into a lookup. Optional; parts 1 and 2
stand without it.

**Explicitly rejected: a `force` flag on `append_entry`.** It would be used every time, by every
session, because the refusal is unactionable — which is the same failure one level down.

## Tests added

None — not fixed. The regression test for part 1 is that `append_entry` succeeds against a
ledger with unpushed commits touching it, and that a `pre-push` run against a ledger whose
remote mark has advanced refuses with a remedy the pusher can perform. Both must be observed
red under mutation before they count.

## Workarounds

Park the entry in a gitignored path and fold it in after a push — the hint's own fallback, and
what was done here (`.codescout/tmp/pending-ledger-entry-F-alias-adr-collision.md`). It works
and it is lossy: the entry is invisible to every `find`, every `Status: open` sweep, and every
reader of the ledger until someone remembers it.

## Resume

**Claimed 2026-09-10** by peer sessionId `26cb9b5b-2c9c-489e-97d9-3a907c8b2941` (`codescout-29`)
on their operator's instruction, through the catalog. What they closed is the **Evidence**
subsection they added at `887a8b5a`, not a numbered Fix part: `git rev-list '@{upstream}'..HEAD --
<ledger>` returns 0 while the guard refuses, because git's default history simplification omits a
MERGE that touched the path while the guard's revwalk diffs against `parent(0)` and sees it. The
refusal said *"push this ledger's commits"* and never said WHICH, so the reader could not check
it. **Parts 1, 2 and 3 below remain open.**

Decide between part 1 (site the check at a divergent push) and part 2 (answer the narrower
working-tree question) — they are alternatives, not stages. Ship part 3 either way. Before
starting, re-run the reproduction: this guard was last touched around 2026-09-06 per its own
CI-failure comment, and `ledger_unpushed_commits` was mid-refactor on 2026-09-10 (widening from
`bool` to `Vec<String>` so the refusal can name the offending commits), so its signature and
behaviour may both have moved.

## References

- `src/librarian/tools/append_entry.rs:423` — `ledger_unpushed_commits`; `:170` — the refusal.
- `src/librarian/tools/doctor.rs:3172` — `scan_entry_defined_twice`, the detection half.
- `docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md` — the
  collision this guard was built to answer.
- `CLAUDE.md` § *Testing Discipline* — the `pre-push` "ASK THE AUTHOR" precedent: a guard whose
  remedy names a party who cannot act, measured at 2 → 14 commits over eight hours.
