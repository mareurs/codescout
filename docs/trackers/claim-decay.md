---
id: '7d813ce54cb9f1c5'
kind: tracker
status: active
title: Claim Decay — durable claims with no scheduled repair
tags:
- decay
- drift
- dogfood
- doctor-candidates
topic: claim decay, unrepaired assertions, drift detection
entry_prefix: DC
expects_augmentation: docs/augmentations/docs-trackers-claim-decay.yaml
---

# Claim Decay — durable claims with no scheduled repair

**The class:** a durable written claim that is (or becomes) false, where **no process
event is scheduled that would ever repair it**. The defect is not the wrong text — it is
the *absent repair trigger*. A claim nobody is required to re-check survives until a
reader happens to follow it.

This ledger is **evidence, not narrative.** Each `DC-N` is a structured row plus a short
section citing where the story is told in full. It exists to be *queried*: the point is to
find which decay kinds recur on which surfaces with no instrument watching, and to promote
the recurring ones into automated checks — `librarian(action="doctor")`,
`audit_doc_refs`, `link_scan`, or a test.

## Why this ledger and not an existing one

| Neighbour | Covers | Why this is different |
|---|---|---|
| `F-N` / `W-N` session logs | Per-work-stream friction + wins, narrated | DC is cross-stream and structured; a DC row **points at** the F-N that narrates it |
| `R-N` reconnaissance-patterns | Meta about the recon *skill* | DC is about the corpus, not the skill |
| `U-N` codescout-usage-frictions | Friction *using* codescout tools | DC is about claims decaying in artifacts |
| `T-N` tool-usage-patterns | Tool-*selection* quality | Unrelated axis |
| Statements spec + `**Valid:**` | Declared validity on **tracker entries** | DC covers claims *anywhere* — code comments, guides, CI config, memories — and records **why nothing repaired it**. It is the evidence base that would justify extending Statements coverage past tracker entries |

### The inclusion test

**Is the claim's truth coupled to a substrate that can change after the claim is written,
with no event scheduled to re-check it?** That is the whole test.

Whether the claim was *true at write time* is **not** the discriminator, and using it as
one is a mistake this ledger made on its own first day. `DC-1` was true and decayed;
`DC-2` was false and would have *become* true had the archive completed. Both are hostage
to an unwatched substrate, which is the thing being tracked.

Three states, two of them in scope. (Distinction contributed by peer session
`codescout-77` on 2026-08-26, after it cost a real misclassification.)

| State | What it is | Example | Goes to |
|---|---|---|---|
| **`decayed`** | True when written; substrate moved; nothing re-checks | `DC-1` (10.4 KB → 33.4 KB), `DC-3` (prefix count) | **DC** |
| **`anticipated`** | False when written, but truth is hostage to an event that could still fire, and nothing re-checks either way | `DC-2` / `bug-fix-session-log:F-69` (prospective `archive/` path) | **DC** |
| **`never-true`** | False when written, and **no** substrate change could ever have made it true | a citation added 16 days *after* its target was deleted; a path that never existed at any commit; a conclusion its own predicate could not support | **`F-N` / `W-N`** — authoring error, not decay |

**Name these states, never number them.** This table was referred to once as *“states one
and two”* in a cross-session message, against a peer's prose that ordered the same three
differently — which inverted `anticipated` and `never-true`, the exact pair the test exists
to separate. The ledger's row order is not a shared referent between two readers; the names
are. The inversion was caught in the message and never reached this file.

**`never-true` impersonates `decayed`**, which is the trap: it reads as
correct-then-invalidated from the prose and separates only on history.

**The probe below is mandatory, not advisory.** *“Was this true when written”* presents as
a fact about prose and is only ever a fact about history. On this ledger's first day every
unaided attempt at it went wrong somewhere: `codescout-77` expected the kotlin-lsp citation
to be `decayed` and found `never-true` only by running `--diff-filter`; `CLAUDE.md`
describes that same file as *“pruned as a dupe”*, which reads as `decayed` and is not; and
this ledger shipped with two incompatible criteria and did not notice for a commit. Run the
probe before classifying — including on your own claims, and especially when the prose
sounds settled.

```
git log --all --diff-filter=AD --name-status -- <target-path>   # the target's lifecycle
git log -S '<text of the citing line>' -- <citing-file>          # when the citation landed
```

