---
kind: bug
status: mitigated
title: The author of a tree-reddening write is the one party never told
tags:
- cluster/gate-keyed-on-unobservable-event
topic: shared-checkout authorship
---

# BUG: the author of a tree-reddening write is the one party never told

## Summary

On a shared checkout, uncommitted work that does not compile reds every other session's
gate. `de546287` built the **reader-side** half of the answer: a red now names who holds
the dirty files it points at.

**The author is still told nothing.** The reader can route around the lock; only the
author can end it. And the author's own build is fine by construction — theirs is the tree
that compiles once they finish the edit — so nothing in their loop ever mentions that N
other sessions are reading their failure.

## Symptom (Effect)

2026-09-08, ~10:50–10:57. `5399543d`'s uncommitted `src/agent/write_guard.rs` did not
compile. Two sessions' gates went red; a third nearly shipped with an `unverified:` caveat
about a failure that was not theirs and had already cleared.

`5399543d`, in their own words this evening: *"I did not know I had reddened anyone this
morning. I found out from a peer message, minutes later, after two sessions had already
paid."*

They fixed the error because their own test would not build — not because they knew it was
load-bearing for anyone.

## Root cause

Every existing mechanism fires for an observer who is **already worried**:

| mechanism | fires for | author's state |
|---|---|---|
| the gate red itself | the reader | never runs it — their tree is mid-edit |
| `wip_authors` (`de546287`) | the reader | printed on someone else's terminal |
| a peer message | the reader, manually | requires a peer to notice and choose to send |

This is `CLAUDE.md` § *Observer Blindness* position 3 — *the check that runs when nobody
is worried*. At the moment it mattered the author was not worried at all, so nothing
conditioned on concern can reach them.

**It is also why the class's instance count is a lower bound by construction rather than
by sampling.** The system self-heals: the author repairs the file for their own reasons,
and the ordinary outcome of an instance is that it leaves no artifact in any ledger. Only
the instances a peer chose to report exist at all. That is the recording-filter law, where
widening the sample changes nothing.


### 2026-09-12 — the warner produced one himself, and the report named the wrong author

One morning, one shared checkout, reported here only because three parties each chose to
say something. Times are mine where I measured them and theirs where I did not.

- Earlier that morning, `b0b9bc40` flagged a mid-edit window in `src/lsp/client.rs` to
  `f3c594ce` — their report, not my observation.
- At `09:45` I read a tree where `group_by_file_ranked` was defined in
  `src/tools/file_group.rs` and the import in `src/tools/symbol/display.rs` was not yet
  switched, so `display.rs:158` did not build. Two other sessions were reading that
  directory at the time. `b0b9bc40` resolved it at `09:54` in `48d39f53`. By their own
  account that is roughly four hours after they flagged the same shape to someone else.
- `f3c594ce` reported the red — **to the wrong author.** I was editing `symbols.rs` in
  the same directory in the same minute, so adjacency named me.
  `scripts/file-provenance.py` named `b0b9bc40`, corroborated independently when
  `fmt-mine.sh` refused `f3c594ce` that same file and named the same session.

**Their statement of the asymmetry is sharper than this file's** and is worth keeping in
their words: *"a mid-edit window is invisible from inside it — the writer sees a sequence
of intentional steps, everyone else sees a tree that does not build."*

Two things this adds to § Root cause above.

**It is the § *Observer Blindness* signature stated plainly: knowing the class prevented
nothing.** The author had spent that morning telling other sessions about this exact
failure mode, and produced one anyway, in a file two sessions were reading. That is the
same shape CLAUDE.md records for its four-in-one-evening measurement, and it is the
argument against any remedy shaped like *be more careful before you step away*.

**A red can have more than one author, and "the author" singular is the wrong noun.**
That same build carried an unwired `fallback_gap` from `b80a27d4` — a dead-code warning,
not the `E0425`, but enough to appear in the same output. A report naming one author is
half right in exactly the way that stalls the repair: the party told may fix their half,
see the tree still red, and reasonably conclude the diagnosis was wrong. Splitting the
red by author before reporting is what made this one resolve in nine minutes.

**Corrected 2026-09-12, the same day, by measurement.** This paragraph first claimed *"no
mechanism does that today — `wip_authors` lists every uncommitted file the failure names
without saying which line belongs to which session."* **That is false, and it was written
from reading rather than running.** `b0b9bc40` read
`src/tools/run_command/attribution.rs:193-232` and refused it; the discriminating run
settles it. Fed a synthetic red naming two uncommitted files with two different authors,
`scripts/attribute-red.py` returns both, each with its own author, liveness and socket:

