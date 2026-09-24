---
kind: bug
status: mitigated
tags:
- cluster/transient-shared-state-lies-to-readers
closed: 2026-09-14
opened: 2026-09-14
owner: marius
related: []
severity: medium
unverified: 'TRACKED b8f756b54ac934a5 — The race itself is NOT closed -- only its legibility, and its arming for whoever runs scripts/gate.sh. cli_doc now names the cause instead of reading as the reader''s own feature-gating regression, and the gate lanes build in a per-session CARGO_TARGET_DIR keyed on CLAUDE_CODE_SESSION_ID. WHAT REMAINS IS ADOPTION, NOT A DECISION -- the earlier form of this field named direction 1 as an unmade operator call, and a scoped variant of it shipped in 58b6bafc; four sessions had adopted it by 2026-09-15, 61 G across four isolated trees. A session that types the four commands by hand still shares target/ and can still replace target/debug/codescout inside another lane''s run phase, so the script is a mechanism for whoever runs it and a policy for everyone else. RETRACTED 2026-09-15: this field previously named tests/cross_process_write_lock.rs and tests/librarian/mcp_integration.rs as carrying the same by-path exposure. Neither does -- mcp_integration is #[ignore]d against a binary the 2026-05-16 dissolution deleted, and cross_process_write_lock declares no required-features so it runs in both lanes and passes against a lean binary (measured, 5.04s). cli_doc is the only detector this corpus can have, which is why it must never be made to skip. Measured cost of the isolated lanes, first cold run: 13 G and 3m10s, against 358 G free and a 113 G shared tree.'
---

# BUG: the gate-ordering guarantee is true sequentially and false under concurrency — a peer's lean lane re-arms the trap inside your default lane

## Summary

CLAUDE.md § *Development Commands* states:

> *"Ending on the default lane rebuilds it, so following the gate cannot arm the trap for anyone
> else — **provided both lanes actually run**."*

That holds for one session in isolation. It does not hold when two sessions run the gate at
overlapping times, and **the stated condition does not catch the case**: both parties run both
lanes, both follow the documented order, and the guarantee still fails.

The premise the sentence does not state is *sequential*. Session A's lean lane arms the trap the
moment it finishes and disarms it when A's default lane completes — a window of minutes. Any
session whose `cli_doc` tests execute inside that window gets the librarian-less binary, and
nothing either session did was wrong.

Six sessions shared this checkout when it was measured, so the union of those windows is not small.

## Symptom (Effect)

13 of 15 `tests/cli_doc.rs` tests fail:

```
error: unrecognized subcommand 'doc'
code=2
```

`tests/cli_doc.rs` resolves `target/debug/codescout` **by path at run time**, so it runs whatever
binary the last writer left. `--no-default-features` switches the librarian off, and with it the
`doc` subcommand.

**It reads as a feature-gating regression in whatever the reader just committed** — which is the
IC-12 shape: the standard diagnostic reports the lie as truth rather than as an outage.

## Reproduction

