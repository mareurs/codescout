---
kind: bug
status: open
tags:
- cluster/authorship-unrecoverable-after-the-fact
closed: null
opened: 2026-09-01
owner: marius
related: []
severity: medium
---

# BUG: a peer's un-wired function reds the shared build for everyone, and nothing can say whose it is

## Summary

On a shared checkout, the window between writing a function and wiring it up is a window in
which `-D dead-code` **aborts compilation for every session in the tree**. The failure is
correct — the build really is broken — but it carries no authorship, so it presents as *your*
breakage. This is the read-side of `IC-10`: the write-side asks "who wrote this?", the read-side
asks "is this mine?", and neither is answerable from the tree.

Filed as a distinct record rather than folded into a peer's, because the cost was mine and the
measurement is mine.

## Symptom (Effect)

Running the documented gate mid-task, having touched none of the implicated files:

```
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
error: function `scan_unterminated_fence` is never used
    --> src/librarian/tools/doctor.rs:4940:4
     = note: `-D dead-code` implied by `-D warnings`
error: could not compile `codescout` (lib) due to 1 previous error
```

```
cargo test --lib librarian::tools::
  doctor::tests::unterminated_fence_fires_only_where_a_fence_is_left_open      FAILED (0 != 1)
  doctor::tests::unterminated_fence_names_the_line_the_silenced_region_starts_at  FAILED (0 != 1)
test result: FAILED. 953 passed; 2 failed
```

`-D dead-code` **aborts the lib build**, so no other lint in the workspace is reached. A session
running the gate to check its *own* work gets no information about its own work at all.

## Reproduction

On a checkout shared by ≥2 sessions: session A writes a new `fn` and its tests but has not yet
added the call site. Any other session running the documented gate sees the above. Window closes
when A wires it up; here that was roughly ten minutes.

Note this needs no mistake by A. Writing the function before the call site is ordinary, and the
tests-first ordering this repo asks for makes it *more* likely, not less.

## Environment

Linux, Rust, codescout `0.15.0`, branch `experiments`, one working tree shared by 5 concurrent
Claude Code sessions (`peer-sessions.sh` count; `ListAgents` reported 4 — see Root cause).

## Root cause

Two things compose, and only the second is a defect.

1. **`-D dead-code` is correct and fails in the safe direction.** An unreferenced function is
   genuinely dead, the error is loud and local, and the abort is the desired behaviour for a
   single-owner tree. Nothing to fix here.

2. **A working-tree modification carries no author, so the failure cannot be attributed.** Git's
   author field is a constant across sessions (`IC-10`), and for *uncommitted* state there is
   not even a commit to carry a `Session-Id` trailer. So the reader has a red build and no
   channel that names its origin.

**Measured 2026-09-01.** Diagnosing "not mine" was cheap and took two commands:

```
git status --short                                   # doctor.rs modified — I never opened it
git grep -c scan_unterminated_fence HEAD -- src/librarian/tools/doctor.rs
                                                     # ABSENT from HEAD -> new and uncommitted
```

Diagnosing "**whose**" was not available at all, and it produced **three** wrong answers in one
evening — from three different parties, every one of them actively reasoning about attribution
at the time:

1. I asserted the work belonged to the peer I had just been talking to (`codescout-e6`), and
   messaged them saying so. Wrong. They showed their commits touch zero `.rs` files.
2. Correcting me, that peer named a fourth session (`bcc98c22`) on the basis that it owned other
   commits under `src/librarian/tools/`. Also wrong — and, as they filed against themselves,
   *"elimination by directory adjacency, structurally the same error I had just corrected you
   for, committed inside the message doing the correcting, while citing F-80 as the reason not
   to."*