```
  tests/mcp_smoke_scripts_reference_real_tools.rs
      written by f3c594ce-…  [LIVE]  attach-alias-advisory-anyhow — pid 703051
  docs/superpowers/plans/2026-09-10-parameter-alias-collapse.md
      written by b0b9bc40-…  [LIVE]  codescout-75 — pid 1456596
```

One entry per file, per-file authorship, already correct. **So the defect is not in the
diagnostic — it is that the diagnostic's output does not survive the hop to the party who
needs it**, which has a different remedy and is therefore its own file rather than a § Fix
constraint here. Two candidate causes, not yet separated:

- **A relayed red loses the attribution.** `wip_authors` is computed over the text
  `run_command` saw, never over what a session then quotes at a peer. The reporting
  session did see all three error lines spanning both files, and still addressed one
  owner.
- **Native `Bash` bypasses `run_command` entirely**, so the diagnostic never runs at all.
  CLAUDE.md already states this — *"on a `Bash` gate the silence means nothing"* — and
  sessions here have been running gates through `Bash` all day.

The instance above cannot separate them without knowing which shell the reporting session
used, and this file does not guess.

*Mechanism read by `b0b9bc40`, who refused the unmeasured claim rather than inheriting it;
the two-author run by `b80a27d4`, who had made it.*
## Reproduction

Two sessions, one checkout, shared `target/`. A saves uncommitted Rust that does not
compile and keeps working. Nothing in A's session mentions this. B's `cargo test` reds.

## Fix
**MITIGATED — NOT FIXED — in `5072c097`, patch-id `18a454244c0b063abb2933230e75fcb85301b495`.**
This file's own stated first debt is paid and the underlying gap is not, and the distinction
is load-bearing rather than cautious: **marking this `fixed` would be the same
false-coverage move the bug is about.** An author who writes a compiling edit that reds a
test still receives nothing. What changed is that the mechanism no longer implies it covered
that case.

**What shipped, at both read surfaces.** The notice now carries its own scope —

> `SCOPE: this compiles your tree and never runs its tests, so silence from this check is
> not a green gate — an edit that compiles and reds a test produces nothing here.`

— which is this section's *"the notice's own text should say it covers compilation and not
test outcomes"* verbatim. And `build_check.rs`'s header gained a **fifth CEILINGS bullet**,
so check-versus-test moves out of the COST section — where it sat as an invariant the module
asserted it MET (*"the check has to match the gate's blast radius"*) — and into the list of
gaps admitted *"because silence looks identical to health"*. The COST sentence now states
what it can and cannot match. Two surfaces deliberately: the header for a reader who opens
the file, the notice for the many who never will.

**Why published rather than closed, stated in the code.** Running the gate's
`cargo test --workspace` on every source write costs minutes and holds the shared build
lock — which is the blocking this module exists to avoid. So the bound is disclosed, not
removed.

**Tests + mutation.** `the_notice_names_its_scope_so_silence_is_not_an_all_clear`, two
assertions on the two halves; 34 green in `agent::build_check`; gate
`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`. Both halves mutated in an isolated worktree, **one kill
each** — which is what establishes they are independently guarded rather than one claim
asserted twice.

**The red was observed by MUTATION, not by a pre-fix run, and that is a departure worth
naming.** A reproduction-first red in this checkout is byte-identical to a real regression
for every peer compiling against it — `2026-09-14-a-reproduction-first-red-is-a-true-red-no-observer-can-attribute.md`,
still open, with eight live sessions today. TDD's *"watch it fail"* was satisfied in
isolation instead, at a zero-length shared-red window.

**And the mutations falsified the test's own annotation**, corrected in the same commit: the
doc comment claimed a meaning-preserving rewrite would keep it green, and both
meaning-preserving mutations (`never runs its tests` → `never runs them`; `so silence from
this check` → `so a clean result here`) red it. The comment now says so and accepts the false
positive, the alternative being a sentence pin that reds on every rewording.

**Left open deliberately.** The residual is the original complaint, narrowed: the author of a
compiling-but-test-reddening write is still the one party never told. Closing this file would
remove it from the open-bug query and stop the next person looking — which is precisely the
mechanism recorded below.

