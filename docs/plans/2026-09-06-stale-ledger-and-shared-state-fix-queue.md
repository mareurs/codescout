---
kind: plan
status: active
opened: 2026-09-06
owner: marius
tags: []
---

# Stale-ledger and shared-state fix queue

> Derived 2026-09-06 from a severity-ranked backlog triage that **failed in an instructive way**.
> Of the 13 `high` rows live in `docs/issues/` at 14:00, four were already fixed and none of them
> knew it. Every task below was chosen only after its mechanism was checked against the code or
> the running system — because on this ledger, `severity:` is the author's estimate *at filing
> time* and nothing updates it.

## Status

| # | task | severity | verified | state |
|---|---|---|---|---|
| 1 | a doctor check for bug files whose fix already shipped | — | 4 instances in one day | **done — `7790f343`** |
| 2 | audit host identity travels with a transported catalog | high | reproduced live, still worsening | **done — `d4f0bafb`** |
| 3 | the `pre-commit` stash window on a shared checkout | high | config present, Fix says nothing attempted | not started |

**Not tasks, and why** — at the bottom.

## Provenance — what the triage actually found

The four stale rows, all archived 2026-09-06 at `9e20d3ad` and `9c1709b0`:

| bug | filed | fixed at | gap |
|---|---|---|---|
| qdrant artifact-grain unreachable | 2026-09-03 | `6f032dbd` | 3 days |
| chunk line ranges body-relative | 2026-09-02 | `36afd405` | 4 days |
| `doc(update)` freezes chunks | 2026-09-04 | `fdad1a99` | **same day** |
| indexer stamps content seen before it embeds | 2026-09-02 | partly `fdad1a99` | frontmatter already retracted its own headline number |

Two distinct failure shapes, and they need different remedies:

- **Fix landed, file never updated** (rows 1, 2). The fix commit did not name the bug file; the
  bug file recorded no SHA. Nothing connects them, so nothing can notice.
- **Fix landed, file's *mechanism* still verifies, only its *prognosis* is wrong** (row 3). Every
  sentence about `update.rs` checks out today. Re-reading the file *confirms* it. Only running
  the sequence refutes it. This is the harder shape and no static check will ever catch it —
  the repo's existing rule is what catches it: *run the reproduction before reading the fix plan.*

Task 1 addresses the first shape only, and should say so at the refusal site.

---

## 1 — a doctor check for bug files whose fix already shipped

**Why first.** It is the only task here that pays back the cost of this triage. Four instances
in one day is well past this repo's own promotion threshold, and the alternative — "read more
carefully next time" — is the wrong instrument by CLAUDE.md § *Observer Blindness*: the party
who would notice is the one who already believes the file.

**The signal, and why it is the right one.** `doctor` already has
`non_terminal_status_with_fix_anchor`. It fires on an open bug that **records** a fix anchor,
so for all four of today's rows it read `0` — a true answer to a question nobody had asked.
The signal that *did* exist was in the source: every one of these fixes left a doc comment
citing its bug file **by path**:

| bug | citing site |
|---|---|
| qdrant artifact-grain | `artifact_store.rs:132`, `retrieval/artifact.rs:99`, `catalog/find.rs:306` |
| chunk line ranges | `catalog/chunk.rs:60`, `entry_token.rs:132` |

**A live bug file cited from a source doc comment is a cheap tell that its fix already
shipped**, and it is currently wired to nothing.

**Two false-positive classes to design against** — both observed this session, both raised by
`codescout-ae` (sessionId `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`), who has explicitly ceded
this task:

1. **A fix in flight cites its still-open bug file.** `src/librarian/reindex_progress.rs` cited a
   live bug file in its module docs for a whole session before that bug was archived. A naive
   check fires on the working tree of whoever is doing the very thing it exists to reward.
2. **Doc comments cite bug files as RATIONALE, not as "I fixed this"** — *"this is why X is
   shaped this way, see `<bug>`"*. The path alone cannot separate those two.

Both point at the same shape: **fire only when the bug is still open N days after the citing
commit landed**, and report as a **worklist**, not a verdict. That is `doctor`'s stated posture
anyway, and it makes (1) impossible by construction and (2) merely noisy rather than wrong.

**Sketch.** New check beside the existing catalog-drift ones in `src/librarian/tools/doctor.rs`.
For each artifact with `kind: bug` and a non-terminal status, find source files citing its
`rel_path`; for each, take the citing commit's date via `git log -1 --format=%ct -- <file>`;
report when `now - commit_date > N days`. Testable against a seeded catalog plus a stub for the
commit date — the git call is the only impure part and should be behind a parameter for exactly
that reason.

