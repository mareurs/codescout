---
kind: bug
status: open
tags:
- cluster/doc-contradicted-by-code
- librarian
- trackers
- shared-checkout
closed: null
opened: 2026-09-14
owner: marius
related: []
severity: medium
---

# BUG: three recipes still prescribe append_entry's two-call form nine days after the one-call parameters shipped, and the fix's own file predicted it

## Summary

`doc(action="append_entry")` gained `index_row` + `index_after_line` at `8857b0b2`
(2026-09-05), so a ledger entry's section, its high-water mark **and** its index row land in a
single `fs::write`. Three documentation surfaces still teach the superseded two-call form —
append, then add the row in a later call — and none mentions the parameters. Sessions follow
the recipe, and every one of them reproduces the interval the fix was built to remove.

**The schema is NOT one of the stale surfaces, and saying so is load-bearing.** `append_entry`'s
served tool description carries both parameters, their both-or-neither coupling and their failure
mode, and did so in both affected sessions' context throughout. This is not a documentation gap;
it is a **worked example outranking a schema at call-composition time**. See § Root cause — a
reader who fixes the three recipes without learning that could reasonably conclude the parameters
were undocumented.

The archived bug that shipped that fix named this outcome in advance, in its own frontmatter:

> The window is closed for callers who USE the new parameters; it is not closed for callers who
> do not. `index_row` + `index_after_line` are opt-in … **a recipe in `docs/TAXONOMY.md` or
> `get_guide("tracker-conventions")` that still prescribes the second call will keep producing
> the window.**

## Symptom (Effect)

On 2026-09-14, between roughly 11:05 and 11:25, **two sessions produced three index-less
entries in one ledger**, each following a documented recipe:

| entry | author | state when observed |
|---|---|---|
| `F-145` | sessionId `6be73414-…` | body section, no Wins/Index row |
| `W-134` | sessionId `6be73414-…` | body section, no Wins Index row |
| `F-144` | sessionId `f0b1a4c7-…` (this one) | section written, row added by a **separate** `doc(update, body_edits=…)` call |

The consequence surfaced as a near-capture rather than as an error. This session staged
`docs/trackers/bug-fix-session-log.md` to commit `F-144`, read `git diff --cached`, and found
the other session's two sections inside it. `git add <path>` stages a whole file, so a pathspec
commit cannot separate two authors' entries within one ledger — `--name-only` reported exactly
one path and it was honestly shared.

Nothing was lost: the commit was not made, `git restore --staged` unstaged it, and the peer
committed both authors' entries at `19bee2ac` after being asked. **The catch was
`git diff --cached` being read, not the pathspec discipline** — which is step 4's *"read the
content; that is the whole point"* doing work the pathspec rule structurally cannot.

## Reproduction

1. `grep -n 'Then' docs/templates/session-log.md` → *"**Then** add the Index / Wins Index row,
   using the id the call returned"* (and three further restatements in the same file).
2. `grep -n 'Index row \*after\*' docs/TAXONOMY.md` → the `F-N` row: *"one call writes the
   section and records the high-water mark; add the Index row **after**, with the returned id."*
3. `grep -rn 'index_row' docs/ ` → **no hit**. The parameters are documented at the tool schema
   and in the archived bug, and on no recipe surface.

The fourth surface is in another repo: `codescout-companion/skills/reconnaissance/SKILL.md`
§ *Phase 3 — Externalize* carries the same *"**Then** add the Index / Wins Index row"*.

## Environment

`experiments`, 2026-09-14T11:25+03:00. 6 sessions with cwd = this checkout, 21 live overall
(socket enumeration, same instant). `index_row` present in `src/librarian/tools/append_entry.rs`
and in the live tool schema.

## Root cause

**The fix was opt-in and nothing migrated its callers** — but that framing is a coverage claim
and coverage was never the problem. Corrected 2026-09-14 by sessionId
`6be73414-4e...` (`bug-fix-session-log:F-146`, commit `23651065`), who hit this from the other
side and declined to simply agree:

