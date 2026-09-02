---
id: '3922c2a0fd0dfcfc'
kind: tracker
status: active
title: Observer Blindness — defect classes the right party structurally cannot see (OB-N)
owners:
- marius
tags:
- observer-structure
- defect-classes
- mechanism-design
- epistemics
- mineable
topic: observer blindness and unconditional mechanisms
entry_high_water_OB: 14
entry_prefix: OB
---

# Observer Blindness — defect classes the right party structurally cannot see (OB-N)

**Declared ledger.** `entry_prefix: OB`. Entries are `## OB-N — <title>` body sections;
allocate with `artifact(action="append_entry", id=<this artifact>, id_prefix="OB",
anchor_heading="## Template for new entries", title=…, body=…)`. Never hand-allocate.
`edit_markdown` is refused.

## What this ledger is for

An inventory of **defect classes indexed by their observer structure** — who
structurally cannot see the defect, who can, and the check that runs whether or not
anyone suspects a problem.

The unit is a *class*, never an instance. An instance is a bug file, an `F-N`, or an
`R-N`; a class earns an `OB-N` only once you can name the **mechanism of blindness** —
the reason the best-placed party gets no signal.

**Both halves of the defining property matter:**

1. **The party best placed to catch it is structurally unable to.** Not "was careless",
   not "was rushed" — *unable*, for a stateable reason. If a more careful version of the
   same person would have caught it, it is not an `OB`.
2. **The failure returns a plausible answer, not an error.** Nothing downstream fires,
   because nothing looks wrong. A wrong count, a clean-looking list, a green test, a
   working binary. This is why the class survives review chains that catch louder bugs.

**The payoff is mechanism design, not awareness.** Awareness is the one remedy the
structure rules out — see *The vigilance finding* below. So an entry without a named
observer and at least a candidate mechanism is not ready; write it as an `R-N` or a bug
file until it is.

## The vigilance finding — why this ledger exists at all

> **Measured 2026-08-30: four instances of one class, three sessions, one evening —
> every one committed by an author actively writing about that class.** One
> reintroduced a bare integer *in the commit fixing a bare integer*, roughly ten
> minutes after withdrawing an ordinal for identical reasons. A second shipped an
> unanchored ordinal inside a commit whose whole argument was that unanchored
> positional references rot.

Knowing the class prevented **none** of the four. What caught one was a standing policy
— *verify before contradicting a peer* — that fires unconditionally, whether or not the
reader suspects anything.

This is the exploitable part, and it is a claim about instruments rather than about
effort: **for a class in this ledger, "be careful" is not a weak remedy, it is the wrong
instrument.** A reader who converts an `OB-N` back into "remember to check X" has
discarded the finding. Say so in the entry.