If the citing line **postdates** the target's deletion, it was wrong when written — `F-N`.
Measured 2026-08-26: across 6 dead `docs/issues/archive/` citations, `codescout-77` found
three real ones and **none was DC** — two authoring errors (one citing a file deleted 16
days earlier, one citing a file that has never existed at any path) and one `DC-2`-shaped.

> ⚠️ **That zero is not evidence the DC class is small.** It describes the *dead*
> population only, and a citation that decayed and was then repaired never shows up as
> dead at all. The honest reading is *“among dead archive-citations in this sample, none
> was correct-when-written”* — survivorship, not prevalence. (`codescout-77` stated the
> scope alongside the finding; it is recorded here so a later reader cannot mistake the
> one for the other.)

Also out of scope: a plain typo; any claim where a trigger **did** fire and someone ignored
it (a process friction, `F-N`, not an absent trigger); and a statement that is **true but
incomplete** — one that licenses a false inference without ever becoming false itself.

That last one is the subtlest boundary and it has a worked example. `CLAUDE.md` says the
kotlin-lsp bug file *“was pruned as a dupe in `c6184884`”*. Probed 2026-08-26
(`--diff-filter=AD`, both paths): `c6184884` (2026-04-30) did delete
`docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`, so the sentence is accurate
and **cannot decay** — it is a fact about history. What it omits is the second copy:
`cbf7456d` added `docs/issues/archive/…` on 2026-04-21, and `771d9516` deleted it
separately on 2026-05-02. One sentence, one commit named, two paths and two deletions in
fact — which leaves a reader to infer the archive path was live until the prune, and that
inference is what reads as `decayed`.

Nothing there is hostage to a changing substrate, so it is not DC however much the
misreading it produces looks like one. It belongs in `F-N` (filed by `codescout-77` as
`bug-fix-session-log:F-70`). Keeping this boundary sharp matters: scope is the thing this
ledger has already gotten wrong twice, and *“true, and misleading”* is the case most likely
to be argued into it.

## The questions this ledger is built to answer

Every field exists to serve one of these. If a proposed field serves none, leave it out.

1. **Which `decay_kind` recurs most with no shipped check?** → what to automate next.
2. **Which `surface` leaks most?** → where instrumentation is missing, not where authors are careless.
3. **How many were `automatable: yes` but still found by a human?** → the coverage gap, in one number.
4. **What is the `repair_trigger` distribution?** → distinguishes "no gate exists" from
   "a gate exists and is capped below failing", which need opposite fixes.
5. **How were they `found_by`?** → if `recon-scout` dominates, detection depends on a
   skill someone has to remember to invoke.

Query them:

```
artifact(action="get", id="<this id>",
         entry_filter={"and": [{"automatable": {"eq": "yes"}},
                               {"status": {"eq": "open"}}]})
```

## How to append — two writes, in this order

This is a **filterable** ledger (`entry_collection: "claims"`), so a row and a section are
separate things and you need both. The row is what `entry_filter` reads; the
`## DC-N — <title>` heading is the only shape `link_scan` accepts as a definition, so
without it the entry can never be cited.

```
# 1. Row — the server assigns the id atomically
artifact(action="append_entry", id="<this id>",
         entry_collection="claims", id_prefix="DC", entry={...})

# 2. Section — using the id step 1 returned
artifact(action="update", id="<this id>", patch={body_edits: [{
    heading: "## Template for new entries", action: "insert_before",
    content: "## DC-N — <title>\n\n**Site:** ...\n"}]})
```

> ⚠️ Never `artifact_augment(merge=true, params={claims: [...]})` — RFC 7396 replaces the
> array wholesale rather than merging. That call took `tool-usage-patterns` from 19 entries
> to 1 on 2026-08-16, and the catalog is not in git.

To flip a row's status as a check ships, use `update_entry`, never a params patch:

```
artifact(action="update_entry", id="<this id>", entry_collection="claims",
         entry_id="DC-1", fields={"status": "check-shipped"})
```

## Entries

## DC-1 — `server.rs` calls tracker-conventions "10.4 KB"; it is 33.4 KB, and the claim was exact the day it was written

**Site:** `src/server.rs:5184` — a doc comment on `an_artifact_call_naming_a_tracker_path_delivers_the_tracker_guide`.

**Asserted:** `` `tracker-conventions` (10.4 KB: frontmatter, the status vocabulary, archive-through-the-catalog) ``

**Actual:** 33,402 bytes at HEAD; 34,288 with the pending guide edit. **3.2× drift in 10 days.**