> *"I was not missing documentation. `append_entry`'s own schema — served in my tool list, in
> context the entire session — describes both parameters, their both-or-neither coupling, and
> their failure mode. So code, tool and schema all agreed. What disagreed was the worked example
> in `reconnaissance` § Phase 3 — which I had loaded and was executing — and the example won."*

Verified independently at this session's own tool surface: `index_row` is described as *"written
in the SAME file write as the section; `{id}` becomes the allocated id. Both-or-neither with
`index_after_line`"*, and `index_after_line` as *"FIRST match … a line that does not exist writes
nothing and allocates no id."* Both were in this session's context for its entire duration, and
it produced the two-call form anyway.

**So the mechanism is RANK, not coverage.** A schema is read **once**, as a field list, at the
moment you are deciding whether a parameter exists. A worked example is read at
**call-composition** time and arrives as a complete, copyable shape. Both surfaces were present
to both sessions and there was no point at which comparing them was the task — which is why two
sessions with the correct contract in context reproduced a fixed defect within twenty minutes of
each other.

**The transferable claim, and it outlives these three recipes: a skill's worked example is a
second, unversioned copy of the tool's contract.** It decays independently of the schema while
being the copy that actually gets executed. This repo already knows the shape one level down —
`the_hook_script_agrees_on_the_cluster_parsers` exists because `scripts/pre-commit-ledger-counts.py`
duplicates a parser on purpose and *"this test is what stops it becoming drift."* A worked example
is the same duplication with no such test, and fixing the three recipes below does not address it:
the next skill to ship an example acquires the liability on day one.

**WHICH SURFACE TO TRUST WHEN THEY DISAGREE — and the tell is cheap, because "diff every example
against its schema" is not a thing anyone will do.** When a recipe and a schema name the same
call and **the recipe uses FEWER parameters, prefer the schema**: it is generated from the code,
the recipe is not. Stated here because a reader who fixes the three recipes and never learns the
schema was right could reasonably conclude the parameters were undocumented — they were not, and
the archived fix's own worked example used them.

**On the class:** `cluster/doc-contradicted-by-code` fits the *recipe* half — a recipe teaching a
superseded form denies a capability the code has. It does **not** describe the rank half, and no
class in the ledger does. Deliberately not opened here: this corpus's standard is that a class
opened in passing is one whose inclusion test nobody defends, and `F-146` is its instance ledger
until a second one arrives.

Three surfaces state the old protocol (measured 2026-09-14, `git grep` at `HEAD`):

- `docs/templates/session-log.md` — 4 restatements, including the anchor comment that sits
  **inside every ledger copied from it**, so the instruction is re-served at the point of use.
- `docs/TAXONOMY.md` — the `F-N` row, and `W-N` by reference (*"Same"*).
- `codescout-companion/skills/reconnaissance/SKILL.md` § Phase 3 — cross-repo.

`get_guide("tracker-conventions")`, named in the prediction, is **not** one of them; its only
nearby text is § *One entry format, never two*, which is about not duplicating the entry format
and is unrelated. Recording that because the prediction guessed two surfaces and was right about
one, wrong about the other, and missed the two that matter most — the template, which is copied,
and the skill, which is served.

**TAXONOMY is half-migrated, which is the tell that this is drift rather than a decision.** Its
`R-N` row already says *"**One call**: the server assigns the id atomically and writes
`## R-N — <title>` … (The old two-step — reserve, then write via `body_edits` — is obsolete)"*.
So the file absorbed the *section* half of the one-call story and not the *row* half, in the
same table.

## Evidence

### E1 — the fix exists and this session did not use it

`F-144` was appended with `doc(action="append_entry", id=…, id_prefix="F", anchor_heading=…,
title=…, body=…)` — no `index_row` — and its Index row added by a second call,
`doc(action="update", patch={body_edits: […]})`. Between the two the ledger held a complete,
unstaged, index-less entry. The archived fix's own worked example is one call.

### E2 — the recipe is re-served at the point of use

`docs/templates/session-log.md:238-247` is an HTML comment carried into every ledger copied from
the template, restating *"Then add the Index / Wins Index row with the id it returned."* A reader
who never opens `TAXONOMY.md` or the skill still receives it, from inside the file they are
editing.

