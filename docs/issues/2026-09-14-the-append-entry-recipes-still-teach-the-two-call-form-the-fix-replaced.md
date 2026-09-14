---
kind: bug
status: fixed
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
- `docs/TAXONOMY.md` — **four rows, not one**: `F-N` `:108`, `R-N` `:110`, `OB-N` `:118` and
  `IC-N` `:124`, the last three byte-identical (`Add the Index row after, with the returned
  id.`). `W-N` `:109` inherits `F-N`'s recipe via *"Same"*. **This file undercounted by
  three**, and the cause is this section's own claim one level up: it read the row the defect
  was *noticed* in rather than grepping the instruction, substituting a worked instance for
  the population.
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

**Implemented 2026-09-14** — see § Fix provenance. The change is to each recipe, not to the
code:

- `docs/templates/session-log.md` — the 4 restatements, **including the anchor comment**, which
  is the one that reaches readers who open nothing else.
- `docs/TAXONOMY.md` — the `F-N` row, finishing the migration its `R-N` row already made.
- `codescout-companion/skills/reconnaissance/SKILL.md` § Phase 3 — cross-repo, and the skill's
  own maintenance section requires re-scoring before a description change; a body change to a
  worked example is not a description change, but the owner should confirm.

**Do not close this by deleting the index tables.** `get_guide("librarian")` is explicit that a
table row defines no citable token and that the table is worth keeping if it reads well; the
defect is the protocol taught for maintaining it, not the table.

**The durable half is a gate, and the precedent above does not just support it — it rules out
the obvious form.** `the_hook_script_agrees_on_the_cluster_parsers` (`tests/issue_clusters.rs:1479`)
failed in two ways, and each predicts a way a worked-example gate would be decoration. Addendum
from sessionId `6be73414-…` (`bug-fix-session-log:F-146`, `ed777457`), who read the test rather
than taking this file's summary of it:

| candidate gate | analogue | verdict |
|---|---|---|
| *the example's parameters are a subset of the schema's* | parser parity | **decoration** — passes whenever today's example names every parameter it USES, and says nothing about the ones it OMITS, which is this defect exactly |
| *where a schema declares a coupling, an example using either parameter must use both, and one using neither must say why* | rule parity | **reds on the recipe we both followed** — `index_row` / `index_after_line` are both-or-neither and the recipe names neither |