Not deterministically reproducible from one session by construction — it needs two. Observed
2026-09-14, session `9403d62d` (this file's author) and `aa272bed` (`codescout-bb`), both in
`/home/marius/work/claude/codescout`, local times UTC+3:

```
9403d62d  gate run 1:  lean finished 12:36:00  ->  default finished 12:37:26   <- RED
9403d62d  gate run 2:  lean finished 12:38:18  ->  default finished 12:40:00   <- clean
```

The panic carries its own clock — `2026-09-14T09:37:26.818385Z` = 12:37:26 local — so `cli_doc`
executed at the very end of the default lane, **86 seconds after this session's own lean lane had
already finished**. The lane order was correct and the binary was still wrong.

## Environment

- Date: 2026-09-14, branch `experiments`
- 6 sessions in this checkout (5 peers plus me), 21 machine-wide across 5 profiles, by socket
  enumeration at 2026-09-14T09:32:40Z

## Root cause

`target/` is shared across every session in the checkout, and `target/debug/codescout` is a single
mutable path with no owner. The gate's ordering rule makes each session's *own* exit state safe; it
cannot make the *interval* safe, because the interval belongs to whoever else is building.

## Evidence

`aa272bed` supplied its three lean-lane windows, which is the one fact this session could not
observe:

```
lean 12:27:36 -> default rebuilt 12:30:07     (librarian-less for 2m31s)
lean 12:33:42 -> default rebuilt 12:35:28     (1m46s)
lean 12:41:30 -> default rebuilt 12:43:28     (1m58s)
```

The second window ends 32s before this session's default lane starts; the third begins 4m04s after
the failure. **Neither contains 12:37:26**, so that peer is ruled out.

**That is a scope statement, not an identification.** Four other sessions held this checkout and
none can be named from here — CLAUDE.md § *Observer Blindness* forbids closing an authorship
question by elimination, and "not `aa272bed`" is not a positive identifier. The class does not
depend on naming the party; it depends on the window existing.

The first reading of this red — *"most likely a concurrent lean lane, six sessions share this
checkout"* — was adjacency reasoning with a mechanism's shape, offered before any timestamp was
compared. It happened to name the right class and the wrong session, and it was falsified by the
peer volunteering the data rather than accepting the attribution.

Measured 2026-09-14 by sessionId held at registry name `codescout-b7`, by polling the mtime of
`target/debug/codescout`:

| window | duration | commits | writes to `target/debug/codescout` |
|---|---|---|---|
| 12:37:26 – 13:11:46 | 34m | 7 | **3** (12:37:26, 12:46:13, 13:11:46) |
| 13:11:46 – 15:56:05 | 2h44m | 5 | **0** |

**The rate is bursty, and that is worse for this bug rather than better.** A uniform low rate
would make a collision unlikely — a background hazard anyone might hit at random. What the data
shows instead is that writes concentrate exactly when two sessions run lanes at once, which is
precisely the activity that creates the build→run window. Exposure and hazard share a cause, so
the collision is likely *conditional* on the activity, and **the denominator is concurrent gate
runs, not wall-clock minutes.** That reframing is `codescout-b7`'s and is the substantive half of
this measurement.

Three caveats, stated because the number is softer than it looks:

1. `commits` is a **proxy** for gate runs, not a measure of them. Five commits with zero rebuilds
   most likely means those sessions ran no full gate — or ran one before 13:11:46, and mtime
   cannot distinguish the two.
2. mtime shows only the **last** write, so "zero since 13:11:46" is solid (mtime is monotonic)
   and nothing earlier is reconstructable from it.
3. The monitor watching this died with a reboot and took its log with it. **The mtime outlived
   the instrument** and answered a longer question than the instrument was built to ask — the
   inverse of this corpus's usual failure, where the instrument survives and the thing it
   measured has moved.

Partial closure on caveat 1, from this session's own lane timestamps: gates ran 12:35:20–12:37:26,
12:38:18–12:40:00, ~13:0x, plus two later full gates — one of them in an isolated worktree with its
own `CARGO_TARGET_DIR`, which by construction wrote nothing to the shared binary. So at least two
of the first window's three writes are plausibly this session's, and the three writes are **not**
three distinct sessions.

## Hypotheses tried

- *"The lean lane left it and the default lane had not rebuilt yet."* Falsified by the clock: this
  session's own lean lane finished at 12:36:00, 86s before the failure, and its default lane was
  the one running.
- *"A stale test binary."* Falsified by the error text — `unrecognized subcommand 'doc'` is the
  shipped binary's own clap output, not a test-harness artifact.

### SETTLED 2026-09-14 — the window IS build→run inside ONE lane. `aa272bed` was right.

Raised by `aa272bed` and left open here. Settled by measurement, by a different route than the
one this section proposed — the proposed route (stamp the binary's mtime around a default lane)
is opportunistic and needs a peer to write during the window; the lock itself is observable
directly and needs nobody's cooperation.

**Cargo holds `target/debug/.cargo-lock` through the BUILD phase and releases it before running
tests.** Sampled with `lsof -t`:

| phase | sample | `lock_holders` |
|---|---|---|
| `cli_doc` **run** (3 test processes alive) | 3 consecutive | `[]` |
| `cargo check --workspace` **build** (control) | 4 consecutive | `[3353947]` |

**The control is what makes the empty result a measurement rather than a broken probe** — without
it, `lsof` returning nothing is indistinguishable from `lsof` not seeing flock holders at all.
Confirmed a third time, independently and by accident: a later `cargo test --test cli_doc` printed
`Blocking waiting for file lock on build directory` while a peer's cargo held it.

**So the exposure is not "a peer who ran a lean lane recently".** Any peer cargo can take the lock
and replace `target/debug/codescout` *at any instant inside your own run phase*. A peer following
the gate **perfectly** still writes a librarian-less binary to the shared path once, mid-sequence,
and your `cli_doc` can execute in that instant. **Their compliance cannot help you and neither can
yours** — which is what makes *"provided both lanes actually run"* a condition every party
satisfies while the guarantee fails.

One consequence worth stating because it reads backwards: `cli_doc` declares
`required-features = ["librarian"]`, so the **lean lane never runs it**. The lean lane only ever
WRITES the hazard; only a default lane can READ it.

`cargo test` builds, then runs. This session's default lane built a librarian-bearing binary and
executed `cli_doc` ~86s later against a librarian-less one. If the vulnerable interval is
build→run *within* a single lane, then the exposure is not "a session that ran a lean lane
recently" but **any concurrent write to `target/debug/codescout` at any moment inside your own
lane** — and sequencing your lanes correctly buys nothing, because these lanes *were* sequenced
correctly.

`tests/cli_doc.rs:11` uses `Command::cargo_bin("codescout")`, which resolves the path at run time
rather than through `CARGO_BIN_EXE_*`, so cargo offers no freshness guarantee between the build and
the execution. That is consistent with the refinement and does not establish it.

**What settled it:** not the mtime stamp proposed here — that needs a peer to write inside the
window and is therefore opportunistic. Observing the lock directly answers the same question
deterministically, needs no cooperation, and took one command plus its control.

One adjacent observation, which supports concurrency generally and the refinement not at all:
`target/debug/codescout` was written at **12:46:13**, six minutes after this session's last lane
ended at 12:40:00, by a session this one cannot name.
## Fix

**MITIGATED, not fixed — direction 2 shipped, and the distinction is the point.** The race is
untouched; what changed is that it now reads as what it is.

**2. Shipped, then strengthened.** [`pinned_binary`] in `tests/cli_doc.rs`, used by `run_cmd` — the
single chokepoint all 15 tests route through. It takes a **hardlink** to the built binary once,
pinning the INODE, and checks that pinned copy advertises `doc`. Cargo replaces the binary by
rename, so the link keeps the original inode alive for the life of the suite and **no peer can swap
it mid-run**. Its first form was a per-call precondition, which narrowed the window to microseconds
rather than closing it; the pin removes the class, and checking once is only correct BECAUSE of the
pin.

*Observed RED, end-to-end, in an isolated `CARGO_TARGET_DIR` so nothing shared was armed:* built
the test target WITH `librarian` and pointed it at a genuinely `--no-default-features` binary,
which is the incident exactly. **15 of 15 fail and all 15 name the cause**, against the recorded
baseline of 13 of 15 failing on raw `unrecognized subcommand 'doc'`. Going 13→15 is the
improvement: the two that used to PASS include
[`the_old_artifact_subcommand_is_gone`], whose absence assertion is monotone under losing the
whole verb set — so the one test that looks like it would catch a librarian-less binary was
structurally the one that could not.

Discrimination checked in both directions rather than one: `doc --help` → **rc=2**
(`unrecognized subcommand 'doc'`) on the lean binary, **rc=0** on the librarian-bearing one.

**Called per `run_cmd`, not once behind a `OnceLock`** — now that the window is known to be
build→run, the replacing write can land *mid-suite*, and a one-shot check at suite start would
pass and leave every later test as confusing as before.

**1. SHIPPED as `scripts/gate.sh`, in the variant that dodges the symlink** — a per-session
`CARGO_TARGET_DIR` keyed on `$CLAUDE_CODE_SESSION_ID`, wrapping the four gate commands and
leaving `cargo rb` on the shared tree, so `~/.cargo/bin/codescout` keeps resolving to a binary
something rebuilds. It prints the four exit codes AND **exits non-zero if any lane failed**, which
the `;`-chained form structurally cannot, because it ends in `echo`.

*Cost, measured on the first cold run rather than quoted:* **13 G** per session, 3 m 10 s cold with
`sccache` already warm from the shared tree. Against 358 G free and a 113 G shared `target/`, four
concurrent sessions is ~52 G. The script prints its own tree size on **every** run, so the number
is re-derived at the point of use instead of decaying in a doc.

*Verified end to end in the isolated tree:* `Running tests/cli_doc.rs
(~/.cache/codescout-gate/<sid>/debug/deps/cli_doc-…)`, 15 passed — so the pin below works **under**
the isolation, not merely beside it. `CLIPPY=0 LEAN=0 DEFAULT=0`. `FMT=1`, and that is the guard
working: `fmt-mine.sh` formatted one file of this session's and **refused** a peer's unformatted
`src/agent/mod.rs`, naming the owner. `FMT` is therefore the one ambiguous code, and the script now
says so at the refusal site rather than leaving a bare `1` to be read as this session's failure.

**Its limit, which is real and not modesty:** a session that types the four commands by hand still
shares `target/`. It is a mechanism for whoever runs it and a **policy** for everyone else —
§ *Observer Blindness* position 3's weaker shape. The four commands in `CLAUDE.md` stay the
canonical statement of what runs and in what order, and stay pinned byte-for-byte by
`claude_md_gate_lists_its_four_commands_in_the_load_bearing_order`; the script is named in a bullet
BELOW the directive, which is why that test needed no change.

**A consequence found only by building it, and it generalises past this script.** Isolating the
target dir silently disabled a test: `src/lsp/manager.rs` resolved
`CARGO_MANIFEST_DIR/target/debug/codescout` and **early-returned** when absent, so under a custom
target dir it would have passed having run nothing — and the loss would be invisible in exactly the
run that introduced it. Fixed in the same change to honour `CARGO_TARGET_DIR` (`CARGO_BIN_EXE_*` is
not available: Cargo sets it for integration tests, and that is a unit test inside the lib).

**The general form, raised by `d52899fd-…` on reading the fix:** *any test that resolves an artifact
BY PATH and early-returns on absence converts an environment change into silent coverage loss.*
**Which makes `tests/cli_doc.rs` load-bearing in a way that reads like noise:** it REDS rather than
skipping when the binary is wrong, and that is the only reason this whole defect was ever visible.
If anyone ever "fixes" that noise by adding a skip-if-absent guard, the trap goes silent and the
multi-session window becomes undetectable. **Do not make `cli_doc` skip.**

**WHY THE NAIVE FORM WAS REJECTED — a bare per-session `CARGO_TARGET_DIR` exported for the whole
session, rather than scoped to the gate lanes.** Recorded because it is the obvious reading of
"direction 1" and a reader would otherwise retry it. Two independent reasons, either sufficient:

- **It breaks the live MCP binary, machine-wide.** `~/.cargo/bin/codescout` is a symlink to
  `<repo>/target/release/codescout`. `CARGO_TARGET_DIR` moves **both** profiles, so `cargo rb`
  would write into the session's private dir while the symlink kept pointing at a path nothing
  rebuilds — permanently stale, for every session on every profile. `cargo rb` + `/mcp` is the
  documented live-MCP loop, so this is not a side effect, it is the loop.
- **The only available lever is per-PROFILE, not per-session.** All three profiles carry an `env`
  block in `settings.json`, which is the mechanism — but 3 of the 4 sessions in this checkout ran
  under `.claude-sdd` at the time of measurement. A profile-level `CARGO_TARGET_DIR` leaves those
  three sharing a dir: it converts a 6-way race into a 3-way one **among the sessions most likely
  to collide**, and reads as a fix.

That variant is what shipped, and building it **falsified one of the costs predicted for it here**:
it was expected to edit the four-command gate sentence pinned by
`claude_md_gate_lists_its_four_commands_in_the_load_bearing_order` (`src/prompts/mod.rs`). It does
not. That test scopes between `"**Run `./scripts/fmt-mine.sh`"` and `"before completing any
task.**"` and asserts order *within* that slice, so naming the script in a bullet BELOW the
directive leaves it untouched and green. The prediction was made by reading the warning about the
pin; the refutation came from reading the test. The other predicted cost stands and is not
negotiable: it is a **policy every session must adopt** rather than a mechanism — § *Observer
Blindness* position 3's weaker shape.

**Why the mitigation was closer to sufficient than it looked, and what replaced it.** Cargo
replaces the binary by **rename** — inode 196951675 → 197002269 across one rebuild, measured — not
by writing in place. **That is what made a hardlink a fix rather than a gesture**, so the
per-`run_cmd` check was replaced by [`pinned_binary`], which takes the link once and checks once.
The earlier reasoning — that per-call checking left only a microsecond window and pinning was not
worth a `static` plus a cleanup path — was overturned by the operator, correctly: a microsecond
window is still a window, and the pin removes the class rather than narrowing it. Recorded because
the judgement was mine and the reversal was right.

**RETRACTED 2026-09-15 — "same exposure, not addressed here" was FALSE, and it was a worklist item
sent to a reader for work that does not exist.** This paragraph named
`tests/cross_process_write_lock.rs` and `tests/librarian/mcp_integration.rs` as carrying the
by-path exposure the pin fixed in `cli_doc`. Checked at the bytes, neither does:

- **`tests/librarian/mcp_integration.rs` is not an instance — it never runs.** It is
  `#[ignore = "requires standalone librarian binary which no longer exists post-dissolution"]`,
  and `cargo_bin("librarian-mcp")` names a binary `Cargo.toml` declares no target for; the
  2026-05-16 dissolution deleted it. `tests/librarian/main.rs` already annotates it inert, with the
  count (*"19 tests, 17 of which run"*), which is § *Testing Discipline*'s **annotate an inert
  fixture as inert** already obeyed. Citing it here credited it with an exposure it cannot have.
- **`tests/cross_process_write_lock.rs` has the mechanism and no observable consequence.** It
  declares no `required-features`, so it runs in **both** lanes — and therefore passes against a
  librarian-less binary by design. Measured 2026-09-15 in an isolated tree:
  `cargo test --no-default-features --test cross_process_write_lock` →
  `write_lock_contention_produces_recoverable_error ... ok`, 5.04s. **That run IS the condition a
  mid-run swap creates**, which is what makes it a measurement rather than an argument. Its
  `env!("CARGO_BIN_EXE_codescout")` also already tracks `CARGO_TARGET_DIR`, so the isolation
  defect found in `src/lsp/manager.rs` does not reach it, and its own archived bug
  (`2026-08-27-cross-process-write-lock-test-passes-when-it-does-not-run.md`) already closed the
  skip-branch half.

**What the check turned up instead, and it is the sharper fact: `cli_doc` is not one detector among
several — it is the ONLY one the corpus can have.** Exactly three test files execute a binary by
path: the two above and `cli_doc`. A test that runs in **both** lanes passes in both, so it is blind
to a lane swap **by construction**; only a `required-features = ["librarian"]` target is asymmetric
enough to notice. Of the three librarian-gated targets (`audit_doc_refs`, `cli_doc`, `librarian`),
`cli_doc` is the single one that both executes the binary and executes at all.

**So "Do not make `cli_doc` skip" is structural, not a preference** — the property that makes it the
only possible detector is the same property that makes it poisonable, and if it goes quiet there is
no second instrument. **The one live residue:** the by-path mechanism does still sit in
`cross_process_write_lock.rs`. It is inert only because that test asserts nothing a lean binary
fails. Add one librarian-gated assertion there and it inherits the full defect silently.

Direction 3 is retired as insufficient: stating the premise in `CLAUDE.md` cannot help when no
behaviour change by any party closes the window — which is now measured, not argued. It has been
written anyway (the `CLAUDE.md` gate-order bullet now carries it), and it is worth having for the
reader who hits this; it is simply not a fix.

Both commits are cited in § *Fix provenance* below, with their roles named. **The pairs are there
rather than here for a reason worth the line:** prose hashes are exactly what `doctor`'s
`terminal_status_without_fix_anchor` reads as an anchor that is not one — it fired on this record
while four commit-like hashes sat in its prose, because a reader scanning for provenance finds one
and stops looking. Naming them in prose is not recording them.

**Deliberately NOT archived.** The mitigation is verified and the gate is green, but the defect
this record names is still live and its only closure is an operator decision. Archiving would
remove the one queryable surface where that decision is visible — a `mitigated` record in
`docs/issues/` is the honest place for a fix that made an outage legible without preventing it.

## Fix provenance

- **SHA:** `412415bd` (`experiments`)
- **patch-id:** `213ec5cc5d2f4ddd77a56e1022a7407d79ba2b08`
- **SHA:** `58b6bafc` (`experiments`)
- **patch-id:** `cd244fa46a481105e2c9f2b5de9b64ef38649f31`

**The second SUPERSEDES the first's mechanism, and the plural form cannot say so.** Each
`- **patch-id:**` binds to the `- **SHA:**` above it and to nothing else, so the ordering of the
pairs carries no meaning any parser reads — which is why it is stated in prose here, inside the
declared section, rather than left for a reader to infer from sequence.

`412415bd` made `cli_doc` say whose outage it is, via a per-`run_cmd` check that the binary
advertises `doc`, and settled the window with the `lsof` measurement in § *Evidence*. **Its
diagnosis stands; its mechanism does not.** `58b6bafc` is what runs today: [`pinned_binary`]
(hardlink taken once, checked once — link count 3 observed, so it shares the inode rather than
copying), `scripts/gate.sh`, and the `src/lsp/manager.rs` silent-skip that the isolation itself
would otherwise have introduced.

Both patch-ids were re-derived with `git show <sha> | git patch-id --stable` rather than copied
forward; `412415bd`'s matched the previously recorded value byte-for-byte.
## Attribution

The finding is a **pair**, and neither half stands alone.

**`aa272bed` (`codescout-bb`) held the formulation:** that the guarantee's stated condition —
*"provided both lanes actually run"* — is satisfied by **both** parties while the guarantee still
fails, because the unstated premise is sequential execution. They also supplied the three lean-lane
windows above, which is what ruled out the obvious suspect and made the concurrency reading
necessary rather than convenient.

**`9403d62d` (this file's author) held the byte-level confirmation:** `error: unrecognized
subcommand 'doc'`, code=2, which is what rules out "some other cause wearing those test names", plus
the lane timestamps that falsified the first attribution.

Recorded as a pair at `aa272bed`'s own correction of a more generous split offered by this session:
*"a formulation with no byte-level confirmation under it is a story."*
## References

- CLAUDE.md § *Development Commands* — the guarantee, and `73066479`, the commit that ordered the
  lean lane third
- [`docs/conventions/gate-ordering.md`](docs/conventions/gate-ordering.md) — the measurements behind
  that order
