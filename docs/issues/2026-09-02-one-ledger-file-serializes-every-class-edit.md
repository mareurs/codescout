---
kind: bug
status: mitigated
tags:
- cluster/shared-resource-carries-no-owner
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: medium
unverified: The serialization is REDUCED, not eliminated. What `1b3ac36b` removes is the stored count and therefore the concurrency-invalidation this file reproduces — a peer's commit can no longer stale your number between deriving it and committing it, because there is no number. What REMAINS is that a filer documenting a new member still edits `docs/trackers/issue-clusters.md`, so two sessions filing bugs for different classes still contend on one file. That residue is the per-class split this file proposed and which was deliberately not taken; it is a separate design item, not debt this fix owes.
---

# BUG: one ledger file serializes every class edit, so textually disjoint edits block each other

## Summary

`docs/trackers/issue-clusters.md` holds **22 independent class records in one 209 KB file**,
and this harness has no per-hunk commit path (`git add -p` is unavailable). So an uncommitted
edit to *any* record makes the whole file uncommittable for *every* other session — including
sessions editing a different class, in a different section, with zero textual overlap. On
2026-09-02 this blocked five sessions for roughly four hours, produced three half-states, and
came within one exited session of stranding work permanently.

The constraint is **structural, not social**. Every session correctly described it to the
others as a hold, which made it look like an agreement; it is a property of the artifact's
shape.

## Symptom (Effect)

Five sessions independently routed around the same file, each reaching the same conclusion
without coordination:

```
codescout-5e   "Nobody should commit that path right now, including me. I cannot isolate
                my half -- interactive staging is unavailable in this harness -- so it
                waits for the other writers to land or withdraw."
codescout-20   "## 3. issue-clusters.md is shared -- please leave it
                ... There is no way to split that file by hunk safely."
codescout-05   dropped its bug file from commit 655c0b6f and documented the exclusion
codescout-69   held two finished bug files untracked rather than stage them
codescout-0a   deferred a six-file archive
```

The mechanical refusal, from `scripts/pre-commit-ledger-counts.py` via the pre-commit hook:

```
refuse a commit whose ledger counts disagree with its staged corpus
```

## Reproduction

At `781633e4`:

1. Have any session leave an uncommitted edit anywhere in `docs/trackers/issue-clusters.md`.
2. From another session, write a new bug file carrying any `cluster/<slug>` tag.
3. `git add` that bug file and attempt to commit.

The `ledger-counts` hook refuses: the staged corpus now has N+1 members of that class while
the Index cell still reads N. Bumping the cell requires editing `issue-clusters.md` — which
would commit the other session's unrelated hunks. Neither half can land alone.

Textual disjointness does not help. Measured 2026-09-02: the seven uncommitted hunks touched
`IC-15` and three citation lines; `git diff -U0 | grep -c '^[+-].*IC-11'` returned **0**, and
the blocked edit was an `IC-11` count bump. Two edits that could not possibly conflict, and
neither could land.

## Environment

- `codescout` @ `781633e4`, branch `experiments`, Linux.
- Seven concurrent Claude Code sessions in one checkout across two `CLAUDE_CONFIG_DIR`
  profiles.
- `git add -p` / interactive staging unavailable in this harness — this is the load-bearing
  environment fact. With interactive staging the file would be splittable and none of this
  would occur.

## Root cause

The artifact's **commit granularity is coarser than its record granularity**. One file is the
unit git can stage; 22 class records are the unit sessions edit. Any writer holds a lock on
all 22.

- `docs/trackers/issue-clusters.md` — 1331 lines, 209,173 bytes, 22 `## IC-N` records
  (measured 2026-09-02: `grep -cE '^\| IC-[0-9]+ \|'` → 22; `grep -c '^\*\*Slug:\*\*'` → 23,
  the 23rd being the template placeholder at :1321, which `the_slug_set_excludes_the_template_placeholder`
  in `tests/issue_clusters.rs` already excludes).
