---
id: b5a1dec5201da97b
kind: spec
status: active
title: Cross-host entry-id collision — detection first, then a partial guard
tags:
- librarian
- append_entry
- entry-id
- cross-machine
- doctor
- design
topic: entry id allocation across hosts
---

# Cross-host entry-id collision — detection first, then a partial guard

## Problem

`append_entry` allocates `PREFIX-N` from three inputs: the committed
`entry_high_water_<PREFIX>` frontmatter mark, the ids the markdown body already
claims, and a machine-local reservation. **All three are per-checkout.** Two hosts
on divergent branches read their own copies, allocate the same id, and nothing
detects or repairs it at merge.

Measured 2026-08-31 on `docs/trackers/reconnaissance-patterns.md`:

```
desktop:  entry_high_water_R: 146      highest R-N heading: 146
laptop:   entry_high_water_R: 147      highest R-N heading: 147
```

The laptop's `R-147` was unpushed. Both of the desktop's committed inputs resolved
the next id to 147, so an `append_entry` there would have minted a second `R-147`.
The allocation was deliberately not executed — the record describes a reachable
state, not damage done.

Full record: `docs/issues/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md`.
Class: `docs/trackers/issue-clusters.md` `IC-17`, whose remedy clause is *isolate the
resource or add an owner field, never a better listing*.

## Why the sibling guard does not generalise

`append_entry` already refuses allocation from a linked worktree
(`src/librarian/tools/append_entry.rs:93-103`). Its stated rationale — an entry id
is ledger-wide state, and nothing repairs a collision at merge because
`merge_worktree`'s renumber covers params rows only — holds word for word for two
hosts. **Only the detection does not:** `is_main_checkout_artifact` reads a worktree
`.git` pointer, and a second clone is not a worktree.

## The asymmetry that shapes this design

The bug file offers "refuse when the ledger differs from its upstream counterpart"
with two cheap approximations. Working through it surfaces a limit the record does
not state, and it is the reason detection is built first.

**The check catches only the direction where I am ahead.** `@{upstream}` is a local
remote-tracking ref, fresh only as of the last fetch — measured on this clone
2026-09-02, two hours stale. So:

> Both hosts in sync at mark 146. A allocates 147, commits, **pushes**. B has not
> fetched, still reads 146, holds no unpushed commits — the guard allows — and
> allocates 147. **Collision.**

The 2026-08-31 incident is caught only because *both* hosts happened to hold
unpushed work.

**And the guard does not prevent the collision even when it fires.** If I hold
unpushed ledger commits, a peer at origin allocates from origin's mark and collides
with my entries whether or not I am refused. What the refusal converts is an
*invisible* divergence into a *pushed* one: once pushed, a peer that fetches reads
the true mark. **The refusal's entire value is in its remedy text**, and the design
below says so at the refusal site rather than in a doc.

**One direction is permanently closed to any local check.** An unpushed peer commit
is unreachable, fetch or no fetch. Prevention here is partial by construction, which
is why the complete half is built first.

## Component A — `doctor` check `entry_defined_twice`

Detection. Complete, cheap, and impossible to false-positive.

Scan each ledger body for `## <PREFIX>-<N> — <title>` definition headings, count by
token, and report any token defined two or more times **within one artifact**.
Read-only worklist, shaped like the existing `entry_*` checks, reported as
`{check, path, detail}`.

**"Ledger" means an artifact DECLARING `entry_prefix` in frontmatter, never one
inferred from content.** `get_guide("tracker-conventions")` § *Declaring a ledger*
measured 27 unaugmented trackers under `docs/trackers/` of which only **three** were
ledgers; the rest are design docs, research notes and finished session logs. A design
doc quoting `## R-4` twice in prose is not a duplicate definition, and inferring
ledgership from content would fire on exactly those files. The check reuses the same
declaration the allocator and the librarian guard already key on, so it cannot drift
from them.

**Scope is one artifact, deliberately.** A token defined in both a live ledger and
its archive companion is the compaction ladder working as designed —
`get_guide("tracker-conventions")` states the resolver binds a token to its sole
*active* definer, so archived-plus-active is not a collision. Cross-artifact
duplication is `link_scan`'s `ambiguous` bucket and `doctor`'s `prefix_conflicts`,
both of which already exist. This check owns only the case neither can see: **two
active definitions inside one file**, which is what a cross-host merge produces.

**This state is invisible today, and the reason is specific.** In
`src/librarian/tools/link_scan/resolve.rs:319-329`, `CitationKind::EntryToken`
short-circuits:

```rust
let definers = index.definers(&citation.raw);
if definers.iter().any(|d| d.artifact_id == src_id) {
    return Some(Outcome::SelfCite { dst_id: src_id.to_string() });
}
match definers.len() { 0 => ... }
```

A same-file duplicate pushes two `DefinerRef`s carrying the **same** `artifact_id`,
so an entry citing a duplicated sibling in its own ledger returns `SelfCite` and
never reaches the ambiguity branch — and citing a sibling is the commonest citation
shape inside a ledger. The collision is therefore invisible precisely where it is
most likely to be cited from. `doctor`'s existing checks include
`entry_without_definition` and `ledger_defines_nothing`; there is no *defined twice*.

