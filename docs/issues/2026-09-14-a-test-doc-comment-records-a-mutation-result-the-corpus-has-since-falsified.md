---
id: '33efc480b2da9ac3'
kind: bug
status: open
title: 'BUG: a test doc comment records a mutation result the corpus has since falsified, and the same commit ships its contradiction 671 lines away'
tags:
- cluster/doc-contradicted-by-code
- testing
- trackers
closed: null
opened: 2026-09-14
owner: marius
related: []
severity: medium
---

## Summary

`tests/issue_clusters.rs:805-808` records a mutation result as a standing property of a test:

> measured 2026-09-01, **zero** bug files carry a `cluster/` tag in flow style (`tags: [a, b]`),
> so deleting the inline arm from the Python leaves the corpus-driven check **green**. Verified
> by mutation, not assumed — that deletion was made and the other test passed.

It is **false at HEAD**. Re-derived 2026-09-14 at `c9561e1b`: dropping the inline-`[a, b]` arm
from `cluster_tags` moves three counts and **reds** `the_hook_script_agrees_on_the_cluster_parsers`.

The prose was true when written. The **corpus** moved under it — the first flow-style tag landed
2026-09-03, two days later — and nothing detected the flip. `tests/issue_clusters.rs:1476`, added
in **the same commit** (`3be0088e`), already asserted the opposite. The file has carried both
halves of a self-contradiction for thirteen days and neither half reds.

## Symptom (Effect)

A reader deciding whether a guard is worth building reads `:805` and concludes the corpus cannot
reach the branch, so the mutation is invisible and the coverage is decoration.

That is precisely how it was used. A peer session (sessionId `6be73414-6293-4a4e-95a4-4bada8327f08`)
cited this reasoning in `bug-fix-session-log:F-146` to argue a proposed documentation gate would be
decoration, writing *"verified by mutation"* over a claim it had not re-run. It withdrew the claim
on its own re-derivation. The comment is the class of evidence that stops the next person looking —
it does not merely fail to help, it actively discourages the check that would correct it.

## Reproduction

Read-only for steps 1 and 3; step 2 writes only to a scratch copy.

1. The premise, at HEAD:

       $ git grep -clE '^tags: *\[.*cluster/' -- 'docs/issues/*.md' 'docs/issues/archive/*.md' | wc -l
       3

   The comment's premise says **zero**.

2. The mutation, isolated from `cargo` (the script resolves `LEDGER` and `LEDGER_DIR` from **cwd**,
   not from `__file__`, so a scratch copy run from the repo root reads the same corpus — no
   `target/` lock, no build):

       cp scripts/pre-commit-ledger-counts.py "$SCRATCH/mutated.py"
       # collapse cluster_tags' if/else to the block-style branch alone,
       # i.e. delete the `rest.startswith("[") and rest.endswith("]")` arm
       python3 scripts/pre-commit-ledger-counts.py --source=worktree --json > "$SCRATCH/base.json"
       python3 "$SCRATCH/mutated.py"            --source=worktree --json > "$SCRATCH/mut.json"

3. Diff the two. Three entries of `actual` move; `claimed` and `declared` are empty either way.

## Environment

- Branch `experiments`, HEAD `c9561e1b`, worktree clean of this session's work.
- `tests/issue_clusters.rs` 31 tests; `scripts/pre-commit-ledger-counts.py` 8 declared `HOOK_RULES`.
- Shared checkout — the corpus this depends on is written by every session that files a bug.

## Root cause

**A mutation result over a live corpus is contingent on that corpus, and was recorded as a
permanent property of the test.**

`CLAUDE.md` § *Testing Discipline* already requires that a **count** of a defect population arrive
with its unit, its instant and its tree, on the grounds that one population yielded four defensible
numbers inside an hour. A mutation result taken against the same population decays for exactly the
same reason and carries **none** of that discipline — it is written once and read forever as a
property of the test rather than of the day.

The script says as much about itself. `scripts/pre-commit-ledger-counts.py:853` notes the count
*"moves with every filing"*, which is the entire reason the fixture sibling
`the_hook_script_agrees_on_both_yaml_tag_styles` (`:816`) exists: it hardcodes both tag styles and
so cannot decay. **The durable coverage is in the fixture; the decayed claim is in the prose beside
it**, and a reader who trusts the prose never reaches the fixture.

**A variation on `IC-11` worth naming, because it widens the class's trigger.** The claim reads
*"the code later gained or lost the capability"*. Here **no code changed**. Three ordinary bug
filings changed the **corpus**, and the capability the comment denies is a kill the test thereby
gained. The authors of `07b7819c` and `81416a3e` were filing unrelated bugs; nothing told them a
test's doc comment rested on their choice of YAML tag style. That is the class's own observer
problem one level out — the party who invalidates the prose is not the party who can see it.