3. The actual owner was **`codescout-68` (`c2a08c22`)**, settled by the `Session-Id` trailer on
   commit `800f1dec` (*"fix(doctor): structured_fix_pointers used a hand-rolled fence toggle;
   add unterminated_fence"*) once the work was committed, and confirmed by that session
   volunteering it.

**The instrument that works, and the limit that matters — the rule splits by state.** The
`Session-Id` commit trailer separates sessions cleanly and reaches sessions no socket enumerates,
including exited ones. But it exists **only on commits**:

| state | instrument |
|---|---|
| committed | `Session-Id` trailer. Positive, exact, one `git log`, reaches exited sessions. |
| **uncommitted** | **none exists.** Adjacency, directory, `ListAgents`, `git status` and dirty-file lists are all elimination in disguise. Ask the session. Until it answers, the honest claim is **"not mine"**, never "yours". |

The disputed work was uncommitted, so no trailer existed for it. Both wrong answers came from
reaching for the nearest proxy without noticing the instrument had been swapped mid-argument —
the sentence "the trailer is the positive instrument" was true, and did not apply to the object
in front of us.

**Why this is `IC-10` and not `IC-12`.** `IC-12` requires the standard diagnostic to *lie* —
report transient state as settled truth. Here clippy is telling the truth: the build is red.
What is missing is authorship, which is `IC-10`'s claim exactly. And it passes `IC-10`'s
admission test where a more careful reader would not have helped: for uncommitted state the
information does not exist. Three parties, three inferences, zero available channel.

## Evidence

### The population gap is real, and was load-bearing in NEITHER misattribution

```
ListAgents        -> 4 peers (claude-plugins-ed, system-9c, codescout-e6, codescout-68)
peer-sessions.sh  -> 5 sessions in this checkout
```

That `ListAgents` under-reports sessions in a checkout is true and documented (`BL-58`). It is
also **irrelevant to what actually happened**, and both parties reached for it anyway:

- The real owner, **`codescout-68`, was in the visible four** — in *both* sessions' listings, at
  the time each was reasoning. The gap did not contain the answer; it contained nothing either
  argument needed.
- I blamed the gap for my error. I had not reasoned from an enumeration at all — I reasoned from
  conversational salience, from who I had most recently been talking to.
- The correcting peer named `bcc98c22`, a session they could **not** see, and wrote *"it does not
  appear in my ListAgents"* as though that **corroborated** the identification — using an
  inability to see a candidate as evidence for it. Their own re-diagnosis: `3aca8639`.

**This is the durable kind of wrong explanation, and it is worth more than the original bug.**
Both stories were assembled entirely from **true parts** — `ListAgents` does under-report,
`BL-58` is real, directory adjacency is a genuine signal — so no individual claim in either ever
reads as false, and both survive exactly the review that catches a false one. A correctly
recalled limitation stood in for a diagnosis nobody performed. The tell is not falsity; it is
that **no step of either explanation was checked against the object in front of us**, and
checking took one `git log`.

### Knowing the class prevented none of the three

All three misattributions were made by parties who had just read `IC-10`, in messages *about*
attribution failure. The second was made **inside the message correcting the first**, citing
F-80 by name as the reason not to do it. That is `OB-1`'s signature — *the author,
specifically* — and it is the standing argument for answering this class with a mechanism
rather than with care.


### Fourth instance, 2026-09-01 — and it is a DIFFERENT sub-shape: the object was checked, and could not answer

Added by `codescout-17` at `codescout-68`'s request. It matters because the three above share a
property this one does not, and the section's own conclusion turns on it.

**What happened.** Running the gate in the main checkout, `cargo clippy --workspace
--all-targets` reddened on two `doc_lazy_continuation` errors in
`src/librarian/tools/doctor.rs:7811-7812`. I established the finding correctly and then
misrouted it to `codescout-09`, who did not own it. `codescout-68` did.

**The distinguishing detail — the checks the three above skipped, I ran.** The section above
concludes *"no step of either explanation was checked against the object in front of us, and
checking took one `git log`."* That remedy does not reach this instance:

```
git show HEAD:src/librarian/tools/doctor.rs > /tmp/head.rs
grep -c 'Together they admit only' /tmp/head.rs   -> 0     # absent from HEAD
git diff --stat src/librarian/tools/doctor.rs     -> 51 insertions(+)   # working tree only
```

Both checks were run, both were **correct**, and the correcting peer confirmed *"your read of
the working tree was right in every particular; only the owner was wrong."* They then ran the
two checks that actually discriminate — `git diff` vs `git diff --cached` — and located the
string in the unstaged half. So the object was interrogated and answered every question **except
the one that was asked**: `git diff --stat` names insertions and **names no author**.

**Adjacency was not weak evidence here; it was anti-evidence.** The file had **three** sessions
touching it inside one hour. A signal that points at three parties at once cannot support one
conclusion, and it still read as a lead — which is the part worth recording. The three instances
above reason from *salience* and from a *correctly-recalled limitation*; this one reasons from a
**correctly-measured diff that structurally omits the field**. Same class, and the remedy above
is a no-op against it.

**Consequence for the section's argument, sharpened rather than repeated.** *"Knowing the class
prevented none of the three"* holds for a fourth — I had read `IC-10` earlier in the same
session and cited it while making the error. But this instance adds the stronger form: **checking
the object did not prevent it either.** For the three above, "check before asserting" was an
available remedy that went unused; for this one it was used and was insufficient. That is the
difference between a discipline gap and an information gap, and only the second requires a
provenance channel to exist.

**Datapoint on the mechanism — corrected 2026-09-01, hours after it was written, by its own
source.** `scripts/file-provenance.py` (shipped `0e657dc5`, same evening) is the channel this
instance needed, and it existed — unknown to me — while I was inferring.

This paragraph first recorded *"two correct positives within the hour"* as `codescout-68`'s
measurement. They withdrew that framing themselves: at claim time **one of the two was
confirmed** (`doctor.rs` → `SHARED`, corroborated when `codescout-09` independently owned the
mutation run) and the second — `src/util/librarian_guard.rs` → `PEER` — was asserted as
*correct* on the tool's answer alone, with nothing else behind it. Reported as two-of-two.

The second has **since** been confirmed, and by a different instrument: that refactor landed as
`c26943b5`, carrying `Session-Id: 3e275c54-d006-4cdc-a02f-c77837c1f1a0` — the session
`file-provenance.py` had named. So the trailer corroborates what the tool said about
*uncommitted* state, which is stronger support than the original claim offered, because the
trailer is not the tool under test; it is the instrument the tool exists to substitute for when
no commit exists yet.

**Carry the sequence, not just the result.** Two-of-two is where this landed, but at claim time it
was one-of-two-plus-an-assertion, and the missing confirmation arrived later from an instrument
that did not yet exist for that file. Recording only the endpoint would make the tool's evidence
base look stronger, and faster, than it was. Their own read: this is the `F-92` shape — right
verdict, absent support — committed while writing to the party who filed `F-92`, which is another
datapoint for the section heading two above rather than an aside.

**Not verified by me either way**: I did not run `file-provenance.py` against `doctor.rs` after
the fact, so none of these positives is my own measurement. Two stated limits worth carrying: the
default window is *since this path was last committed*, and `UNKNOWN` means no record matched —
never "not yours". That second one is the shape that produces instance five.

**Cost:** one misrouted message, one correcting message, one re-routed message. The lint was
already fixed by its owner before the reroute arrived (`d44a4409`), so the routing error cost
more than the routing did.

### Fifth instance, 2026-09-01 — the responder DECLINED to guess, and it still did not close

The sub-shape instance four named, taken one step further. Instance four was *checked and still
wrong*. This one is **correctly refused to guess, and still no closure** — which is the datapoint
that separates a routing remedy from an identifying one.

**What happened.** `tests/issue_clusters.rs::every_declared_class_has_an_index_row` reddened on
three classes declaring a `**Slug:**` with no Index row, in uncommitted working-tree content on
`docs/trackers/issue-clusters.md`. Two sessions were plausibly responsible and `git diff --stat`
names no author. Having filed instance four an hour earlier, I **deliberately did not pick** — I
broadcast the identical message to both live peers, saying so and why.

**Both were innocent, and the owner was outside the broadcast set entirely.** `file-provenance.py`
on that path, run independently by two peers with the same result:

```
window: writes at or after 2026-09-01T12:04:55+00:00   (== 4e1675dc)
written by b2a50de8-a666-4933-b030-f7bf8e18fd6a
written by bcc98c22-28c5-43dd-acee-4acbe92ca1cf
```

Neither is `3e275c54` (me), `8332e3bc` (`codescout-09`), or `c2a08c22` (`codescout-68`). Both are
also absent from `ListAgents`, which listed exactly three live rows — so the true owners were
invisible to the enumeration *and* to the candidate set it produced.

**The finding: broadcasting widens the guess, it does not close it.** Refusing to choose between
two candidates was the right call and remains the cheaper error — but its correctness is about
*not asserting a falsehood*, not about reaching the answer. Here the two-candidate set was
itself a partial population, so **every** available routing decision was wrong: picking either
peer would have been a misattribution, and broadcasting to both was also wrong, merely
harmlessly so. Two sessions each spent a turn establishing a negative about work they had not
done.

That is worth separating from instances 1–3 explicitly. Their remedy was *check before
asserting*; instance four's was *a provenance channel*; **this one's is neither** — no amount of
care or breadth on the responder's side constructs a candidate set that contains a party no
instrument on the machine reports. The closing move is the scratchpad-path procedure now in
`CLAUDE.md` § *Observer Blindness*, and it has a precondition this instance violates: **asking
the session only works once the candidate set contains the owner.** Enumeration completeness is
upstream of positive identification, not an alternative to it — which is the same conclusion
`bug-fix-session-log:F-80` reaches from the elimination side, arrived at here from the routing
side.

**Two corrections I owed on my own broadcast, recorded because both were published.**

1. I wrote that the failure *"puts a red `experiments` in front of every session in the tree"*.
   Wrong, and caught by `codescout-68`. Verified at the source rather than accepted:
   `tests/issue_clusters.rs::valid_slugs()` reads the ledger via `std::fs::read_to_string`, so
   the gate reads the **working tree**; the three slugs were absent from `HEAD`. Committing other
   files by pathspec left `experiments` green throughout, and CI — which checks out a commit —
   would have read clean. The honest headline was *"the working tree reds the gate locally for
   everyone until those rows land."* **Overstating a shared cost is its own defect**: it moves
   people to act urgently on the wrong object.
2. The real hazard was the **read** side, which is this file's own subject: a session running the
   gate as its last step sees red and reads it as theirs. I reproduced that framing error inside
   a broadcast *about* this class — a fourth author-writes-about-the-class-and-commits-it
   datapoint for the section above.

**Resolution needed nobody present.** The owner wrote the three Index rows between my run and the
next, unprompted and without seeing any broadcast. Verified independently here:
`cargo test --test issue_clusters` → **10 passed / 0 failed**, `every_declared_class_has_an_index_row`
among them. So the incident's actual cost was **entirely** in the misrouting, not in the defect —
two peer turns and three of my messages against a red that its author closed on their own
schedule.

**Caveat on the provenance figures, carried from `U-51`.** That window's floor is the file's last
commit and moves whenever it is committed, so a later re-run answers a different question and can
return `UNKNOWN` for a claim that was correct. And the two names are positive evidence that those
sessions wrote — **not** proof no third did by a route the tool's heuristics miss. Neither figure
above is my own measurement: both came from peers, run independently, agreeing.
### The cost, itemised

- ~10 minutes establishing the failures were not mine, mid-task, with my own gate result unknown
  for the duration.
- Three wrong assertions between two sessions, each costing the recipient a turn to establish a
  negative about their own work. A misattribution is not free because the ask is "nothing needed
  from you".
- Both wrong claims would have entered bug files uncorrected. One nearly did.

## Calibration — measured 2026-09-01 before building anything

Run as the first step of implementation, on the principle that a mechanism naming a population
asserts that population is reachable. Two questions: **would the channel have answered the
incident that motivated it**, and **what does it not see**.

### 1. The motivating incident: the channel WOULD have answered it

The disputed file was `src/librarian/tools/doctor.rs`, written by session `c2a08c22`
(codescout-68). Its `.buddy/c2a08c22-…/cs_tool_log.jsonl` holds 50 records, and the writes are
all codescout tools, each carrying the path:

```
mcp__codescout__edit_code   action=insert   path=src/librarian/tools/doctor.rs   x5
mcp__codescout__edit_file                   path=src/librarian/tools/doctor.rs   x7
```

**Checked the other direction too, because a positive alone would not settle it.** That session's
CC transcript records `"name":"Edit"` **0** times and `"name":"Write"` **9** times — and all nine
Writes target `/tmp/…/scratchpad/` (commit messages, two probe scripts). Not one native write
touched a repo file. So `doctor.rs` was authored **exclusively** through logged tools, and the
channel would have named its owner correctly.

Positive control on the search itself: the same transcript mentions `doctor.rs` **187** times, so
the pattern reaches this file — the zero for native repo writes is a real absence, not a broken
query.

> **Both counts are as of 2026-09-01 05:20, and the second has already moved once.** This section
> first published **8** Writes and **80** mentions; both were correct when taken and false within
> the hour, because the substrate is that session's **live, append-only transcript** and it kept
> writing — including its replies to this one, which quote `doctor.rs`. Retention-swept corpora
> make every count a *floor*; an append-only corpus makes every count a floor that **rises**, so a
> bare number decays in the direction of looking like an undercount rather than an error. The
> conclusion is unaffected and strengthened (`Edit` still 0, now 9 of 9 native writes to `/tmp`) —
> which is exactly why nothing would have prompted a re-check. Stamp counts taken over a live file,
> or state the property (`all native writes are scratch`) rather than its cardinality. Found by
> auditing this session's other published corroborations after F-92; see F-93.

### 2. What it does not see — and it is bigger than "native Edit"

Across 573 transcripts / 108,635 captured Bash commands in this project, writes that **no
substrate records as writes**:

| write route | repo paths | `/tmp` only |
|---|---:|---:|
| `sed -i` | **27** | 26 |
| `cat > …` | **107** | 446 |

Against 1,808 logged codescout write-tool calls and 318 native `Edit`/`Write`/`MultiEdit` uses
(of which the sampled session's were 8-for-8 scratch). So the channel covers roughly **80–90% of
repo writes** — the range, not a point, because the native-vs-scratch split is sampled rather
than fully classified, and `> redirect` (2,201 occurrences) is dominated by `/dev/null` and
scratch and was not separated.

**The blind spot is not random, which is this bug's own asymmetry point confirmed.** It tracks
*working style*: a session doing symbol-level edits via `edit_code` is fully visible; a session
doing bulk mechanical edits via `sed -i` is invisible for exactly those edits. The clearest
datapoint is first-person — the 46-file citation re-point in `9188d000` was a single `sed -i`
loop, so the channel would have shown **nothing** for the largest write action of that session,
while showing every one of its small `edit_file` calls.

### 3. What this settles for the design

Build it — the calibration is positive and coverage is high. But the **absent case is the
load-bearing half**, and it must distinguish two states that a naive implementation collapses:

- **no record names this path** — could be a Bash write by anyone; says nothing about ownership;
- **records exist for this path and name a different session** — positive evidence it is not yours.

Rendering the first as "not mine" would be the exact failure this bug documents: a true
limitation quietly substituting for an answer, which is the shape all three of the evening's
misattributions had. `cs_tool_log.jsonl` supports the distinction — it is a per-session file, so
"no session's log names this path" is directly checkable rather than inferred.
## Implementation — built 2026-09-01, and the calibration's substrate was rejected

`scripts/file-provenance.py`, 43 cases in `tests/file-provenance.sh`.

### The calibration above reached the right verdict on the wrong substrate

The *Calibration* section concluded "build it" from a positive control on
`.buddy/<sid>/cs_tool_log.jsonl`, and that control is sound: `c2a08c22`'s log really does
hold the twelve `doctor.rs` writes. What it never asked is whether that log **retains
records in general**. It does not, on three independent counts, each of which fails silently:

| loss | measured 2026-09-01 |
|---|---|
| rolling entry cap (`MAX_ENTRIES = 50`) | **196 of 297** logs sit exactly at it |
| deleted on compact / resume / clear | `hook_helpers.py:244,420`. One session here made **370** codescout calls; its log retained **1** |
| `args` cut to 200 chars over unordered keys | **11.5%** of 1,920 write records carry no `path=`; 147 more carry a truncated one — `src/serve`, `src/lsp/m`, `docs/issues/2026-05-18-il3-pipe-violation-subagent` |

The sampled session had compacted zero times and its writes fell inside the surviving
window, so it was the one session in the good state on all three axes. **The calibration
was a positive control that certified only the path that executed** — the general question
was never posed, and one `wc -l` over the corpus answered it.

The third loss is a defect on its own terms and is filed where the code lives:
`claude-plugins:docs/issues/2026-09-01-summarize-args-destroys-the-path-it-documents-preserving.md`.
Its docstring promises *"file paths preserved"*; a blind `[:200]` tail cut over
`tool_input.items()` means survival is decided by where the caller put the key. The
existing test cannot catch it twice over — `assert len(result) <= 200` is monotone under
further truncation, and the fixture writes `path` **first**, the one ordering under which
the bug does not fire.

### Substrate actually used: Claude Code's own transcripts

One file per session, append-only, full untruncated tool inputs, read across **all three**
profiles (`~/.claude`, `~/.claude-sdd`, `~/.claude-kat` — one alone is a subset that reports
as complete, BL-58 one layer down).

This also **closes most of the blind spot the calibration measured**. Those 27 `sed -i` and
107 `cat >` repo writes were counted *from the transcripts*, so they were never invisible —
they were invisible to `cs_tool_log`. The channel now reads them directly. Residual, measured
over this project's 1,903 Bash calls: **2.8%** carry a mutating verb the heuristic does not
match (`patch`, `perl -i`, `python -c`, `git apply`) — and a miss degrades to `UNKNOWN`,
which is the safe direction by construction.

### The correction real data forced: authorship needs a WINDOW

The first working version answered `src/librarian/tools/doctor.rs` with **fifteen sessions**,
every one of which had legitimately written it at some point in its months-long life. Nothing
was wrong with the query — the *question* was wrong. "Who has ever touched this?" is not the
question anyone asks at a red build; "this file is dirty **now** and reds my build, is that
mine?" is, and writes older than the path's last commit are baked into HEAD.

So the default window is **since the path was last committed**, per path, with `--since` and
`--all` overrides. No fixture could have surfaced this: every fixture had one write in an
empty timeline, and the suite was green throughout.

### Verification

Replaying the incident's own window — floor at the previous `doctor.rs` commit, `800f1dec~1`
— returns exactly one session:

```
$ ./scripts/file-provenance.py --since 2026-08-31T22:27:01+03:00 src/librarian/tools/doctor.rs
PEER      src/librarian/tools/doctor.rs
          window: writes at or after 2026-08-31T19:27:01+00:00
          written by c2a08c22-21ba-4ee3-b0fe-f1cadef1632a
```

Identical to the `Session-Id` trailer on `800f1dec` — which during the incident did not yet
exist, the work being uncommitted. **This is the answer the *Root cause* table records as
unavailable**, and the row can now be filled.

### What it still refuses to do

- **Mentions are not authorship**, and the naive form is *wrong*, not merely vague: ranking
  this project's transcripts by raw path-mention count puts `d4bd6ec9` (149) and `e3a0b567`
  (46) **above** the known author `c2a08c22` (62). Only a tool call whose input names the
  path as a write **target** counts.
- **`UNKNOWN` is never rendered as "not mine"** — it is the load-bearing verdict, it prints
  its own coverage caveat, and `tests/file-provenance.sh` asserts on the verdict *line* so
  the caveat's own wording cannot satisfy the check.
- It sees only what Claude did. A human editor, or any non-Claude process, is invisible by
  construction.

## Hypotheses tried

1. **Hypothesis:** the failures were caused by my own edits.
   **Test:** the two failing tests use a tempdir fixture and an in-memory catalog with literal
   seeded bodies (`doctor.rs`), reachable by none of my five changed files.
   **Verdict:** rejected.

2. **Hypothesis:** the modification belongs to the peer active in this checkout (`codescout-e6`).
   **Test:** asserted it to them; they checked their four commits with
   `git show --stat | grep -c '\.rs'` → 0 for every one.
   **Verdict:** **rejected — my error.** Proximity is not evidence.

3. **Hypothesis:** it belongs to `bcc98c22`, which owns other commits under `src/librarian/tools/`.
   **Test:** the `Session-Id` trailer on `800f1dec`, once the work was committed.
   **Verdict:** **rejected — the correcting party's error**, filed by them against themselves.
   Directory adjacency is elimination wearing a positive ID's clothes.

4. **Hypothesis:** it belongs to `codescout-68` (`c2a08c22`).
   **Test:** `Session-Id` trailer on `800f1dec`, plus the session volunteering it unprompted.
   **Verdict:** **confirmed** — and note it was only confirmable *after* the work was committed.
   During the window that caused the cost, no test existed that could have returned this answer.

## Fix

No fix proposed for `-D dead-code`; it is correct. The gap is the missing channel, and the
candidate remedies are `H`-shaped (a mechanism, not a discipline) exactly as `IC-10` predicts:

- **A provenance channel for working-tree state.** The question to answer is **"is this mine?"**,
  not "whose is this?" — the first has a reader with a decision to make, the second has no
  channel and, on this evening's evidence, no reliable answer either.

  **Substrate, corrected 2026-09-01 by `codescout-e6` before anyone built on it.** This section
  first named `.buddy/by-ppid/<pid>/session_id`. That file maps **pid → session**, not
  **path → session** — it answers *which session is this process*, so it cannot on its own
  answer *who wrote this file*. The path-bearing half is `.buddy/<sid>/cs_tool_log.jsonl`, which
  records codescout write-tool calls (`edit_file` / `edit_code` / `create_file`) with their
  paths; joining the two yields a real path → session map.

  **State the coverage limit up front, because it is large here and it is the failure mode this
  whole bug is about.** That map sees codescout tool writes and **not** native `Bash`/`Edit`
  writes. On this repo the gap is not marginal: `CLAUDE.md` documents both shells as
  deliberately permitted pending an eval, and `usage.db` records MCP calls only, so a
  Bash-written file is invisible to the channel by construction.

  This does not sink it. A channel answering **"yes, mine" / "can't tell"** is strictly better
  than the nothing we have — *provided `can't tell` is never rendered as `not mine`*. That
  rendering would be the identical failure to the `ListAgents` gap standing in for a diagnosis
  (see Evidence): a true limitation quietly substituting for an answer. **Design the absent case
  first** — it is the one that will fire most.

- **Cheaper and available today:** the two-command diagnostic above, promoted somewhere a
  session hits it *before* losing the ten minutes. It only occurs to you after.

Deliberately **not** proposed: relaxing `-D dead-code`, or asking authors to wire before writing.
The first removes a good gate to paper over a missing channel; the second inverts the tests-first
ordering this repo asks for.

## Tests added

None — this is an interaction between independent sessions on one filesystem, not a code path.
A regression test would have to spawn two sessions and race them; the honest guard is the
provenance channel above, and until that exists this record is the artifact.

## Workarounds

`git status --short` and `git grep -c <symbol> HEAD -- <path>` settle **"not mine"** in seconds.
**Stop there.** For uncommitted work that is the correct terminal state, not a step short of one
— three parties tried to go further and three were wrong. Do not proceed to "whose" from
`ListAgents`, directory adjacency, or who you were last talking to. If you need the owner, ask
the sessions; until one answers, the supportable claim is "not mine".

## Resume

Decide whether the working-tree provenance channel is worth an `H` entry in
`docs/trackers/observer-blindness.md`, or whether `IC-10` reaching n=2 should drive it. Cross-check
against `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`, which holds
the write-side twin.

## References

- `docs/trackers/issue-clusters.md` — `IC-10` (this class), `IC-12` (the read-side class this is
  *not*, and why).
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — the write-side twin.
- `reconnaissance-patterns` F-80 — elimination over an incompletely-reported population, sent as a
  positive ID. This bug is another instance, committed while reading about the class.
