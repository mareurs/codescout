---
kind: bug
status: fixed
tags:
- cluster/repro-env-diverges-from-gate-env
closed: 2026-09-15
opened: 2026-09-13
owner: marius
related: []
severity: high
---

# A gate script edited in the worktree has already shipped

## Summary

`scripts/pre-commit-run.sh` invokes `scripts/pre-commit-ledger-counts.py` **from the working
tree**, so an uncommitted edit to that script is live for every session in the checkout the
instant it is saved. There is no commit boundary to reason about, and no way for the author to
stage the change "not yet".

The script then reads its corpus with `--source=index` by default. So an author migrating a
corpus and adding the gate that enforces it occupies a state where:

- their own `cargo test` is **green** — the Rust twin reads the worktree, where the migration is
  applied; and
- every other session's **commit is refused** — the hook reads the index, where it is not.

The author is the one party who structurally cannot observe their own blast radius.

## Symptom (Effect)

A session committing an unrelated file is refused by a rule about a file they never touched, with
a remedy addressed to someone else. Measured 2026-09-13, verbatim:

```
$ python3 scripts/pre-commit-ledger-counts.py
the Index table stores mechanism status again:
  IC-1 -- Index row carries a fifth cell: `partial; **split taken → IC-17**`
  ... 22 more rows, plus the header line
exit=1
```

The refused session had staged exactly one file, `docs/issues/archive/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md`.
It matched `pre-commit-run.sh:157`'s pathspec (`docs/issues/.*\.md`), which is all it takes.

## Reproduction

Verified live, not constructed. On a checkout where `docs/trackers/issue-clusters.md` has been
migrated in the worktree but not committed, and a new rule enforcing the migration has been added
to the script but not committed:

```
worktree  | id | class | slug | promotes to |              (migrated)
HEAD      | id | class | slug | promotes to | mechanism |  (not)

$ cargo test --test issue_clusters     → 27 passed; 0 failed      (reads worktree)
$ python3 scripts/pre-commit-ledger-counts.py  → exit=1           (reads index)
```

A pathspec commit makes it worse rather than better: git builds a temporary index from HEAD plus
the named paths, so the author's unstaged fix is invisible even to the author's own commit.

## Root cause

Two independently reasonable decisions:

1. `scripts/pre-commit-run.sh` runs the checker by path, from the worktree — correct, since a hook
   that ran a committed copy could never be tested before landing.
2. The checker defaults to `--source=index` — correct, since a gate must judge what is being
   committed, not what happens to be lying on disk.

Together they mean a gate's **code** ships at save time while its **data** ships at commit time.
Any rule whose truth depends on a corpus edit is therefore false for every session during the
window between the two, and the window is opened by an action that feels local.

## Why this is not simply "commit them together"

That is the right practice and it does not close the hole. Even a perfectly disciplined author has
a window between writing the check and committing it, and during that window the refusal falls on
*other people*, silently, with no signal to the author. The author's own gate is green throughout.

The narrower true statement, which is the one worth carrying: **a gate script edited in the
worktree has already shipped.** Treat saving it as the deploy.

## Suggested fix

**Candidate 1 SHIPPED 2026-09-15**, `e2706eac`, patch-id
`f7cbd6bbe14bedb15fcb932ee373c2cdb4b76228`.

The reproduction sharpened it from *"a cheap routing hint"* into something narrower and
worse: **the refusal is correct and the reader's next action is wrong.** Every refusal here
names a corpus file and sends the reader to it. When that file is dirty the bytes they open
are not the bytes the verdict was computed from, so the refusal says *"expected it to change
and to contain one of: `<slug>`"* while the file on disk contains exactly that slug.
Measured in an isolated repo, the same grep against both copies: worktree `1`, index `0`.
The rational conclusion from that pair is *"the gate is broken"*, and the action it licenses
is `--no-verify`. Candidate 3 (remedy text) had already shipped and could not reach this,
because the text it improved is the text doing the misdirecting.

**What it does.** On a refusal, and only on a refusal, `_emit_divergence_note()` names every
corpus path whose worktree bytes differ from its index bytes, says the verdict was computed
from the index, and warns that opening those paths can show the opposite. It does **not**
suppress or soften the verdict — the verdict is right about the index, which is what the
commit publishes.

**Where it lives, and why there.** In `_emit_sequence_tail()`, which all **seven** `return 1`
sites already route through — so a check added later inherits it without its author knowing
the hazard exists. A head position would read better and would be one more thing to remember;
`CLAUDE.md` § *Observer Blindness* position 3 prefers the placement that cannot be omitted.

**The question it puts to the other party is answerable, which is the OB-20 ceiling.** It asks
whether the edit is ready to **stage** — three answers they can actually give (yes, not yet,
I will revert it), each determining the reader's next move — rather than *"is it yours"*,
whose true state is often neither branch. It also says plainly that waiting clears nothing:
the index moves only when somebody stages.

**A wiring defect caught while writing it, now guarded.** `_SOURCE` was first assigned inside
the `--source=` branch of the arg loop. **The hook passes no arguments**, so that assignment
never runs on the one path that refuses real commits — green under every test driving the
script with an explicit flag, dead in production. It is `cluster/declared-not-wired` in
miniature, inside the fix for a different class. Mutation M4 restores it and reds 10
assertions.