**PARTLY SHIPPED, and this section's "Not implemented" is stale — corrected 2026-09-16.**
`src/agent/build_check.rs` is the author-side half this file asks for: *"tell the author their
own uncommitted write broke the shared tree."* It is wired and REACHED, not merely present —
`on_source_write` is called on every source write (`src/agent/mod.rs:914`), delivered through
`take_build_notice` / `pending_notice` (`:920-921`), consumed in `server.rs`, and its
reachability is pinned by `marking_a_rust_file_dirty_reaches_the_build_check`, whose own
comment notes that a skip-guard would be monotone under the deletion it exists to catch.

**THE RESIDUAL IS A SCOPE GAP, AND THE MODULE STATES THE INVARIANT IT CANNOT MEET.** The check
runs `cargo check --workspace --all-targets` (`build_check.rs:439-442`). The gate runs
`cargo test`. `--all-targets` makes it COMPILE test targets — load-bearing, and the module says
so — but `check` never RUNS them. Its own header reads *"The gate runs tests, so a broken test
module reds peers the same as a broken lib does; the check has to match the gate's blast
radius"*; it matches on compilation and cannot match on execution. So **an uncommitted edit that
COMPILES and reds a TEST still tells its author nothing**, which is the same distinction
CLAUDE.md § Development Commands already draws for the gate itself (*"it is `test`, not `check`:
a `check` compiles the lean test targets and never runs them, so a lean-only runtime failure is
invisible to it"*).

**Measured 2026-09-16, and every red that day was in the uncovered half.** Three failures in one
gate run, all on code that compiled clean:
`tools::read_file::tests::read_file_buffer_single_oversized_line_still_fits_the_threshold`
(`read_file.rs:2277`), `librarian::tools::doctor::tests::admits_relevance_exemption_allow_list_stays_exhaustive_over_check_all`
(`doctor.rs:13572`), and
`librarian::tools::update::tests::doctor_does_not_observe_a_catalog_row_that_has_fallen_behind_its_file`
(`update.rs:1182`). The **reader** side worked exactly as designed — `run_command`'s attribution
named both holders, `29420e72` and `9e022ef0`, and routing by it reached both in one message
each. The **author** side could not fire by construction. Both holders confirmed the reds were
deliberate or in-flight rather than regressions, which no instrument in the tree could have
established.

**AND THE CONCLUSION TWO SESSIONS REACHED OUT LOUD THAT DAY WAS WRONG, which is why this
correction is worth more than the datapoint.** `e5691fad` and `9e022ef0` agreed in writing that
*no mechanism exists for this and the nearest one backfires* — reasoned from `gate.sh` isolating
build artifacts but not source, and from `OB-23`'s stand-down shape. Neither of us opened
`build_check.rs`. A mechanism exists, ships, is reached, and has a **scope gap**; "absent" and
"narrower than its name" prescribe opposite work, and we were one file-read from the second.
That is `81d2cdcbbdfc03a6`'s class arriving here too — a shipped remedy implying a scope it does
not have, where the false coverage is the half that stops the next person looking.

**THE MODULE ADMITS FOUR CEILINGS AND PRESENTS CHECK-VERSUS-TEST AS MET — which is the whole
asymmetry.** `build_check.rs:54-62`, under a heading that says why it exists (*"named here
because silence looks identical to health"*): a write that does not go through codescout
produces **no trigger** (native `Edit`/`Write`/`Bash` reach no tool — measured, **8 of 72
sessions run cargo entirely through `Bash`**); **Rust only**; a **deliberate mutation** trips
it, with `CODESCOUT_NO_BUILD_CHECK=1` as the escape; and a **lean build** emits nothing.

**The 8-of-72 figure is ONE derivation cited in TWO places — do not read it as corroboration.**
It appears in `build_check.rs:57-58` and in `docs/PROBES.md`:190's `attribute-red.py` row, both
dated 2026-09-08 over 3 profiles and 72 sessions with any cargo activity. Same measurement,
two surfaces. § *Testing Discipline* says check independence rather than agreement, and two
citations of one number agree because they are one number. PROBES.md also carries the bound
that matters when quoting it: the figure is a **session count**, and the call ratio (1058 of
5256, 20.1%) is a proxy only — quoting the ratio invites *"~80% covered"*, a per-population
claim standing in for a per-member one, when such a session gets **nothing** rather than being
partly degraded.

Check-versus-test appears nowhere in that list. It sits in the COST section as an invariant the
module asserts it **meets** — *"the check has to match the gate's blast radius"*. So of five
gaps, four are admitted and one is presented as satisfied, and it is the one that swallowed all
three of 2026-09-16's reds. A reader auditing this mechanism's limits finds an honest CEILINGS
list and is told the remaining edge is covered.

**And an intent channel DOES exist — author-private, which sharpens the deliberate-red claim
rather than refuting it.** `CODESCOUT_NO_BUILD_CHECK=1` suppresses the AUTHOR's own notice and
tells no peer anything; `DISABLE_ENV` explicitly mirrors `CODESCOUT_NO_WIP_ATTRIBUTION` on the
reader side (`:68-70`), so both halves can be silenced independently and neither silencing is
visible to the other party. *"A shared checkout has no channel for this red is deliberate"* is
therefore not quite right: the channel is local to the author by construction, which is worse
than absent, because a suppressing author may believe they have declared something.

*Ceilings re-read and wiring re-verified by `9e022ef0-eb76-49f0-b175-4d68979290cf`, who had
asserted the mechanism's absence an hour earlier and checked before accepting the correction;
the only-gap-presented-as-satisfied framing is theirs. Their count of "three of four" is one
short — the section names four and the fifth is the one presented as met — which strengthens it.*

**Not implemented for the test-failure half, and deliberately not designed here.**

**THE READER-SIDE TWIN IS ALREADY CLASSED, and the citation is re-runnable past the display
cap.** `docs/PROBES.md`:190 — the `attribute-red.py` row — reads: *"This makes the tool an
instance of `IC-14` (guard narrower than its name): it attributes a red that came through
`run_command`, and the remainder is protected by nothing."* That is this residual with the
nouns changed. So the **reader** half of this mechanism already sits in `IC-14` with a measured
ceiling analysis, while this file is tagged `cluster/gate-keyed-on-unobservable-event` —
correct for its title and not for the residual. **A second tag is not the route:** CLAUDE.md
requires exactly one `cluster/` per bug file, so whoever takes the repair inherits a
classification question rather than a free retag. `attribute-red.py` also appears in `IC-18`'s
Members line; both candidates stand, since one row can argue name-over-promises-coverage and
selector-narrower-than-population in different sentences. Left unadjudicated.

**`IC-14` sits at byte 2569 of 4896 on that line and the display cap is 2000 — 569 bytes past
where two sessions' readings stopped.** Read it without the cap:

```sh
awk 'NR==190 {n=index($0,"IC-14"); print substr($0,n-90,200)}' docs/PROBES.md
```

**That one boundary produced two symptoms inside a single check, which is why it is recorded
here rather than as a note.** `9e022ef0` cited the line. `e5691fad` could not verify it (the
row truncates at 2000 of 4896 in every tool reading) **and, separately, reported the row absent
from a `grep` whose own output said `4 matches (capped)`**. Same cap, twice, inside the act of
checking a claim about instruments that under-report their own scope — one session unable to
confirm, the other concluding absence, neither seeing the boundary they shared. `awk` over the
raw line is the escape for both, and a citation anyone can re-run is what replaces an
assertion. Running the
gate's tests on every source write is not a candidate on cost, and the cheap alternative — an
advisory that a peer *might* be mid-edit — is `OB-23`'s measured shape, where the prescribed
response is stand down and complying removes the observer whose build log resolves the anomaly.
What is owed first is the bound stated at the mechanism: the notice's own text should say it
covers compilation and not test outcomes, so its silence stops reading as an all-clear.

Not implemented. The shape has to fire **without anyone being worried**, which rules out
anything the author must remember.

**Option 1 is the one being built** — chosen by `5399543d`'s operator from four candidates,
after recon retired option 3's delivery half (see below).

1. **Author-side, on write.** After a source write through `edit_code` / `edit_file` /
   `create_file`, if the checkout is shared, compile-check in the background and surface the
   result to the **author** on their next call. The trigger happens anyway, which is what
   Observer Blindness position 3 asks for. **No transport, no disk state, no per-sid record:**
   it is entirely inside the author's own session, and `Agent` (`src/agent/mod.rs:53`) is
   per-server and therefore per-session, already carrying
   `indexing: Arc<Mutex<IndexingState>>` with `Idle | Running | Done | Failed` to model it on.
2. **Author-side, on idle.** The same check when a session goes idle holding dirty files that
   do not compile — cheaper, and idle is when the author can act.
3. **Reader-side broadcast.** The session that gets the red messages the resolved author.
   Rejected as primary on its **trigger**: it makes the author's notification depend on a peer
   running a gate, which is the same conditional-on-someone-else's-activity shape. Its
   *delivery* is separately impossible — see below.

**Do not build it as advice.** *"Announce before you leave the tree dirty"* is a policy the
author must remember at the one moment they have no reason to — `skill-frictions:SKF-22`'s
failure mode, and the reason this file exists rather than a line in `CLAUDE.md`.

### Option 3 has no transport, and the address is not the client

Recon by `5399543d`, 2026-09-09: **`cc-socks` appears zero times in `src/**/*.rs`.** What
exists is `src/peer/` — `PeerClient`, `PeerEnvelope`, `PROTOCOL_VERSION` — which is
codescout-server-to-codescout-server delegation over `codescout-peer-*` sockets derived by
`socket_discovery::peer_socket_path_for_workspace`. Different channel, different protocol,
different endpoints. It cannot reach a Claude Code session.

The `uds:` path `scripts/attribute-red.py` prints is real and correct —
`SessionRow::messaging_socket_path` carries it. **What is missing is not the ADDRESS, it is the
CLIENT**, and that distinction matters because *"we know where they are"* reads as most of the
way there. Closing it would mean codescout speaking the harness's private, unversioned session
protocol: not ours to depend on, broken by any harness change, and reverse-engineering another
program's IPC to inject messages into someone's session is the wrong shape regardless of
whether it works. **Do not build that.**

A drop-box variant — write the finding to a per-sid record, author collects on their next call
— repairs the delivery half. It does not repair the trigger, which is the half option 3 was
already rejected on, and option 1 needs neither.

### The check cannot have its own `target/`, and that is a design input

Measured 2026-09-09 by `5399543d`, verified independently here: `target/` is **97 GB** on a
disk **91% full with 169 GB free**. A separate `CARGO_TARGET_DIR` for the background check is
therefore not affordable, so the check must use the **shared** `target/` — which means it takes
the cargo lock, and **the mechanism becomes the contention it exists to reduce.**

Answer it in the design rather than in a caveat: **skip when the lock is held, never block.**
Skipping is silence, which already matches `wip_author_diagnostic`'s contract that `None` means
both *nothing to say* and *could not find out* — so it introduces no new ambiguity, and the
existing refusal to render silence as an exoneration covers it unchanged.
### Three findings from building option 1 (2026-09-09, `5399543d`)

All three correct the sketch above rather than annotating it. Verified here where checkable.

**1. `--all-targets` is load-bearing, and this file's option 1 was wrong about the command.**
The sketch says *"compile-check just that file"*. **That is not available in Rust** — the unit
of compilation is the crate. Worse, the narrower command is blind to the motivating bug: the
incident was an `expect_err` on a `Debug`-less type **inside a `#[cfg(test)]` module**, and
`cargo check` without `--all-targets` does not build test targets at all. Measured:

```
cargo check --workspace                 exit=0    errors=0   ~3.0s   <- BLIND
cargo check --workspace --all-targets   exit=101  errors=2   ~6.7s
```

So the cheap form is **monotone under exactly the failure class the feature exists to catch**
— it returns the same clean answer whether the tree is fine or holds the original defect. The
real cost is ~7 s incremental, not the per-file check the sketch assumed. This is the same
shape as `CLAUDE.md` § *Development Commands*' rule that the long clippy form is the gate
rather than garnish.

**2. A feature-gate constraint the design did not anticipate.** `src/agent/` is ungated
(`src/lib.rs:24`); `src/librarian/` is `#[cfg(feature = "librarian")]` (`:39-40`) — verified.
So the module **cannot** reach `SessionRegistry`: doing so would delete the feature from lean
builds *and* make its tests invisible to the lean lane, which is the vacuity `CLAUDE.md`
records as making `LEAN exit=0` worthless. Resolution: the rules (`checkout_is_shared`,
`errors_naming`, `render_notice`) stay **pure and ungated** so both lanes run them, and only
the registry row loader is gated. **Consequence, named at the site rather than discovered: a
lean build emits no notice.**

**3. Filter on `is_primary` spans only — a peer's compile error can carry a SECONDARY span
pointing into your file.** Filtering on *"any span names my file"* would attribute their break
to you, at the exact moment someone is looking for a party to blame. That is the misrouting
this whole class is about, reproduced one layer in, inside the fix for it. Guarded by a test
that reds if the filter is relaxed.

Finding 3 is the one worth reading twice: the author-side mechanism, built to stop
misattribution, had a misattribution available in its own parser — found by writing it rather
than by review.

### A prevention-side candidate, on a different axis from options 1-3 (2026-09-12)

Everything above is about **telling** the author. This is about the window not existing,
and it is recorded here rather than in a tracker because a reader working this file is
the reader who needs it — and because § Root cause argues that an instance left in
message history is the one that never gets counted. The same holds for a remedy.

**The constraint that produced it.** Any remedy firing when the author *leaves* has to
fire on a state the author does not experience as leaving. `b0b9bc40` did not step away
from a broken tree; they were between two edits of one refactor, and *"between two
edits"* is most of a session. That rules out every trigger-shaped remedy.

**Their candidate, which dissolves the constraint instead of meeting it:** the
intermediate state exists because the refactor was **two tool calls**. `edit_code` adds
`group_by_file_ranked` to `file_group.rs`; a second call switches the import in
`display.rs`. Between them the tree does not build — not because anyone left, but because
no primitive applies both at once. An **atomic multi-file edit** (one call, N edits across
N files, applied together or not at all) fires on nothing and requires noticing nothing,
because the state it prevents is never reachable. Precedent is one layer down already:
`edit_file(edits=[…])` and `doc(patch={body_edits: […]})` are both atomic **within one
file**, and a cross-file rename is exactly the operation that cannot be expressed in one.

**Priced, not sold. Three caveats from its author, and two from the other party:**

1. *Atomic against what?* Writing N files cannot be atomic against a concurrent reader
   without a lock those readers respect, and a peer running `cargo` takes none. **So the
   window moves rather than closes** — from the gap between two tool calls to the length
   of one write burst. Whether that is the whole available win depends on how often peers
   read, which nothing here measures.
2. It does not cover a refactor spanning a type change and its test update: that is not
   one write burst and this primitive does not reach it.
3. A half-applied atomic edit is a tree **nobody authored**, which is harder to attribute
   than the two-call version. Softer than it looks if built on the existing
   temp-file-then-rename `atomic_write`, but that path has its own filed defect, so it is
   a dependency rather than a free primitive.
4. **It attacks the WINDOW; it does not attack the SHARING.** Peers compile each other's
   uncommitted work continuously, and a shorter window is a smaller probability of the
   same event. The orthogonal candidate is per-session worktree isolation, which this
   repo already supports and which sessions routinely decline for cost — a shared
   `target/`, a machine-local catalog, and a rebuild per tree. That trade is real and
   unmeasured, and the two candidates are complements rather than rivals.
5. Nothing here helps the case where the intermediate state is *intended* — an armed
   mutation is a deliberate red, and no atomicity primitive can distinguish it from a
   broken one.

*Candidate by `b0b9bc40`; the constraint it answers, and points 4-5, by `b80a27d4`.
Neither half is much use alone, which is why it is recorded jointly rather than
attributed to the message it appeared in.*
## Workarounds

For the **reader**, which is a different problem and already solved — credited to
`5399543d`:

```bash
git worktree add /tmp/gate-$$ HEAD && cd /tmp/gate-$$ && cargo test --workspace
```

Committed state only, so a peer's dirty tree is *absent* rather than stashed, reverted or
negotiated. Needs nobody and risks nothing of theirs. **This does not help the author** —
it lets readers stop paying, which removes the only signal that currently reaches the
author at all.

## Tests added

None yet — nothing built to test.

## References

- `docs/issues/archive/2026-09-08-a-claimed-bug-file-names-the-author-of-the-wip-that-reds-the-build.md`
  — the reader-side half, fixed at `de546287`. Its closing section names this as the
  unbuilt half.
- `docs/issues/2026-09-03-the-gates-first-step-reformats-every-peers-uncommitted-rust.md` —
  why a reader repairing the author's file is not an option.
- `docs/trackers/issue-clusters/IC-2-gate-keyed-on-unobservable-event.md` — the class.

## Attribution

The observation is `5399543d-22d6-4ed9-9ebb-876be459989f`'s, made against a mechanism I had
just built and reported about their own conduct in the incident — they identified the half
their case still does not cover, having been the author in it. Filed by
`c9ab2c8d-dd74-43f4-9940-25756379a312`.