**The point of this entry.** Measured at `73ccb495` (2026-08-16), the commit that wrote
the comment: the guide was **10,377 bytes** — *exactly* 10.4 KB. The author measured
correctly. There is no carelessness to find here, which is precisely why it belongs in
this ledger and not in a friction log: the claim was true, the substrate moved, and no
event exists that would ever tell anyone.

**Repair trigger:** `none`. Nothing fires when a guide's body grows. `include_str!`
means the size is fixed at build time, so not even a runtime assertion would notice; the
comment and the file it describes are coupled only by a human having read both.

**Detectable by:** a test asserting any KB/byte figure written beside a guide topic name
against `topic_body(topic).len()`. Cheaper and probably better: **delete the number.** It
is decoration in a doc comment — nothing reads it, and a figure with no consumer is a
liability that pays no rent.

**Found by:** recon-scout, incidentally — checking whether a pending guide edit would
invalidate a size claim before making it.

**Valid:** dated 2026-08-26

The byte counts are true of `73ccb495` and HEAD respectively; both were measured with
`git show <ref>:<path> | wc -c` this session.

**Rests on:** `topic_body` returning the whole file via `include_str!`
(`src/prompts/mod.rs:505`), so the delivered guide body and the file size are the same
number.



### Update 2026-08-27 — repaired, plus three things this entry got wrong about itself

**Repaired.** Two sites, two different repairs, because the number does different work in
each:

- **`src/server.rs`** — the figure sat in a test doc comment listing what the guide
  covers. It did no work there, so it was **deleted**. That site now has no decay surface
  at all; this is the entry's own first recommendation (*"delete the number"*), taken.
- **`src/librarian/adapter.rs`** — here the magnitude *is* the point (it explains the
  byte tension `BL-25` records), so deletion would gut the comment. It now states the
  durable **relation** — the two largest guides in the corpus, by a wide margin — with a
  **dated** snapshot beside it. The relation survives growth; the figure is honest about
  being an instant.

**A test was considered and declined.** The obvious guard is a test parsing every `N KB`
figure near a guide topic name and checking it against `topic_body(topic).len()`. Declined:
it would guard a single dated figure at the cost of a fragile comment parser that itself
becomes something to maintain. Deleting one site and dating the other costs nothing and
leaves less to go wrong. Recorded as a decision, not an oversight.

**Three ways this entry decayed while sitting in a decay ledger** — which is the most
useful thing it produced:

1. **The figure moved again.** 33,402 → **35,492 bytes** in the day *after* filing — a
   further 2,090 B, part of it from the filer's own edit to that same guide. Against the
   10,377 B the comment was written from, the drift is now **3.4× in 11 days**, not 3.2×
   in 10.
2. **The `Site:` pointer decayed.** This entry recorded `src/server.rs:5184`; the line
   was **5245** a day later. A line number is a measured value about a substrate that
   changes — it is a member of the very class this entry describes, and it went stale
   inside the record of that class.
3. **It named one site when there were two.** `src/librarian/adapter.rs:197` carried the
   same stale `10.4 KB`, unrecorded. Same lesson as `bug-fix-session-log:F-69`: when a
   scout finds a defect *class*, enumerate it across the corpus before writing the entry —
   one `git grep` would have found both.

**And the observation worth carrying forward.** The `adapter.rs` sentence held **two**
figures: `10.4 KB` (242% wrong) and `19.9 KB` (against a real 20,545 B — **accurate to
1%**). Two claims, one sentence, written the same day by the same author, and nothing
distinguishes the rotten one from the sound one to a reader. Age is not the discriminator
and neither is authorship; only re-measurement is. That is the argument for dating figures
rather than trusting them.

The heading above still reads *"it is 33.4 KB"*. Left as written, deliberately — it was
true at filing, and this ledger's whole thesis is that such claims are not errors. This
block is the correction.

#### Watch item — four more sites, currently accurate, deliberately not swept

The repair grep (`git grep '10\.4 KB\|19\.9 KB' -- src/`) found the `19.9 KB` figure for
`librarian.md` in **four further places**, none of them recorded when this entry was
filed:

- `src/prompts/README.md:25` — *"`librarian` alone is 19.9 KB"*
- `src/prompts/mod.rs:449` — same sentence, in a doc comment
- `src/prompts/mod.rs:457` and `:459` — inside a **`PULL_ONLY_GUIDE_TOPICS` reason
  string**, which is user-facing text rather than a comment