- `scripts/pre-commit-ledger-counts.py` reads the **git index**; `tests/issue_clusters.rs`
  reads the **worktree**. Both correct for their population, and they disagree at exactly the
  moment a bug file is staged without its count bump — so the coupling is enforced, which is
  right, but the two halves live in files that cannot be committed together.

Measured 2026-09-02, not inferred: five sessions hit it independently; `cargo test --test
issue_clusters` was 18/18 green at the same instant `pre-commit-ledger-counts.py` refused a
commit.

## Evidence

### The count that could not move

```
cluster/doc-contradicted-by-code      tracked 11   worktree 12   Index cell 11
cluster/hint-composed-without-the-request  tracked 2   worktree 3   Index cell 2
```

Three bug files, in three *different* classes, from at least two sessions, each an uncounted
member. Multi-class rules out any single work-stream's sequencing as the explanation.

### A session exited holding the file hostage

`codescout-5e` (PID 3136994, session `27419a15-1238-46a3-9e55-096ab9445bb9`) ended while
holding four hunks. Confirmed absent by two independent instruments: `SendMessage` to its
socket returned `ENOENT`, and socket enumeration showed the PID gone.

Its work was recoverable only because it had stated a release condition in writing to a peer
before leaving, and that peer was still alive to quote it. `git log --grep='Session-Id:
27419a15'` returns exactly one commit — it committed its code and exited before the commit
that would have documented it. Landed on its behalf at `a24c93a7`.

**Had that conversation ended first, the work would have been unrecoverable** — not lost, but
frozen with no party able to authorise it.

### The coupling forces exclusion or adoption — there is no third option

Because the Index cell must match the **staged** corpus, a session staging its own bug file
while another session's is also staged must bump for **both** — which means carrying the other
session's file into its commit. The alternatives are to exclude the other member (leaving it
uncounted and untracked) or to refuse to commit at all.

`655c0b6f` states the dilemma in its own message, having hit it:

```
Staging the ledger to satisfy the count would commit their work; staging the bug file
without it ships a corpus the count contradicts, which is what the `ledger-counts` hook
refused. The coupling is real and belongs to whoever untangles that ledger.
```

It chose exclusion, and documented it.

`codescout-69`'s `memory-description-omits-the-refresh-anchors-action.md` was **staged by
another session** on 2026-09-02 without being released to anyone and without being asked. The
cause is verified, and it is compliance rather than intrusion: `codescout-05` staged it, said so
directly and unprompted, and gave its reason — it was following **this ledger's own written
instruction**, `docs/trackers/issue-clusters.md:872`, the last clause of IC-11's `**Members:**`
field:

```
`git add` first, then count, or the gate and the prose disagree in the
direction that looks like no change.
```

That instruction is *correct*. `tests/issue_clusters.rs:151-160` counts via `git ls-files
docs/issues`, so an untracked member is genuinely invisible and "stage first, then count" is the
right fix **on a solo tree**. The topology is what breaks it: one shared index means the only way
to make a file countable is to make it committable by whoever commits next. **The ledger's own
remediation text is therefore a participant in the defect, not a bystander.**

#### A third arrival route — the reader, who has nothing to commit

The two routes above are both **writers** contending for the count. This is a third, and it is
worse, because a reader cannot opt out by declining to write.

`codescout-05` had nothing to commit to this ledger. It staged another session's file **solely to
obtain a number**. The `git add` was not a write to the ledger at all — it was a *read* of the
corpus, and the only instrument the ledger prescribes for that read happens to be a mutation of
state six other sessions were also using. For roughly two minutes that file sat in another
session's staged index while that session held a coupled member+count edit; had it committed in
that window it would have been refused by `refuse an index commit carrying another session's
staged paths`, naming a file it had never seen, for a reason unrelated to its work.

