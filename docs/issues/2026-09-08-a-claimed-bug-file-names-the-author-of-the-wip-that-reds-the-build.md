---
id: df517af91b43a5f7
kind: bug
status: investigating
title: A claimed bug file names the author of the WIP that reds the shared build, and routing goes past it
tags:
- cluster/authorship-unrecoverable-after-the-fact
topic: shared-checkout authorship
---

# BUG: a claimed bug file names the author of the WIP that reds the build, and routing goes past it

## Summary

On a shared checkout, uncommitted WIP that does not compile reds both test lanes for every
session in the tree. `IC-10`'s claim, as narrowed on 2026-09-07, is that authorship for such
state is unrecoverable — *"There is no attribution channel for those, so every party infers it
from proximity."*

**Here there was one, and it went unread.** The file being edited was
`src/agent/write_guard.rs`; the bug being worked was
`docs/issues/2026-09-08-the-write-lock-refusal-cannot-tell-a-cleared-record-from-an-absent-one.md`,
whose frontmatter carried `claimed_by: 5399543d-22d6-4ed9-9ebb-876be459989f` — put there through
`hints.claimable` twenty minutes before the red. The identifier was in the artifact under
discussion, and the routing still went to the wrong session.

This is filed as a **refinement** of `IC-10` rather than a sixth instance of it, in the shape the
class already uses for `a-sessions-self-reported-name-is-not-self-verifiable`. It is not a new
case of a missing channel; it is a case of the channel existing, being correct, and not being
consulted — which is a different remedy.

## Symptom (Effect)

Between roughly 10:50 and 10:57 on 2026-09-08, `cargo check --lib --profile test` failed
tree-wide:

```
error[E0277]: `WriteGuard` doesn't implement `std::fmt::Debug`
  --> src/agent/write_guard.rs:536   (expect_err requires T: Debug)
```

Two sessions were affected and neither could act:

- `ad379a7c` (`codescout-e7`) observed the red and investigated it as possibly their own.
- `59112612` (`codescout-92`) **attributed the file to `ad379a7c`** and was about to land their
  cluster-refusal fix with the full gate red, recording an `unverified:` rather than archiving
  clean. `ad379a7c` stopped it by telling them the red was stale.

So the cost did not fall on the author at all. It fell on a third party's **ship decision**, in
the form of a caveat they would have written truthfully about a failure that was not theirs and
had already been fixed.

## Two failures compose, and fixing either alone would not have prevented it

Separating them is the point of this file; they were run together in the first telling.

| | failure | who it reaches | what fixing it alone buys |
|---|---|---|---|
| 1 | **misrouting** — the alarm went to `ad379a7c`, who had not written the file | the wrong session | they still could not fix it — see 2 |
| 2 | **unanswerability** — even correctly routed, the recipient cannot repair another session's uncommitted Rust; doing so is the defect `docs/issues/2026-09-03-the-gates-first-step-reformats-every-peers-uncommitted-rust.md` describes | the right session, uselessly | the alarm arrives somewhere it can be *reported* but not *acted on* |

This is `CLAUDE.md` § *Testing Discipline*'s arrival-versus-answerability law holding one layer up:
the addressee can be named correctly and still have no move available.

## Root cause

`IC-10`'s Claim is right that git captures no author for uncommitted state. What it misses is that
**the librarian does, whenever the work is claimed against a bug file** — `claimed_by` is written
through the catalog, lands in committed frontmatter, and is queryable by
`doc(action="find", kind="bug", filter={"status": {"eq": "taken"}})`.

That channel is:

- **narrow** — it names the author of a *bug*, not of a *file*, and only when someone claimed one;
- **not indexed by path** — nothing maps `src/agent/write_guard.rs` back to the claim, so finding
  it requires already suspecting which bug is being worked;
- **not consulted by anything that observes a red.** The build failure and the claim record live
  in two systems that never meet.

The third bullet is the defect. The first two are why nobody thought to look.

## Why the author cannot see this

The author's own build is fine — theirs is the tree that compiles once they finish the edit. A
transient WIP red is repaired **by the author, for the author's own reasons, without ever learning
it was load-bearing for anyone.** I fixed the `Debug` error because my test would not build, not
because I knew two sessions were reading the failure.

**That makes the class's instance count a lower bound by construction, and not by sampling.** The
system self-heals silently, so the ordinary outcome of an instance is that it leaves no artifact
in any ledger — `CLAUDE.md` § *Testing Discipline*'s recording-filter law, where widening the
sample changes nothing. This one is recorded only because `ad379a7c` chose to report it after it
had already cleared.

## Reproduction

Two sessions, one checkout, shared `target/`. A writes uncommitted Rust that does not compile. B
runs any `cargo test` / `cargo check` and gets a correct failure naming a file B never touched.
`git status` shows the file dirty and no author. `git log` shows nothing — it is uncommitted.

## Fix