## Evidence

### E1 — the mutation, re-derived 2026-09-14 at `c9561e1b`

Mutated copy's `--json` diffed against the unmutated one's, same instant, same tree:

    actual: doc-contradicted-by-code:             39 -> 38
    actual: selector-narrower-than-its-population: 43 -> 42
    actual: unclassified:                         28 -> 27
    claimed: [] (0 entries, both)   declared: {} (0 entries, both)

Control that the mutation is real, not a no-op: `python3 mutated.py --fixture-tags` on the inline
fixture returns `[]` where the unmutated script returns the tag.

The step that turns moved counts into a RED is `assert_eq!(mine, theirs)` over `actual` in
`the_hook_script_agrees_on_the_cluster_parsers` (`tests/issue_clusters.rs:1479`) — an equality over
a `BTreeMap`, so any one moved entry fails it.

### E2 — the premise was true when written, and the date it stopped being true

    $ git grep -clE '^tags: *\[.*cluster/' 3be0088e -- 'docs/issues/*.md' 'docs/issues/archive/*.md' | wc -l
    0

First flow-style tag: `07b7819c`, 2026-09-03. Then `81416a3e`, 2026-09-03. Then `9f5c1785`,
2026-09-10. So the comment was accurate for **two days** and has been wrong for **eleven**.

### E3 — both halves of the contradiction ship in one commit

`git log -S` pins each string to `3be0088e` (2026-09-01) and finds no later edit:

| line | claim | true when written | true at HEAD |
|---|---|---|---|
| `:805-808` | dropping the inline arm leaves the corpus-driven check **green** | yes | **no** |
| `:1476` | *"Mutation that must kill this: … or drop the inline-`[a, b]` arm"* | **no** | yes |

Neither was ever checked against the other, and the corpus drift flipped **both** — so at no
instant in thirteen days was the pair consistent, and at no instant did anything red.

## Hypotheses tried

- *"The peer mis-ran the mutation."* — Falsified: re-derived independently here, same three
  numbers. The error was in extending a **vacuity** result (`claimed` / `declared` compare empty to
  empty, filed at `docs/issues/2026-09-13-two-of-three-parity-arms-compare-empty-to-empty.md`) to
  `actual`, a member that result does not cover. `CLAUDE.md`'s population-vs-member law run
  backwards.
- *"The `:1476` must-kill line is the stale one."* — Falsified: it is correct at HEAD. It was the
  wrong half on the day it was written.

## Fix

**Not implemented, and the mechanism is deliberately not designed here.**

The constraint any fix has to meet, stated so the next session does not re-derive it:

- **Do not replace the number with a fresher number.** *"measured 2026-09-14, three files"* decays
  on the next filing, which is the defect, not a repair of it.
- **Do not delete `:1476`.** It is correct at HEAD and names a real required kill.
- The honest minimum is to **delete the decayed measurement from `:805-808`** and keep the
  structural reason the split exists — the fixture pins both styles so the test does not depend on
  what the corpus happens to contain. That sentence cannot decay because it makes no claim about
  the corpus.
- Anything stronger is a mechanism question (what could red when a doc comment's corpus premise
  goes false) and should go through the architecture skill rather than be improvised here.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None. The defect is a false statement in a doc comment; no assertion reads it.

Worth recording as the reason this is hard: `the_hook_script_agrees_on_both_yaml_tag_styles` is
**green and correct** throughout, because its fixtures hardcode both styles. The test guarding the
behaviour is fine; only the prose explaining why it exists went false. No mutation of the
production path reaches it.

## Workarounds

Re-derive before citing any mutation result taken against the live corpus — the reproduction above
is two commands and needs no `cargo` run, so it costs seconds and holds no `target/` lock.

## Resume

Open. Filed on notice during a peer exchange; not scheduled.

## References

- `tests/issue_clusters.rs:805-808`, `:816`, `:1476`, `:1479`
- `scripts/pre-commit-ledger-counts.py` — `cluster_tags` (`:331`), the incidental-exercise note (`:853`)
- `3be0088e` — the commit that wrote both halves
- `07b7819c`, `81416a3e`, `9f5c1785` — the three filings that moved the corpus
- `docs/issues/2026-09-13-two-of-three-parity-arms-compare-empty-to-empty.md` — the vacuity result
  this was wrongly extended from
- `docs/issues/2026-09-14-the-append-entry-recipes-still-teach-the-two-call-form-the-fix-replaced.md`
  — § Fix, where the withdrawn claim was recorded and then flipped
- `bug-fix-session-log:F-146` — the peer's entry, corrected in place by its author