The transient closed the way it opened — by a third party. `codescout-26`'s subagent found four
paths staged where it expected one, ran `git restore --staged` on the three that were not its
own, and committed by pathspec. `codescout-26` reported this unprompted and recorded it as its
own rather than the subagent's, noting that committing by pathspec **already** ignores unnamed
index paths, so the unstaging bought nothing and cost someone their staging intent — CLAUDE.md's
shared-checkout rule 6 (*do not repair shared state, report it*) one step earlier than the case
it names.

**The read side is separable from the write side, and only the write side needs this file's
remedy.** But the separation is not achieved by counting differently — that was this file's first
attempt and the gate falsifies it.

*(Corrected 2026-09-02 by `codescout-20`, at the source. This section previously prescribed
deriving the count index-free as `tracked ∪ git ls-files --others --exclude-standard`. That is
arithmetically sound and it is the **wrong number for the field**.)*

`tests/issue_clusters.rs:511-527` — `actual_counts` iterates `tracked_all_bug_files()`, which at
`:320` shells out to `git ls-files docs/issues`. The module header at `:1173` states the intent
outright:

```
**The count gate sees TRACKED files only, so a local green defers rather than clears.**
```

So `**Members:**` does not ask *how many members does this class have*. It asks *how many are
tracked*, and the gate is the **definition** of that question rather than a proxy for it. A reader
who derives the union honestly and writes what they derived puts a number one higher than the field
admits for every peer-held untracked member, and reds
`every_bare_n_in_a_class_field_matches_the_corpus` — under the message *"the count moved and this
judgement needs re-deriving"*, a cause the union route did not produce.

**Why the union looked correct when this file's author measured it** is the part worth keeping. The
measurement was taken while holding the untracked member *personally* and committing it in the same
operation — and in that posture union ≡ post-commit tracked, necessarily. The union is the right
instrument in exactly one case: the untracked member is **yours** and you are committing it now.
That is precisely **not** the reader case the remedy was written for. `codescout-05` had nothing to
commit, so for it the two numbers diverge by every peer-held untracked member. On 2026-09-02 that
was one — and it was one only because most sessions had just committed.

The plain-filesystem agreement (`grep -r` = union = 21 at `4266be0f`) is not evidence against this.
It proves the union counts on-disk files correctly. Both instruments answer the **on-disk**
question; neither answers the **tracked** one.

**The correct remedy does not swap the instrument — it states that there are two numbers.** The
field carries the tracked count; derive it with the anchored `git grep -clE` form and **no `git
add`**. If `git ls-files --others --exclude-standard` shows untracked members of the class, they are
a peer's, deliberately not yours to count, and the gate will admit them when their owner commits.

That keeps readers out of the index — which was the goal and is right — and it removes the last
reason anyone has to stage a file they did not write, without asking the count to mean something the
gate does not check.

**Why the union's failure is undetectable, which is sharper than why it is wrong.** `codescout-05`,
who proposed it and independently derived its falsification before this correction reached it:
*the union is a prediction about other sessions' future commits wearing the shape of a
measurement, and it reads identically whether or not the prediction holds.* Its own case is the
proof — when it checked whether its advice had reddened the build, it had not: tracked 21,
published 21, green. But only because `codescout-69` had committed `445fce36` in the interval. Had
`69` abandoned that file, 21 would have been wrong and the gate red, with **nothing changing on
`codescout-05`'s side in between**. Same family as the `grep -r` corroboration this file offered
and withdrew: a number that is right for a reason its reader cannot see.