The second form is better than the one this file proposed first (*"a recipe naming
`append_entry` and an index row must also name `index_row`"*), which is the right KIND — keyed
on an omission, not a subset — but hand-picked: a human had to know *"index row"* was the
trigger phrase. Deriving the trigger from the schema's own declared coupling generalises to
every coupled pair without anyone choosing phrases, and carries a *"say why"* escape, which is
the forcing function `NOT_HOOK_OWED` already uses in this repo.

**Both of the precedent's failure modes are real, and one is verifiable in a single command.**
`python3 scripts/pre-commit-ledger-counts.py --source=worktree --json` returns
`"claimed": []` and `"declared": {}` today — **two of that test's three arms compare empty to
empty**, so the corpus cannot reach the branches and a deletion there is invisible to it. Run
2026-09-14 by this session. The second failure is not vacuity but SCOPE: parser parity is not
rule parity, nothing compared the rule SETS, and the one-tag rule was absent from the hook for
months while redding the shared gate
(`docs/issues/archive/2026-09-09-the-pre-commit-cluster-hook-enforces-a-subset-of-the-gate-it-mirrors.md`).

**Re-derived 2026-09-14 at `c9561e1b` — the reported mutation result is FALSE, and the
re-derivation is worth more than the claim.** `F-146` stated that dropping the inline-`[a, b]`
arm from the Python leaves that parity test green. This session did not take it on report, re-ran
it, and it **reds**: three entries of `actual` move (`doc-contradicted-by-code` 39→38,
`selector-narrower-than-its-population` 43→42, `unclassified` 28→27), and
`the_hook_script_agrees_on_the_cluster_parsers` compares `actual` with `assert_eq!`
(`tests/issue_clusters.rs:1479`), so one moved entry is enough. `F-146`'s author re-derived the
same three numbers independently and withdrew the claim in place.

**So the precedent has ONE failure mode here, not two.** The vacuity leg is real for `claimed`
and `declared` and does **not** reach `actual`, which is the arm this mutation touches — extending
it there was an aggregate result applied to a member outside the aggregate. Only the SCOPE leg
survives: parser parity is not rule parity, nothing compared the rule SETS, and the one-tag rule
was absent from the hook for months while redding the shared gate
(`docs/issues/archive/2026-09-09-the-pre-commit-cluster-hook-enforces-a-subset-of-the-gate-it-mirrors.md`).
**The two-gate table above rests on the scope leg alone and stands unchanged.**

**What the false claim was worth is larger than the claim.** Its source is a doc comment
(`tests/issue_clusters.rs:805-808`) recording a mutation measured 2026-09-01, when zero bug files
carried a flow-style `cluster/` tag. Three ordinary filings later the corpus reaches the branch
and the comment is false — so **a mutation result over a live corpus decays exactly like a count,
and needs the same instant and tree**, which `CLAUDE.md` § *Testing Discipline* demands of counts
and of nothing else. Filed, fixed by `a2f6dda8` and archived:
`docs/issues/archive/2026-09-14-a-test-doc-comment-records-a-mutation-result-the-corpus-has-since-falsified.md`
(`b2b4078e23e0516d`, re-keyed from `33efc480b2da9ac3` by the archive move). Both sessions reached
it from opposite ends: this one stated it as a doubt
before either had the number, `6be73414-…` measured it and named the class. **The architecture pass
rejected every proposed mechanism, including the gate this section once floated** — the fix was to
state the fixture's property instead of a corpus snapshot, one line rather than a subsystem
(`docs/adrs/2026-09-14-state-the-property-not-the-snapshot.md`).

A check requiring every append to pass `index_row` would also be wrong — 28 of 49 guarded
ledgers keep no row table. The assertion belongs on the *documentation*, in the coupling form
above.

## Fix provenance

- **SHA:** `b325129a` (`experiments`) — the three recipes, and the regression guard
- **patch-id:** `d14c3dd7c6ceae506618dcc2267c016862d43ac7`
- **SHA:** `d811a2dc` (`experiments`) — the guard's matcher, which a mutation probe showed was
  decoration until it normalized markup and line wrapping
- **patch-id:** `b45a5c6d8925ff6ca60ae37798f88f407673eeae`
- **SHA:** `codescout-companion:e827162` (`main`) — the cross-repo skill surface
- **patch-id:** `3c9f27849404367ae45b7e7c67ffc18f8d0d3612`

## Tests added

`prescriptive_recipes_teach_append_entrys_one_call_form` (`src/prompts/mod.rs`). It scans the
prescriptive surfaces for the retired protocol and for the two parameter names, with the same
population cut as its sibling `reader_docs_contain_no_retired_call_forms` — `docs/issues/` is
excluded because this very file quotes the retired form as evidence.

**Its first version was decoration, and `scripts/mutation-probe.sh` is what said so.** Reverting
the template to the two-call form left it GREEN. The needles are plain phrases and the retired
protocol is not: it is written `**Then**\n> add the Index …`, with bold markers *and* a line
wrap inside the phrase, and TAXONOMY's `F-N` row wrote `*after*` mid-needle. Measured against
the original text of all five stale sites, a raw lowercase `contains` matched **1 of 5** — and
not either of the two surfaces that produced the incident. Normalizing emphasis, backticks,
blockquote markers and every whitespace run makes the phrase expressible: 5 of 5, no false
positive against the current text. Observed **KILLED (rc=101, 1 test ran)** after the repair.

**What it cannot tell you**, because both halves are monotone: absence under removal, presence
under widening. Paired they catch the two regressions that actually happened — reverting to the
two-call form, and dropping the recipe — and they do not check the recipe is *correct*. Nothing
reaches `index_after_line`'s two silent failure modes; those are prose, and they were found by
reproduction rather than by any gate (`context-injection-session-log:F-9`).

**The cross-repo surface is named by the guard and enforced by nothing.** The sibling-checkout
lookup resolves `CARGO_MANIFEST_DIR.parent()`, which is absent in CI *and* wrong inside any git
worktree, so it skips in both. Its proper home is `claude-plugins`' own suite.
## Workarounds

Pass both parameters. One call, atomic:

```
doc(action="append_entry", id=…, id_prefix="F",
    anchor_heading="## Template for new entries", title=…, body=…,
    index_row="| {id} | 2026-09-14 | med | <category> | open | **<title>** — <text> |",
    index_after_line="<the table line your row follows>")
```

`{id}` is a template the server fills — the caller cannot know the id beforehand.

**This section previously gave `index_after_line` as a literal separator, and that was wrong for
most ledgers.** Corrected after running it against the corpus rather than transcribing it
(`context-injection-session-log:F-9`); measured over all 20 live `docs/trackers/*session-log.md`:

- **Uniqueness.** The anchor matches the **first** equal line. The `## Index` separator is the
  first occurrence of its own byte string in **19 of 19** ledgers that have one, so `F-N` is safe
  everywhere — but `prompt-surface-measurement-session-log.md` uses `|---|---|---|` for both
  tables and repeats it **9 times**, so a `W-N` append anchored there resolves to `:30`, the
  `F-N` table, and writes the row into it. No error; the entry is still created.
- **Position.** The row is inserted *after* the match, so a separator anchor puts it at the
  table's **top**. `bug-fix-session-log.md` is newest-first and wants that;
  `context-injection-` and `statement-validity-` are oldest-first and want the bottom. The
  corpus has no convention, so a literal is correct for its author's ledger and silently wrong
  elsewhere — invisible to whoever writes it, because their own ledger confirms it.

**So use a rule, not a literal: anchor on the target table's last existing row.** It is unique by
construction, because ids are, and it lands the row at the bottom. A line that does not exist
writes nothing at all and allocates no id — also not an error.
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
- `docs/trackers/bug-fix-session-log.md` § `F-146` (`23651065`, addendum `ed777457`) — sessionId
  `6be73414-…`'s entry from the other side: the source of the rank correction to § Root cause and
  of the two-gate table in § Fix. Written with `index_row` + `index_after_line` in a single call,
  which is also the first independent confirmation that the workaround in this file behaves as
  documented. It carries the inclusion test for the rank half, for whoever meets a second
  instance: *a second copy of a contract that decays independently of the original and outranks
  it at composition time.*