**Done when.** The check exists, is registered in `doctor`'s report, has a test that RED-s on a
seeded stale pair and stays silent on a fresh one, and `docs/PROBES.md` gains its row naming the
blind spot (it cannot see shape 2 above — a fix that never cites its bug file).

### Done — `7790f343`

Shipped as `open_bug_cited_from_source`, with 7 tests. Gate green on all four commands.

**The mutation run is the part worth reading.** Four guards, three killed cleanly, and the
fourth **survived**: the archive-path filter's test passed with the filter disabled, because
the fixture seeded `status: fixed` and the SQL status predicate excluded the row before the
path filter was ever consulted. The test was named for the filter and measured the predicate.
Six green tests hid it. The filter is not dead code — a bug whose *status* is live while its
*file* already sits under `archive/` is ordinary catalog/disk drift and one existed in this
catalog the same afternoon — so the remedy was a second test that isolates it
(`an_open_status_bug_under_archive_is_silent`), which REDs under the same mutation. **A
mutation that dies tells you a test is live; a mutation that survives tells you either the
code is dead or the test is aimed wrong, and only investigating separates them.**

**First real run:** 83 live bugs, 494 source files scanned, 14 cited from source, 13
suppressed as unsettled, **1 reported** — and it was the subtle case, not the motivating one.
`2026-08-26-wine-lane-flakes` is legitimately still open (an unreproduced flake), but its
*Resume* prescribed instrumentation the citing commit had already shipped, at a line range
that had since drifted. Corrected in the same commit.

**Two things this does NOT do**, so nobody credits it with them: it cannot see a fix that
never cited its bug file (staleness shape 1 with no citation), and it cannot see a citation
wrapped across two lines by a formatter. Both are stated in
`catalog_health.open_bug_source_citations` rather than only in the source, because the reader
of a `0` is not the reader of a doc comment.

## 2 — audit host identity travels with a transported catalog

`docs/issues/archive/2026-09-04-a-transported-catalog-carries-its-host-identity.md` · `high` ·
`cluster/authorship-unrecoverable-after-the-fact`

**Verified live 2026-09-06, and still worsening.** `resolve_host_id`
(`src/librarian/catalog/audit/host.rs`) persists the audit host id in **`catalog_meta`** and
mints one only when that key is absent. `~/.local/share/librarian/catalog.db` was replaced
wholesale on 2026-09-04, so this machine adopted the sender's id. `candidate_name()` on this
host yields `archlinux`, so a fresh mint could never be `ripper-65e654` — the id arrived with
the bytes.

The consequence is in a **git-tracked, `merge=union`** file:
`.codescout/audit/ripper-65e654-202609.jsonl`. Every row is stamped `"host": "ripper-65e654"`,
including rows whose `actor` is a session on this laptop. **Two of my own reindexes this session
appended to it** — 5,568 rows and then 16 more — under a host id that is not this host. That
file has been the top line of `git status` all session.

`shard_file_name`'s own doc comment states the invariant being lost: *"One file per host per
month: month bounds the file size, and host keeps two machines off each other's lines entirely."*

**Shape of the fix, to be decided at the file.** The id must be derived from the *machine*, not
carried in the *catalog* — or the catalog must record which machine minted it so a mismatch is
detectable on open. The second is strictly more informative and is what makes the already-mixed
shard triable rather than just stopping the bleeding.

**Done when.** A transported catalog does not adopt the sender's identity; a regression test
covers the transport case (seed `catalog_meta` with a foreign id, assert the resolved id is not
it); and the disposition of the already-contaminated shard is recorded — repaired, partitioned,
or explicitly accepted.

## 3 — the `pre-commit` stash window on a shared checkout

`docs/issues/2026-09-03-pre-commit-stash-window-feeds-peers-wrong-bytes-or-enoent.md` (`high`)
and `docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md` — one
mechanism, two victim classes, deliberately kept as separate files with a shared
`cluster/transient-shared-state-lies-to-readers` tag.

`pre-commit` stashes **all** unstaged work repo-wide before running hooks. This checkout had
**6 live sessions across 2 profiles at 14:11**, three of them in this tree, so the window opens
several times an hour and the tree holds nobody's intent while it is open.

**Why last despite the highest per-incident cost.** The Fix section is explicit that nothing has
been attempted and none of the directions are free — stop stashing, or advertise the window with
a marker file peers can check. It is a deployment decision, not a contained code change. A
per-session workaround exists and works: `pre-commit` stashes *unstaged* changes, so staging
protects in-flight work. I used it deliberately this session.

**Read the file's own warning before proposing a rule.** It records a remedy
("staleness before deletion") that was derived while a since-retracted mechanism stood, sounded
right, and is meaningless under the true one.

### 3a — the mitigation is currently politeness, and that is the actual defect

