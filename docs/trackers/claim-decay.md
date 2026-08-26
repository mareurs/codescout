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

**Status:** `fixed-once` — the instance was repaired in `fcb86c16` (patch-id
`51a4af509be224cc87839ebbada5d35f68ee3b4a`), and the guide now carries the rule
(*"cite where the file IS"*). Nothing yet prevents recurrence.

**Narrated in:** `bug-fix-session-log:F-69`.

**Valid:** dated 2026-08-26

**Rests on:** `cap_code_comment`'s High→Med forcing and `matches_archive`'s citer-side
keying, both read in `severity.rs` on 2026-08-26.

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
