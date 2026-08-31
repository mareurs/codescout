---
status: open
opened: 2026-08-31
severity: high
owner: marius
related: []
tags: [librarian, append_entry, entry-id, cross-machine, collision, allocator]
kind: bug
---

# BUG: append_entry's committed high-water mark does not guard against a second HOST, only a second worktree

## Summary

`append_entry` refuses id allocation from a linked worktree because two trees would issue
the same `PREFIX-N`. Two *hosts* on divergent branches hit the identical failure with no
guard at all: both read their own committed `entry_high_water_<PREFIX>`, both allocate the
same next id, and nothing detects or repairs it at merge.

## Symptom (Effect)

Measured live 2026-08-31 on `docs/trackers/reconnaissance-patterns.md`:

```
desktop:  entry_high_water_R: 146      highest R-N heading: 146
laptop:   entry_high_water_R: 147      highest R-N heading: 147
```

The laptop issued `R-147` in a commit that is not pushed. Both of the allocator's
committed inputs on the desktop (frontmatter high-water 146, body max 146) resolve the
next id to **147**. An `append_entry` on the desktop would mint a second `R-147`.

No error, no warning. The collision is only visible after both branches merge, as an
ambiguous token whose citations resolve to nothing or to the wrong entry.

**Not executed.** The allocation was deliberately not run once the divergence was measured,
so this report describes a reachable state, not damage done.

## Reproduction

1. Two clones of this repo on separate hosts, both on `experiments`.
2. Host A: `artifact(action="append_entry", id_prefix="R", …)` against a ledger →
   allocates `R-<n>`, bumps committed `entry_high_water_R` to `<n>`, commits, does NOT push.
3. Host B (still at the older commit, so still reading `entry_high_water_R: <n-1>`):
   the same call allocates `R-<n>` again.
4. Merge. Two `## R-<n> — <title>` headings now exist in one namespace.

Observed 2026-08-31 with the desktop at what is now `4d2e5e58` (patch-id
`3687655cd2dc5849e87278015774349302fd977d`; the original SHA `97d3a4ec` was orphaned by a
rebase hours later — itself an instance of why this project pairs a SHA with a patch-id)
and the laptop at `6710b384`, common ancestor `2f434fba`.

The commit SHAs are context only. The defect is carried entirely by the two frontmatter
values quoted under Symptom, which is why those are recorded as values rather than as a diff.

## Environment

Arch Linux, codescout 0.15.0 at `2f434fba`, two hosts sharing one GitHub remote, catalogs
machine-local (`~/.local/share/librarian/catalog.db`, gitignored).

## Root cause

The allocator's three inputs are the committed frontmatter high-water mark, the ids the
markdown body already claims, and a machine-local reservation. `get_guide
("tracker-conventions")` § *Entry ids* states the first is *"the only one of the
allocator's three inputs that survives a fresh clone, an `artifact(action="move")`, and
compaction."*

That is accurate and insufficient. All three inputs are **per-checkout**. The frontmatter
mark travels with the repo, but a host that has not pulled reads the pre-bump value, so it
is only as fresh as the last fetch. Divergent branches make it stale by construction.

The worktree twin of this defect **is** guarded — the same guide states `append_entry`
*"refuses id allocation from a worktree session, on the same grounds it refuses `cites`:
an entry id is ledger-wide state and must key to the main tracker. Left unguarded, the
worktree's shadow row is a different `artifact_id`, so both trees issue the same number —
and nothing repairs it at merge, because the renumber only covers params rows."* Every
clause of that argument holds for two hosts. Only the detection does not: `is_linked_worktree`
reads a worktree `.git` pointer, and a second clone is not a worktree.

*measured 2026-08-31: the two `awk` reads of each host's frontmatter quoted under Symptom,
plus each body's max heading. The allocation itself was not executed.*

## Evidence

### The divergence, both hosts

As quoted under Symptom. Desktop and laptop differ by exactly one in both the committed
mark and the body max — the laptop's `R-147` sits in `6710b384`, unpushed, and
`origin/experiments` is at `801767d7`, behind both.

### The guard that exists for the sibling case

`get_guide("tracker-conventions")` § *Entry ids*, the "From the MAIN checkout only" bullet.
Its stated rationale is ledger-wide state and unrepairability at merge — neither of which
is worktree-specific.

## Hypotheses tried

1. **Hypothesis:** the committed high-water mark is sufficient because it travels in git.
   **Test:** read it on both hosts while their branches were diverged.
   **Verdict:** rejected — it travels, but a host reads whatever revision it has checked
   out, so "travels" and "is current" are different properties. **Evidence:** *The
   divergence, both hosts*.

## Fix

Not implemented; this is a report.

Options, none costed yet:

- **Detect and refuse, mirroring the worktree guard.** Refuse allocation when the ledger
  file differs from its upstream counterpart — cheap approximations are "the current branch
  has unpushed commits touching this file" or "`@{upstream}` is not an ancestor of HEAD".
  Matches the existing precedent and fails loud.
- **Detect and warn.** Allocate but attach a `corrections`-style advisory naming the risk.
  Weaker; this project's repair-and-continue law reserves silent repair for cases with
  exactly one correct reading, and this has none.
- **Make collisions repairable after the fact.** A `doctor` check for two active definers
  of one token, plus a renumber that covers body headings rather than only params rows —
  the gap the worktree rationale explicitly names.

Note the third is useful regardless: it is the only option that helps a collision that has
already merged.

## Tests added

None — no fix written. A regression test should assert that allocation is refused (or
flagged) when the ledger's committed high-water mark is not the one on the upstream branch.
A test that only exercises a single-checkout allocation is monotone under this defect and
would pass throughout.

## Workarounds

Pull before allocating an entry id, and prefer allocating from whichever host is furthest
ahead. Neither is enforceable, and a fetch immediately before the write still races a peer
push.

## Resume

Price the three fix options above. Start by reading how `append_entry` reaches its refusal
for worktrees (`references` on the worktree guard from
`src/librarian/tools/append_entry.rs:97`) and check whether that refusal site has access to
git upstream state, since option 1's cost is dominated by that question.

## References

- `get_guide("tracker-conventions")` § *Entry ids* — both the high-water-mark claim and the
  worktree refusal this report compares against
- `docs/superpowers/specs/2026-08-31-cross-machine-catalog-integration-design.md` — this is
  a measured counterexample to that spec's § 2.3 premise that no concurrent-edit conflict
  has been observed; it is an id-allocation conflict rather than a params one
- `docs/trackers/reconnaissance-patterns.md` — the ledger the divergence was measured on
