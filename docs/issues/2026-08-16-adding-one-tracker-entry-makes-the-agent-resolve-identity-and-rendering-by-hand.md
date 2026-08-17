---
status: open
opened: 2026-08-16
closed:
severity: medium
owner: marius
related: []
tags: [friction, trackers, librarian, agent-ergonomics, id-allocation]
kind: bug
---

# BUG: adding one tracker entry makes the agent hand-resolve the next id, the row format, and the snapshot — three jobs the tool should own

## Summary

"Append one entry to a tracker" is a single intent. Performing it currently
costs an agent four manual sub-tasks, none of which are about the content:
find the next free `N`, know which of two incompatible workflows this tracker
uses, learn the rendered row's column order by copying a neighbour, and re-render
the snapshot. Every one is mechanical, every one is a place to be wrong, and the
id-allocation step is a genuine race when two sessions are live.

The capability is half-built, which is what makes it worth filing rather than
accepting: `append_entry` already takes `id_prefix` and allocates `BL-25` for
you. Prose trackers get none of that, and neither kind re-renders.

## Symptom (Effect)

Measured over one session, 2026-08-16, adding five entries across three trackers.

**Allocating the id.** For `reconnaissance-patterns.md` (prose, `R-N`):

```
grep(pattern="^## R-9[0-9]", path=...)                      -> 0 matches
grep(pattern="^## R-[0-9]+", path=..., mode="files")        -> 61 matches, no numbers
run_command("grep -o '^## R-[0-9]*' ... | sort -t- -k2 -n | tail -3")
                                                            -> R-87, R-88, R-89
grep(pattern="R-90|R-91", glob="docs/trackers/*.md")        -> 0 (collision check)
```

Four calls, one of them tripping the IL3 pipe advisory, to learn a single
integer. And the check was necessary: commit `ebdff151`'s subject claims it added
"R-90", which does not exist in any tracker — so the obvious `tail -1` answer
would have been contradicted by a commit message, and only the fourth call
settled it.

**Two workflows for one act.** `prompt-hamsa-audit-log` and
`open-issue-work-queue` take `artifact(action="append_entry", id_prefix=…)`.
`reconnaissance-patterns` takes `artifact(action="update", patch={body_edits:
[{heading, action:"insert_after", …}]})` with a hand-written markdown section.
Nothing in a `find` result says which. The agent learns it by trying one.

**Row format by imitation.** Adding `BL-25..28` to the rendered snapshot meant
reading a neighbouring row to recover the column order
(`| id | phase | task | status | \`bug\` |`) and the convention that done rows
bold their status. No schema is consulted; a wrong column order would render as
a plausible table.

**And then it is still not durable** — see the sibling bug
`docs/issues/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md`.

## Reproduction

Add one entry to any prose tracker (`docs/trackers/reconnaissance-patterns.md`)
and one to any augmented tracker (`docs/trackers/open-issue-work-queue.md`).
Count the calls that concern content versus bookkeeping.

## Environment

codescout `experiments` at `bb11bba3`. Two Claude Code sessions live in the same
working tree, which is what makes the id race concrete rather than theoretical.

## Root cause

Entry identity and entry rendering are treated as *document* concerns, so they
fall to whoever is editing the document — here, the agent.

1. **Id allocation exists for one archetype only.** `append_entry`'s `id_prefix`
   scans the collection and allocates the next free id server-side. Prose
   trackers have no collection to scan, so `R-N`/`F-N`/`W-N`/`T-N` are allocated
   by grep-and-eyeball. The taxonomy in `docs/TAXONOMY.md` lists seven such
   prefixes; none is machine-allocated.
2. **The archetype is not surfaced.** `artifact(action="find")` returns
   `kind`/`status`/`title`/`path` and no indication of whether an
   `entry_collection` is declared, which is the single fact that decides the
   workflow.
3. **The rendered table is a convention, not a contract.** `render_template`
   exists in the augmentation, but nothing regenerates the body from it on write,
   so the column order lives only in the existing rows.

Consequence at the concurrency level: two sessions grepping for the highest `R-N`
within the same minute both compute `R-90`. Nothing detects the collision — the
second write is a valid markdown heading. `id_prefix` is race-free because the
catalog allocates under a transaction; the prose path has no equivalent.

measured 2026-08-16: the four-call id lookup above; the `ebdff151` R-90
discrepancy; `grep -o 'BL-[0-9]*' … | tail` to find BL-24 before appending.

## Evidence

### The half-built capability is the argument

`append_entry(id_prefix="BL")` returning `{"id": "BL-25"}` is exactly the right
shape — the caller states intent, the tool resolves identity. That it exists for
augmented trackers and not for the seven prose prefixes is what makes this a gap
rather than a design position.

### The collision check was not paranoia