Candidates 2 and 3 are unchanged: 2 (refuse to run a dirty checker) still disables the gate
exactly while someone is working on it, and 3 shipped earlier.
## Workarounds

- The author reverts their gate edit until they are cleared to commit. Reverting one's own
  uncommitted change needs no authorization — it restores shared state rather than changing it —
  which is what was done here.
- Commit the gate and the corpus edit as one commit, and expect a window before it.

## Tests added

`tests/pre-commit-ledger-divergence.sh` — **24 assertions across 7 cases**, wired into the
existing throwaway-git-repo CI job. `python3` + `git` only, deliberately: a case that skips
itself for a missing toolchain reports success while testing nothing.

**This file's own "Tests added: None" was scoped to the wrong instrument.** It said a
regression test *"would have to assert about a two-tree divergence, which
`tests/issue_clusters.rs` cannot construct without staging into a shared index"*. True of
that Rust test, and false of a shell harness: `tests/count-with-members.sh`,
`tests/mutation-probe.sh` and `tests/pre-commit-dead-artifact-ids.sh` all already build
throwaway repos under `mktemp -d`, where staging is free and reaches nobody.

**Cases 2–4 are the discrimination set**, each removing exactly one of the three conditions
the note requires. The mutation table is what shows they are not redundant — run against an
isolated **copy of both the script and the suite**, never the shared tree, because the suite
resolves `SCRIPT` to the repo's own file and mutating it in place ships a broken hook to
every concurrent session:

| mutation | result |
|---|---|
| drop the call from the shared tail | **KILLED** — cases 1, 5, 7 |
| drop the `--source=index` guard | **KILLED** — case 4, and *only* case 4 |
| drop the not-diverged guard | **KILLED** — case 2, and *only* case 2 |
| `_SOURCE` assigned inside the arg loop | **KILLED** — cases 1, 5, 7 |

**`mutation-probe.sh` does not fit this suite, and the reason is worth recording rather than
working around.** It carries only the mutated file into its worktree — the suite is
uncommitted, so that worktree builds at HEAD without it — and its verdict parser needs
cargo's `running N tests` line, which a shell suite never prints. Both would render
`INCONCLUSIVE`, which `--strict` correctly refuses to read as a finding.

**Four fixture faults, each found by running it and each now annotated on the line it
constrains.** Every one returned a plausible refusal rather than an error, so the suite
looked like it was exercising the rule it named while exercising a different one — two of
them passed their assertions against the wrong rule before being caught:

- the Index slug cell takes the **bare** slug; `cluster/<slug>` matches nothing in
  `parse_index_counts` and a different check fires;
- `valid_slugs` rejects any slug containing a **digit**, so `demo-1` is silently not a slug —
  `valid` comes back empty and the hook reds on its own row-count guard;
- the roster carries no `## IC-N` sections — those live in the per-class files, and
  `read_ledger` concatenates the two;
- the hook refuses a table with **≤ 10** parseable rows, its own vacuity guard, so a
  one-class fixture cannot reach any rule under test.

Case 6 also records a premise that was simply wrong: **outside a git repo the hook never
reaches a refusal at all** — `read_ledger("index")` returns `None` and `main` returns 0 by
design. The first draft asserted exit 1 there and failed against correct code. What it pins
now is the reachable claim: silent, zero, and no traceback.
## Resume

Nothing in flight. The remedy text (candidate 3) shipped with the rule; candidate 1 is unbuilt.

## References

- `scripts/pre-commit-run.sh:157` — the pathspec that decides which commits run the checker.
- `docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md` — the
  **closest prior art, and a different bug.** There the two worlds are *population vs content*
  inside one gate: `tracked_all_bug_files()` lists the index while `actual_counts()` reads the
  worktree, so a peer's half-written file moves the count. Here they are *code vs data* across the
  hook boundary: the checker's own source ships from the worktree while its corpus is read from
  the index. Same family, opposite halves — and the consequence differs, because this one leaves
  the author green while refusing everyone else.
  **Found by a prior-art check run AFTER this file was committed, not before.** That ordering is
  the defect worth recording: `git grep -il 'reads the index'` over `docs/issues/` would have
  surfaced it in one command at filing time. A peer filing on the same day proposed a fix that
  would have reintroduced a defect closed 48 hours earlier, for want of the same one command
  (`fc7ff085`, withdrawn). *"Already documented" is checkable* — check it before filing, not after.
- `docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md` — the same
  shape at a different pair of guards; reported as its third instance of the day by the session
  this refusal landed on.
- Reported from the receiving end by sessionId `9403d62d-116b-46ea-ac9b-004acff2b1cb`, who
  measured the worktree/HEAD column divergence before asking, and who declined the
  `git add` workaround on the grounds that staging into a shared index mutates state every other
  session reads. The framing *"a gate shipped ahead of its migration"* is theirs; the sharpening
  to *"edited in the worktree has already shipped"* came out of the exchange.