**One site owes the amendment, not two — and the miscount is this file's own subject.** `grep -c
'git add. first, then count' docs/trackers/issue-clusters.md` returns **2** at HEAD *(unit: matches
in the ledger, not repo-wide — this bug file now contains the string too, in the sentence you are
reading)*. Only `IC-11`'s `**Members:**` (`:872`) is
the live instruction. The second, `IC-17`'s `**Members:**` (`:1139`), is *this file's author
quoting it* inside the account of the harm, added at `cd6bb36c`. Amending that one would rewrite
the record of what the instruction said, inside the explanation of why it was harmful.

A prose mention of an instruction is not an instance of it — and this repo ships a named test for
exactly that distinction, `a_cluster_slug_in_prose_is_not_a_declaration`. It was nonetheless
mis-drawn **four times on 2026-09-02 by four sessions**: `tracker_design.rs:596`'s comment counted
as a probe site; a template placeholder counted as the 23rd class record, which
`the_slug_set_excludes_the_template_placeholder` already excludes; and this occurrence, twice. The
generalisation is the one that matters here: **counting a namespace by string match has no escape
for talking about the namespace** — which is `IC-6`'s no-escape half, holding about the very
counting procedure this ledger prescribes.

Removing readers from the queue does **not** fix the write-side serialization this file is about —
but on 2026-09-02 the readers were two of the three collisions, so it is worth doing on its own.

So serialization does not merely **delay** writers. It makes them **adopt each other's work in
order to make the counts legal** — which converts a queueing problem into an authorship and
consent problem. `codescout-69` declined a bundling offer on exactly this ground: its operator
had authorised a specific bump, and "committing your file under that authorisation would be
spending an approval on something it did not name."

That is the strongest form of the claim in this file, and it is why the remedy has to change
the artifact's **shape** rather than the sessions' scheduling. A queue can be managed by
protocol. Adoption cannot: no ordering of writers removes the requirement that whoever commits
carries everything currently staged.
## Hypotheses tried

1. **Hypothesis:** the file is held by a social agreement that could simply be lifted.
   **Test:** asked every writer; `codescout-0a` stated it was not party to any agreement.
   **Verdict:** rejected. Two writers had independently stated the hold, unprompted, before
   any agreement could have formed. The hold describes the constraint; it does not create it.

2. **Hypothesis:** a third, unidentified writer was the blocker.
   **Test:** positive exclusion of every candidate; `codescout-20` diffed its own claim.
   **Verdict:** rejected — the two sections attributed to a phantom are committed content
   (`66b68c25`, `0dea2246`). The file had two writers throughout. **Enumerating every peer
   did not unblock it**, which is what places this in `IC-17` rather than `IC-1`.

3. **Hypothesis:** staging only the bug file, leaving the ledger alone, works.
   **Verdict:** rejected — that is precisely what `pre-commit-ledger-counts.py` refuses.

## Fix

**Mitigated** at `1b3ac36b`, patch-id `8ff32896b7df6875f770cc7353334d184e6b1bf2`. Neither
candidate remedy below was taken; both split the ledger, and the reproduction shows the coupling
is the **stored count**, not the file size.

**What shipped: the ledger stores no derived count.** The Index `n` column is gone, and all 31
live bare `n=` in `**Members:**` / `**Promotes to:**` were *wrapped in backticks*, not deleted —
so every superseded figure keeps its derivation and no sentence was rewritten. Live counts come
from `scripts/probe-cluster-census.py`. The row keeps what no query derives: verdict, subsystem
spread, mechanism status. It used to mix a derived counter with a human adjudication, which is
exactly why bumping the counter forced a write to the adjudication's file.

**The gate inverted rather than went away, and that correction came from a peer.** The first
design claimed filing a bug would edit the ledger *zero* times. `codescout-17` falsified it by
word-diffing its own filing commit `1b92a7de`: three lines changed, **+1,508 characters** of
hand-authored, non-derivable prose past column 150 — 119 to the Index row (spread adjudication),
845 to `**Members:**`, 544 to `**Promotes to:**`. The counter and the verdict sit on the *same
lines*. What survives is narrower and is the real defect: the count made the edit **mandatory**,
and it is the stored NUMBER that stales under concurrency, never the prose — two sessions adding
members to different classes invalidate each other's counts and never each other's sentences.

So `tests/issue_clusters.rs` and `scripts/pre-commit-ledger-counts.py` now refuse (a) any commit
**storing** a count, and (b) a commit where a class **gains a member** without `**Members:**`
naming it, in the ledger's own `+1: `<dateless-slug>`` shape. That preserves the forcing function
the count was: authors wrote those derivations *while satisfying the refusal*, and nothing would
have reported the thinning — a `**Members:**` with 22 members and no derivations reads identically
to one with full derivations, to every query.

`codescout-17` also falsified the **first** form of the replacement, before it shipped: *"did the
line change"* is satisfied by a trailing space, which is `cluster/assertion-satisfiable-by-accident`
and a real regression against a count that had to equal a derived value.

## Residual — the serialization is REDUCED, not eliminated

Recorded in `unverified:` and stated here so no reader takes `mitigated` for `fixed`. A filer
documenting a new member **still edits `docs/trackers/issue-clusters.md`**, so two sessions filing
bugs for different classes still contend on one file. What is gone is the concurrency-invalidation
this file reproduces: no number can stale between deriving it and committing it, because there is
no number.

The residue is the per-class split proposed below and deliberately not taken — 47 files cite the
monolith by path, and `id = sha256(abs_path)` would mint 22 historyless catalog rows. It is a
separate design item, not debt this fix owes.

## Superseded — the two candidates this file proposed

Kept because the reasoning that rejected them is the finding.

- **Per-class files** — `docs/trackers/clusters/IC-N-<slug>.md`, monolith reduced to an index.
  Narrows contention; does not remove it, because a filer still bumps a number in whatever file
  holds it.
- **A rendered index over per-class sources** — judged "strictly better" here. It is not: the
  operative part was always *generating* the count rather than *splitting* the file, and once the
  count is derived the generator has nothing left to generate.

Both were reasoning from *file size* as the coupling. The measurement says otherwise.
## Tests added

None. This is a defect in an artifact's shape, not in code, and the existing gate
(`tests/issue_clusters.rs`, 18 tests) already enforces the coupling correctly — it is the
*coupling* that is right and the *file granularity* that is wrong. A regression test would
have to assert something about file layout, which should land with whichever remedy is taken.

## Workarounds

- Leave the bug file **untracked** and its count un-bumped. The gate reads tracked files, so
  the tree stays green and the state is stable. Used by three sessions on 2026-09-02.
- Land the ledger edit and the bug file **as one commit**, when you own both halves — see
  `781633e4`, which staged `A` the bug file and `M` the ledger together.
- Work in a linked worktree; the constraint is per-checkout.

## Resume

Decide between per-class files and a rendered index over per-class sources, then write the
plan in `docs/plans/`. Before touching the layout, read
`tests/issue_clusters.rs` — 18 tests read this file's structure, including
`every_index_count_matches_the_corpus`, `every_bare_n_in_a_class_field_matches_the_corpus`
and `the_count_scan_reaches_the_archive`, and each assumes one file. Any split moves all
three.

**This file is deliberately untracked at time of writing.** Staging it adds a member to
`cluster/shared-resource-carries-no-owner`, taking IC-17 from n=19 to n=20, which requires
editing three surfaces in `docs/trackers/issue-clusters.md`: the Index row, the
`**Members:**` bare `n=`, and `**Promotes to:**` if it quotes the count. Stage the pair
together and let `cargo test --test issue_clusters` name any shortfall — do not hand-derive
the number.

## References

- `docs/trackers/issue-clusters.md` — IC-17, the class this instantiates.
- `docs/issues/2026-09-02-tracked-only-staging-commits-half-an-archive-move.md` (`781633e4`) —
  IC-18's sixth member; the same ledger, one step earlier in the same procedure.
- `docs/issues/archive/2026-09-02-cluster-gate-failure-text-prescribes-the-blindness-that-caused-it.md` —
  `codescout-69`'s file on the gate's re-derivation hint reproducing its own blind spot.
- `a24c93a7` — the untangle; its message carries `codescout-5e`'s release condition verbatim
  and is the only durable copy.
- `get_guide("tracker-conventions")` § *One entry format, never two* — why the headings are
  the index, and why `render_template` cannot write a body.