Corrected 2026-09-06 by sessionId `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`, retracting a narrower
claim we had both been working from. We had said *"on this tree a mutation window and any peer
commit are mutually exclusive"*. That is accurate and **too narrow**: the stash is
**unconditional**. It fires on every commit by every session, whether or not anyone is
measuring. Nothing about a mutation window makes it special except that the mutating session
happens to be harmed.

What made 2026-09-06 safe was one session messaging another that it was idle, and the other
one holding its gate and its commit until told otherwise. **That is a message, not a
mechanism**, and it worked because two sessions were being careful at each other for twenty
minutes.

This is CLAUDE.md § *Observer Blindness*, third position, almost verbatim: *"a trigger the
model must notice is a policy, not a mechanism"*, and the preferred shape is *"making the
correct path end in a safe state, so compliance leaves nothing armed."* Neither exists here.
A session that follows every documented rule — enumerate peers, stage early, commit by
pathspec — still silently reverts every other session's unstaged work for the length of its
hook run.

**And the blindness has its ROLES SWAPPED relative to every other instance in this corpus,
which is the actionable part.** The usual shape is *the author cannot see it*. Here a second
party can see it perfectly, in real time:

```
[INFO] Stashing unstaged files to /home/marius/.cache/pre-commit/patch<ts>-<pid>
[INFO] Restored changes from /home/marius/.cache/pre-commit/patch<ts>-<pid>
```

Those two lines print in the **committer's** terminal and nowhere in the affected session's.
So the party who *could* report it has no reason to — nothing is wrong from where they sit —
and the party it happens to cannot see it at all. That is why "be careful" fails in both
directions at once, and it is also the cheapest available mitigation short of a real fix: a
committer who knows what those lines mean can say so, and today one did. Derivation:
sessionId `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`.

The item above was itself filed through a commit that stashed that same session's seven
uncommitted files — evidence better than the prose, and the reason this paragraph exists.

### The one part that IS a rule, and can be followed before any fix lands

The hazard became visible on 2026-09-06 for one reason: a session **announced a work window**,
and the peer checked what its next action would do to it. Stated plainly by that peer
(`cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`) and worth recording in their terms: they did not
deduce the interaction, they read the announcement and computed against it. Without it they
would have committed, the mutation would have survived, and the likely outcome is a working
guard deleted with neither session ever knowing.

**The obvious lesson — "sessions should announce more" — is wrong in the usual way.** More
announcements is noise, and noise is what makes the next real one skippable. The property that
paid was **precision about STATE, not frequency**:

| announcement | what a peer can compute from it |
|---|---|
| "I'm working in `doctor.rs`" | nothing — no interaction is derivable |
| "uncommitted single-line mutations in one function, ~10 min, only `doctor::tests` affected, build stays green" | that a commit would stash it, that the stash would invert the measurement, and that holding costs ~10 minutes |

So the announcement must carry **file, committed-or-not, blast radius, and duration**. The
middle one is the load-bearing field and the one most likely to be dropped, because it is the
only one that is not about *your* work — it is about what someone else's routine action would
do to it.

This does **not** soften the politeness point above: the mitigation is still not a mechanism,
and nothing makes anyone announce. But the gap between an announcement that is merely polite
and one that is useful lives entirely in what the message contains, and that half *is*
reducible to a rule someone can follow today.

**So the fix to aim at is not "warn harder".** Candidates, none costed yet:

- **Make the correct path safe.** If `pre-commit` can be configured to stash nothing (several
  hooks here already read the index directly via `git show :<path>`), the hazard disappears
  rather than being scheduled around.
- **Make the window observable.** A marker file written for the duration of the hook run, of
  the same shape as `git status --porcelain`, so a peer about to measure can *check* rather
  than *be told*. This is the weaker option — it converts a silent hazard into a precondition,
  which still requires the other party to look.

Ranked below task 3's main body only because it shares its fix space; if the first candidate
lands, both are closed at once.


### 3b — a gate result on this tree is worthless without a dirty-file check, and that is now a precondition

Distinct from 3a: **no stash is involved**. This is the plain case — peers have uncommitted
`.rs` edits in the tree at the moment you run the gate, so your green (or your red) describes
a tree that is nobody's intent, including yours.

Measured twice on 2026-09-06, and the second is much worse than the first:

| | what happened | why it was survivable |
|---|---|---|
| ~15:40 | a peer's half-written test stub in `mv.rs` failed to **compile** | loud, obviously foreign, diagnosable from the error text alone |
| ~19:5x | two peers' in-flight edits produced **two passing-looking test failures** in `librarian::catalog::chunk` and `librarian::tools::append_entry` | nothing about the output said "not yours" |

