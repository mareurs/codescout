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
entry_high_water_OB: 7
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
| OB-7 | 2026-09-01 | a declaration is well-formed, and nothing in production reaches it — including for the compiler, which cannot lint `pub` in a lib crate | the author of the declaration | wrong instrument | **partial** — decidable for 1 of 3 families |
| OB-6 | 2026-09-01 | a gate collapses "cannot observe" into the confident answer — three states exist, two are offered | the gate, and every reader of its output | wrong instrument | **designed** — exemplar shipped at `b9cc75b4` |
| OB-5 | 2026-08-31 | a summary keyed only by findings cannot say which checks RAN — absence reads as a clean bill of health | the reader of the report | wrong instrument | **none yet** — designed, 3 lines |
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

- **Publish the derivation, not the value.** *"8 verbs; the block prints 9 lines because
  clap appends its own `help`; and it is 8-or-nothing because the `#[cfg]` sits on the
  whole `Commands::Artifact` variant"* survives re-measurement. A bare `8` does not.
- **Report a search count together with the key you searched for**, so a reader can see
  what it could not have found.
- **Name which part of an instruction is the evidence.** *"Check it exits 0 and lists
  `find`"*, never *"check that it looks right."*
- **Unconditional policy that caught one:** *verify before contradicting a peer.* It
  fires on an event that happens anyway rather than on suspicion.
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
## OB-2 — the session that arms a shared-state trap gets no signal

**Valid:** conditional — reopens if the gate's command order changes, or if a second
terminal command is appended after the default-features lane

**Rests on:** `docs/issues/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md`

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

**Mechanism status: shipped** — `73066479`. The documented gate now runs the lean lane
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
**Re-verified 2026-08-31: `ListAgents` reported 2 peers while 7 other sockets were
live.**

**Vigilance:** wrong instrument. A short view makes elimination weak, so a careful reader
hedges — but a **disjoint** view makes elimination *unrelated* to the answer, so hedging
still draws the wrong conclusion. Care cannot rescue an instrument whose units differ
from the question's.

**Mechanism status:** shipped as practice — enumerate from the OS before attributing, and
message invisible sessions directly at `uds:/run/user/1000/cc-socks/<pid>.sock`. Not
gated; a candidate `H-N`.

**Tell, reusable beyond this instance:** the instrument's units differ from the
question's — *sessions a transport knows about* vs *processes with this cwd*. And
repeated "it must be the remaining one" reasoning is equally well produced by the answer
lying outside the set entirely.

**Status:** validated — measured twice, by two sessions, with the membership rotating
between readings (which demonstrates arbitrariness where one snapshot could only show
disjointness)

## OB-4 — a liveness marker with a good hit rate is more dangerous than a bad one

**Valid:** conditional — closes when `.worktrees/bench`'s gitdir is repointed or the corpus is re-created under `codescout`'s own `.git`