Not implemented, and the shape is not obvious. Three candidates, cheapest first:

1. **Read-side, no new mechanism.** When a gate red names a file you did not touch, query the
   claim ledger before attributing:
   `doc(action="find", kind="bug", filter={"status": {"eq": "taken"}})` and read `claimed_by`.
   Cheap, already possible today, and requires the reader to think of it — a policy, not a
   mechanism, which is `skill-frictions:SKF-22`'s failure mode.
2. **Widen the claim's key from bug to path.** `claimed_paths` alongside `claimed_by` would let a
   reader go file → session directly. Costs a field and a discipline nobody has today.
3. **Write-side, and probably the right one:** a red that names a dirty file could resolve the
   file's author from the socket enumeration + `scripts/file-provenance.py` at failure time and
   say so. That is the mechanism `IC-10` calls `H`-target, and it is the half `Mechanism status:
   shipped (partial)` still records as absent.

Deliberately not chosen here: this file's job is to correct the class's Claim, which currently
tells a router the channel does not exist.


### Substrate repaired 2026-09-08 — option 3's instrument now works; option 3 is still unbuilt

Taken by `ad379a7c` to implement option 3, and **running the reproduction first inverted the
plan**. Option 3 proposes resolving a red's author *"from the socket enumeration +
`scripts/file-provenance.py` at failure time"*. Run before writing anything, that probe returned
`UNKNOWN` on every dirty file in this tree — including a bug file written through `doc()` ten
minutes earlier. **The wiring would have shipped a confident silence into exactly the situation it
was built for.**

Cause and repair: `249dcf2690afcd35`. The probe's librarian selector still matched
`mcp__codescout__artifact` six days after `ceb5b57a` renamed that tool to `doc`, so all 501
write-action `doc` calls in this project were invisible. Fixed at `5ec4bac8` (patch-id
`61e43235f4b7b9412613fff2ccd62845fef8499b`), with `doc(action="move")`'s destination at `1e90561c`
(patch-id `fcfc1957d165ea503c5c7208018250773ca03807`) — that second one found by *using* the first
when a peer's staged archive move blocked its own fix commit.

**This bug's own claim is now demonstrated rather than argued.** Its Root cause says the channel
exists, is correct, and is *"not consulted by anything that observes a red"*. That was true and
incomplete: nothing consulted it, **and it would have answered wrongly if anything had.** The
second half is repaired; the first is not.

**Left open deliberately, with the two constraints measured** so whoever takes option 3 does not
re-derive them:

- The scan costs **5.0s** and does not cache, so it can only run on a *detected* red, never inline
  on every command.
- Native `Bash` bypasses `run_command` entirely, so a `run_command`-hosted hint has a reachability
  ceiling — § *Testing Discipline*'s *"loudness is a property of a PATH"*. Name it at the site
  rather than discovering it later.

Claim released to `investigating` rather than `open`: work happened and this section records it.
## Tests added

None. There is nothing to regression-test yet — the finding is that a record which exists is not
read. A test would have to assert about a consumer that has not been built.

## Workarounds

Run `cargo fmt -- --check` before the gate's rewriting `cargo fmt` step (read-only, whole
workspace, exit 0 means the writing form is provably a no-op) — this is the *reverse* direction of
the same shared-tree problem and is a real mitigation, credited to `59112612`. **It is TOCTOU**: a
blank line appearing after a clean check is exactly the window, observed the same morning.

And when you see a red you did not cause: say so out loud to the peers in the tree *before* it
lands in their attribution. Pre-announcing worked here — after `ad379a7c` flagged it, a second red
(`peer::server::tests::run_exits_after_idle_timeout_with_no_connections`, load-sensitive and
separately filed) was announced ahead of time and correctly read as load rather than content by
both parties.

## References

- `docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md` — the same
  read-side problem via `-D dead-code`. **Its title and Root cause are narrower than its class**:
  this instance fails compilation by a type error, so it does not belong under that title even
  though the class fits.
- `docs/issues/2026-09-03-the-gates-first-step-reformats-every-peers-uncommitted-rust.md` — the
  write-side, and the reason failure 2 above is unanswerable.
- `docs/issues/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md` — the same
  shared-`target/` substrate, different mechanism.
- `docs/trackers/issue-clusters/IC-10-authorship-unrecoverable-after-the-fact.md` — the class whose
  Claim this refines.

## Attribution

Observed and reported by sessionId `ad379a7c-a0cf-4c61-bcdb-f0696fea8c30`, who also stopped
`59112612-5fc8-4b31-8c8c-e19220d99eac` from shipping against the stale red, verified the fix window
independently (`cargo check` exit 0 at 10:58), and supplied the two-failures-compose split and the
argument that this does not fit the existing file's title — both of which are the shape of this
write-up rather than a detail in it.

**The WIP was mine**, sessionId `5399543d-22d6-4ed9-9ebb-876be459989f`. I did not observe the red,
which is the finding.