**Left alone on purpose.** `librarian.md` measures **20,545 B** today — the claim is
accurate to **1%**, so there is nothing to repair, and dating four currently-true figures
would be churn. They are recorded here because they are the same *class*: undated absolute
byte counts about a file that grows, with no consumer that would fail. If `librarian.md`
ever grows the way `tracker-conventions.md` did, all four rot at once and in silence — and
the `mod.rs:457` pair is the one that matters, because a reason string is read by an agent,
not only by a maintainer.

This is the **third** time this single entry has under-enumerated its own class: one site
became two, and two became six. That is not a criticism of the filing — it is the measured
rate at which a `git grep` beats a scout's memory, and the argument for running the class
sweep *before* writing the entry rather than after (`bug-fix-session-log:F-69`).


**Valid:** dated 2026-08-27
## DC-2 — Bug-file citations written at a future `archive/` path: the sweep that repairs them is triggered by an event that never fired

**Site:** `src/librarian/tools/doctor.rs:2790` and `:5766`;
`src/prompts/guides/tracker-conventions.md:384`;
`docs/trackers/bug-fix-session-log.md:5647`.

**Asserted:** `docs/issues/archive/<slug>.md` for two bugs.
**Actual:** both sat at `docs/issues/<slug>.md` with `status: open`.

**Repair trigger:** `event-never-fires`. The guide's archive sweep is triggered *by the
archive move*. `c0ec5ace` wrote the citations at fix-authoring time, naming the path the
files *would* occupy; `9bd9632e` then recorded that fix as **partial**, so the move never
happened. No event, no sweep, no owner.

**Detectable by:** `audit_doc_refs` already reports all four as missing paths — but
`cap_code_comment` (`src/librarian/tools/audit_doc_refs/severity.rs:199-205`) forces
High→Med on the two `.rs` sites, by design, so `--fail-on high` cannot gate them. Note
`ArchiveDrop` does **not** apply: it keys on whether the *citing* document is archived
(`severity.rs:251`), not the target. A cheaper targeted check: flag any citation to
`docs/issues/archive/<slug>` where `docs/issues/<slug>` exists on disk — a state that is
always wrong and needs no severity judgement.

**Status:** `check-shipped` — the instance was repaired in `fcb86c16` (patch-id
`51a4af509be224cc87839ebbada5d35f68ee3b4a`), and the guide now carries the rule
(*"cite where the file IS"*). Recurrence **is** now prevented: the proposed check was
built verbatim and ships — see § *Update 2026-08-27* below.