The second one is the dangerous shape. The tree **compiled**. The failures were plausible,
in the librarian, in the same subsystem as the change under test — a host-identity fix in
`audit/host.rs`. The natural reading of `DEFAULT exit=101` with a chunk-reuse assertion in it
is *"my change broke chunk reuse"*. It could not have: `resolve_host_id`'s only callers are
`shard::export`, `tools/audit_log` and `tools/doctor`, none of which those tests reach. But
that is an **argument**, constructed after the fact, and the failure was sitting there looking
like evidence for the opposite.

**So the rule is promoted from "worth doing" to a precondition of reporting a gate result at
all:**

```
git status --short -- '*.rs'      # before citing ANY gate run on this checkout
```

Non-empty means the run describes a tree that includes work you did not write. That does not
make the result useless — it makes it a result **about a different tree**, which has to be
said out loud when reporting it.

Two things that make the check sufficient rather than merely advisable:

- **Attribute positively, never by adjacency.** `scripts/file-provenance.py <paths>` returned
  the sessionIds for both files here (`cda3afe5` for `append_entry.rs`, `4a2f34f7` for
  `chunk.rs`) — one of them a session this one had never spoken to. Guessing from subject-area
  proximity would have named the wrong party, which is the failure mode already recorded
  earlier in this same session.
- **Read your own test names out of the lane, not the totals.** `8744 passed, 2 failed` is
  compatible with your change being the cause and with it being untouched. `17 passed, 0
  failed` over `librarian::catalog::audit::host::tests` is the sentence that settles it.

The companion tell for the 3a case is different and both are needed: a stash reverts work so
`git status` is **clean** during the window. The bracket that catches that one is capturing
`git rev-parse HEAD` and the file's mtime before and after the run and refusing to trust a
result where either moved — used successfully for this task's mutation runs, and cheaper than
the message-passing it replaces.

#### The precondition is necessary and NOT sufficient — a peer's red is often the healthy state

Contributed by `codescout-7f` (sessionId `4a2f34f7-0669-487d-9ce9-39b77881642f`), whose own
work is the case that proves it.

`git status --short -- '*.rs'` tells you a peer is **present**. It does not tell you what their
red *means*. Under test-first, **a failing test is the expected state of a healthy in-progress
change** — so "peer file dirty **and** peer test red" is not evidence of breakage, and treating
it as such is its own error. Both failures in this task's gate run were misread on first pass:
one was a peer's intermediate fixture state, the other was a deliberate TDD red, and both were
reported as failures by sessions sampling someone else's correct work mid-stride.

**The discriminator is cheap and decisive** — ask whether the failing test exists at `HEAD` at
all:

```
git show HEAD:<file> | grep -c 'fn <failing_test_name>'
```

`0` means the test is **born-red**: it does not exist in committed history, its first run ever
is the failure you are looking at, and it cannot be a regression of anything. That settled
`a_chunk_that_only_moved_ordinal_keeps_its_id_and_its_vector` here in one line, where an hour
of reasoning about call graphs had only produced an argument.

So the full sequence before citing a gate result on this tree is three steps, not one: dirty
check, positive attribution, then **read the failing test's name and ask whether it exists at
`HEAD`**. Only the third distinguishes *someone is working* from *something is wrong*.
## Not in this queue, and why

- **`experiments` CI has been red for 4 days** (last green 2026-09-02 06:45), three independent
  causes, **no owner**. Reported by `codescout-ae`: `Audit Doc Refs` is now fixed by their push;
  still red are `a_p50_session_stays_under_the_committed_emission_byte_ceiling` (its `doc(move)`
  fails returning `list_collections(artifact)` — a Qdrant path) and two `append_entry`
  upstream-guard tests on macOS/Windows, with windows-gnu exiting 100 at build stage. This is
  **not in the bug ledger**, is arguably more urgent than anything above, and is excluded here
  only because it is not what the triage was asked to rank. Someone should own it.
- **The `doc(update)` STAMP-entrance regression test.** Named in
  `docs/issues/archive/2026-09-04-doc-update-stamps-the-content-hash-without-rebuilding-chunks.md`
  § *Tests added*. Ceded to `codescout-ae`, who proposed a ~10-line source-level pin asserting
  `update.rs` never writes `embedded_sha256`. Not duplicated here so we do not both write it.
- **The orphaned `artifacts` Qdrant collection** (2425 points,
  `docs/issues/2026-09-06-the-legacy-artifacts-qdrant-collection-is-orphaned-not-cleaned-up.md`).
  Filed `low` and it is genuinely low: no query path reads it, so it costs disk and nothing else.
  Its second half — a `doctor` check for collections outside the current prefix — is a better
  candidate once task 1's `doctor` work is in hand.