`git log` shows `ebdff151` subject: *"docs: external-user bug reports, six
tool-quirk findings, and R-90"*. `grep(pattern="R-90|R-91", glob="docs/trackers/*.md")`
returns nothing. A commit message asserts an id that no tracker holds; an agent
trusting either the commit log or a bare `tail -1` gets a different answer than
the one that is correct.

## Hypotheses tried

1. **Hypothesis** — prose trackers should simply be migrated to augmented ones,
   making `id_prefix` cover everything. **Test** — none run. **Verdict** —
   deferred, and probably too broad: `reconnaissance-patterns` entries are long
   prose with headings and no fixed field set, which is why they are prose. The
   fix likelier lies in allocating ids for prose, not in reshaping the content.

## Fix

Sketch, smallest first — this needs a design decision, not just an implementation:

1. **Allocate prose ids server-side.** An action that, given a tracker and a
   prefix, scans the body's headings for `^#+ <PREFIX>-(\d+)` and returns the
   next free number under the same transaction `append_entry` uses. Removes the
   four-call lookup and the two-session race in one step.
2. **Surface the archetype in `find`.** Add `entry_collection` (or a boolean) to
   result rows so the caller knows which workflow applies without probing.
3. **Regenerate the rendered section from `render_template` on entry write.**
   Also fixes the sibling staleness bug; the two should probably be done
   together.

## Tests added

None yet — no fix chosen. For (1) the test is: two concurrent allocations against
the same prefix return different numbers.

## Workarounds

For prose trackers, allocate with a single bounded command rather than four
calls, and check the number is unused across all trackers before writing:

```
grep -o '^## R-[0-9]*' docs/trackers/reconnaissance-patterns.md | sort -t- -k2 -n | tail -1
grep -rn "R-<next>" docs/trackers/          # collision check, do not skip
```

Do not trust a commit message's claim that an id exists.

## Resume

Decide between fixes 1-3 with the user before implementing — this is an
ergonomics change to a surface many trackers depend on, and (3) overlaps the
sibling staleness bug, so they should be scoped together or explicitly split.
Start by counting how many of the seven `docs/TAXONOMY.md` prefixes are prose
versus augmented; if most are already augmented, (1) may not be worth building.

**ANSWERED 2026-08-17 — the escape hatch does not apply.** Of the **10 numeric
prefixes** in `docs/TAXONOMY.md` (F, W, R, U, H, T, WIN, A, PV, CAP), exactly
**one** is machine-allocated: **PV-N**, whose row names
`artifact(action="append_entry", id_prefix="PV", entry_collection="items")` and
calls it "atomic monotonic id". The other nine are hand-allocated. Two of them
(T-N, WIN-N) *had* augmented backing and still prescribed a hand-built array —
fixed in `9943164e`. So (1) is worth building.

**And the substrate check in this file's Fix section is out of date, in a way
that makes the fix smaller.** Verified against
`src/librarian/catalog/augmentation.rs::append_entry` on 2026-08-17:

- The allocator is **already cross-process atomic** — it runs inside a single
  `IMMEDIATE` transaction, documented as safe under both intra-process and
  cross-process concurrency with `busy_timeout`. The two-session race this bug
  describes is already solved *for callers that can reach it*.
- The allocator **already scans the body**. `body_claimed_indices(body,
  id_prefix)` folds in ids claimed by both `## PREFIX-N` sections *and*
  `| PREFIX-N |` index rows, and takes `max(params_next, body_max + 1)`, warning
  when params lags the body. Fix (1)'s "scan the body's headings for
  `^#+ <PREFIX>-(\d+)`" is therefore **already implemented** — just unreachable
  without an `entry_collection`.

So the gap is **coupling, not absence**: allocation is welded to a params write,
so a prose tracker cannot reach an allocator that would otherwise serve it.

**One correction to Fix (1)'s shape.** It says an action that "returns the next
free number". A lookup that returns an id for the caller to then write is still
read-then-write, and a peer can take the id in between — measured on 2026-08-17
with a four-minute margin (R-98). Atomicity is the property, not the lookup: the
id must be assigned by the call that writes the entry.

**One requirement Fix (1) does not yet name.** `link_scan`'s `def_re` is
`^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+` — a heading defines its token *only* as
`R-N — title`. So a server-side writer that formats the heading itself removes a
second defect class: an entry can never be born undefined. See HY-9 in
`docs/trackers/tracker-hygiene-log.md`.

Design now lives in **CAP-5** (`docs/trackers/capability-proposals.md`), which
carries the revised proposal: invert the dependency and extract the allocator so
both a params writer and a body writer depend on it, rather than adding an
`append_section` sibling that would encode a storage distinction as an API one.
This bug remains the friction evidence; CAP-5 is the design.

## References

- `docs/TAXONOMY.md` — the seven id prefixes, none machine-allocated
- `src/librarian/catalog/augmentation.rs` — `append_entry` / `id_prefix` allocation
- `docs/issues/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md` — sibling; fix (3) covers both
- commit `ebdff151` — the commit message asserting an R-90 that no tracker holds