*(Status corrected 2026-09-01. It read `fixed-once`, with a trailing "Nothing yet
prevents recurrence", after the Update immediately below had already recorded the check
shipping — so this line contradicted its own section three paragraphs later. `params`
had been advanced to `check-shipped` and the body had not, which is the disagreement
`doctor`'s `params_status_drift` reported. The params side was right.)*

**Narrated in:** `bug-fix-session-log:F-69`.

**Valid:** dated 2026-08-26

**Rests on:** `cap_code_comment`'s High→Med forcing and `matches_archive`'s citer-side
keying, both read in `severity.rs` on 2026-08-26.


### Update 2026-08-27 — the proposed check was built verbatim and now ships

**Status is now `check-shipped`, and this is the ledger's first closed class** — not just
a repaired instance.

The `Detectable by` paragraph above ends with a proposal: *"flag any citation to
`docs/issues/archive/<slug>` where `docs/issues/<slug>` exists on disk — a state that is
always wrong and needs no severity judgement."* That is now
`librarian(action="doctor")`'s **`premature_archive_citation`** check, built to that
description.

- Detector: `564505d7` (patch-id `fa537bd2ae3c9dc7c55c06fe8d3d6688c6b461bf`), on
  `experiments`.
- Sites: `fcb86c16` (patch-id `51a4af509be224cc87839ebbada5d35f68ee3b4a`).
- Reads **0** on the binary rebuilt 2026-08-27 — a *true* zero, since `fcb86c16` had
  already repaired the four motivating sites. The positive control is at unit level and
  does fire.

**Why it needed no threshold**, unlike its neighbour `cited_prefix_with_no_definer`
(which needs `MIN_CITATIONS`/`MIN_FILES` to stay quiet on `UTF-8`): this state is wrong in
every world, so there is no tuning constant to get wrong later. Both remedies are
legitimate — repoint the citation, or complete the archive — and the check prefers
neither.

**What it does not cover, stated rather than left to be discovered.** Scope is catalogued
artifacts only, so it reaches markdown and **not** `.rs` doc comments. Of DC-2's four
original citations it would reach the two in markdown — exactly the two that carried full
severity anyway. The two `.rs` sites remain `audit_doc_refs`' territory, `Med`-capped by
design, which is why `repair_trigger` is `gate-exists-but-capped` and not simply
`gate-exists`.

One design note recorded at build time: `link_scan::extract` was **not** used, because it
emits `RelPathLink` only for `Event::Start(Tag::Link)` — markdown links — while inline
code routes to `scan_tokens`. The backticked-path form that actually occurs in this repo
would have been invisible to it, and the check would have read green forever.

**Valid:** conditional — until `cap_code_comment`'s High→Med forcing changes, at which
point the `.rs` half becomes gateable and this entry can close outright.
## DC-3 — TAXONOMY.md's "Thirteen are declared today" was exact when measured and became wrong two days later

**Site:** `docs/TAXONOMY.md:21`.

**Asserted:** *"**Thirteen are declared today** (measured 2026-08-19 …): AA, CAP, F, FND,
FT, GF, H, HY, R, S, SD, U, W."*

**Actual:** fourteen from 2026-08-21 — `711a25cf` declared `entry_prefix: CTX` on
`docs/trackers/context-performance.md` two days after the measurement. Fifteen once `DC`
was added by this session. **Wrong for five days.**

**Repair trigger:** `none`. Declaring a new prefix is a routine, encouraged act; nothing
about it re-counts the prose that states the total. The measurement even names its own
method (*"by reading `entry_prefix:` out of every frontmatter block"*) — which is to say
it is trivially re-runnable, and was never re-run.

**Detectable by:** a `doctor` check counting `entry_prefix:` declarations and comparing
against the figure `TAXONOMY.md` states.

**Why this one matters most.** It is the second instance of a single shape, and the shape
is narrow enough to automate: **a number written in prose about a corpus that grows, with
no consumer that would ever fail.** DC-1 is a byte count beside an `include_str!` target;
DC-3 is a cardinality beside a set of frontmatter declarations. Both were *exact when
written* — neither is an authoring error — and both were found by a human reading nearby
text for an unrelated reason. Two instances is the threshold at which this stops being
anecdote: **a written count is a claim with a measurable referent and no repair trigger**,
and that is a checkable class, not a discipline problem.

**Found by:** incidental — noticed while adding the `DC-N` row to the same table.

**Valid:** dated 2026-08-26

**Rests on:** `711a25cf` (2026-08-21) being the commit that declared `CTX`, measured with
`git log --diff-filter=A` and `git log -S` this session.

## DC-4 — Four IC-N deferral rationales kept their verdicts after the counts beneath them moved

**Site:** `docs/trackers/issue-clusters.md`, the `**Promotes to:**` fields of `IC-4`, `IC-5`,
`IC-6`, `IC-7`. Same shape in a bug file: `## Candidate remedies` in
`docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` still ranked
path-scoping first as *"cheapest, degrades safely"* while `### Re-ranking, third time` in the
same file had already withdrawn it.

**Measured 2026-08-31.** The four fields were written at 22:39 and were all true then. An
archive backfill at 22:49 added 78 `cluster/` tags, and by 23:39 every one of them still stated
its original count: `IC-6` said *"n=2, below threshold"* against 27; `IC-5` said *"all three sit
in the Windows/wine lane"* against 11 across six subsystems, seven of them outside that lane;
`IC-4` said *"three instances across three subsystems"* against 8; `IC-7` said *"two of three"*
against four. Each survived at least one later edit to its own entry — the `n` column and the
`**Members:**` line were both refreshed while the sentence reasoning from them was not.

**The property, and why review does not catch it: the premise and the conclusion live in the
same record, and only the premise is maintained.** A count is obviously data and gets updated;
the sentence that reasoned from it is prose and reads as settled. Nothing diffs a file against
itself, so a reader who came for the number sees a current number and a reader who came for the
verdict sees a verdict, and neither is looking at both. That is also why five surfaced within
90 minutes: one backfill moved many premises at once, and every conclusion resting on them held
still.

**One instance was excluded by the inclusion test, and it was mine.** The same file's
*"Six of nine clear the count threshold"* looks like a fifth member and is not. The mandatory
probe (`git log -S 'clear the count threshold'` / `'clear the promotion threshold'`) puts its
introduction at `ba7d0af1`, which is where I replaced the original *"Three of nine clear the
**promotion** threshold"* — correct, since promotion reads count **and** spread — with a
count-only predicate, and kept a hand-listed set of six. Nine classes sat at n≥3 at that moment.
So it was **false when written**, and no substrate change could have made it true; counts only
grew. That is `never-true`, which this ledger routes to `F-N` as an authoring error rather than
decay. Recorded here because it presents as decay from the prose and separates only on history —
the exact trap § *The inclusion test* warns about, met on the filer's own claim.

**The check proposed in `detectable_by` is deliberately narrow.** It does not try to detect stale
prose; it fires only where a field states a number that a live query can re-derive — parse
`n=(\d+)` out of `**Promotes to:**` / `**Mechanism status:**` and compare against the entry's own
membership query. A field naming a count that disagrees with its own tracker is wrong in every
world, so it needs no threshold, which is the property `premature_archive_citation` (`DC-2`) was
also chosen for. It would have caught four of the five, and none of the excluded one — correctly,
since that one states a count no query disagrees with, only a derivation nobody ran.

## DC-5 — A count of this class's own instances is itself a member of it — "third time tonight" was false within the hour

**Valid:** invariant

**Site:** `c7be203f`'s commit message — *"Third time tonight across two sessions"* — and the same
sentence sent to a peer. Immutable in both surfaces. The general form is any tally of an **open**
defect class published where nothing re-reads it.

**What decayed:** the number, within roughly forty minutes, by two independent routes. The peer
found a fourth instance in `CLAUDE.md` § *Testing Discipline* (`01791c67`); and three more were
already sitting in `42376ac2` and `c7be203f` — **my own fixes, from the same hour, uncounted by
the sentence counting them.**

**The derivation, since the whole entry is about not publishing a bare figure.** Unit: *a claim
in a committed artifact, true when written, false after a later change, with nothing re-reading
it, found 2026-09-01.*

| # | claim | invalidated by |
|---|---|---|
| 1 | bug-file frontmatter `title:` still "15" | the H1 corrected to 13 |
| 2 | `cluster-promotion-session-log:F-1` — *"correction not yet written to `OB-7`"* | `a2aedd49` writing it |
| 3 | `OB-7` — *"fifteen tests"* | the bug file's derivation moving to 13 |
| 4 | `IC-3` — `dyn Trait` named as the blind spot | `OB-7`'s dispatch-side probe refuting it |
| 5 | `OB-9` — *"and nothing does"* | a peer doing it, within the hour |
| 6 | peer's *"the discrepancy is unresolved"* (`7a2f14aa`) | the discrepancy being resolved (`fe085987`) |
| 7 | peer's *"All four were measured 2026-08-30"* | its own addition of a fifth law (`01791c67`) |

**Seven under that unit; I published three and the peer published four.** Neither figure carried
its unit, so the two are not reconcilable without re-deriving both — which is `CLAUDE.md`'s *"a
count of a defect population must arrive with its unit or not at all"*, arriving as a bill rather
than as advice.

**Why this is not repairable and what to do instead.** There is no fixed point. A count of this
class's instances is a claim of exactly the kind it counts, so it decays by the same mechanism,
and **authoring the entry that documents it confers no protection** — both tallies above were
written by parties actively writing about the class. A commit message cannot be amended after
the fact and an open population admits no settling query, so no `doctor` check is proposed here.
The remedy is at the write and it is one sentence: **for an open class, publish the unit and the
derivation, never the tally.** A dated derivation ages honestly; a bare number just becomes
wrong.

**Note on this row's own fields.** `found_by` is recorded as `incidental` because the schema's
enum has no `peer` value — the finding came from a cross-session exchange, which is now this
corpus's most productive discovery channel and is not expressible in the vocabulary. Small gap,
recorded rather than worked around silently. The schema did fire correctly on two invented
values in the first append attempt, which is the guard working.

## Template for new entries

Copy the shape below; the server assigns the id.

```
## DC-N — <one-line title: the claim, and that nothing repairs it>

**Site:** `path:line` — where the claim is written.

**Asserted:** what it says.   **Actual:** what is true, measured today.

**Repair trigger:** which event was supposed to fire, and why it didn't.

**Detectable by:** the instrument that would catch it, or why none can.

**Narrated in:** `<ledger>:<ID>` — the F-N / R-N / bug file telling the full story.

**Valid:** dated YYYY-MM-DD
```
