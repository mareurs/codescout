---
status: open
opened: 2026-09-13
closed:
severity: high
owner: marius
related: []
tags:
- cluster/repro-env-diverges-from-gate-env
kind: bug
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

The refused session had staged exactly one file, `docs/issues/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md`.
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

Not started. Candidates, none costed:

1. **Have the checker notice the divergence and say so.** When the worktree copy of a gated corpus
   file differs from the index copy, prepend a line naming that fact and the session that holds
   it. Cheap, and it converts a mystifying refusal into a routing instruction. Does not prevent
   the refusal — deliberately, since the refusal is correct about the index.
2. **Refuse to run a checker whose own script is dirty**, or warn loudly. Blunt, and it disables
   the gate exactly while someone is working on it.
3. **Accept it and fix the remedy text**, which is what shipped here: the refusal now names a
   second addressee and an instrument (`scripts/file-provenance.py`) for finding them. That
   addresses the cost to the reader, not the cause.

## Workarounds

- The author reverts their gate edit until they are cleared to commit. Reverting one's own
  uncommitted change needs no authorization — it restores shared state rather than changing it —
  which is what was done here.
- Commit the gate and the corpus edit as one commit, and expect a window before it.

## Tests added

None. A regression test would have to assert about a two-tree divergence, which
`tests/issue_clusters.rs` cannot construct without staging into a shared index — the same reason
`a_class_gaining_a_member_names_it` is declared HOOK_ONLY.

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