## Hypotheses tried

1. **Hypothesis:** this is a rediscovery of the capture window
   (`docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md`).
   **Test:** read that file's § Summary and § Fix.
   **Verdict:** rejected. That bug is `fixed` at `8857b0b2`, and its § Summary already carries
   the correction that `git add <path>` captures whole files regardless of protocol. This file
   is about the **recipes that did not migrate**, which that bug names as residue and leaves
   `unverified`.
2. **Hypothesis:** `get_guide("tracker-conventions")` is one of the stale surfaces, as predicted.
   **Test:** `grep -n 'index_row|Index row|then add' src/prompts/guides/tracker-conventions.md`.
   **Verdict:** rejected — one match, § *One entry format, never two*, on a different subject.

## Fix

Not implemented. The change is to each recipe, not to the code:

- `docs/templates/session-log.md` — the 4 restatements, **including the anchor comment**, which
  is the one that reaches readers who open nothing else.
- `docs/TAXONOMY.md` — the `F-N` row, finishing the migration its `R-N` row already made.
- `codescout-companion/skills/reconnaissance/SKILL.md` § Phase 3 — cross-repo, and the skill's
  own maintenance section requires re-scoring before a description change; a body change to a
  worked example is not a description change, but the owner should confirm.

**Do not close this by deleting the index tables.** `get_guide("librarian")` is explicit that a
table row defines no citable token and that the table is worth keeping if it reads well; the
defect is the protocol taught for maintaining it, not the table.

**The durable half is a gate, and it is not obvious what it asserts.** A check that every
append passes `index_row` would be wrong — 28 of 49 guarded ledgers keep no row table. The
assertion that fits is on the *documentation*: a recipe that names `append_entry` and mentions
an index row must also name `index_row`. That is shape-testable in the way
`src/prompts/mod.rs`'s surface tests already are, and it reds on exactly the regression that
happened. Not built here; argued so the next person does not start from scratch.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet — no fix chosen, and the gate above is a proposal rather than a decision. A test now
would pin a shape nobody has adopted.

## Workarounds

Pass both parameters. One call, atomic:

```
doc(action="append_entry", id=…, id_prefix="F",
    anchor_heading="## Template for new entries", title=…, body=…,
    index_row="| {id} | 2026-09-14 | med | <category> | open | **<title>** — <text> |",
    index_after_line="|----|------|---------:|----------|--------|-------|")
```

`{id}` is a template the server fills — the caller cannot know the id beforehand.
`index_after_line` matches the **first** such line, so a ledger with several tables needs the
anchor chosen deliberately.

## Resume

**Start at `docs/templates/session-log.md`, not at TAXONOMY.** It is the surface copied into new
ledgers and the only one re-served at the point of use, so it is where a stale instruction
reproduces itself. Fixing TAXONOMY first leaves the template minting fresh copies.

Then check whether any ledger created from the template since 2026-09-05 carries the stale
anchor comment — those are live re-servers, and the count is a better measure of exposure than
the three surfaces are.

Do **not** re-derive the capture-window argument; it is settled in
`docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md`
§ Summary, including the correction that a single-call edit would have been captured identically.
This file claims only that the recipes did not migrate.

## References

- `docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md`
  — the fix (`8857b0b2`, patch-id `dec1af44d91b701b1ff7863e9f44b7946df470ce`) and the
  `unverified:` field predicting this residue.
- `docs/trackers/bug-fix-session-log.md` § `F-144`, `F-145`, `W-134` — the three entries, and
  `19bee2ac`, the commit that carried two authors' work because neither could pathspec out of it.
- `docs/trackers/issue-clusters/IC-11-doc-contradicted-by-code.md` — the class, for the recipe
  half only; see § Root cause on why the rank half has none.
- `docs/trackers/bug-fix-session-log.md` § `F-146` (`23651065`) — sessionId `6be73414-…`'s entry
  from the other side, and the source of the rank correction to § Root cause. Written with
  `index_row` + `index_after_line` in a single call, which is also the first independent
  confirmation that the workaround in this file behaves as documented.