`doctor` is the right home rather than `link_scan`: this is a property of one
artifact's body, not of the citation graph, and it must be reportable on a corpus
whose citations all resolve.

## Component B — `append_entry` freshness refusal

Prevention. Partial, and labelled partial.

Before allocation, refuse when the ledger's own file has commits in
`@{upstream}..HEAD`. Sited with — and immediately after — the worktree guard, which
means **before `resolve_write_target`**: the existing comment at
`append_entry.rs:87-91` records that a refusal firing after it still leaves behind a
shadow row, augmentation, fork event and lineage link (the 2026-07-17 regression).

**Per-file, not per-branch, and this is load-bearing.** Measured on this checkout
2026-09-02, HEAD 34 commits ahead of `origin/experiments`:

| ledger | unpushed commits touching it | verdict |
|---|---|---|
| `docs/trackers/reconnaissance-patterns.md` | 0 | allow |
| `docs/trackers/issue-clusters.md` | 10 | refuse |
| `docs/trackers/tracker-hygiene-log.md` | 0 | allow |

A branch-wide check refuses all three, permanently — `experiments` is pushed rarely
and being tens of commits ahead is the normal state here. The per-file form
discriminates, and that discrimination is the whole difference between a usable
guard and one that is disabled within a day.

## Error handling

- B returns `RecoverableError::with_hint`, matching the worktree guard's shape. The
  hint names the remedy — **push this ledger's commits** — because that, not the
  refusal, is what removes the hazard.
- **No configured upstream, or a non-git root, allows the allocation.** A repository
  with no remote has no second host, so refusing there is a pure false positive with
  no recoverable reading.
- A git error of any other kind also allows, and says nothing. B is a partial guard;
  degrading it to a hard failure on an unreadable repo would trade a real capability
  for no safety.
- Machinery is in-crate: `git2::Repository::discover`, `branch.upstream()`, and
  `graph_ahead_behind`, with precedent at `src/retrieval/index_state.rs:327`
  (`behind_count`).

## Testing

Against `CLAUDE.md` § *Testing Discipline*:

- **A asserts both directions.** A duplicated token is reported **and** a clean
  ledger is silent. An existence-only assertion ("a finding mentioning `R-147` is
  produced") is monotone under widening and would pass a check that fires on every
  ledger.
- **B's load-bearing assertion is discrimination, not refusal.** A two-repo fixture
  (origin plus clone): refuse when **this ledger** has unpushed commits, **and still
  allocate** when only an unrelated file does. A refusal-only test passes a
  branch-wide implementation, which the measurement above shows is unusable — so the
  refusal case alone is not evidence.
- **B's allow-paths are tested, not assumed:** no upstream configured, and a non-git
  root, each allocate successfully.
- **Mutation once per guarded site**, not once per feature. A's site and B's site
  kill different tests.
- The two-repo fixture's second clone is the load-bearing detail: with one clone the
  test cannot distinguish per-file from per-branch, and both implementations pass.

## Deliberately out of scope

- **The renumber / repair half** (the bug's option 3b). Rewriting a citable token is
  a separate decision from detecting a duplicate: every existing citation of the
  renumbered entry silently re-points. Detection first; repair is its own design.
- **Fetching before the comparison.** It closes the peer-pushed direction at the
  cost of a network round-trip inside `append_entry`, and still cannot see an
  unpushed peer. Revisit only if the doctor check shows collisions arriving through
  that specific path.
- **Host-partitioned id spaces.** The natural form is a suffix, and
  `get_guide("tracker-conventions")` § *Entry ids* records that `R-147a` is not a
  valid token at all — digit-to-letter is not a word boundary, so a suffixed id can
  never be defined or cited. That is `IC-6` (*parsers over a namespace owe an escape
  and a disambiguator*), and this design does not touch the grammar.

## Success criteria

1. `librarian(action="doctor")` reports `entry_defined_twice` on a ledger carrying
   two `## R-147 — …` headings, and reports nothing on the same ledger with one.
2. The check stays silent on a non-ledger artifact containing two identical
   `## R-147 — …` headings, because it declares no `entry_prefix`.
3. `append_entry` refuses allocation against a ledger with unpushed commits, naming
   the push remedy, and allocates normally against a ledger without them **in the
   same repository, in the same test**.
4. No change to the entry-id grammar, and no new dependency.

## References

- `docs/issues/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md` — the record
- `docs/trackers/issue-clusters.md` `IC-17` — the class; its `Mechanism status` was
  corrected at `3151201a` (patch-id `6de1f659eabfd098dcfc52140b92f7de1448f41f`) and now
  names `entry_high_water_<PREFIX>` as one of three genuinely unowned resources
- `src/librarian/tools/append_entry.rs:93-103` — the worktree refusal this mirrors
- `src/librarian/tools/link_scan/resolve.rs:319-329` — why a same-file duplicate is invisible
- `src/retrieval/index_state.rs:327` — `behind_count`, the git2 precedent
- `get_guide("tracker-conventions")` § *Entry ids* — the high-water claim, the
  worktree refusal, and the no-suffix rule
