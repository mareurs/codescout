---
id: df517af91b43a5f7
kind: bug
status: taken
title: A claimed bug file names the author of the WIP that reds the shared build, and routing goes past it
tags:
- cluster/authorship-unrecoverable-after-the-fact
topic: shared-checkout authorship
claimed_at: 2026-09-08
claimed_by: c9ab2c8d-dd74-43f4-9940-25756379a312
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
### Option 3 built 2026-09-08 — `scripts/attribute-red.py`, wired into `run_command`

The consumer this file says does not exist now exists. A red that names a file with
uncommitted changes resolves that file's author and, when they are live, prints the `uds:`
socket to reach them — automatically, on any non-zero `run_command` exit.

**Two stages, because the answer is not cheap and the common case must stay free.** Both
numbers re-measured rather than carried down from the section above: `git status
--porcelain` costs **0.01 s**, the transcript scan **7.0 s** (6.98 / 7.02, two consecutive
runs, no caching). The 5.0 s recorded above was right on 2026-09-02 and is a **40%
understatement** six days later — the cost tracks the corpus, so re-derive it rather than
citing either figure. So: is anything dirty at all (0.01 s, usually ends it) → does the red
name one of those paths (free, pure text) → only then, who wrote it (7.0 s). A green
command spawns nothing whatsoever.

**Running the reproduction inverted the plan a second time.** The first version used
`fp.scan()`'s records wholesale and so reported a path's **lifetime** authors. Tried against
this tree it named two peers for a bug file *this session had edited minutes earlier*, and
did not name the actual editor — a confident wrong name delivered at the exact moment
someone is looking for a party to blame, which is worse than the silence it replaced. The
floor has to be derived per path from `git log -1 --format=%cI`, exactly as
`file-provenance.py`'s own `main()` does. That is now the suite's *THE WINDOW* section.

**The ceilings are named at the site**, per the constraint left above. Three, all silent:
native `Bash` bypasses `run_command`; a red that exits **0** never reaches stage 1; no
`python3`, no answer. `--explain` distinguishes those from a clean tree and the wired path
deliberately does not — a diagnostic about the diagnostic lands inside a failure the reader
is already parsing.

The scripts are `include_str!`'d into the binary and materialized to a temp dir. Reading
them from the workspace under analysis was considered and rejected: it hands arbitrary code
execution to any checkout a failing command runs in.

**What it still does not do, and this file's failure 2 is why.** It answers *misrouting*
only. The recipient of a correctly-routed alarm still cannot repair another session's
uncommitted Rust, so the move it enables is *ask*, not *fix* — and the output says so in
those words. Naming the holder is also not naming the culprit: the text says it names who
WROTE the file, never who broke the build.
## Tests added

**39 cases** in `tests/attribute-red.sh`, **10 of 10** production mutations killed — including
both halves of the window (floor never derived; comparison inverted), the self-vs-peer split,
the dirty intersection, the diagnostic-path anchors widened to a bare token, and deletion of
the scope footer. One mutation had to be re-run: replacing the footer with invalid Python
failed **27** assertions including stage-1 ones the footer cannot reach, which is a crashed
interpreter reading as a kill. The count is the tell; a real deletion killed **2**.

**10 Rust cases** across `src/tools/run_command/attribution.rs` and `tests.rs`. **Four exist
because a mutation survived**, and all four are one shape — an assertion satisfied by
something other than the thing it names:

| survivor | why the assertion could not see it |
|---|---|
| both `wip_authors` attachment sites | every other case called `wip_author_diagnostic` or `format_run_command` **directly**; nothing traversed `handle_successful_output`, so the wiring was the un-wired-function shape with a green suite over it |
| …and then survived **again** | the replacement cases carried a `let Some(who) = … else { skip }` tolerance for a missing `python3` — satisfied by exactly the state a deleted attachment produces. The environment check is now a separate observation taken first |
| the opt-out | asserted `is_none()` against a red naming a **clean** path, so stage 2 produced the silence and deleting the opt-out changed nothing |
| `materialize` | passed against files a **previous run** had left in the content-hashed temp dir; `materialize_into` now takes its base as a parameter |

The second row is the one worth carrying forward: a skip-guard written for a genuine
environmental gap made the test **monotone under the deletion it existed to catch**, and it
was added *while fixing* the first survivor.

Not covered, stated rather than implied: write-then-rename **atomicity** has no assertion —
replacing the rename with a direct write leaves no stray `.tmp`, so the existing check cannot
see it. The case for it is a concurrent-reader race a test cannot schedule.
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