**Rests on:** `docs/issues/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md` § Residual; reported by `codescout-fe`, verified here 2026-08-31.

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
`no-local-embedding` (correctly deleted, and the source of the marker's false credibility).

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

## OB-6 — a gate collapses "cannot observe" into the confident answer

**Valid:** invariant

**Rests on:** `issue-clusters:IC-2` (`cluster/gate-keyed-on-unobservable-event`, n=16 at promotion); the shipped exemplar is `index_envelope`'s doc comment in `src/tools/config/mod.rs`, which states the remedy at the site.

**Class:** a gate whose condition is an event **outside its observation boundary** substitutes a proxy — a caller-supplied flag, a monotone stamp, a per-process map, a chunk count — and then maps the proxy's *unknown* onto its *confident* value. The proxy is not the defect; collapsing three states into two is.

**Blind party:** the gate, and therefore every reader of its output. The reason is scope, not care: the event is conversation-scoped (a compaction, a `/clear`), harness-scoped (an `/mcp` reconnect, what a parent session already holds), or peer-process-scoped (whether a companion hook is still alive), and the gate is process-scoped. It holds no handle to the scope that would answer the question. **No amount of care inside the process reaches a fact the process has no channel to** — which is the admission test, and it is why this is not "someone forgot to check".

**Who can see it:** an instrument on the **event's own side of the boundary**. For the filesystem-scoped half that is the filesystem itself — `git_sync_status`, a file count, an mtime — and the gate simply never looked. For the harness-scoped half it must be a **positive signal written by the party that can observe the event**: the companion hook already reads Claude Code's `input.source` enum (`{startup, resume, clear, compact}`) and throws it away one line from where `post_compact` needs it. Note what does *not* work: `hook_at` looks like a liveness signal and is not, measured across five healthy sessions at ages 0.6 h–25.0 h, so a staleness window would have to exceed ~25 h to avoid false-deactivating a healthy session — it detects nothing. A proxy that is *nearly* the right substrate is the most expensive kind.

**Plausible-answer property:** an extra guide body, an open gate, a shut gate, a re-cleared ledger, `up_to_date` on an index 286 commits behind HEAD. Nothing throws, so nothing downstream fires. The shipped `chunks > 0 → "up_to_date"` mapping is the class in one line: *a statement about non-emptiness wearing the name of a statement about currency*.

**Vigilance:** wrong instrument — and the rendezvous gate is the proof, because **one gate produced three separate members**: it latches open when the hook goes quiet, it can never be stamped again if it misses its `SessionStart`, and an `/mcp` reconnect leaves it permanently inactive. Three ways for one proxy to be wrong, found separately across twelve days by people who already knew about the previous one. Care applied to a proxy sharpens a wrong answer rather than correcting it.

**Mechanism status:** designed, with a shipped exemplar — `b9cc75b4` (patch-id `5a5c761072c44b54dc80f224d89355dd2d31e498`) closed one member and demonstrates all three moves.

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

1. **Dead in production (9).** The code exists; only tests call it. **The partition is available and was measured**: `grep` annotates every hit with its enclosing symbol path, and test call sites carry a `tests/` prefix — `[tests/names_path_containing_generalises_and_normalises_separators]` versus `[names_tracker_path]`. Call-site rather than file granularity is **required, not merely preferable**: `references` groups `src/librarian/adapter.rs:1451` under a production file and `grep` shows it is `tests/…`, so a file-level partition misclassifies it. **But the check does not "decide" this, and the overstatement is the load-bearing part.** It decides for **by-name call sites only**: a symbol reached through `dyn Trait` dispatch, a function pointer, a macro expansion or a re-export alias has zero textual non-test hits and would be flagged dead — in a codebase whose dominant idioms are `impl Tool` and `Arc<dyn CodeEmbedder>`, which is where the check would be run. `references` (LSP, semantic) catches dispatch; `grep` (text) does not, and citing an LSP-verified result to justify a text-search check conflates two substrates. **The failure direction is the dangerous one**: a false *dead-in-production* finding authorises a deletion on a negative search result. **And the zero-caller state is unexercised** — nine symbols probed on 2026-09-01, every one confirming the *has-production-callers* state; no negative control was found, so the state the check exists to detect has never been observed. Roughly six of the nine members are Rust symbols and in scope; the rest are a shell hook and a cargo feature, needing their own reachability notion. Recorded at `cluster-promotion-session-log:F-1`.
2. **Schema or doc declares what the code ignores (3).** The check is a round-trip — for each declared field, assert the handler reads it — and it is **weak**: reading a field is not using it, so this detects the absent read and not the ignored value.
3. **A matcher that can never match (6).** `op-4-path-predicate-can-never-fire`, `triggered-operator-rules-route-nothing-in-production`. This is IC-3's original phrasing — *for each declared selector, assert some production path emits it* — and it is the **hard** one: it needs the set of values production actually emits at a call site, which is a dataflow question no index answers today.

**Do not record this as `designed` for the class.** One family has a decidable check, one has a weak proxy for a check, one has no mechanism at all. `IC-9` was carried at `designed` on exactly this conflation — a member that *named* a check while recording it as insufficient — and the cost was a later reader re-running the measurement it warned against.

**Instances:** the 18 members of `cluster/declared-not-wired`, spanning the embedder chunk budget, the live Qdrant payload (579,311 chunks with empty `ast_kind`/`ast_header`), `grep`'s overflow hints, `link_scan`'s bucket split, `audit_doc_refs`' stubbed-off LSP, the operator-rule router, the constitution glob, librarian's error downcast, the CLI's `doctor`, and a CI lane that compiled no tests behind `server-stack`. `OB-5`'s *Known-open residual* is this class seen from the reporting side — a check whose `extend()` line is deleted still reports `0`, because the enum still declares it — and the two should be cited across rather than merged: `OB-5` is about a summary that cannot say what **ran**, this is about a capability nothing **reaches**.

**Status:** open — promoted from `IC-3` on 2026-09-01 at n=18, the second-largest class in the corpus. **The `IC-15` boundary was raised before the archive tagging pass and settled during it**, on a remedy test reducible to one question: *was a caller-supplied value accepted?* Yes — the path ran and discarded it, remedy is round-trip or refuse, `IC-15`. No — the capability exists and no call site reaches it, remedy is find a caller, `IC-3`. Exactly two members moved; `cli-doctor-exposes-no-fix-flag` stayed because the flag does not exist at the boundary at all, so nothing is accepted to be dropped. Worth carrying beyond this pair: every one of these reads as *"declares X but does not do X"*, a sentence fitting at least four classes here — matching on it is how `IC-9` acquired two misfits, and the **remedy**, never the description, is the discriminator.

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
