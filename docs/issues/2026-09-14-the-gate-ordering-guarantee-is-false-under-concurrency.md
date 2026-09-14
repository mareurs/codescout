---
kind: bug
status: open
tags:
- cluster/transient-shared-state-lies-to-readers
closed: null
opened: 2026-09-14
owner: marius
related: []
severity: medium
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

### OPEN — is the window build→run inside ONE lane, rather than lean→default across two?

Raised by `aa272bed` and **not settled**, recorded here rather than resolved by plausibility.

`cargo test` builds, then runs. This session's default lane built a librarian-bearing binary and
executed `cli_doc` ~86s later against a librarian-less one. If the vulnerable interval is
build→run *within* a single lane, then the exposure is not "a session that ran a lean lane
recently" but **any concurrent write to `target/debug/codescout` at any moment inside your own
lane** — and sequencing your lanes correctly buys nothing, because these lanes *were* sequenced
correctly.

`tests/cli_doc.rs:11` uses `Command::cargo_bin("codescout")`, which resolves the path at run time
rather than through `CARGO_BIN_EXE_*`, so cargo offers no freshness guarantee between the build and
the execution. That is consistent with the refinement and does not establish it.

**What would settle it:** whether anything wrote `target/debug/codescout` between this lane's build
and its `cli_doc` execution. **That evidence is gone** — the file's mtime had been overwritten by
12:46:13 when the question was asked. It is answerable prospectively, by stamping the binary's mtime
immediately before and after a default lane, and not retrospectively from this incident.

One adjacent observation, which supports concurrency generally and the refinement not at all:
`target/debug/codescout` was written at **12:46:13**, six minutes after this session's last lane
ended at 12:40:00, by a session this one cannot name.
## Fix

None yet. Three directions, none costed:

1. **Isolate the resource** — a per-session `CARGO_TARGET_DIR`. Removes the class outright; costs
   disk and every session's warm-cache rebuild.
2. **Make the reader assert what it got** — have `cli_doc` check the binary advertises `doc` and
   emit *"this binary was built without the librarian; another session's lean lane is mid-flight"*
   instead of 13 assertion failures. Does not prevent it; converts an outage that reads as a
   regression into an outage that reads as an outage, which is the IC-12 remedy shape.
3. **State the premise in CLAUDE.md.** Cheapest and weakest: the guarantee is currently read as
   unconditional-given-compliance, and a reader who hits this concludes their own diff broke the
   build. Note that the sentence is pinned byte-for-byte by
   `claude_md_gate_lists_its_four_commands_in_the_load_bearing_order` (`src/prompts/mod.rs`), so
   editing it moves that test too.

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