**And a sharper form, measured 2026-09-01 across two sessions in one night: a better
*instrument* is not the remedy either, when the failure is on an axis no instrument was
pointed at.** The tally is the argument — **zero arithmetic errors between the two sessions,
and roughly a dozen wrong sentences.** Every wrong claim was a **referent error** (a correct
measurement of the wrong object) or a **stale premise** (a claim true when written, false an
hour later, with nothing downstream re-reading it). Neither is detectable by measuring more
carefully or by measuring with a better tool, because in both the measurement is *already
correct*. Sharpening the instrument sharpens a right answer to the wrong question. So when an
`OB-N` prompts "what should I have used instead?", the honest answer is often *nothing on that
axis* — check the referent, or re-read the premise, both of which are lookups rather than
measurements. (`codescout-e8`, reaching this entry's law from the opposite direction.)

The corollary tells you what to build instead: **a check that runs when nobody is
worried.** Three shapes, in descending order of preference —

| shape | why it beats vigilance | example |
|---|---|---|
| **make the correct path end in a safe state** | nothing to remember; doing it right leaves nothing armed | `OB-2` — the gate order |
| **an unconditional policy tied to a different trigger** | fires on an event that occurs anyway, not on suspicion | `OB-1` — verify before contradicting a peer |
| **publish the derivation, not the result** | the reader re-checks instead of re-deriving under their own rule | `OB-1` — `8`, and *why* it can only be 0 or 8 |

## How to mine this

Fields are **line-anchored bold labels**, so they are greppable without a schema:

```
grep -A1 '^\*\*Blind party:\*\*'   docs/trackers/observer-blindness.md
grep -c  '^\*\*Vigilance:\*\* wrong instrument'  docs/trackers/observer-blindness.md
grep -B4 '^\*\*Mechanism status:\*\* none yet'   docs/trackers/observer-blindness.md
```

The third is the worklist: a class with no mechanism is an open design task, and is
exactly what `H-N` (hookify candidates) and `I-N` (test-escape interventions) consume.

**Every one of those counts is off by one, and here is the rule** — written out because
omitting it would be an instance of `OB-1` in the page that defines `OB-1`. The template
at the bottom carries the same labels, and although it sits inside a fenced block (so
the librarian's own field detection skips it, by design), **`grep` does not know that**.
A naive `grep -c` therefore returns *entries + 1*.

Use the definer count as the denominator, since it cannot be inflated the same way:

```
grep -c '^## OB-[0-9]* —' docs/trackers/observer-blindness.md   # real entry count
```

Or exclude the template outright: `sed '/^## Template for new entries/,$d'` before the
grep. Verified 2026-08-31 at 3 entries — definers **3**, `**Blind party:**` **4**.

**Deliberately a prose ledger — no `entry_collection`.** Params are catalog-only and
git-ignored, so a fresh clone would mine an empty table while the prose sat right there;
the grep above works from any checkout. This matches `R-N` / `U-N` / `H-N`.

## Relationship to the neighbouring trackers — read this before adding a row

This ledger was created 2026-08-31 after surveying 60 existing trackers, in a session
whose main work was **deleting** four transcriptions of one command list. It is a new
surface, so it owes an account of why it is not a fifth copy of something.

| tracker | its unit | why `OB-N` is not that |
|---|---|---|
| `test-escape-hardening.md` (`I-N`) | an **intervention** to build | Its Overview already argues half of this — *"lenses must move LEFT into standing mechanisms so they catch by default without a human remembering."* Reached from the cost side (expensive reviews should get cheaper); `OB` reaches it from the observer side (this party *cannot* see it at any price). Its rows are code defects escaping a test suite; `OB` spans counts, queries, docs, advice and process. **`I-N` is a natural consumer of `OB` rows, not a duplicate of them.** |
| `reconnaissance-patterns.md` (`R-N`) | an observation about the **recon skill** | `R-138`/`R-139` are the originating instances of `OB-1` and stay there. `R-N` records what a scout hit or missed; `OB-N` records a class's observer structure and its mechanism. Cite across, do not copy. |
| `codescout-usage-hookify.md` (`H-N`) | a **hook design proposal** | The output when an `OB` mechanism is implementable as a gate. `OB` says what must be caught and by whom; `H-N` says how the hook decides warn-vs-deny. |
| `claim-decay.md` (`DC-N`) | a durable claim with **no scheduled repair** | Decay is about **time** — true when written, false later. `OB` is about **standpoint** — underdetermined at the moment of writing, to everyone but the author. A claim can be both; file it in both and cite across. |

**If a candidate row fits one of those better, put it there.** This ledger earns its keep
only for classes where the *observer structure* is the load-bearing fact.

## Index

| id | date | class | blind party | vigilance | mechanism status |
|---|---|---|---|---|---|
| OB-14 | 2026-09-02 | **the sampling frame — what the corpus contains, what one row means, how rows were selected — is the premise every number rests on and the one no downstream check reads.** A frame error moves numerator and denominator together, so every consistency check passes | **the analyst**, whose entire access to the population runs *through* the frame; auditing it needs a view of the population that does not come through it | wrong instrument, and uniquely so for the **review** half: 13 rounds plus a sustained adversarial review that reversed 4 conclusions, and none of five parties questioned the frame — `OB-4`'s shared-substrate law applied to reviewers rather than instruments | **designed** — show the data owner the corpus census and the sampling frame *before* the findings; census is 3 queries plus a per-producer byte share. Not promoted: 2 instances, 1 work stream |
| OB-13 | 2026-09-02 | **deleting a token makes every negative assertion naming it vacuously true — permanently, silently, while staying green.** The instance was *written as a forward-looking guard and made vacuous by the very event it anticipated* (`prompts/mod.rs:2603`, "After Task 14 lands…") | **the author performing the deletion.** The diff enumerates removals; the assertions that MENTION the token are not in it — they survive untouched, compile, pass, and read as guards. Nothing in the act of deleting points backwards | wrong instrument — the reverse direction is **closed**: token→assertions is a grep, assertions→"which go vacuous" is unanswerable, since the discriminator is who OWNS the token and that is not in the text. Measured: a selector over 93 assertions returned **28**, overwhelmingly fixtures | `designed, not built` — H-N: trigger on a token leaving the tree, `grep -rn '!.*contains.*<token>'`. **Plus a repair-side check**, because the blindness recurs in the fix: the prescribed replacement needle was itself vacuous, by a different mechanism, and passed |
| OB-12 | 2026-09-02 | a section that falsifies a sibling emits no signal — documents have no dependency edges; 5 instances, 2 documents, one of them falsified by its own review in the same pass | the author adding the new section, who is facing forward | wrong instrument — three claims survived a full session of a reader actively ruling on that document | **partial** — currency marker shipped 2026-09-02 (0/5 marked misleading, 3/3 unmarked stale); the un-noticed case has no mechanism and none is proposed |
| OB-11 | 2026-09-01 | a session-keyed ledger answers for a party the protocol never named — a subagent's FIRST fetch reports as a repeat, and its auto-inject is suppressed as already-delivered | the codescout MCP server: `agent_id` rides harness `SubagentStart`/`Stop` events and no MCP tool call ever carries it | wrong instrument — the guide text told the parent to make it *worse* | **partial** — wording fixed at 3 sites, note pinned negatively, companion hook overrides the brief; the **keying** is unfixable from inside this repo |
| OB-10 | 2026-09-01 | a mutual-exclusion resource is invisible to the session HOLDING it — the holder's own workflow succeeds and clears the condition as a side effect of finishing | the holder, a population of one against everyone else | wrong instrument | **none yet** — owner field on the resource is the candidate; enumeration is 1 verified / 4 unverified |
| OB-9 | 2026-09-01 | plausibility is a filter with a resolution limit — a near-miss number fits inside it; 4 instances, 4 caught by re-derivation, 0 by reading | the reader | wrong instrument | **partial** — remedy shipped under `OB-1`; this row adds its scope condition |
| OB-8 | 2026-09-01 | a shared resource carries no owner, so seeing the peer does not help — four captures in 34 min with enumeration complete | the writing session | wrong instrument | **partial** — outbound gate shipped; inbound not closeable per-session |
| OB-7 | 2026-09-01 | a declaration is well-formed, and nothing in production reaches it — including for the compiler, which cannot lint `pub` in a lib crate | the author of the declaration | wrong instrument | **partial** — decidable for 1 of 3 families |
| OB-6 | 2026-09-01 | a gate collapses "cannot observe" into the confident answer — three states exist, two are offered | the gate, and every reader of its output | wrong instrument | **designed** — exemplar shipped at `b9cc75b4` |
| OB-5 | 2026-08-31 | a summary keyed only by findings cannot say which checks RAN — and which direction the silence is misread in belongs to the **reader**, not the instrument (2 instances, opposite wrong conclusions) | the reader of the report | wrong instrument | **none yet** — the enumeration *is* the work; the original "3 lines" estimate is retracted in-entry |
| OB-4 | 2026-08-31 | a liveness marker with a good hit rate (2/3) spends the trust it earned — and three git-based instruments agreeing is **one** instrument | the session doing cleanup | wrong instrument | **none yet** — worklist |
| OB-3 | 2026-08-31 | a peer/agent listing is arbitrary w.r.t. the real population | the session reading the listing | wrong instrument | shipped (OS enumeration) |
| OB-2 | 2026-08-31 | shared `target/` left in a feature-clobbered state | the session that arms it | wrong instrument | **shipped** (gate ends safe) |
| OB-1 | 2026-08-31 | a published claim omits the parameter its author's context supplied | the author, specifically | wrong instrument | partial (derivation + one policy) |

## OB-1 — the parameter your own context supplies for free

**Valid:** invariant

**Rests on:** a published claim is evaluated in the reader's context, not the author's.

**Class:** an under-specified published claim — a count, a query result, an instruction.

**Blind party:** the **author**, and *specifically* the author. Any parameter their
context supplies silently is one they cannot perceive as missing, because they read
their own words already holding it.

**Who can see it:** a reader who does **not** share the author's context. Note this
predicts *which* reviewer, not merely that review helps — a more careful reader with the
same context is no better placed than the author.

**Plausible-answer property:** the reader resolves the missing parameter differently and
gets a confident wrong answer. Never an error.

**Vigilance:** wrong instrument. 4/4 instances committed by authors actively writing
about the class.

**Mechanism status:** partial.

**Instances, 2026-09-01 — graded against the `Class` field rather than by resemblance.**
Two sessions re-derived this class from scratch across four exchanges without either
recognising it, which is independent confirmation of an entry that was argued rather than
measured into existence. Three of that night's four qualify, and the grading is the point:

- `bug-fix-session-log:F-93` — a **count** omitting *when* it was taken, over a peer's live
  append-only transcript. `Write` 8 and `doctor.rs` mentions 80 were true when published and
  read 9 and 187 within the hour. The author's context supplies the instant for free.
- The withdrawn `same-session — 0` row in
  `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — a **count**
  omitting its denominator, which was 1 and not 11, inviting a 100% rate off n=1.
- The blind `grep` on the same page — a **query result** omitting the key, which enumerated
  only entries already known to exist and so could not match the one arriving. This is the
  case the *"report a search count together with the key you searched for"* mechanism above
  already predicts, arrived at independently.

**`bug-fix-session-log:F-92` is deliberately NOT cited here**, and the reason is sharper than
the one first recorded. The first exclusion said its claim was *unverified* rather than
*under-specified* — true, and it leaves F-92 looking like a boundary case. It is not a boundary
case; it **inverts the blind-party test**, which is the field this whole class hangs on.

OB-1's premise is that the parameter is supplied **silently**, so the author *cannot perceive it
as missing*. F-92's missing parameter was the query window — which, in its own words, it *"had
designed forty minutes earlier and which the tool prints on its own output line."* Supplied
loudly, on screen, and unread. So the distinction is **not-supplied vs supplied-and-unread**, and
the remedies diverge for a mechanical reason rather than a taxonomic one: publishing a parameter
the author already has in front of them changes nothing at all. OB-1 says *publish the parameter
your reader lacks*; F-92 says *read the one already printed*. A reader inheriting F-92 under this
tag reaches for a remedy that is a literal no-op on it.

*Reached by `codescout-68` on re-reading F-92's text rather than a summary of it — as a
**published null result**: they drafted the opposing objection (that OB-1's own "report the key
you searched for" mechanism would have caught F-92, so the exclusion was too clean), verified it,
and it died. Recorded at `77258f29` as `W-91`'s first denominator datapoint. The confirming
re-derivation is the case CLAUDE.md's recording-filter law says has no natural artifact, so it is
worth noting that this one exists only because someone chose to write down a check that changed
nothing.*

**Two mechanisms reach this class, and naming only one leaves a solo session with no remedy.**
The *Who can see it* field says *a reader who does not share the author's context*. That is
usually read as **another party**, and a session working alone then inherits a diagnosis with no
action attached. `R-49` is the second access route, and it is not a weaker version of the first:
its claim is that **an author an hour later is partially such a reader** — temporal separation
degrades shared context the way a different session never had it. Hence its own wording, *"the
countermeasure is temporal, not attentional — re-read on re-entry, not harder on write."*

| | context | latency | availability |
|---|---|---|---|
| peer channel | never shared | immediate | needs a peer |
| `R-49` re-entry | shared, then decayed | delayed | always |

Both fired **unaided** on 2026-09-01, verified verbatim in their commit messages. `800f1dec` —
*"the bug file's own first version overstated the blast radius… verified false and corrected in
this commit"* — is `W-91`'s promote-when trigger, found alone. `2e5dc738` — *"re-reading my own
bug file an hour after writing it found two of its four Fix options CIRCULAR"* — likewise.

*This paragraph exists because a claim in the other direction was withdrawn. This session
published **"none of the eight corrections tonight was self-caught; every one crossed the session
boundary"** — and the eight **were the peer exchanges**, so the population was selected by the
very property the claim announced discovering. Not false: **vacuous**, no member able to falsify
it. That is CLAUDE.md § Testing Discipline's member-selection law made about a corpus rather than
a test, and the third over-clean grouping in one session — the others a zero whose denominator
was a tenth of its neighbours', and a "three splits" tally holding one member that was not a
split. None contained a false statement; each licensed a wrong count.*

**The vigilance count above is deliberately NOT bumped.** Doing so needs each instance judged
against *"committed by an author actively writing about the class"*, which has not been
derived for these three — and bumping an underived count inside the entry documenting
underived counts is a joke this corpus has made twice already.

- **Publish the derivation, not the value.** *"8 verbs; the block prints 9 lines because
  clap appends its own `help`; and it is 8-or-nothing because the `#[cfg]` sits on the
  whole `Commands::Artifact` variant"* survives re-measurement. A bare `8` does not.
- **Report a search count together with the key you searched for**, so a reader can see
  what it could not have found.
- **Name which part of an instruction is the evidence.** *"Check it exits 0 and lists
  `find`"*, never *"check that it looks right."*
- **Unconditional policy that caught one:** *verify before contradicting a peer.* It
  fires on an event that happens anyway rather than on suspicion. **It is bidirectional, and
  was stated one-directionally here until 2026-09-01.** It fired the other way that night: a
  peer corrected a commit stat I had reported, I verified before *agreeing*, and the correction
  was wrong. Had I accepted it, I would have withdrawn an accurate claim — **worse than
  accepting a wrong one, because a withdrawal ends the thread and nothing later re-reads it.**
  The trigger is *about to change your position on a peer's say-so*, in either direction.
  **The counterfactual is what makes this load-bearing rather than a courtesy, and it was
  named by the peer whose correction it defeated.** Both of that night's productive findings
  — a misattribution that yielded a rule, and a referent error that yielded `OB-4`'s limit —
  existed *only because the receiving party checked before moving*. Being wrong produced
  neither; being **converted** did. A version of the exchange in which each side accepted the
  other's correction politely is not a slower version with the same output, it is one that
  produces **nothing** — both corrections absorbed silently, both underlying claims left
  wrong. Politeness and verification look identical from outside and differ entirely in what
  they leave behind.
- **Not yet mechanised:** nothing gates a bare integer in a doc. Candidate `H-N`.

### Sub-pattern — the remedy is the part that escapes verification

Found 2026-08-31 by two sessions independently, each about **themselves**, on the same
evening. Every unverified claim either of us published sat in the **remedy** half of a
finding — never the diagnosis:

- *This session, `OB-4`.* Diagnosis verified at five points: directory present at 174M,
  gitdir target absent, 84 citations across 16 files, both script line references read at
  the bytes, and the baseline SHA found in the script itself rather than taken from the
  report. The **recovery command** was transcribed unchecked — and is false; it refuses,
  because the path exists.
- *This session, `OB-5`.* Conflation verified at four sites in `doctor.rs`. The **remedy**
  — *"seed `by_check` with every check name at 0, roughly three lines"* — was published
  without checking whether a name list existed. It does not; there are 20 `scan_*`
  functions and every name is a bare `&str` at its own `Violation::new` site.
- *`codescout-fe`,* in their own words: *"checking the remedy rather than the diagnosis,
  which is the half I have been getting wrong all night"* — three self-reported instances
  (a recovery command, a line number, a framing).

**Why, and this is a mechanism rather than an excuse: attention follows contest.** The
diagnosis is the claim you expect to be challenged, so it draws your scrutiny. The remedy
is *offered* rather than argued — it reads as helpfulness rather than as an assertion — so
it never enters the part of your attention that demands evidence. The asymmetry is
**largest exactly when the diagnosis was hardest-won**, because effort spent establishing
it feels like effort spent on the finding as a whole, and the remedy rides out on that
credit.

> **The check, and it is why this earns a sub-pattern rather than an anecdote: in any
> finding you are about to publish, the sentence beginning "the fix is…" is a separate
> claim with its own evidence requirement — and it usually has none.** Locate it and verify
> it as if someone else had written it. Unlike "be careful", this names a specific sentence
> to look at, which is what makes it runnable rather than aspirational.

**Instances (2026-08-30):**

| kind | published | parameter silently supplied | reader gets |
|---|---|---|---|
| count | *"restores it to 4"*, then *"to 8"* | the **counting rule** — 4 off a trimmed view, 8 verbs, **9** lines with clap's `help` | re-measures, gets 9, "corrects" a true 8 back toward 4 |
| query | *"transcribed in four places"* | key was **fenced blocks containing `cargo test`** | a fifth site in parenthetical prose; clean count, nothing marks the omission |
| query | *"three clippy forms"* | key was **`cargo clippy`** | a wrapper form (`scripts/build-windows.sh clippy …`) that no `cargo`-keyed audit can reach |
| instruction | *"verify positively with `artifact --help`"* | that the evidence is the **exit code** | reads the output and counts it — inheriting a trap the author never faced |

The fourth is the sharpest: the author's own probe was `>/dev/null 2>&1 && echo present`
— output discarded, exit status only. **Their practice was flawless and no self-check
existed**, because re-reading their own advice means reading it already knowing which
half is load-bearing. The other three at least admitted a re-measurement that could
contradict them.

**Cite:** `reconnaissance-patterns:R-139` (the law), `reconnaissance-patterns:R-138`
(the query-shaped child), `334cf64b`, `2f412a1c`, `bdfd7a62`.

**Status:** validated — 4 instances, 3 sessions, one evening

### Sub-pattern — resolve the referent from the artifact, never from the sentence pointing at it

**Three instances in one night, 2026-08-31/09-01, same shape and same price.** Named by
`codescout-e8` after the third, on itself.

| what was written from context | what the artifact would have said | the command |
|---|---|---|
| a commit SHA (`2dd5e4d3`) quoted after `git show --stat HEAD \| tail -4` — output that confirms a commit while never containing its identifier | the SHA | `git log -1 --format=%h` |
| a commit's actual stat recalled rather than read | `19/1`, not the remembered figure | `git show --stat <sha>` |
| a referent inferred from a peer's prose — *"the commit that fixes stale claims"*, near-verbatim one commit's title — while holding both SHAs and with the disputed `17/2` string greppable across both | which commit the claim was about | `grep` the disputed string |

**What makes them one family is the price, and that is what separates this from `OB-9`.** Each
check is **one command against an artifact already in hand**, so it costs nothing and therefore
admits no excuse — where `OB-9`'s remedy concedes that re-deriving a population is expensive and
asks for the derivation to be published instead. Different price, different rule. This is why
the identifier case files here rather than there.

**The blind party is `OB-1`'s, exactly.** In all three the author's own context supplies the
referent for free — *the commit I just made*, *what I just changed*, *obviously the one they
meant* — so no lookup presents itself as missing. Nothing is skipped; the question never forms.
That is why "look it up" is not the finding and the **trigger** is: whenever a value names
something outside the sentence you are writing, resolve it against the thing itself before the
sentence ships.

**Sharpest of the three is the last**, because it survived a verification step: two instruments
agreed perfectly on the wrong object. See `OB-4` § *Third instance* — instrument agreement
verifies the measurement and never the referent, which is the limit on that entry's own remedy.

**Fourth instance, four hours after this sub-pattern was committed — and it adds the half the
other three do not have: PROPAGATION.** This session's context carries a marker reading
`from=c0ab9bc4-…`. I read that as *"the session this one was compacted from"*, never checked what
the marker denotes, and passed it to a peer as fact while flagging a possible defect in a
commit-trailer mechanism. `CLAUDE_CODE_SESSION_ID` was one command away and reads
`bcc98c22-…`; the commits in question touch `scripts/install-hooks.sh` and siblings, which this
session has never edited; and every stamped commit postdates the 02:00 hook install, hence
postdates the compaction, hence could only carry `bcc98c22`. Three independent checks, none run.

> **Why this one is worse than the solo form.** The other three stopped at their author. This
> one reached a second party, who then did everything right — verified against real commits,
> reasoned carefully, reached the **correct conclusion** (one work stream, one session, trailer
> upheld) — and attached a **wrong attribution** to it, because the premise being reasoned over
> was mine and was not among the things they could check. A referent error that crosses a party
> boundary gets **laundered into a conclusion the recipient can defend**, and their verification
> is genuine and does not touch it. The recipient is now the blind party, structurally: nothing
> in their evidence bears on a premise they inherited.
>
> **So the rule gains a second clause.** Resolve the referent from the artifact *before you
> hand it to anyone* — an unverified referent kept to yourself decays into one wrong sentence,
> and the same referent passed on decays into someone else's defended finding. And when you
> receive a premise you cannot check, say which part of your conclusion rests on it, so the
> party who can check it knows to.
>
> **The receiving end supplies the mechanism, and it is worse than "they could not check it"**
> — reported by `codescout-8a`, which is the only party that could observe this. Three
> properties made the premise ride: it arrived **labelled as verified** (*"the session this one
> was compacted from"*, stated as fact rather than as inference); it was **unfalsifiable from
> that side** (a peer cannot read my `CLAUDE_CODE_SESSION_ID`); and it slotted in as
> **background rather than as claim**, so it was never a candidate for checking at all.
>
> The consequence is the part to keep: **checkable claims attract the verification effort, and
> unchecked premises ride along beside them.** The unchecked premise is not merely unexamined
> — it is *protected* by the checkable material next to it, because a diligent recipient
> spends their effort where effort can land and finishes feeling they verified the message.
> More rigour on the recipient's part makes this **worse**, not better: it raises confidence in
> a conclusion whose weakest input was never in scope. Which is why the fix has to sit with the
> sender — the one party for whom the premise *is* checkable.

### Sub-pattern — mark provenance at the WRITE, because nobody downstream can

Found 2026-08-31, and it is the sharpest thing in `OB-1` because it names a defect the
reader is *structurally* unable to catch.

**The incident.** A peer measured a figure with an instrument they later identified as
wrong, and retracted it. I had meanwhile quoted that figure into a commit message as a
measurement — I never ran it. They then read my commit as an independent re-derivation
and **un-retracted the number on the strength of it**.

> **A citation loop: one measurement, two documents, reading as two witnesses.**

It is worse than the sibling error of mistaking adjacency for evidence
(`reconnaissance-patterns:R-145`), in three ways that generalise:

- **Genre supplies the authority.** A commit message's register is *report of work done*,
  so a bare number inside one reads as measured by default. No ambiguous phrasing is
  needed; the document type does the work.
- **It is durable.** A conversational restatement evaporates; a committed one is quoted
  onward.
- **It inverted a correction** rather than merely inflating confidence — the loop
  *destroyed a true datapoint* instead of adding a false one.

**The asymmetry is the actionable part, and it is why this belongs in this ledger:**
from outside, a document *reporting* a measurement is indistinguishable from one
*performing* it. **Only the author knows which.** Both instances of this loop were broken
by the party who introduced it — no reader caught either, and none could have.

> **So provenance is marked at the write, never audited at the read. Cite a figure you
> did not take as "reported by X", never bare — least of all in an artifact whose genre
> implies measurement.** One clause at write time; unrecoverable afterwards.

**Instances:** `2f434fba`'s message (the 2765 figure; corrected in code at `390bf4f0`,
the message itself immutable), and `reconnaissance-patterns:R-142`'s first instance.
Framing of the genre/durability/inversion asymmetry is `codescout-fe`'s, recorded by them
at `e8dd2445`.
### Sub-pattern — the third position: supplied CORRECTLY, to the wrong audience

Found 2026-09-02. `OB-1`'s discriminator above separates two positions and turns on which
remedy applies: **not-supplied** (publish the parameter) and **supplied-and-unread** (`F-92` —
where publishing again is a literal no-op, because it is already on screen). There is a third,
and it takes a remedy neither of those prescribes.

**The incident.** `docs/trackers/issue-clusters.md` publishes each class's membership as a query
plus an `n`, and guards it deliberately — *"trust the query; re-run it before trusting the
count."* That sentence defends the count's **freshness**. The count's **population** is bounded
too, precisely and with measurements: a `cluster/<slug>` tag is required of `docs/issues/*.md`
and **not** of `docs/issues/archive/*.md`, whose untagged majority is a deliberate exclusion
because *"forcing a fit would corrupt the counts that promotion reads."* That bound is written
down. It is written down in `tests/issue_clusters.rs`'s module header — the layer where a bound
is **enforced**, which no reader of the number has any reason to open.

So the parameter is neither missing nor ignored. It is correctly published to an audience that
is not the one reading the number, and the author cannot perceive the gap for `OB-1`'s own
reason: they wrote the gate, so the scope is in their model when they re-read their own ledger.

**Why it earns a row rather than folding into the parent: the remedy inverts.** Publishing the
parameter is redundant — it is already published, better than the ledger would have put it.
Reading harder is impossible — the reader does not know the other surface exists. The only
action left is to **move it**: a number and the scope that validates it must co-locate at the
point of **reading**, never at the point of enforcement.

**And it is worse than a plain omission, because the careful sentence supplies the
reassurance.** A document that names one failure mode implies by omission that the others are
handled. Re-running the query exactly as instructed returns a *fresh* number that is still scoped
to a 34%-tagged corpus, and *"trust the query"* is precisely what stops a reader asking what it
ranges over. The audit that found this measured 29.5% archive coverage, read it as convention
drift, and was one step from a 236-file retro-tagging campaign the header forbids in writing — a
campaign that would have inflated every class with members matching no class, which is the number
promotion reads. It was caught only by opening `tests/issue_clusters.rs` for an unrelated reason.

**It also completes a series, which is the grading argument for filing it here rather than as a
new class.** `OB-1`'s instances are a count missing its *instant*, a count missing its
*denominator*, and a query result missing its *key*. This is a count missing its *population* —
the same `Class` field, a fourth parameter, and the third position of supply.

**Two cheap tells, neither needing judgement.** A coverage ratio that is neither ~0% nor ~100% is
a boundary someone drew before it is drift — true drift tends toward complete, a stable middle is
usually a decision. And before proposing any campaign over a population, grep the **enforcement**
layer for that population's name — `tests/`, `scripts/pre-commit-*`, hooks — not only the
documentation layer. Here one `grep -l 'docs/issues' tests/` was the whole difference.

Instance and full measurements: `reconnaissance-patterns:R-170`. Mechanism shipped `7331b3e4`,
which moved the population statement into `issue-clusters.md` beside the sentence that had
defended only freshness.
## OB-2 — the session that arms a shared-state trap gets no signal

**Valid:** conditional — reopens if the gate's command order changes, or if a second
terminal command is appended after the default-features lane

**Rests on:** `docs/issues/archive/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md`

**Class:** a shared mutable artifact (here `target/debug/codescout`) left in a degraded
state by a correct, documented action.

**Blind party:** the session that **arms** it. The lean lane succeeds, reports green, and
exits; nothing in its output mentions the binary it just replaced.

**Who can see it:** the *next* session, which experiences it as 10 of 11 `cli_artifact`
tests failing with `unrecognized subcommand 'artifact'` — reading exactly like a
librarian feature-gating regression in whatever they just wrote.

**Plausible-answer property:** the victim gets a specific, credible, wrong diagnosis
pointing at their own diff. Worse than an unexplained failure, because it is actionable
in the wrong direction.

**Vigilance:** wrong instrument, and this is the entry that proves the shape is
*convertible* rather than merely lamentable. The remedy had been circulating for a day
as *"remember to rebuild after the lean lane"* — an instruction addressed to precisely
the party defined as receiving no signal.

**Mechanism status:** shipped — `73066479`. The documented gate now runs the lean lane
**third** and the default lane **last**, so following the instructions correctly leaves
nothing armed. The safe state is a *consequence* of compliance rather than an extra step.

Two limits kept deliberately, because an overstated fix is its own `OB`:

- It closes the **terminal state**, not the **window**. During the lean lane the binary
  really is librarian-less; the reorder bounds that by the rest of your own run.
- It is **not free**: 71.8s documented order vs 80.4s reordered vs 79.6s for the
  rejected appended-`cargo build`. Both remedies cost ~8s, and the reorder wins on
  *structure* — an appended build step is one more proposition that can silently not
  have run, which is the same class again.

**Cite:** `73066479`, `d92f5f9c`, and the bug file above.

**Status:** validated — mechanism shipped and verified by running it (0 verbs after the
lean lane, all 8 after the default lane)

## OB-3 — a peer listing is arbitrary with respect to the real population

**Valid:** dated 2026-08-31

**Rests on:** `reconnaissance-patterns:R-134`

**Class:** a discovery tool whose result set is a *subset* of the real population by an
undocumented rule, consumed as if it were the population.

**Blind party:** the session reading the listing. Omission is not marked, so a short list
and a complete list are indistinguishable in the output.

**Who can see it:** a second, independent instrument over the same population — here the
OS: `pgrep -x claude` plus `readlink /proc/$p/cwd`, and the sockets under
`/run/user/1000/cc-socks/`.

**Plausible-answer property:** you reason by elimination over the visible set and reach a
confident attribution. Six misattributions in one afternoon across five sessions.

**Do not quote a single under-count ratio — the ratio is a property of (profile × checkout
population), not of the instrument.** Measured 2026-09-01 on one population from two
vantages: a `~/.claude` session sees `ListAgents` return **1 of 4** same-checkout peers,
while a `~/.claude-sdd` session on the identical population sees **2 of 4**. Both are
correct and neither generalises. This entry previously carried *"reported 2 peers while 7
other sockets were live"*, which is the same mistake one layer down — and a raw socket count
is wrong on its own terms besides, since 7 of the 20 socket files had no process at all.

**What survives every vantage, and is the thing to cite: the instrument errs in BOTH
directions at once.** It misses same-checkout peers running under another profile *and*
reports other-checkout peers running under yours — from `~/.claude` three of the four names
returned were in an unrelated repo. Under-count and over-count simultaneously, which is why
no single ratio characterises it and why a reader who patches the number rather than the
framing will publish the next wrong one. (Both figures above supplied by peers on their own
vantages, which is the only way this was measurable at all.)

**Vigilance:** wrong instrument. A short view makes elimination weak, so a careful reader
hedges — but a **disjoint** view makes elimination *unrelated* to the answer, so hedging
still draws the wrong conclusion. Care cannot rescue an instrument whose units differ
from the question's.

**Mechanism status:** shipped as practice — enumerate from the OS before attributing, and
message invisible sessions directly at `uds:/run/user/1000/cc-socks/<pid>.sock`. Not gated;
a candidate `H-N`.

**But enumeration is NOT the remedy for attribution, and conflating them is this entry's
most available misreading.** Corrected 2026-09-01: a session mis-attributed a write by
*elimination* ("X is the only other busy session"), and a **larger correct population makes a
wrong elimination more persuasive, not less** — `docs/PROBES.md` says so directly. So
*"the probe existed and nobody ran it"* is true and is not the finding. Enumeration bounds
the population; it does not attribute a write. **Asking is the discriminator even when the
population is fully known** — which is a stronger claim than "run the probe first", and the
one the evening's five misattributions actually support: four were resolved by asking, none
by counting.

**Tell, reusable beyond this instance:** the instrument's units differ from the
question's — *sessions a transport knows about* vs *processes with this cwd*. And
repeated "it must be the remaining one" reasoning is equally well produced by the answer
lying outside the set entirely.

**Status:** validated — measured twice, by two sessions, with the membership rotating
between readings (which demonstrates arbitrariness where one snapshot could only show
disjointness); re-measured 2026-09-01 from two *profiles* on one population, which is what
showed the ratio to be vantage-dependent and the both-directions property to be the
invariant. A third instrument axis is `SendMessage` itself: resolution **by name** is
profile-scoped and fails for an invisible peer, while the **socket address** works — the same
split one layer up, and the reason a send-by-name failure must never be read as unreachability.

## OB-4 — a liveness marker with a good hit rate is more dangerous than a bad one

**Valid:** conditional — closes when `.worktrees/bench`'s gitdir is repointed or the corpus is re-created under `codescout`'s own `.git`

**Rests on:** `docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md` § Residual; reported by `codescout-fe`, verified here 2026-08-31.

**Class:** a structural marker read as a liveness signal, when what determines liveness is *reference from elsewhere*.

**Blind party:** the session doing cleanup. `.worktrees/bench/.git` contains
`gitdir: /home/marius/work/claude/code-explorer/.git/worktrees/bench`, and that path has
not existed for months. Every locally available signal says orphan debris: a dangling
pointer, a pre-rename path, 174M, and **a bug already filed and archived about exactly
this pointer** (`docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md`),
which reads as confirmation that the thing is known residue.

**Who can see it:** a citation sweep across the surfaces that *consume* it —
`grep -rn 'worktrees/bench' scripts/ docs/ --include='*.toml'`. Verified 2026-08-31: **16
files, 84 references**, including two live scripts (`scripts/run-tc-benchmark.sh:18`,
`scripts/sweep-bm25-boost.sh:10`), `docs/PROBES.md:116`, and `docs/trackers/retrieval-benchmark.md`,
which holds the corpus definition. It is the pinned retrieval-benchmark corpus at `ede25e69`.

**Corrected 2026-08-31, and the correction is itself an `OB-1`.** This entry first said
recovery costs *"a `git worktree add --detach` plus a 163M re-index"*. That was transcribed
from the report without checking, and it is false: the command **refuses**, because the path
exists and is non-empty. Verified here — `git worktree list --porcelain` lists **only** the
main checkout, `git -C .worktrees/bench rev-parse HEAD` returns `fatal: not a git repository:
(null)`, and the directory holds 16 visible entries (**24** with `ls -A` — the two numbers
are one measurement under two counting rules, which is the harmless form of `OB-1`). The
commit object *is* reachable (`git cat-file -t ede25e69…` → `commit`), so recovery is
possible, but it is **move-then-add plus a 163M re-index**, not one command. I verified the
directory, the gitdir, the citations and the line references, and published the one claim I
had not checked — the remedy.

**Three instruments agree, and their agreement is the trap — this is the sharpest part.**
A careful person asking *"what worktrees exist here?"* reaches for `git worktree list`. It
returns a **clean, complete-looking answer with no trace of 174M of load-bearing corpus**,
because bench is not a registered worktree at all. `git status` is silent too — `.gitignore`
ignores `.worktrees/` **twice** (`:7` and `:117`). And the gitdir pointer dangles. Three
independent-*looking* signals all say *nothing here*, and their unanimity reads as
corroboration.

> They are not independent. **All three consult git, and the corpus's liveness is not a git
> fact** — it is a fact about `scripts/` and `docs/`. Instruments sharing a substrate are
> **one instrument**, and their agreement carries no more evidence than any one of them.
> This is `reconnaissance-patterns:R-138`'s *"two agreeing sweeps are one sweep"* generalised
> from queries to instruments: independence of **mechanism** is what makes agreement
> informative, and neither independence of authorship nor of tool name supplies it.

That is also why the citation grep works: **it never consults git.** When corroborating a
negative, ask what substrate each confirming instrument reads, and count distinct substrates
rather than distinct commands.

**Plausible-answer property:** you get a confident *"this is dead"* reading supported by a
real, verifiable dangling pointer — and by two further signals that appear to confirm it. The evidence is genuinely true — it simply answers a
different question (*what does this directory's git metadata point at?*) than the one being
asked (*does anything still depend on this directory?*). Kin to the `du`-proves-size-never-absence
finding, `reconnaissance-patterns:R-135`.

**Vigilance:** wrong instrument, and this entry is here for *why*. The marker had a **2/3
hit rate on the local sample**: three sibling directories shared the same dangling gitdir,
and two of them (`bench-legacy` 173M, `no-local-embedding` 12M) genuinely were dead and were
correctly deleted. So the heuristic had a track record before it failed, and a more careful
reading of the pointer still returns "orphan" — care applied to the *wrong instrument* only
sharpens a wrong answer.

> **Generalisation worth mining: a predictor that is usually right is more dangerous than
> one that is usually wrong, because it earns the trust it later spends.** A marker that
> mispredicted every time would have been discarded; one at 2/3 gets promoted to a rule of
> thumb, and the third case is deleted with confidence. When a heuristic's accuracy is what
> justifies skipping the direct check, its accuracy is the hazard.

**Second instance, 2026-09-01 — different instrument, and the unit is a *ratio* rather than a
marker.** A ledger entry reported `ListAgents` enumerating **4 of 20** live sessions. Measured in
units matched to the question, the figure is **1 of 4**: the denominator counted socket *files*,
7 of which were dead processes, and the numerator counted peers, 3 of which were in an unrelated
checkout. What carried it into two ledgers is that **20% and 25% read as agreement**. An
implausible ratio is caught by its own reader; a plausible one is not — and near-miss is exactly
what plausibility means here. Same law as the 2/3 marker above, with the accuracy accidental
rather than earned, which is the worse case: nothing had to build trust first. Derivation and
full count table: `cluster-promotion-session-log:F-3`.

> **This mechanism has its own row since 2026-09-01 — `OB-9`, *a near-miss number sits inside the
> reader's resolution limit* (`3cff5fb7`, patch-id `40f6831e967491430979abb9d68fa8f356b7a88f`).**
> The paragraph above is kept because the two entries split a question this instance happens to
> answer twice. `OB-4` asks **why a value is trusted** — here, accidental accuracy standing in for
> earned accuracy. `OB-9` asks **why a wrong value is not caught downstream** — the reader's
> resolution limit. Different remedies, so neither subsumes the other; read `OB-9` first if the
> question is about a *number*.

**The confirmation of that correction is itself subject to this entry's substrate rule.** Two
sessions' `ListAgents` outputs agreed on the composition of the four — three `backend-kotlin`,
one codescout peer — from opposite vantage points, each listing naming the other. That is
genuinely useful and it is **one instrument**, per the blockquote above: same mechanism, two
callers. The independent substrate is `/proc/<pid>/cwd`, which is what supplies the *denominator*
(5 live sessions rooted in this checkout) and which neither `ListAgents` run could have produced.
Count distinct substrates, not distinct sessions.

**Third instance, 2026-09-01, and it is the sharpest limit on this entry's own remedy: agreement
between instruments verifies the MEASUREMENT and never the REFERENT.** A peer moved to correct a
reported commit stat, and reported having *"confirmed on two instruments before saying so"* —
`git show --stat` and `--numstat`, which agreed exactly. Both were correct. Both read
`42376ac2`, whose prediction of `16/3` matched its actual `16/3` perfectly; the claim under
discussion was about `c7be203f`, predicted `17/2` against an actual `19/1`. The instruments were
not the failure and neither was the arithmetic — **the object was**, and instrument agreement is
structurally incapable of detecting that, because both were handed the same wrong referent and
agreed about it faithfully.

> So *"count distinct substrates"* is necessary and not sufficient. It disciplines **how** you
> measure and says nothing about **what**. Before citing agreement as corroboration, state the
> object each instrument was pointed at and check it is the one under discussion — a step that
> feels redundant precisely because the referent is the part you are not thinking about while
> choosing instruments. The failure returns a confident, internally consistent, fully verified
> wrong answer, which is this ledger's defining property arriving through the verification step
> itself.

Recorded as the class, not the incident: it is structurally available to anyone, and the session
writing it up had committed a purer form of it hours earlier — quoting a commit SHA (`2dd5e4d3`)
read off `git show --stat HEAD | tail -4`, output that confirms a commit while never containing
its identifier. Same shape, one instrument instead of two: the referent was never checked.

**Mechanism status:** none yet — this is the open worklist row.

- **Practice, effective immediately:** before deleting anything large, grep for **citations**
  rather than inspecting the thing itself. Liveness is a property of the reference graph.
- **Candidate `H-N`:** a pre-delete hook on `rm -rf`/`git worktree remove` targeting a path
  cited from `scripts/`, `docs/` or `*.toml` — warn with the citation list. The citation grep
  already exists as a one-liner; nothing runs it unprompted, which is the entire gap.
- **Cheapest structural fix, and it removes the class rather than guarding it:** repoint or
  re-create the worktree under `codescout`'s own `.git` so the marker stops lying. Then the
  pointer and the reference graph agree, and no check is needed.

**Instances:** `.worktrees/bench` (live, nearly deleted twice); `bench-legacy` and
`no-local-embedding` (correctly deleted, and the source of the marker's false credibility); and
the `4 of 20` peer-enumeration ratio (2026-09-01), whose nearness to the true `1 of 4` is the
entire reason it survived review in two ledgers.

**Status:** validated — reported by `codescout-fe`, corrected by them, independently verified
here at each step (directory present at 174M; gitdir target absent; 84 citations across 16
files; `git worktree list --porcelain` naming only the main checkout; `.gitignore:7` and
`:117`; `ede25e69` reachable as a commit object)

## OB-5 — a summary keyed only by findings cannot say which checks ran

**Valid:** conditional — closes when `by_check` is seeded with every check name at zero

**Rests on:** an instrument that reports *findings* rather than *coverage* answers "what did I find?" when the reader is asking "what was looked at?" — a many-to-one map from world-states onto one output.

**Class:** state conflation inside a **single** instrument. Distinct from `OB-4`, which is several instruments sharing one substrate — see *Why this is not OB-4* below, because the distinction determines the remedy.

**Blind party:** the reader of the report. `doctor`'s `summary.by_check` is built by iterating the violations that exist (`src/librarian/tools/doctor.rs:401-404`), so a check contributes a key **only if it found something**. Three different world-states collapse onto *absent key*:

1. the check ran and found nothing — the happy case;
2. the check was removed, commented out, or never wired in — the invocations are hardcoded `all_violations.extend(scan_*(…))` calls with **no registry**, so a deleted line leaves no trace anywhere in the output;
3. the check ran but was held back by its own gate — several are threshold-gated on citation volume *by design* (`:214-221`), which is correct behaviour that is nonetheless indistinguishable from (2).

**Who can see it:** anyone holding the check inventory from a *different* source — reading `doctor.rs`, or querying the database directly. `codescout-fe` resolved a live instance by going to the database, which worked; the entry exists because that is a workaround, not the fix.

**Plausible-answer property:** absence reads as **a clean bill of health**, which is the default happy interpretation. This is the worst available direction for a conflation to fail in: a missing check and a passing check both look like "fine", so the report is most reassuring exactly when it is least informative.

**Vigilance:** wrong instrument. No amount of care reading the report recovers the missing state — the information is not in it. A reader would have to already know the full check inventory, in which case they did not need the summary.

**Why this is not `OB-4`, and why that matters.** `codescout-fe` offered it as a third instance of *"instruments sharing a substrate are one instrument"*, using *substrate* to mean the shared summary object. Judged here as a **neighbouring class, not that one**, on the remedy test:

| | `OB-4` — shared substrate | `OB-5` — state conflation |
|---|---|---|
| how many instruments | three, agreeing | one |
| does the instrument *know*? | **no** — `git worktree list` has no access to the fact that `scripts/` cites the path | **yes** — the list of checks that ran is in the same function |
| fix | add an instrument reading a **different substrate** | **emit the discriminator** from inside the existing one |
| second substrate is… | the only fix | a workaround |

Going to a second source happens to work for both, which is why they feel alike. But for `OB-4` there is no *"emit the discriminator"* option at all, and here there is — and it is three lines.

**Mechanism status:** none yet — designed, and the design is **not** what this entry first said.

> **Correction 2026-08-31, and it is the second time in this ledger that the unverified
> claim was the REMEDY rather than the diagnosis.** This entry originally proposed *"seed
> `by_check` with every check name at 0 — roughly three lines."* All three parts of that
> were wrong, corrected by `codescout-fe` and verified here. See `OB-1` § *the remedy is the
> part that escapes verification*.

- **There is no list to seed from.** 20 `scan_*` definitions, and each check's name exists
  only as a bare `&str` passed to `Violation::new(check, …)` (`:142-150`) inside its own
  function. Creating the enumeration **is** the work; it was never three lines.
- **A hand-written `const` array reintroduces the defect one level up — and this is a
  reproduction, not a prediction.** The entry first framed it as a hazard `codescout-fe`
  foresaw. It is a design **this file has already run and removed.** `Violation`'s own doc
  comment (`doctor.rs:120-139`), ten lines above the bug, reads:

  > *"Deliberately not enumerated here: this list named eight checks and had gone stale by
  > three (`snapshot_drift`, `ledger_defines_nothing`, `entry_without_definition`) before
  > anyone noticed, because nothing gates a doc comment against the `scan_*` functions that
  > emit these strings. The authoritative set is the `Violation::new` call sites."*

  **37.5% stale, unnoticed.** Seeding `by_check` from a hand-written list rebuilds the
  artifact whose measured failure is documented directly above the defect it would be used
  to fix.

### The two defects are each other's naive remedy

This is the structurally interesting part, and it is why *"compiler-enforced or it is a
regression"* is stronger than a preference.

| | **Design A** — hand-maintained list | **Design B** — current: no list, call sites authoritative |
|---|---|---|
| fails how | goes stale silently — 3 of 8 | `by_check` cannot report coverage — **this entry** |
| naive fix | delete the list | add a list |
| which lands you in | **B** | **A** |

Neither direction is right, and **the deletion was correct at the time** — Design A really
was broken. The mistake was not removing it; it was that removing it left coverage
unreportable, and **nobody noticed, because that loss is silent.** Which is `OB-5` itself:
the class consumed the removal of its own remedy, then hid the consequence.

> **Generalisable, and worth checking whenever a fix restores a previously-deleted design:
> ask whether the two defects are each other's remedy.** If they are, no amount of choosing
> between them helps — both directions are round trips — and the only exit is a third
> design with neither failure mode. Here that is an enumeration the compiler maintains: a
> list that *cannot* go stale, so Design A's failure is unavailable while Design B's is
> fixed.

A history that reads as *"we tried the list and removed it"* is easy to mistake for
*"a list is wrong"*. It is not — it is *a **hand-maintained** list is wrong*, and the
adjective is the whole finding.
- **The robust form is compiler-enumerated, and this repo already ships one.**
  `src/librarian/augmentation_sidecar.rs:263-269` destructures exhaustively **on purpose**:
  adding a field to `AugmentationSidecar` breaks compilation until that field is compared,
  because otherwise *"a new shape field would travel in the sidecar, restore correctly, and
  silently never be drift-checked — `sidecar_shape_drift` would report healthy on the one
  field nobody wired up."* Its closing line is the best statement of this ledger's whole
  principle: **"the compiler is the only reviewer guaranteed to be present on the day that
  field is added."** An unconditional check, in the strongest available form — it cannot be
  tired, rushed, or unaware of the class.
- **Report gated checks as a third value** — `"threshold_not_met"` rather than `0` — to
  separate (3) from (1). Optional; (1)-vs-(2) is the load-bearing split.
- **`:196-307` is not the whole call surface.** `scan_frontmatter_id_mismatches` is defined
  at `:3846` and invoked at `:957`, outside that block. Any enumeration must cover **both**
  sites or it will confidently report a live check as never-run — which would be this same
  class, produced by its own fix.
- **Generalisable rule for any reporting tool:** report **coverage alongside findings**. A report that enumerates only what it found cannot distinguish a clean subject from an unexamined one, and `docs/adrs/2026-08-27-negative-results-name-their-scope.md` already states the principle for negative results — this is the same rule applied to a *summary object* rather than to a single zero.

**Instances — two, and the second is already closed:**

1. `summary.by_check` (verified at `doctor.rs:401-404`, `:142-150`, `:196-307`, `:957`, 2026-08-31). Reported by `codescout-fe` from a live encounter with `worktree_scoped_row`, resolved by them via the database — a workaround that answered their question and left the instrument still unable to answer. **Open.**
2. `AugmentationSidecar`'s shape fields (`augmentation_sidecar.rs:263-269`). An unwired field would be **reported healthy**, which is the same conflation — unexamined presented as clean. **Closed** by an exhaustive destructure, shipped by `git-travel-augmentation-shape` 2026-08-30, before either of us had a name for the class.

That second instance is why the class is now validated rather than provisional, and it is worth more than a count: **it supplies the working remedy.** The fix for instance 1 is not to be invented, it is to be copied from instance 2.

**Fixed 2026-08-31 in `09cd1b46`** (patch-id `c3d3f02b367f02e615b54c02e68d472a1d83ffa4`) by
`codescout-fe`. `by_check` now names all **29** checks, `0` where clean; verified live here
on a real run — `total: 136, shown: 136`, 29 keys, zeros present. A macro generates the
`Check` enum and its `ALL` from one list of arms with the wire strings pinned beside each
variant, plus a `debug_assert` in `Violation::new` so a scan emitting an unlisted name
panics in the suite. That assert fired zero times across 198 doctor tests, which is a
**positive control on the 29-name list** rather than an author's assurance.

**The RED was purer than either of us predicted.** A clean catalog produced
`by_check: {}` — not "the clean check is missing from a populated map" but a *wholly
empty* one, indistinguishable from a doctor with no checks at all.

### The class had colonised its own regression suite

The finding worth more than the bug. `call_reports_entry_validity_scoped_out_rows_in_catalog_health`
(`doctor.rs:11824`) asserted the check key was **absent** to mean *"found nothing for the
active project"*. Verified in the commit diff: the removed line is an `.is_none()`, and the
replacement's own comment states why — *"absence equally meant 'the check never ran'… which
`is_none()` could not"* distinguish.

So a test written to guard the behaviour **encoded the ambiguity it was guarding against**,
and no run could have flagged it: the assertion passed under both world-states it conflated.
Generalises: when a class is about a signal being ambiguous, the suite is a *likely carrier*
rather than an unlikely one, because tests are written in the same vocabulary as the code
and inherit its conflations. **Audit the tests for the class when you fix it.**

### Known-open residual — declaration is not execution

Stated by `codescout-fe` in the commit rather than buried, and recorded here so this entry
does not read as closed.

**A check whose `extend()` line is deleted still reports `0`**, because the enum still
declares it. Narrower than the original hole — which lost *ran-clean* as well — but not
zero: the fix proves a check is **declared**, never that it **ran**. That is exactly the
mirror test proposed while the work was in flight, and it is not closed.

Closing it needs per-check **execution** tracking, and the complication is structural
rather than effort: it is not one-scan-one-check. Verified here — **20** `scan_*` functions
emit **29** check names across **30** `Violation::new` sites, so several functions own
several names (`scan_artifact_paths` alone emits six) and a naive "this scan ran" flag
would mark all of its names live when only one path executed.

**Mechanism status for this residual: none yet.**

**Superseded shape below** — kept because the entry's value is the reasoning, not the
state:

**Was being fixed 2026-08-31** by `codescout-fe`, operator-approved, in `doctor.rs`, TDD from a
failing test asserting that a check which ran and found nothing appears in `by_check` with
`0`. Shape: a `Check` enum whose `ALL` list is generated by the same macro invocation as its
variants, and `Violation::new(check: &str, …)` → `Violation::new(check: Check, …)` across
**30** call sites — which is what makes it airtight, since a scan then *cannot* emit a name
outside the enum. Not this session's work; recorded so the entry and the fix do not diverge.

**Status:** validated — two instances, one closed by the right mechanism, both verified in
source; instance 1 under active repair

### Sub-pattern — which direction the silence is misread in belongs to the READER, not the instrument

The entry above calls *absence reads as a clean bill of health* **the worst available direction**
for a conflation to fail in. Measured 2026-09-01 across two sessions: that is true of a `doctor`
summary and **false as a general property**. The same silence is read as **red** by a reader who
expected a failure, and the wrong action is then the opposite one — so the direction is a fact
about the reader's expectation, not about the instrument.

Both instances were `cargo test` runs filtered by a **remembered** test name, where a filter
matching nothing prints `0 passed; 0 failed` per binary — indistinguishable from a binary that
simply holds no matching test.

| reader expected | silence read as | wrong action |
|---|---|---|
| a gate to pass | **green** — `0 failed` across 25 binaries | publish a gate result for a check that never ran |
| a mutation to kill a named test | **red** — "the mutation was not killed" | discard a **good** test as weak |

**Instance 1 (this session).** `cargo test --workspace -- doc_tool_refs valid_slugs claude_md_…`
reported green having executed **one** of three intended checks: `doc_tool_refs` is a *file* name,
and no test is called `valid_slugs` (the real ones are `every_declared_class_has_an_index_row`,
`the_slug_set_excludes_the_template_placeholder`, …). Caught only because the per-binary
`N filtered out / 0 running` lines did not match what was expected — not by reading the verdict,
which said `ok` throughout.

**Instance 2 (peer, same evening) is a confirming null, and a denominator rather than a catch.**
Five filtered runs checked via `-- --list`, all five non-empty; **two were mutation runs**, where a
silent zero would have condemned a working deliberate-break test. Nothing needed correcting, which
is exactly why it is recorded — a confirming check produces no finding, so this population is empty
by construction unless someone publishes into it (§ *Testing Discipline*'s recording law). It bears
on the base rate, not the hit rate, and must not be absorbed as a near-catch.

**Why this is the parent class one level up.** The discriminator — *did this filter match anything?*
— is held by the harness and absent from the output, exactly as `by_check`'s check inventory is held
by `doctor.rs` and absent from its summary. And the same remedy shape is available and unbuilt:
`-- --list` names the matched set **before** the run. Until something emits it, the reader-side rule
is that **a filtered run owes its matched count**, precisely as a peer count owes its unit.
## OB-6 — a gate collapses "cannot observe" into the confident answer

**Valid:** invariant

**Rests on:** `issue-clusters:IC-2` (`cluster/gate-keyed-on-unobservable-event`, n=16 at promotion); the shipped exemplar is `index_envelope`'s doc comment in `src/tools/config/mod.rs`, which states the remedy at the site.

**Class:** a gate whose condition is an event **outside its observation boundary** substitutes a proxy — a caller-supplied flag, a monotone stamp, a per-process map, a chunk count — and then maps the proxy's *unknown* onto its *confident* value. The proxy is not the defect; collapsing three states into two is.

**Blind party:** the gate, and therefore every reader of its output. The reason is scope, not care: the event is conversation-scoped (a compaction, a `/clear`), harness-scoped (an `/mcp` reconnect, what a parent session already holds), or peer-process-scoped (whether a companion hook is still alive), and the gate is process-scoped. It holds no handle to the scope that would answer the question. **No amount of care inside the process reaches a fact the process has no channel to** — which is the admission test, and it is why this is not "someone forgot to check".

**Who can see it:** an instrument on the **event's own side of the boundary**. For the filesystem-scoped half that is the filesystem itself — `git_sync_status`, a file count, an mtime — and the gate simply never looked. For the harness-scoped half it must be a **positive signal written by the party that can observe the event**: the companion hook already reads Claude Code's `input.source` enum (`{startup, resume, clear, compact}`) and throws it away one line from where `post_compact` needs it. Note what does *not* work: `hook_at` looks like a liveness signal and is not, measured across five healthy sessions at ages 0.6 h–25.0 h, so a staleness window would have to exceed ~25 h to avoid false-deactivating a healthy session — it detects nothing. A proxy that is *nearly* the right substrate is the most expensive kind.

**Plausible-answer property:** an extra guide body, an open gate, a shut gate, a re-cleared ledger, `up_to_date` on an index 286 commits behind HEAD. Nothing throws, so nothing downstream fires. The shipped `chunks > 0 → "up_to_date"` mapping is the class in one line: *a statement about non-emptiness wearing the name of a statement about currency*.

**Vigilance:** wrong instrument — and the rendezvous gate is the proof, because **one gate produced three separate members**: it latches open when the hook goes quiet, it can never be stamped again if it misses its `SessionStart`, and an `/mcp` reconnect leaves it permanently inactive. Three ways for one proxy to be wrong, found separately across twelve days by people who already knew about the previous one. Care applied to a proxy sharpens a wrong answer rather than correcting it.

**Mechanism status:** designed, with a shipped exemplar — `b9cc75b4` (patch-id `5a5c761072c44b54dc80f224d89355dd2d31e498`) closed one member and demonstrates all three moves.

**Second exemplar, 2026-09-01, and it is the generalisable one: a stateful guard installed
mid-flight adopts the CURRENT state as its baseline, and the baseline is exactly what it exists
to detect.** A peer shipped a session-attribution index guard that refuses a bare `git commit`
carrying another session's staged paths. At install the shared index already held
`docs/trackers/observer-blindness.md`, staged by **this** session; the hook's first run had no
prior log to consult, could not observe who staged it, and recorded it as the **installing
session's own**. A guard that reads a peer's staged file as yours is silent on precisely the
capture it was built to refuse — and it is silent from birth, in its own first run, with nothing
anywhere reporting a problem.

> **The peer's manual repair had already found move (1) without naming it**: it hand-wrote the
> owner as `-` rather than picking a session. That *is* the third state. What is owed is making
> it what the **code** does — `install-hooks.sh` seeding every pre-existing staged pair as
> **unknown owner**, never as the installer. Note which way the default must fall: `unknown`
> makes the guard *over*-refuse until the pairs churn out, which is recoverable by reading a
> message; `mine` makes it under-refuse silently, which is not recoverable at all because
> nothing is emitted.

**Why this generalises past git hooks:** every guard that decides by comparing against recorded
history has an install moment at which it has no history, and the cheap thing to do is treat
"present at install" as "baseline, therefore fine". Migrations that mark existing rows valid,
linters baselined on the current tree, and quota trackers seeded from the current usage all take
the same shortcut, and each is then blind to whatever was already wrong on day one. **Ask what
your guard believes about the state it inherited, and prefer a third answer to an optimistic
one.**

1. **Three states, because three exist.** `up_to_date | behind | indexed`, where `indexed` means *populated, freshness indeterminate*. Defaulting the unknown arm to the strong claim is exactly how the original went wrong, and `unwrap_or("indexed")` makes the unrecognised-shape fallback the weak word too.
2. **Omit rather than zero.** `behind_commits` is absent in the indeterminate arm, because a `0` nobody measured is indistinguishable from one that was.
3. **Extract the proxy→claim mapping into a pure function — this is the check, not merely tidiness.** The hardcoded string survived because its arm sat behind a live `RetrievalClient::from_env` call that no test could reach; the suite was green over the half that works. A pure `index_envelope(chunks, files, git_sync)` admits a table test with a row for the `None` input, and **the `None` row is precisely the one that was missing**. That check runs unprompted, because it is an ordinary unit test rather than something a reader has to remember.

For the harness-scoped half add a fourth move, from `post_compact`'s design note: **degrade to current behaviour on absence, never refuse on missing evidence.** Act on a positive signal that the event did *not* happen; treat no-signal as today's blunt path. This mirrors `inherited_stamp`'s existing philosophy and keeps hookless clients working.

**Instances:** the 16 members of `cluster/gate-keyed-on-unobservable-event`. They split by whether the substrate that would answer the question exists at all, and the split predicts their fate. **Filesystem/repo-scoped (7)** — `workspace-status-claims-up-to-date-without-checking-git-sync`, `index-status-claims-complete-without-checking-coverage`, `index-status-lock-contention-reads-as-failed`, `symbols-stale-after-external-git-checkout`, `worktree-write-guard-is-dead-code-in-production`, `companion-test-suite-stamped-live-rendezvous-slots`, `buildrs-source-md-outdir-snapshot-staleness`: **all seven are closed** (six fixed, one wontfix). **Harness/peer-scoped (9)** — the three rendezvous members, `post-compact-clears-the-ledger-with-no-compaction-check`, `served-guide-sections-arrive-after-the-call-they-inform`, `subagents-receive-guides-their-parent-already-holds`, `guide-topics-are-atomic-nodes-in-an-unmodelled-graph`, `path-relative-banner-not-rearmed-after-compaction`, `mcp-reconnect-does-not-refresh-server-instructions`: **five are still live.** Where the substrate exists the class is an ordinary bug and gets fixed; where it does not, the members accumulate — which is the argument for move (1), since a third state is available even when the event is not.

**Status:** open — promoted from `IC-2` on 2026-09-01 at n=16 spanning index status, the LSP cache, `build.rs`, companion hook scripts, the guide ledger and the workspace write guard. Sibling to `OB-4`: that entry asks *why a proxy is trusted* (an accuracy record it later spends), this one asks *what the proxy does when wrong* (returns the confident value rather than admitting it cannot tell). Kept apart because the remedies differ — `OB-4` wants you to change instrument, this one wants the instrument to have a third answer.

## OB-7 — a declaration is well-formed, and nothing in production reaches it

**Valid:** invariant

**Rests on:** `issue-clusters:IC-3` (`cluster/declared-not-wired`, n=18 after the `IC-15` boundary settled; 20 when this entry was drafted, and the split below is re-derived against 18); `CLAUDE.md` § *Testing Discipline* — *"name the concrete caller that reaches it"*, which is this class's remedy stated before the class was.

**Class:** a surface declares a capability that **production never reaches**. Every piece is individually correct — the selector, the matcher, the schema field, the enum variant, the computed header — and no call site connects them. The declaration reads as a shipped feature, and the tests pass.

**Blind party:** the author of the declaration, specifically. Writing `**Serves:** edit_file(path~/.claude)` *is* the act of believing it is served; the mental model in which the wiring exists is what produces the line. **A more careful version of the same author writes the same line**, which is the admission test.

**Who can see it:** only an instrument that starts from the **production emission side** and works back to the declaration — never one that starts from the declaration and checks it is well-formed. Every declaration in all 20 members is well-formed.

**Plausible-answer property:** a green suite, a documented feature, a populated schema. Nothing is malformed and nothing errors. The reason ordinary testing does not reach it is structural rather than accidental: **a unit test constructs its own inputs**, so it hands the matcher a selector production never emits, and passes. The test is not weak — it is *scoped to the half that works*.

**Vigilance:** wrong instrument, and this class has an unusually literal blind observer — **the compiler**. Rust's `dead_code` lint cannot fire on `pub` items in a library crate, because it cannot know the crate's consumers and must assume reachability. codescout declares one (`[lib] name = "codescout", path = "src/lib.rs"`, verified 2026-09-01), so **every `pub fn` under `src/` is exempt by construction regardless of whether anything calls it.** That is the same structure this ledger describes in humans, in a tool: the party best placed to notice holds a parameter (the set of external consumers) that makes noticing impossible. It is why the dead-in-production family survived `-D warnings` on every gate run for months. *(The lib target is measured; the lint's `pub`-exemption rule is reasoned from language semantics and was not demonstrated in-tree — no zero-caller `pub fn` was found to exhibit it. The 22 `#[allow(dead_code)]` suppressions across 12 files are consistent with the lint being live for non-`pub` items, and are corroboration rather than proof.)*

**Mechanism status:** partial — **designed and instrument-verified for one of three families, and honestly unsolved for the other two.** The 20 members split by what disconnects the declaration:

**PROBED 2026-09-01, seeded from the dispatch side, and the residual risk is narrower than the correction above states.** Enumerating all 41 `impl Tool for X` and diffing against `CodeScoutServer::new`'s registry: **`dyn Trait` dispatch does not hide a symbol from a text search**, because a trait object must be *constructed*, and construction is by name (`Arc::new(Grep)`); delegation is by name too (`"register" => RegisterLibrary.call(…)`). Dispatch consumes a name, it does not erase one. The genuine false-positive candidates are **macro-generated names and re-export-only aliases**, which is a much smaller surface than "every trait object". **The same run produced the first observed instance of the state the check exists to detect**, and it was a true positive: `ListFunctions` and `ListDocs` — `impl Tool`, 25 identifier hits, every non-definition one test code, zero registration, and **13 tests exercising the two by name** — 17 if you count the formatter tests guarding the same dead code, 18 for everything in the module; four units, and the bug file publishes the derivation rather than a figure (`docs/issues/archive/2026-09-01-listfunctions-and-listdocs-are-unregistered-tools.md`). *This sentence read "fifteen tests" until 2026-09-01 and is corrected here — `OB-9`'s instance table, three entries down, cites that very 15→13 as a member of its class, so the ledger was carrying the defect in the entry its own row was about.* Note what did **not** run: `references` false-zeroed on the same symbol with its guard firing correctly, and returned `symbol not found` for `GetUsageStats`, so the LSP half of the substrate comparison was unavailable and the conclusion rests on the text instrument alone. Full record at `cluster-promotion-session-log:F-1`.

1. **Dead in production (9).** The code exists; only tests call it. **The partition is available and was measured**: `grep` annotates every hit with its enclosing symbol path, and test call sites carry a `tests/` prefix — `[tests/names_path_containing_generalises_and_normalises_separators]` versus `[names_tracker_path]`. Call-site rather than file granularity is **required, not merely preferable**: `references` groups `src/librarian/adapter.rs:1451` under a production file and `grep` shows it is `tests/…`, so a file-level partition misclassifies it. **The check does not "decide" this in general** — it decides for **by-name call sites**, which the probe above shows covers trait-object dispatch and named delegation, and misses macro-generated names and re-export-only aliases. **The failure direction remains the dangerous one**: a false *dead-in-production* finding authorises a deletion on a negative search result, so a positive finding wants a second instrument before anything is removed — and on 2026-09-01 that second instrument was unavailable. Roughly six of the nine members are Rust symbols and in scope; the rest are a shell hook and a cargo feature, needing their own reachability notion.
2. **Schema or doc declares what the code ignores (3).** The check is a round-trip — for each declared field, assert the handler reads it — and it is **weak**: reading a field is not using it, so this detects the absent read and not the ignored value.
3. **A matcher that can never match (6).** `op-4-path-predicate-can-never-fire`, `triggered-operator-rules-route-nothing-in-production`. This is IC-3's original phrasing — *for each declared selector, assert some production path emits it* — and it is the **hard** one: it needs the set of values production actually emits at a call site, which is a dataflow question no index answers today.

**Do not record this as `designed` for the class.** One family has a decidable check, one has a weak proxy for a check, one has no mechanism at all. `IC-9` was carried at `designed` on exactly this conflation — a member that *named* a check while recording it as insufficient — and the cost was a later reader re-running the measurement it warned against.

**Instances:** the 18 members of `cluster/declared-not-wired`, spanning the embedder chunk budget, the live Qdrant payload (579,311 chunks with empty `ast_kind`/`ast_header`), `grep`'s overflow hints, `link_scan`'s bucket split, `audit_doc_refs`' stubbed-off LSP, the operator-rule router, the constitution glob, librarian's error downcast, the CLI's `doctor`, and a CI lane that compiled no tests behind `server-stack`. `OB-5`'s *Known-open residual* is this class seen from the reporting side — a check whose `extend()` line is deleted still reports `0`, because the enum still declares it — and the two should be cited across rather than merged: `OB-5` is about a summary that cannot say what **ran**, this is about a capability nothing **reaches**.

**Status:** open — promoted from `IC-3` on 2026-09-01 at n=18, the second-largest class in the corpus. **The `IC-15` boundary was raised before the archive tagging pass and settled during it**, on a remedy test reducible to one question: *was a caller-supplied value accepted?* Yes — the path ran and discarded it, remedy is round-trip or refuse, `IC-15`. No — the capability exists and no call site reaches it, remedy is find a caller, `IC-3`. Exactly two members moved; `cli-doctor-exposes-no-fix-flag` stayed because the flag does not exist at the boundary at all, so nothing is accepted to be dropped. Worth carrying beyond this pair: every one of these reads as *"declares X but does not do X"*, a sentence fitting at least four classes here — matching on it is how `IC-9` acquired two misfits, and the **remedy**, never the description, is the discriminator.

## OB-8 — a shared resource carries no owner, so seeing the peer does not help

**Valid:** invariant

**Rests on:** `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` (six instances); `issue-clusters:IC-1`, whose stated falsification condition this entry records as **fired**.

**Class:** a resource shared by construction carries **no ownership record**, so the question *"is anyone else holding this right now?"* has no representation to consult. Not a missing lock — a missing **field**. Nothing models the resource as owned, so there is nothing for a careful party to read.

**Blind party:** the writing session, and the reason is the sharp part. It is **not** that the peer cannot be seen. `git status` and `git diff --cached` return a complete, correct listing of every change at those paths, and **nothing in that listing marks which changes are yours**. The session is not missing the peer; it is missing an attribute the substrate never had. Git has no session dimension, so "mine" is not expressible in the one place the decision is made.

**Who can see it:** nobody, from inside the checkout, after the fact — which is why the remedy is isolation rather than inspection. The only party who can distinguish the changes is the *other* session, and only while it still holds the memory of writing them. That is `OB-1`'s structure at one remove: the parameter that would resolve it is held by someone who is not being asked.

**Plausible-answer property:** the commit succeeds, `--stat` looks reasonable, the hook prints `Passed`, and the resulting history attributes a peer's work to your subject line. A clean commit and a captured one are byte-indistinguishable at every point where a decision is available.

**Vigilance:** wrong instrument, and this entry has the strongest evidence in the ledger for that, because **enumeration was not the binding constraint**. `IC-1` claimed coordination was impossible *because* the peer listing is scoped narrower than the sharing, and stated its own falsifier: *an instance where the writing session could enumerate the peer and still collided*. On 2026-08-31/09-01 two sessions **did** enumerate each other — `ListAgents` named both, and they exchanged eight messages about this precise mechanism — and collided **four times in 34 minutes**: `e0525462` (23:53), `3a5aec7a` (23:55), `1b40dabd` (00:06), `77d4da06` (00:27). The fourth happened with a warning message **in flight**. Every capture post-dates mutual enumeration. Visibility was complete **for that pair** and changed nothing, because knowing who your peer is does not tell you which lines in a shared tree are theirs. *(Narrowed 2026-09-01. Not complete as a **population**: `ListAgents` returns 1 of the 4 live peers whose `cwd` is inside this checkout, while simultaneously reporting 3 peers in an unrelated one — wrong in both directions at once. This strengthens the argument rather than weakening it, because the colliding pair sat inside the visible quarter: the falsification ran on the most favourable sample available for coordination, and coordination failed four times regardless. Full count table: `cluster-promotion-session-log:F-3`.)*

**The shared surfaces, enumerated — the inventory is the useful part:** the **working tree** (a pathspec commit reads it, so path-scoping is no defence on a contended file); the **git index** (`git add` writes to the one per-checkout index, so a peer's bare `git commit` takes your staged work — found 2026-09-01 and absent from every prior remedy discussion); `target/` (`OB-2`); the committed `entry_high_water_<PREFIX>` allocator; the pre-commit **stash cache**; and the machine-local librarian **catalog**. Each is written by whoever acts and read by path, and not one carries a writer.

**Mechanism status:** partial, and the boundary is the finding.

- **Shipped, outbound half:** `scripts/pre-commit-unreviewed-content.sh` refuses a pathspec commit whose content was never staged, forcing the committer to *read* what they are about to commit. It fired correctly on the session that documented the class.
- **It does not close "looked and could not tell."** Reading `git diff --cached` on a co-edited file shows hunks with nothing marking whose they are. The gate closes *committed without looking*; the harder half needs per-hunk provenance the tree does not carry.
- **That tension is RESOLVED, 2026-09-01, and the answer is a composition rather than a choice:** `git add <paths>` **then** `git commit -- <those same paths>`. Staging satisfies the gate (no unstaged content at those paths); the pathspec excludes whatever a peer has staged elsewhere in the shared index. **Neither half is sufficient alone**, which is why the two were argued as alternatives for a day — pathspec alone commits the *working tree*, trading an index capture for a working-tree one, and staging alone leaves a bare `git commit` taking the whole shared index. Probed independently by two sessions across four commits (`0dea224`, `ee4842e6`, `4816d64f`, `f33733de`), one of which landed while the other session held 15 files staged and took none of them.
- **The inbound half is not closeable by any per-session behaviour at all.** Stage, stop, read `--cached`, then commit as a separate call is correct and was followed; the capture landed in the gap between the read and the commit, with no action of that session's in between. It is a time-of-check-to-time-of-use window on state a peer writes and this session can neither observe nor lock.
- **Shipped 2026-09-01, the inbound half's first real mechanism: `scripts/pre-commit-foreign-index.sh` (`99d5acac`).** A `post-index-change` hook records `<session>\t<blob>\t<path>` per staged blob; the pre-commit guard refuses a **bare** `git commit` when the index holds a pair owned by another session, names whose is whose, and says *leave theirs staged — it is not yours to unstage either*. This is the **owner field** this entry says the substrate lacks, added beside git rather than inside it. It fails **open** by design and documents so: it under-reports and never over-reports, so a clean run is not proof the index is yours.

- **Its coverage is narrower than "prevents capture", on two axes, and the second is the one to keep straight.** *Cross-path* (the index holds a peer's staged file) is this guard. *Intra-path* (the pathspec form reads a file a peer edited) is `unreviewed-content`'s. Neither covers the other. And the founding capture, `d617051b`, **would not have been caught by either as an attribution problem**: the committing session staged its own file and a peer wrote into it during the ~40s between check and `git add`, so the log records the committer as stager and the guard exits 0, correctly. A mechanism named for a defect does not necessarily cover the instance that motivated it — check, rather than assuming the exemplar is a member.

- **The argument for it is invisible to exactly the sessions it protects, which is why it is recorded here.** A session already using `git add <paths>` + `git commit -- <paths>` never trips this guard — the pathspec form takes a temporary index, so it exits 0 every time, and the guard reads as redundant. Then comes the one commit that *cannot* use that form: at `a4470394` this session had to commit from the **index** precisely because the working tree held 46 lines of a peer's in-flight section that a pathspec commit would have swept in. That commit was made safe by hand-building a single-hunk patch and `git apply --cached`. **"I already use the safe form" is not evidence the guard is redundant — it is evidence you have not yet hit the case where the safe form is the unsafe one.**

- **Deliberately NOT shipped, pending an operator ruling: a `prepare-commit-msg` hook stamping `Session-Id: <uuid>` into every commit.** It would make *"whose hunks are these?"* answerable from the commit itself — the strongest form of the missing owner field, since it survives the session. Held because it is the one part of the mechanism **uninstalling does not undo**: the hook comes out, the trailers stay, and they are pushed and permanent. That is a different class of decision from a guard, and it changes a documented commit convention. `scripts/prepare-commit-msg-session-id.sh` and `scripts/install-hooks.sh` are committed at `99d5acac` so the capability needs no rebuilding when someone rules.

- **The positive-identification remedy inherits `OB-3`'s defect, and that is a dependency between two classes rather than a caveat on one.** `CLAUDE.md` § *Observer Blindness* now carries the procedure for uncommitted state — **ask the session and have it quote its scratchpad path**, since the harness makes the session id a path component, so the id is *given* rather than inferred. It is the right remedy and it has a precondition nobody stated when it landed: **asking only works once the candidate set contains the owner.** Constructing that set is an *enumeration*, and `OB-3` is the entry recording that our enumerations are arbitrary with respect to the real population. So `OB-8`'s remedy rests on `OB-3`'s known-broken instrument — read them together, not separately.

- **Measured 2026-09-01, and the failure is on the routing side rather than the attribution side, which is new.** Fifth instance in `docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md`: a gate reddened on uncommitted tracker content, two sessions were plausibly responsible, and the responder — having filed the *fourth* instance an hour earlier — **correctly declined to pick**, broadcasting the same message to both live peers and saying why. **Both were innocent.** `scripts/file-provenance.py`, run independently by two sessions and agreeing, named two writers, **neither among `ListAgents`' three live rows**. So every routing decision available was wrong: choosing either peer would have misattributed, broadcasting to both was *also* wrong, merely harmlessly so. **Broadcasting widens the guess; it does not close it** — its correctness is about not asserting a falsehood, never about reaching the answer, and it still costs each recipient a turn establishing a negative about work they did not do.

- **The rule that follows, CORRECTED within the hour by running the instrument that already existed.** This bullet first read *"when the write-derived set contains no reachable session, positive identification is UNAVAILABLE — record the question unresolved."* **That was a claim about the world when it was a fact about the instrument.** `ListAgents` reads `$CLAUDE_CONFIG_DIR/sessions/*.json` and is **per-profile**; `SendMessage` delivers over `/run/user/<uid>/cc-socks/<pid>.sock` and is **per-user, shared**. Measured 2026-09-01: `ListAgents` reported **2** peers; socket enumeration found **16 live sessions across 3 profiles**, and — unit stated, because this line got it wrong once — **six sessions with `cwd` in this checkout, being five peers plus the counting session**. So *"both live peers"* reached **two of the five peers**, and the two write-derived names sat among the three this-checkout sessions `ListAgents` omitted — **reachable throughout** by `uds:/run/user/<uid>/cc-socks/<PID>.sock`. Identification was available the whole time. *(First written as "five of them in this checkout" — a subset of the 16 **sessions**, which is six — beside a clause counting **peers**, which is five. Two units, one sentence, neither named; caught by a peer re-deriving at 6. The units law firing inside a correction of a counting error.)*

- **The sequence, stated so it cannot be reached for in the wrong order:** enumerate over **sockets** (all profiles) → intersect with the **write-derived** set (`scripts/file-provenance.py`) → **ask** the survivors. `ListAgents` belongs nowhere in an authorship question; it answers a narrower one fluently, which is what makes it the attractive wrong tool. Only a write-derived name absent from a **socket-scoped** enumeration makes the question genuinely unresolvable, and that was never established here. **Broadcasting widens the guess rather than closing it** — and on a 16-session machine *"tell everyone plausible"* is not a bounded action, which is a second argument for identifying over broadcasting.

- **The two instruments that "agreed" were not independent — corrected by `codescout-09` 2026-09-01, and it is the sharper half of this whole exchange.** `CLAUDE.md`'s worked example of a *valid* elimination cited `ListAgents`' session list and a filesystem transcript-write set *"both returning the same three"*. **Both were per-profile.** The transcript glob only ever ran under `~/.claude-sdd/projects/…`; re-measured across all three profiles, `~/.claude` held **3 more freshly-written transcripts it never looked at**. So the two instruments shared a scope and agreed **because** of it — **one blind spot counted twice, which is indistinguishable from corroboration at the point of use.** The sentence defining the defect as *"a profile-scoped glob mistaken for the whole"* was itself that defect, in the clause defining it. **Independence is the property to check, not agreement**: two instruments returning the same number is evidence only if their scopes differ, and scope is exactly what a count does not carry.

- **The other sharp part is why the remedy did not fire: it existed, and nothing surfaced it.** `codescout-companion:reaching-peer-sessions` was in the responder's own skill list all session, its description naming the trigger verbatim — *"or when `ListAgents` returns fewer peers than expected"* — which is precisely the state observed. It went uninvoked through three peer messages and a `CLAUDE.md` paragraph *about the gap it closes*. This is `OB-1` at the instrument level rather than the parameter level: **knowing the class did not surface the tool.** A skill whose trigger is a condition the model must notice is a policy, not a mechanism — the same distinction this ledger draws everywhere else — and the standing remedy is to invoke it whenever a peer count is load-bearing and to quote the profile count it prints, rather than to intend to remember. Candidate `H-N`: surface it on the `ListAgents` response itself, which is the only place the precondition is observable.

- **Two cost facts from that instance, both against the responder rather than the defect.** The red was closed by its own author, unprompted, on their own schedule — so the whole measured cost of the incident was the misrouting around it. And the responder's broadcast **overstated the blast radius**, claiming the failure reddened `experiments`; it did not. `tests/issue_clusters.rs::valid_slugs()` reads the ledger with `std::fs::read_to_string`, so the gate reads the **working tree**, and the three offending slugs were absent from `HEAD` — pathspec commits left `experiments` green and CI, which checks out a commit, would have read clean. **Overstating a shared cost is its own defect**: it moves people to act urgently on the wrong object. That correction was made by a peer and verified at the source rather than accepted.

- **Both candidates are now probed, and both changed on contact — which is the case for probing remedies, not an exception to it.** The pathspec form turned out not to be a remedy at all on its own; it is one half of the composition above. `GIT_INDEX_FILE` **works, with a stated expiry.** Copy the shared index (`cp .git/index <private>`), reset the peer's paths *in the copy* (`GIT_INDEX_FILE=<private> git restore --staged --source=HEAD -- <their paths>`), commit from that. It never mutates the shared index — which is the whole point, because unstaging a peer's path to protect your own work is this defect pointed the other way. **The expiry is load-bearing:** the copy is a snapshot of a tree, so the moment a peer commits it is stale against the new `HEAD`, and committing from it would **revert their work**. Measured: a copy taken at 00:41 was invalid by 00:43, three peer commits later. Take it, use it inside one turn, confirm `git log` has not moved, discard it. A private index that outlives its window is a worse defect than the one it prevents — silent, and it destroys committed work rather than merely miscrediting it.

**Only a worktree changes the quantity being shared.** Every other remedy rearranges who touches a shared thing first.

**A THIRD direction, measured 2026-09-01, and it is not covered by the outbound/inbound split
above: PUBLICATION.** Everything above is about *content* — whose hunks land in whose commit.
This is about *reach*. Over six turns I told my user that six commits were "held local, yours to
green-light", and declined a peer's implicit invitation to push on the grounds that one
operator's approval is not standing permission. Then a peer pushed **its own** commit
(`01791c67`), which sat directly on top of my last held one (`34606388`) — and git pushes
**branches, not commits**, so all six went public in the same operation. Verified after the fact:
`git merge-base --is-ancestor 34606388 01791c67` is true, and all six report `PUSHED`.

> **Nobody did anything wrong, which is what makes it structural.** The peer's push was its own
> work, approved by its own operator, on a branch both sessions legitimately share. My restraint
> was correct and cost nothing to exercise. The defect is that **"I will hold this commit pending
> approval" is not a capability a session has on a shared branch** — it reads as a decision and
> is a wish. Any peer publishing anything above your commit publishes yours, and it needs no
> knowledge of you, no error, and no shared file.

**Why the blind party is structurally blind here:** at the moment of deciding to hold, the
session knows its own intent and cannot know a peer's next push. Nothing changes locally when
the publication happens — no file moves, no hook fires, `git status` is unchanged. I found it
only because a later `git rev-list --count origin/experiments..experiments` returned `2` where I
expected `8`, and I ran that for an unrelated reason. **The honest report to a user is
therefore "committed, and publishable by any peer at any time", never "held"** — the second
overstates a control the session does not have, and it overstates it to the one party who might
otherwise have said "then push it" or "then don't commit it yet".

**Mechanism, and it is the only one that actually holds:** do not commit to the shared branch.
A local branch or a worktree is the sole thing that makes "held" true — the same conclusion the
line above reaches for content, arriving independently for reach. Everything else is a
statement about intent on a resource that has no owner field.

**Instances:** the six recorded in the peer-capture bug file; `OB-2`'s `target/` clobber (the same class on a different resource, seen from the arming side); the `entry_high_water_<PREFIX>` cross-host collision; and the pre-commit hook's own whole-tree diff, which attributes by temporal proximity for exactly the reason described here and refused a push on a green run naming a file nothing under `src/` writes.

**Status:** open — promoted from `IC-1` on 2026-09-01, which this entry also **falsifies in part**: `IC-1`'s claim contains a *therefore* (*"its peer listing reaches only peers sharing its config profile. Coordination is therefore impossible by construction"*) and the causal link is broken. Coordination failed with enumeration complete. `IC-1`'s visibility half stands on its own members — `cross-account-agents-cannot-see-each-other`, `listagents-omits-cross-profile-sessions`, `peer-sessions-never-compares-start-time-to-build-time` — and is a real class; it is simply **not the reason** the write side has no remedy. `OB-3` is the visibility half already promoted; this is the ownership half, and they are siblings rather than one class.

## OB-9 — plausibility is a filter with a resolution limit, and a near-miss number fits inside it

**Valid:** invariant

**Rests on:** `OB-1` (*publish the derivation, not the value* — this row is the scope condition
that makes that remedy mandatory rather than stylistic); `OB-4` § *Second instance*, where this
mechanism was first written down as an aside under a different headline law; `CLAUDE.md`
§ *Testing Discipline* — *"a count of a defect population must arrive with its unit or not at
all"*, landed `fe085987`. Framing proposed by `codescout-e8`; instances and the table below
verified at source here.

**Class:** a published number wrong by a **small** margin is never queried, because the only
cheap check a reader has is plausibility, and a near-miss is precisely what plausibility means.
Error magnitude is **anti-correlated with detection**: a figure wrong by 5× is caught by its own
reader, one wrong by 15% is not. A wrong number far from the truth defends nothing; a wrong
number near it defends itself.

**Blind party:** the **reader** — a different party from `OB-1`'s author and `OB-4`'s trusting
session, which is what earns this a row rather than a paragraph. Structurally unable, for a
stateable reason: short of redoing the measurement, the reader's only instrument is whether the
value looks about right, and every member of this class looks about right. A *more careful*
reader does not catch it. Carefulness raises the threshold for what reads as wrong, and these
already sit under it. The check that works is re-deriving from scratch — which is the exact cost
that publishing a number is supposed to save, so the class is funded by the thing it defeats.

**Who can see it:** someone who re-derives independently, and nobody else. Across the four
instances below, **4 of 4 were caught by re-derivation and 0 by inspection** — including by
readers who were, at that moment, writing about this class.

> **That 0 is not as strong as it looks, and the weakness runs one way.** An instance caught by
> inspection produces no artifact — the reader doubts, re-counts, and the wrong figure never
> ships — so the recorded population is selected *against* exactly the outcome that would
> falsify the claim. The honest statement is: of the near-misses that reached a ledger, none
> was caught by reading. Whether reading catches any is not measured here and this corpus
> cannot measure it.
>
> **And it is a third instance of the monotone-population defect, not a repeat of the first two
> — the mechanism differs, and so does the fix** (`codescout-e8`, 2026-09-01).
> `cluster-promotion-session-log:F-1`'s nine probes were selected so that no *member* could
> falsify: the filter ran on the population. Here the filter runs on the **observations** — a
> reader who doubts and re-counts leaves nothing behind, so the refuting outcome is not absent
> from the sample, it is unrecordable in principle. This matters because the obvious remedy is
> not shared: **widening the sample fixes member-selection and does nothing at all for
> observation-selection.** A larger corpus of ledger entries still contains zero catches-by-
> reading, and would at any size. Only instrumenting the act itself — recording the doubt, not
> the correction — could measure it.
>
> **"And nothing does" stood here for one hour and was falsified by a peer doing it**
> (`codescout-e8`, 2026-09-01). It re-derived a "19 open bugs" figure it had published without a
> derivation, found the number **stood** — nothing older than the 14-day threshold — and *said so
> out loud* rather than moving on. That narration is the entire instrument: a null result is
> recordable exactly when someone states it, and it is otherwise indistinguishable from never
> having checked. **Practice, and it is the cheapest mechanism in this ledger:** when a
> re-derivation confirms, publish the confirmation. Nobody is inclined to — there is no finding
> to report — which is precisely why the negative case has no record.
>
> **One correction to how that datapoint was offered, because it changes what it is evidence
> for.** It was framed as telling against the "0 caught by inspection" figure. It does not: a
> re-derivation that *confirms* is not a catch, and the case never entered the wrong-number
> population at all. What it is instead is the first recorded **negative** — an unqualified
> number that was simply right — which bears on the class's **base rate**, a quantity this entry
> does not claim and cannot estimate from four positives. Worth the distinction: absorbing it as
> a near-catch would have made the population look self-correcting when what it gained was a
> denominator.

**Plausible-answer property:** definitional rather than incidental. Plausibility is not a side
effect of this failure, it *is* the failure.

**Vigilance:** wrong instrument, and this is the ledger's cleanest case for that phrasing.
"Read the number more carefully" is not a weaker remedy here, it is a null operation — the
discriminating act is arithmetic, not attention.

**Where it sits relative to its neighbours** — the same adjudication `IC-2`'s *Promotes to*
field already makes between `OB-4` and `OB-6`, extended by one:

- `OB-4` — **why a value is trusted**: an accuracy record it later spends.
- `OB-6` — **what an instrument does when it cannot tell**: returns the confident value.
- `OB-9` — **why the wrong value is not caught downstream**: it lands inside the reader's
  resolution limit.

Remedies differ, so none subsumes another. `OB-1` sits upstream of all three: it says why the
value was published thin in the first place.

**Boundary — what this class does NOT cover, worked on the case that tested it.** During this
entry's own write I cited a commit as `2dd5e4d3`, a SHA that is not a valid object in this repo
and that I had never read: `git show --stat HEAD | tail -4` prints the stat and the trailer and
**not** the identifier, so the commit was confirmed from output that never contained the thing I
then quoted. That is not an `OB-9`, and the reason is **cost, not plausibility**
(`codescout-e8`'s separator, which is sharper than the one I first offered). A hex string carries
no plausibility signal at all, so the reader has *no* margin rather than a narrow one — but the
deciding fact is that its check is **one command** (`git log -1 --format=%h`), where a near-miss
count needs an entire population re-derived. `OB-9`'s remedy is *attach the derivation, because
re-deriving is expensive*; this one's is *never write an identifier you have not read from output
in this turn*, which costs nothing and therefore admits no excuse. **Different price, different
rule** — file the identifier case under `OB-1` (a value whose only verification is a command the
author did not run). Recorded here because a class with no stated boundary absorbs its
neighbours, and this one was one sentence from doing so.

**Instances** — four, in four different measurement subjects, all 2026-08-31/09-01:

| published | true | ratio | subject | caught by |
|---|---|---|---|---|
| `4 of 20` (20%) | `1 of 4` / `2 of 4` — vantage-dependent (25–50%) | ~1.25× | peer enumeration | re-derivation via `scripts/peer-sessions.sh` (`cluster-promotion-session-log:F-3`) |
| `15` tests | `13` | 1.15× | tests reaching `ListFunctions`/`ListDocs` | an independent re-count by a peer (`93bd6f88`) |
| any one of `13`/`15`/`17`/`18` | all four correct, for four different questions | 1.38× spread | one population, four units | two parties counting independently and disagreeing (`fe085987`) |
| `IC-3` n=`20` | n=`18` | 1.11× | cluster size, feeding a promotion threshold | re-running the membership query after the `IC-15` boundary moved (`issue-clusters.md:429`) |

**The third row is why the class is not about wrongness.** Nothing in it is wrong: four correct
counts inside a 13–18 spread. Citing the wrong-unit one is undetectable by the same filter, so
this class covers correct-but-unqualified values and not only errors — which is the half a
reader primed for "check your arithmetic" will still miss.

**Mechanism status:** partial. The remedy is already published and already load-bearing; what
this row contributes is its **scope condition**.

- **Shipped:** *publish the derivation, not the result* (`OB-1`, and the mining table in
  § *The vigilance finding*). `CLAUDE.md` § *Testing Discipline* now states the mandatory form
  for counts.
- **The scope condition:** a derivation is optional for a value whose plausibility a reader can
  actually assess, and **mandatory** for one they cannot — which is every count over a
  population the reader would have to re-enumerate to check. The test is not *"is this number
  important?"* but *"could a reader tell if I were off by 15%?"*.
- **Candidate mechanism, not built:** `claim-decay.md:417` already parses fields stating a
  number a live query can re-derive. A `doctor` check flagging a **quoted count with no adjacent
  query or derivation** in a tracker body would fire on write, unconditionally, without anyone
  suspecting — the shape this ledger prefers. It does not exist.

**Status:** validated — four instances, each verified at source in this session
(`cluster-promotion-session-log.md:241-250`; `93bd6f88`; `fe085987`; `issue-clusters.md:391`
and `:429`, the last confirming n moved 20→18 with *no bug added, removed, fixed or
re-examined* — only the boundary).

## OB-10 — a mutual-exclusion resource is invisible to the session holding it

**Valid:** invariant

**Rests on:** `docs/issues/2026-09-01-an-unstaged-pre-commit-config-blocks-every-session.md`, and the membership test below rather than the enumeration, which is incomplete by construction.

**Class:** a *mutual-exclusion* resource — one whose merely-modified state changes behaviour for every other session — where the holder's own workflow keeps succeeding, and typically **clears the condition as a side effect of finishing**.

**Blind party:** the holder. Not carelessness, and not the same shape as `OB-2`: the holder is still present, still working, and every operation they run returns normally. The signal that would reach them is *their own* failure, and the mechanism guarantees they do not get one — in the instance above, the act that clears the block (`git add` the config) is the act by which the editor finishes their edit, so the outage ends in the same motion that would have revealed it.

**Who can see it:** every other session, immediately and on every attempt. They differ by *not holding the resource* — this is the rare case where the blind party is a population of one and the sighted party is everyone else, which inverts the usual reviewer advice: the remedy is not a second pair of eyes on the holder's work, it is a channel from the victims back to the holder.

**Plausible-answer property:** the holder gets *success*. That is the whole defect — no error, no warning, no slow path, just an ordinary working session. Note this differs from `OB-2`, where the victim gets a plausible WRONG diagnosis; here the victim's diagnosis is accurate and complete about the *what* (`Your pre-commit configuration is unstaged`) and silent about the *who*, so it is actionable only by guessing.

**Vigilance:** wrong instrument, and demonstrably so. The holder in the recorded instance was, at that moment, writing a commit message about shared-checkout hazards and had spent the evening filing observer-blindness findings. Knowing the class prevented nothing, because the class does not present to the holder at all.

**Mechanism status:** none yet. Two candidate shapes, in the order this ledger prefers them:

1. **An owner field on the resource** — the refusal names the file; it should name the holder. The machinery exists: `scripts/pre-commit-foreign-index.sh` already resolves a staged path to a `Session-Id` and prints `SendMessage(to: "uds:/run/user/<uid>/cc-socks/<pid>.sock")`. This converts an unbounded wait into a message and is `IC-17`'s remedy exactly.
2. **Make compliance end in a safe state** — the `OB-2` shape, here spelled *edit-and-land, never edit-and-hold*. Weaker, because it is a practice rather than a mechanism, and it is addressed to the party defined as receiving no signal.

**Membership test — three clauses, and the third is the one that decides.** A file is in this class when a **modified-but-unstaged** (or merely modified, for untracked shared state) copy (1) changes behaviour for a session *other than the editor*, (2) with a failure that does **not** name the editor, and (3) **while the editor's own operations keep succeeding**. Clause 3 was stated in this entry's prose from the start and left out of the test, and that omission is exactly why four candidates sat unverified below: without it the test admits `Cargo.toml`, which a measurement then rejects. Clauses 1 and 2 alone are barely narrower than "shared checkout" — every `.rs` edit affects peers and a broken build names no author either. What makes the class is that the party who could fix it is the one party with no symptom.

**Measured 2026-09-02**, each in an isolated scratch repo so the shared checkout was never touched:

- **Verified — 4.**
  - `.pre-commit-config.yaml` — `pre_commit/commands/run.py:353` refuses before any hook runs, on `git diff --quiet` against that one path. Blocks every session's commits. See the bug file.
  - `rust-toolchain.toml` — an unstaged pin switched the active compiler `1.95.0` → `1.97.1` and forced `Compiling` where the preceding no-op build did nothing. Every peer's next `cargo` invocation silently uses a different compiler and rebuilds; the editor wanted the switch, so nothing looks wrong to them.
  - `.cargo/config.toml` — flipping **one** `rustflags` entry forced recompilation against a no-op baseline. On a 408-file workspace sharing one `target/`, that is a full rebuild imposed on every concurrent session.
  - `.gitignore` — an unstaged edit made a file vanish from `git status`; the peer can no longer see it or `git add` it, and nothing reports why.
- **DROPPED — 1, on measurement rather than judgement.** `Cargo.toml` / `Cargo.lock`. A malformed manifest fails the **holder** too — `error: unclosed table, expected ]`, exit 101 — so the editor gets an immediate, precise signal and clause 3 fails. (An *unknown key* is the other end and is equally out: `warning: unused manifest key`, exit 0, nobody affected.) It is ordinary shared-checkout coupling, which is a real hazard and a different one.
- **A different class, noted so it is not swept in — 3.** `.codescout/workspace.toml`, `.claude/settings.json`, `.claude/settings.local.json` are **untracked**, so there is no staged/unstaged distinction and no git-mediated remedy at all: the file simply is the shared state. `CLAUDE.md` already documents the `workspace.toml` case, where a mis-rooted `[[project]]` silently redirects every per-project memory write and no review can catch it.

**What the pass cost and what it bought:** four candidates in, three confirmed, one refuted, and a test that can now discriminate. The refutation is the useful half — a list that had survived review as "reasoned" contained an entry a five-minute probe rejects.

**A second instance in the same tool, filed separately:** `docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md`. It is `IC-12` rather than this class — there the *reader* is deceived and the writer is fine, the exact inversion — but it shares the substrate and, usefully, the remedy direction: its exposure is the hook runtime, which `9e493b20` cut from ~2000 ms to ~40 ms while fixing something else.

**Instances:** `docs/issues/2026-09-01-an-unstaged-pre-commit-config-blocks-every-session.md` (`IC-17`); adjacent, `docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md` (`IC-12`); `9e493b20`.

**Status:** open — **four verified instances**, one refuted, one session. The enumeration is no longer the honest gap; the **mechanism** is. Nothing in this repo names the holder of a dirty shared resource, and the machinery to do it exists (`scripts/pre-commit-foreign-index.sh` resolves a path to a `Session-Id` and prints its `SendMessage` address). Note the four verified files split by remedy: `.pre-commit-config.yaml` wants an owner field, while `rust-toolchain.toml` / `.cargo/config.toml` / `.gitignore` want *isolation* — which is `IC-17`'s other remedy and, concretely, per-session worktrees.

## OB-11 — a session-keyed ledger answers for a party the protocol never named

**Valid:** conditional — MCP tool calls gain per-agent identity, at which point the ledger can key on it

**Rests on:** Iron Law 6 — the parent-briefs discipline exists *because* the server cannot tell parent from subagent; `docs/issues/archive/2026-09-01-subagent-told-to-skip-guides-it-never-received.md`

**Class:** shared state keyed at a coarser grain than the party reading it

**Blind party:** the codescout MCP server. `guide_hints_emitted` is keyed by Claude Code `session_id`, which a subagent shares with its parent — no separate MCP identity exists for it. The server cannot distinguish them **because the protocol never carries the distinction**: `agent_id` rides the harness's `SubagentStart`/`SubagentStop` events and appears on no MCP tool call. This is not inattention; no amount of care inside the server recovers a field it is never sent.

**Who can see it:** the harness, which mints `agent_id` at dispatch, and the companion hooks that receive it. The asymmetry is the whole entry — **the party holding the identity is not the party holding the ledger**, and a protocol boundary separates them that neither crosses alone.

**Plausible-answer property:** a subagent's *first* fetch is reported as a repeat, and its auto-inject is suppressed as already-delivered. Both are well-formed and confident. Nothing errors, nothing is empty, and the body is still returned — so the failure is invisible at the call site and surfaces only as degraded downstream work.

**Vigilance:** wrong instrument — and this instance is unusually sharp about why. The guide text told the parent to make it *worse*: `iron-laws-detail`'s brief bullet prescribed letting the subagent "short-circuit redundant `get_guide` calls", so a **more** diligent parent produced a **more** starved subagent. Two further sites reinforced it, one of them naming a subagent's own `get_guide` call as evidence the parent had underbriefed — i.e. the correct behaviour was documented as the symptom. Three mutually-confirming wrong signals; caught by a user noticing subagent output quality, by no check.

**Mechanism status:** partial — wording corrected at three sites (2026-09-01), and `repeat_fetch_keeps_body_and_flags_static` now pins the note *negatively*, so drift back toward asserting the caller already fetched it reds the lane. `codescout-companion`'s `SubagentStart` hook tells subagents to fetch regardless of their brief, covered by `tests/test-subagent-guidance.sh`. The **keying** is untouched and unfixable from inside this repo. The companion's `agent-guide-snapshot.mjs` / `agent-guide-restore.mjs` bracket already works the sibling direction (a subagent's marks starving the parent), and its own scope note records that it is inert within the session it runs in — a second party attacking the same boundary from the other side, reaching the same partial result.

**Instances:** `docs/issues/archive/2026-09-01-subagent-told-to-skip-guides-it-never-received.md` (this repo, archived 2026-09-01). Cross-repo, prose-only and deliberately not an edge: `claude-plugins`'s `codescout-companion/docs/issues/archive/2026-08-26-subagent-guide-fetch-starves-parent.md` and `…/2026-08-27-guide-ledger-bracket-is-inert-within-its-own-session.md`.

**Status:** open — 1 session (2026-09-01), user-reported. The sibling direction was independently filed in `claude-plugins` across three earlier sessions; that is corroboration of the keying defect, not a second instance of this row.

## OB-12 — a section that falsifies a sibling emits no signal, and the falsifying author is the blind party

**Valid:** invariant

**Rests on:** `design-backlog-session-log:F-3`, `design-backlog-session-log:F-4`,
`design-backlog-session-log:F-7`, `design-backlog-session-log:F-8`; `capability-proposals:CAP-12`
(measured corpus and the rejected detector); the currency-marker convention shipped 2026-09-02
into `get_guide("tracker-conventions")` § *A `## Resume` states its own currency*.

**Class:** a document's sections carry **no dependency edges**, so a section that falsifies a
sibling emits **no signal anywhere** — not in the diff, not in a link check, not in the
falsified section itself, which continues to read as settled prose.

**Blind party:** the author adding the new section, specifically. They are facing forward —
composing the alternative, the correction, the newer measurement. The sections their new one
refutes were written earlier, are internally coherent, and read as finished. Nothing in the act
of appending points backwards. In the recorded corpus the falsifying section was **written by
the same review, in the same pass**: `run-command-pipeline.md`'s Concern 1 introduced Strategy C
and thereby retired claims in Concerns 2 and 3 that Concerns 2 and 3 went on stating for 106
days. **A review that introduces a new alternative does not automatically re-audit its own
earlier concerns against it.**

**Who can see it:** the first party whose *task* forces two sections into contact — an
implementer following the older one's instruction, who then reads the newer one and finds it
contradicted. Not a more careful reader: a differently-tasked one. That is why every instance
below was found while *acting on* the document and none by reviewing it.

**Plausible-answer property:** the stale section is well-formed. No dangling reference, no
broken link, no error — a true-sounding instruction whose premise a sibling quietly removed.
`run-command-pipeline.md` § *Resume* said *"write `run_pipeline_inner` per strategy A"* while
its own review rejected Strategy A; `capability-proposals.md` CAP-7 § *Resume* says *"Next:
check 1"* above a status block listing check 1's commit and patch-id.

**Vigilance:** wrong instrument, twice over. "Re-read the whole document when you add a
section" is unenforceable and nobody does it at 1,500 lines. And the party it addresses is
the one who just finished writing the *new* section — the moment of least attention to the old
ones. Measured: three of the falsified claims survived a full session of a reader who was
*actively ruling on that very document*, and were caught only when each became the next
concrete task.

**Mechanism status:** partial. The **currency marker** shipped 2026-09-02 —
`get_guide("tracker-conventions")` § *A `## Resume` states its own currency*: a section either
dates its heading or opens with a banner naming what superseded it and where the current state
lives. Measured over the corpus that motivated it: **0 of 5 sections carrying a marker were
misleading; 3 of 3 without one were stale.** What it does **not** cover is the case in this
row's own name — it fires when the author *notices*, and the blind party is defined by not
noticing. `capability-proposals:CAP-12` proposed a `doctor` check and was **rejected on
measurement**: the discriminator is the marker, not age, so an age-keyed check flags every
correctly-dated section and catches nothing the marker misses. **No mechanism detects the
un-noticed case, and none is proposed here** — recording the gap rather than inventing a
remedy for it.

**Membership test — three clauses.** A section is in this class when (1) a **later** section of
the same document removes a premise the earlier one states or relies on, (2) the earlier
section remains **internally coherent** and carries no marker of its supersession, and (3) the
falsifying edit produced **no artifact** naming the earlier section — no cross-reference, no
diff hunk touching it, no check. Clause 3 is what makes it this ledger's business rather than
`claim-decay.md`'s: `DC-N` is about **time** making a claim false, and here the trigger is a
**sibling edit**, which is an event with an author and a timestamp that simply points nowhere.

**Not `R-N`, and not `I-N`.** `reconnaissance-patterns` records what a scout hit or missed —
scouting is how all five instances were caught and is the *detection* story, not the class.
`test-escape-hardening` (`I-N`) is a natural consumer if the un-noticed case ever gets a
mechanism; it has none yet, so there is nothing to hand it.

**Instances (5, two documents):**

| instance | falsifying section | falsified claim |
|---|---|---|
| `F-3` | Architectural review (appended above § *Resume*) | § *Resume* routed to Strategy A, which the review rejected |
| `F-4` | Concern 1 introducing Strategy C | *"per-stage cancellation impossible"* — carried as a cost with no reachable caller |
| `F-7` | Concern 1 introducing Strategy C | *"per-stage timeout impossible"* — true of Rust, false of the capability |
| `F-8` | Concern 1 introducing Strategy C | Concern 2's *"before `pipeline=` goes anywhere"* prerequisite, which C dissolves |
| `CAP-12` corpus | CAP-7's status block | CAP-7 § *Resume*'s *"Next: check 1"*, already shipped at `b34bf10e` |

## OB-13 — deleting a token cannot enumerate the negative assertions that survive it

**Valid:** invariant

**Rests on:** `codescout-0a` (sessionId `2cb44cd3`), which found the instance while scouting Task 4
of the tool-surface collapse and supplied the trigger direction; this session's failed
counter-selector, which established that the opposite direction is closed; and `0a`'s subsequent
correction of its own repair, which is the second half.

**Class:** deleting a token from the tree makes every **negative assertion** naming that token
vacuously true — permanently, silently, and while staying green. `assert!(!rendered.contains(X))`
after `X` ceases to exist is the monotone-under-removal law (`CLAUDE.md` § *Testing Discipline*)
with a specific trigger: **a dead mechanism produces exactly the silence the assertion asserts.**
The suite does not shrink, no test reddens, and the guard survives in the file guarding nothing.

**Blind party:** the author performing the deletion, specifically. They are looking at what is
going away — the diff enumerates removals, and every removal is accounted for. The negative
assertions that *mention* the token are not in the diff: they survive it untouched, compile, pass,
and read as guards. Nothing in the act of deleting points at them. The instance is exact about
this: `src/prompts/mod.rs:2603` carries the comment *"After Task 14 lands, the librarian block must
not be appended"* — it was **written as a forward-looking guard and is made vacuous by the very
event it was written for**, by an author who had that event in mind.

**Who can see it:** not a more careful deleter. Someone starting from **the token** rather than
from the diff — which is a mechanism, not a person.

**The reverse direction is closed, and that is what makes this a mechanism rather than a habit.**
Starting from the token and asking *"which negative assertions mention it"* is a `grep`. Starting
from the assertion inventory and asking *"which will go vacuous"* is **not answerable at any level
of grep sophistication**, because the discriminator is who *owns* the token and ownership is not in
the text. Measured 2026-09-02 by attempting it: 121 negative containment assertions in the suite,
93 parsing to a `(file, line, literal)` triple; the selector *"literal appears only in its own
assertion's file"* returned **28**, and they are overwhelmingly **fixtures the test invented** —
`get.rs:1086` writes `"…## Beta\n\nbeta body\n"` and `:1095` asserts a heading-scoped read excludes
it, which is correct, discriminating, and legitimately unique to that file. The selector measured
fixture authorship. Publish the 28 as the reason the trigger runs the other way: a reviewer's first
instinct is *"why not scan the assertions"*, and *"somebody tried and got 28 wrong answers"* closes
it in one line.

**The blindness recurs in the REPAIR, which is the half that would otherwise be missed.** `0a`
diagnosed the vacuous assertion correctly and prescribed a replacement needle,
`doc(action="event_create")`. The reviewer committed the exact regression the test guards and it
**passed** — the prescribed needle closes the paren immediately while the guide emits
`doc(action="event_create", id=…` with a comma, so the new needle also has zero occurrences. It
diagnosed a vacuous assertion and prescribed one vacuous *by a different mechanism*. Its own
account of why: it demanded an observed RED of the implementer and never of its own repair. The
generalisation is checkable and one command — **a needle is a claim that a string can appear, so
grep for it** — and the repair is not done until the guard has been seen to fail.

**Second instance, and the first found PROSPECTIVELY — which is the mode the class was designed
for.** Measured 2026-09-02, within the hour of filing: `codescout-0a` ran `H-9`'s query
pre-dispatch against a token Task 5 was about to delete, and found
`src/librarian/catalog/augmentation.rs:3027` — `!text.contains("artifact_augment(")` — **before
the code moved**. Same shape as `prompts/mod.rs:2603`, same day, different subsystem, and the
guard it encodes is a real one: *do not prescribe a remedy that refuses in the situation it is
prescribed for.*

That changes what the class is FOR. Discovered as a diagnosis after the fact, it is now usable as
a **checklist item at dispatch time**, which is strictly cheaper — one grep, before the token is
gone. The trigger fires on a token being *scheduled* for deletion, not only on its absence, and
that matters because after the deletion the class is unfileable: the token is gone and nothing
remembers it was there.

**Mechanism status:** `designed, not built` — `H-N` candidate. Trigger: a token leaving the tree,
or a plan scheduling one. Query: `grep -rnw '!.*contains.*<token>'`. Plus the repair-side check
below, which is the cheaper half and covers a failure the trigger cannot.

**Not promoted, and the population is honest about itself:** **two** instances of the deletion half
(`prompts/mod.rs:2603` retrospective, `augmentation.rs:3027` prospective — different subsystems,
same day) and one of the repair half, all from the same work stream. One work stream is not two
subsystems in the sense the promotion bar means, whatever the file paths say.

## OB-14 — the sampling frame is unauditable from inside it, and review inherits it

**Valid:** invariant

**Rests on:** `provenance-probe-session-log:F-13` and `provenance-probe-session-log:F-14` (the two
instances); `provenance-probe-session-log:W-9` and `provenance-probe-session-log:W-10` (the practice
that caught them); `reconnaissance-patterns:R-53` and `reconnaissance-patterns:R-54` (the scouts).

**Class:** the **sampling frame** — what the corpus physically contains, what one row *means*, and
how rows were selected — is the premise every downstream number rests on, and the one premise no
downstream check reads. A frame error therefore yields a body of individually correct, internally
consistent, collectively meaningless figures.

**Blind party:** the analyst — and for a reason stronger than inattention: their entire access to
the population runs *through* the frame. Auditing it would need a view of the population that does
not come through the frame, which is by construction the one view they do not have. Every check
available to them is computed **from** the corpus and is therefore **inside** it.

**Who can see it:** whoever knows the **provenance of the inputs** — how the data came to exist,
which of it was real work, how much of it there is. That is a *different qualification* from
`OB-1`'s *"a reader who does not share the author's context"*, and the difference is this entry: a
peer reviewer with full data access is **worse** placed than a non-analyst who knows the world the
corpus came from, because the reviewer inherits the frame and the outsider never had it.

**Plausible-answer property:** definitional rather than incidental. A frame error moves numerator
and denominator **together**, so every ratio, reconciliation and consistency check passes — those
checks are computed under the frame's own definitions. Measured: thirteen rounds produced no figure
that looked wrong to any of five parties, over a corpus that was ~60% base64 by the study's own
accounting and whose 64 "sessions" were 34.

**Vigilance:** wrong instrument — and this entry carries the ledger's strongest evidence for the
**review** half specifically. The programme ran thirteen rounds *and* a sustained adversarial review
by an independent session which reversed four conclusions (a gate's lift against a null model, a
source-label catch-all, a retrieval-granularity claim, two self-contradicting verdict lines). Not one
of those parties questioned the frame, and they could not: adversarial review is an instrument aimed
at the **reasoning**, while reviewer and analyst read one corpus through one frame. This is `OB-4`'s
*"instruments sharing a substrate are one instrument"* applied to **reviewers** — which predicts that
adding reviewers is a null operation against this class at any level of rigour, and that the four
reversals it did produce are evidence the review was working, not evidence it could have reached here.

**Mechanism status:** `designed` — shape (2) from § *The vigilance finding*, an unconditional policy
tied to a trigger that occurs anyway.

- **The policy: show the data owner the corpus census and the sampling frame BEFORE showing anyone
  the findings.** The trigger is *about to report a measurement*, which happens in every measurement
  task, so nothing has to be noticed. Both instances below were closed by the owner in **one sentence
  each**, against artifacts readable in under a minute — *"the browser MCP is not part of the normal
  flow"* and *"we should have more than 64 sessions, much more."*
- **The census, and it is three queries plus one classification.** (1) `count(*)` against
  `uniqExact(<thing_id>)` — rows per thing; if the ratio is not ~1, rows are not observations.
  (2) `sum(len)` against `sum(max(len) per thing)` — the double-count factor; a large ratio means
  rows nest, so every sum is inflated and every "independent" observation is a prefix of another.
  (3) `uniqExact(<thing_id>)` **on the sample** — the effective n; report that, never the row count.
  (4) Per **producing tool**: calls, bytes, share of bytes. A single producer above ~20% of bytes is
  a validity question before it is a distribution feature — ask what those bytes *are*, which
  character-class statistics answer (base64 run fraction, whitespace fraction) while reading no
  content, so the check stays privacy-safe.
- **What it is not.** It is not "sample more" — that is the reflex answer and it is a null operation
  here, because a larger sample drawn through the same frame reproduces the error at higher
  confidence. In the second instance the sample was 0.1% of the population *and* the fix was not
  simply to take more: 444 sessions drawn in the wrong **unit** would have been equally wrong.

**Instances — two, one work stream, and the population is stated rather than padded.**

| | frame parameter that was wrong | how it presented |
|---|---|---|
| `provenance-probe-session-log:F-13` | **content** — a byte was assumed to be a text byte. `flatten()` is `json.dumps`, so image blocks were stringified and their base64 counted as text: 71/71 payloads >90% base64, median 100% of the payload inside one contiguous base64 run, 37/71 truncated at a 400 KB cap | base64 holds zero codebase-specific tokens *by construction*, so it inflated every share denominator **and** depressed every utilisation figure at once — pushing in both directions, which is why no single number looked absurd |
| `provenance-probe-session-log:F-14` | **unit and selection** — a row was assumed to be a session. One row is a *request*, and each request re-sends the whole conversation: 143 requests per session, a **165×** double-count on summed lengths. 64 sampled rows were 34 distinct sessions; a top band of 13 rows was 6. Strata were equal-n over populations of 567 and 17,003 | an absence claim over 34 sessions was published as a population claim (*"the trigger population is empty"*); the true figure was 2 calls. Bands stratified by size were **conversation-depth** strata wearing size labels |

**Boundary — stated because a class with none absorbs its neighbours.**

- **Not `OB-1`.** OB-1 is a *published claim* omitting a parameter the reader needs, and its remedy
  is to publish the parameter. Here the parameters **were** published — derivations, units, counting
  rules — and were correct under a frame that was not. Publishing harder is a no-op; the defect is
  upstream of publication.
- **Not `OB-3`.** That is one *instrument* whose result set is a subset by an undocumented rule, and
  its remedy is a second instrument on a different substrate. Here the frame is not an instrument
  that under-reports; it is the definition of the population all instruments range over, so a second
  instrument inherits it.
- **Not `OB-9`.** That asks why a wrong value is not caught **downstream**, and its answer is the
  reader's resolution limit. This asks why the value was wrong **upstream**, with the reader's
  plausibility filter working correctly on a number that was correctly computed.
- **Not `issue-clusters:IC-18`.** A selector narrower than its population *never saw* the excluded
  members. Here the selector saw everything it was pointed at; what was wrong was what a row **meant**
  and what the bytes **were**. IC-18's remedy — widen the selector or name its scope — repairs
  neither instance.

**A published confirmation, per `CLAUDE.md` § *Testing Discipline*'s recording law.**
`reconnaissance-patterns:R-54`'s second half — *never let "zero observed" become "empty" without
stating n* — was written as a new rule and is a **re-derivation**: `CLAUDE.md` § *Testing Discipline*'s
*"a count of a defect population must arrive with its unit or not at all"* and `OB-1`'s *"report a
search count together with the key you searched for"* already cover it. Its first half —
**pseudo-replication**, rows nested as prefixes of one another — has no antecedent found in this
corpus. Recorded because a re-derivation that lands on an existing rule leaves no artifact unless
somebody writes it down, and absorbing it as a new finding would have made this ledger look more
productive than it was.

**Status:** open — **two instances, one work stream, one analyst.** Deliberately **not promoted**,
on `OB-13`'s own reasoning: one work stream is not two subsystems whatever the file paths say, and
both instances here share an analyst as well as a corpus, which is a narrower base still. What the
pair does establish is the *review* claim, because that ran across five parties rather than one. A
third instance from a different work stream, or one instance where an internal check or a
frame-sharing reviewer **did** catch a frame error, is what decides this — the second would falsify
it outright.

**Falsified by** a frame error caught by a party who reached the corpus only through the frame —
an internal consistency check, or a reviewer with the analyst's data access and no independent
knowledge of the inputs' provenance. That would make this ordinary carelessness with a careful
remedy available, which is exactly what this ledger's admission test excludes.

## Template for new entries

Copy the field block; keep the labels **exactly** as written, line-anchored, so the greps
in *How to mine this* keep working. Drop no field — write `unknown` or `none yet`, since
an absent label is invisible to a presence sweep while an explicit `none yet` is a
worklist row.

```markdown
## OB-N — <the class, stated as a claim>

**Valid:** invariant | dated YYYY-MM-DD | conditional — <event>

**Rests on:** <ADR, decision, bug file, or principle — not a path:line, which rots>

**Class:** <what kind of thing goes wrong>

**Blind party:** <who cannot see it, and THE REASON they cannot — not that they missed it>

**Who can see it:** <the party or instrument that can, and why they differ>

**Plausible-answer property:** <what the blind party gets instead of an error>

**Vigilance:** wrong instrument | helps | untested — <evidence>

**Mechanism status:** shipped | partial | none yet — <the check that runs unprompted>

**Instances:** <bug files, R-N, F-N, SHAs>

**Status:** open | validated | superseded — <instance count, sessions>
```

**Before you write one, check it is really an `OB`:** would a more careful version of the
same party have caught it? If yes, it is an `F-N` or a bug file. The test is *structural
inability*, not *observed failure*.
