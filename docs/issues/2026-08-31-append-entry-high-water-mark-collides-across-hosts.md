---
kind: bug
status: mitigated
tags:
- cluster/shared-resource-carries-no-owner
- librarian
- append_entry
- entry-id
- cross-machine
- collision
- allocator
closed: 2026-09-02
opened: 2026-08-31
owner: marius
related: []
severity: high
unverified: 'Three open gaps, not one. (1) Peer-ahead direction: detection is complete but prevention is partial by construction and stays so — the guard catches only the direction where THIS host is ahead. `@{upstream}` is a remote-tracking ref, stale until someone fetches, so a peer who allocates and pushes while this host has not fetched still collides undetected, and an unpushed peer commit is unreachable by any local check. That is why status is `mitigated`, not `fixed`. (2) Params-ledger gap, concrete surface: both components cover PROSE ledgers only. Component B''s guard sits inside the `a.entry_collection.is_none()` branch, so the params allocation path has no upstream guard. Component A is built on `entry_sections` (`## PREFIX-N — Title` headings), so a params ledger whose body carries an index TABLE rather than headings yields no EntrySection and a duplicate there is neither prevented nor detected. This is not closed by ''rows are machine-local'': a params ledger''s `entry_high_water_<PREFIX>` frontmatter AND its `| PREFIX-N |` index table are BOTH committed and BOTH merge — `body_claimed_indices` (`src/librarian/catalog/augmentation.rs:1282-1292`) counts index rows toward allocation, so the committed surface collides exactly like the prose one and is simply uncovered by either component. (3) Detector''s real-world track record: a whole-branch review ran entry_defined_twice against this repo''s corpus (1451 markdown files, 37 declared ledgers) and found 3 findings, all false positives, all sub-headings repeating their own entry''s token (e.g. `### A-28` nested under `## A-28`); those are now excluded (0cb617cc) by dropping a definition strictly deeper than, and inside the span of, an earlier definition of the same token. Post-fix measured corpus output is zero findings — the check has never yet fired on a real collision; its true-positive rate is validated only against fixtures, not against a real occurrence.'
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

## Fix provenance

All nine commits below are on branch `entry-id-collision`, **not yet merged to
`experiments`** at the time of writing. `experiments` is rebased after every ship, which
orphans SHAs, so the patch-id is the durable identifier for each of these — the SHA is
recorded alongside it only because `structured_fix_pointers` (`doctor.rs`) reads the
`- **SHA:**` / `- **patch-id:**` pair and treats prose without it as `NOT VERIFIED`.

Component A (detection — `doctor` check `entry_defined_twice`):

- **SHA:** `374c75dc`
  **patch-id:** `de1f07d691d833ce028bfb389050a689f4ae737f`
  feat(doctor): duplicate_definitions, re-derived from headings not DocExtract
- **SHA:** `507bab94`
  **patch-id:** `8b6758da076236dbf1c1da007693143e539797e0`
  fix(doctor): build duplicate_definitions on entry_sections, not headings::parse
- **SHA:** `3f188500`
  **patch-id:** `ef1d059a3dcce94579e37ce407f0af3f474cc598`
  feat(doctor): entry_defined_twice — the cross-host merge collision
- **SHA:** `f9f9a269`
  **patch-id:** `df5f6a0b2b0664f3ce384a2347c11f23ae4b05c2`
  fix(doctor): scope entry_defined_twice to the active project, assert its wiring

Component B (partial prevention — `append_entry` upstream-freshness refusal):

- **SHA:** `ee7218e7`
  **patch-id:** `0aa94f94345c6f1f9726f6a6c51eb26e225315d0`
  feat(append_entry): per-file upstream-freshness helper
- **SHA:** `fb31cd6d`
  **patch-id:** `5ab44ae2f9692c6a0c6090b56585f79740584882`
  fix(append_entry): harden upstream-freshness tests and commit_path pathspec
- **SHA:** `276cd463`
  **patch-id:** `2935a15ef9a373ed6ee4f52cd0061c2ed14318c1`
  feat(append_entry): refuse allocation against an unpushed ledger
- **SHA:** `d0f8fac5`
  **patch-id:** `00616e489ca014253d22bb860cc20c5e5bbe411c`
  fix(append_entry): address review findings on the unpushed-ledger refusal

Cross-cutting fix wave, both components:

- **SHA:** `0cb617cc`
  **patch-id:** `a46d29d58e80446d0cc77d7bc42dad638862a707`
  fix(doctor,append_entry): scope entry_defined_twice to real duplicates, fix guard/messages

All nine passed per-task review, a whole-branch review, and a final fix-wave re-review with
a **Merge** verdict. What each component does and does not cover is in `extra.unverified` on
this artifact's frontmatter, not restated here.
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
