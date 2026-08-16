---
id: '9a892c2a5976e296'
kind: tracker
status: active
title: Open-Issue Work Queue (BL-N)
owners:
- marius
tags:
- backlog
- sequencing
- bugs
- work-queue
topic: work-queue
---

> **Prefix:** `BL-N` — a row in this queue. Work-stream-scoped, defined here, not a project-wide
> namespace (`docs/TAXONOMY.md` § Work-stream-specific prefixes). Deliberately **not** `T-N`, which
> belongs to `docs/trackers/tool-usage-patterns.md`.

## What this is, and what it is not

A **sequencing layer** over the open bug ledger, snapshotted 2026-08-16 from
`artifact(action="find", kind="bug", filter={"status": {"in": ["open", "investigating"]}})` — 17 rows.

It exists because the ledger answers *what is broken* but not *what to pick up next*. A flat
`status="open"` query cannot express readiness, blockers, or the fact that two entries need the same
decision made once. That is all this file adds.

**It does not own bug status.** Every row points at a bug file, and that file is authoritative. If a
row says `open` and the bug says `fixed`, the bug is right and the row is stale. Never close a bug
from here — and never treat the one-line `next` as the instruction. It is a pointer to that bug's
`## Resume`, which carries the real next action along with the caveats.

## Queue — rendered snapshot (2026-08-16)

> **`params` is the source of truth; this table is a snapshot of it.** Params live in the librarian
> catalog (`~/.local/share/librarian/catalog.db`), which is **not** in the repo — so without this
> section the queue would be invisible to git and to any other checkout. Re-render it when rows
> change. Query the live rows with
> `artifact(get, id="9a892c2a5976e296", entry_filter={"status":{"eq":"open"}})`.

| ID | Ph | Task | Status | Bug |
|----|---:|------|--------|-----|
| BL-1 | 1 | json_path: add a `Segment::Wildcard` arm so the overflow hint's own recovery works | **done** | `875e5d03d980ceac` |
| BL-2 | 1 | grep: stop printing a self-refuting "Showing N of N" when collection hit the cap | open | `4059035cf39e6aab` |
| BL-3 | 1 | Tool schemas: stop advertising conditionally-required params as optional | open | `365b599f3573b1c0` |
| BL-4 | 1 | usage.db: derive the backfill gate from the taxonomy, not a hand-maintained integer | open | `dbebda84901961c0` |
| BL-5 | 1 | librarian: split `tracker_design` so its guidance arrives inline | **done** | `3f88d49c38ced0c1` |
| BL-6 | 1 | read_file: give the buffered full-read summary an incompleteness signal | open | `a9644b964edac789` |
| BL-7 | 1 | Write-scope denial should name `approve_write` | open | `0a15c81150c4cce7` |
| BL-8 | 2 | `truncate_compact` cuts from the tail, destroying the overflow signal | open | `c320b6564d1cb003` |
| BL-9 | 2 | `server_instructions` arrives truncated mid-word, dropping the guide pointers | open | `f366e93249f7babd` |
| BL-10 | 2 | `audit_doc_refs` reads bare comment markers as file paths | open | `772fff5739620581` |
| BL-11 | 2 | `context`/`workspace_state_at` never dedup the worktree overlay | open | `b5080e6c7a73ab44` |
| BL-12 | 2 | worktree divergence guard covers writes but not reads | open | `320b97eb87548663` |
| BL-13 | 3 | IL1: run subtract-and-measure on the step-3 wording | blocked | `ab0b30dc9053aa6c` |
| BL-14 | 3 | read_file: `force=true` silently discarded on whole-file reads | blocked | `ce1447504150b25b` |
| BL-15 | 3 | Read-only metadata commands (wc/ls/stat) blocked on source paths | blocked | `30365fe50974fa6b` |
| BL-16 | 3 | Worktree activation diverges memory set and sub-project topology | blocked | `403e3fad0356f171` |
| BL-17 | 4 | Reconcile a bug sitting in `archive/` while still marked `status: open` | open | `897fb0fbd6eb2546` |
| BL-18 | 1 | `artifact(create)`: `augment` silently discarded five of its seven fields | **done** | `29f1ddf259562b7f` |
| BL-19 | 1 | Overflow envelopes with no compact summary waste a whole call | open | `e557d0f2c9429b5d` |
| BL-20 | 1 | params merge-patch wipes entry arrays wholesale — gave entries an update path (`update_entry`) + always-on counts | **done, archived** | `36eda0c2634dbea9` |
| BL-21 | 1 | edit_file's replace_all + batch paths write librarian-managed artifacts with no guard | **done, archived** | `e52abced30ff1dbc` |
| BL-22 | 1 | `move` broke the `id == hash(abs_path)` invariant, so the next reindex cascade-deleted the history | **done, archived** | `18a637f59289192c` |
| BL-23 | 3 | a moved artifact's frontmatter still asserts its pre-move id | open | `6149f4cfeaa6fab9` |
| BL-24 | 2 | usage.db records a sha that need not describe the built code, and drops the dirty bit | open | `a68a76301714137f` |
| BL-25 | 1 | the 2200-byte cap evicts rules into `get_guide` topics nothing triggers — 7 of 10 guides (~46 KB) have no trigger at all | open | `cfcbee6f7d047a55` |
| BL-26 | 2 | `get_guide("librarian-runtime")` says a move preserves the id; a move mints a new one — 2d8c7f39 repaired 3 of 4 copies | open | `db02045fdbaaf860` |
| BL-27 | 3 | `update_entry`'s entry-param guard only fires when `fields` is absent; send both and `entry` is dropped silently | open | `ea21099f9d39f734` |
| BL-28 | 3 | a directory named `--help` holding an initialised codescout project sits untracked in the repo root | open | `ffa936075f1f03fd` |
| BL-29 | 1 | `append_entry` writes catalog-only state, so this very snapshot drifts silently — tool says success, git says clean | open | `0694a4a9946e10fe` |
| BL-30 | 2 | FRICTION: adding one entry costs four bookkeeping sub-tasks — id, workflow, row format, re-render | open | `63d36f5da3b200a7` |

> **Params and body are reconciled** (2026-08-16). BL-1, BL-20 and BL-22 were
> flipped with `artifact(action="update_entry", …)` — 24 rows before, 24 after,
> three rows changed. The note that used to sit here said the flip was unsafe
> because there was no entry-grain update; that was BL-20, and it is now fixed.
> Its own row was the first thing the fix was used on.

Next actions per row live in each bug's `## Resume`, and in the live params — not duplicated here,
because a snapshot that carries instructions goes stale in the way that matters most.
## Phase descriptions

Phases encode **readiness, not importance.** A phase-3 item may matter far more than a phase-1 one;
it simply cannot be started by an agent alone.

### Phase 1 — Ready

The mechanism has been read at the bytes and the edit site is named. An agent can open the bug, go to
the cited line, and work. Eight rows.

Worth noting what makes these ready: each names a `path:line`. That is the difference between a bug
someone can pick up and a bug someone must first re-investigate — and it is why the bug template asks
for `path:line` on every root-cause claim.

### Phase 2 — Investigate first

The defect is real but the mechanism is **inferred** rather than measured, or the emission site has
not been located. Acting directly here means acting on an unverified premise, which this repo has
been bitten by: of five bugs worked on 2026-08-07, all five had a false premise or a wrong
prescription (W-13, `docs/trackers/release-promotion-session-log.md`).

BL-11 is the clearest case — its root cause is explicitly marked inferred, and its own Resume asks
for a worktree reproduction before any fix.

### Phase 3 — Blocked

Gated on something an agent should not decide alone:

- **BL-14, BL-15, BL-16** each present mutually-exclusive options. These are cheap to unblock — each
  needs one answer, not a discussion — and BL-15's answer may be `wontfix`, which is a legitimate
  outcome, not a failure.
- **BL-13** is gated on an external eval run (`../prompt-engineering/`), not a preference. Steps 1
  and 2 of that bug are already shipped and verified live; only the prompt wording awaits
  subtract-and-measure, which per `src/prompts/README.md` governs whether *any* prompt-surface change
  ships.

### Phase 4 — Ledger hygiene

BL-17: one bug sits at `docs/issues/archive/…` while its frontmatter still says `status: open`. It
was fixed (`43fac6c8`) and moved, but the status flip was missed — so it appears in every
"what's open?" query while being physically archived. Exactly the drift the archive-through-the-catalog
rule exists to prevent.

## Sequencing notes

Two clusters are worth taking together rather than one at a time:

- **The overflow/handle cluster** — BL-1, BL-2, BL-6, BL-8 all concern a result that was cut and
  whether the caller can tell. They share a root shape: *a truncated payload that reads as complete.*
  The `grep` byte-budget fix (archived 2026-08-16) is the first of this family and its
  `… [truncated: N of M bytes shown]` marker is the pattern the rest should match. Fixing them as a
  set gives one consistent signal rather than four dialects.
- **The worktree cluster** — BL-11, BL-12, BL-16. BL-16 needs a decision that likely constrains
  BL-12's design, so answer BL-16 first even though BL-12 is nominally less blocked.

BL-3 and BL-1 carry the strongest measured evidence: `missing_required_param` is the largest
non-routing error family (38 hits / 20 sessions) and `json_path_key_miss` is 27 hits / 17 sessions,
both from the 2026-08-15 tool-usage investigation. If picking by impact rather than readiness, start
there.

## History

### 2026-08-16 — opened

Snapshotted 17 open bugs into BL-1..BL-17 with per-row next actions taken from each bug's `## Resume`
rather than invented. Phase assignment reflects readiness as of this date.

Context: this queue was created at the end of a session that fixed three bugs
(`grep` byte budget — archived; IL1 steps 1-2 — verified live; plus the IL1 prompt wording) and filed
three new ones. The remaining 17 are what was left standing.

### 2026-08-16 — BL-18 added, found by building this file

Creating this tracker surfaced its own bug. `artifact(create, augment={…})` accepts only `prompt`
and `params`; the `render_template`, `params_schema` and `entry_collection` passed alongside them
were silently discarded, and the call still returned success. Both had to be re-applied with a
follow-up `artifact_augment(merge=true)`.

Filed as `29f1ddf259562b7f` and queued as BL-18. It is a recurrence of a class already fixed once in
the same file (`artifact(create)` dropping `topic`, archived 2026-07-13), and it is compounded by
`tracker_design`'s own Final step listing `params_schema` and `render_template` among the fields to
pass to `create` — guidance followed exactly here, with both fields lost.

Worth noting for whoever works the queue: **BL-18 was found by using the tooling, not by reading
it.** Three of this session's bugs came the same way. A queue built by hand is also a probe.

### 2026-08-16 — BL-5 and BL-18 fixed together

Taken as a pair because both edit `tracker_design`'s `SYSTEM_PROMPT`: BL-5 had to shrink it, BL-18
had to correct its Final step. Doing them in one pass avoided touching the same 100-line constant
twice.

**BL-5** — `tracker_design` went from **~41,000 to 9,358 bytes**, from overflowing on 6 of 6 calls to
arriving inline. The split (menu inline, one archetype per named fetch) was the planned half; the
unplanned half was `existing_trackers`, which at a cap of 30 with six fields per row was ~7 KB —
larger than the entire archetype menu. Capped at 5 rows of `{id, title, kind}`, with Step 7 rewritten
to send the caller to a semantic `artifact(find)` for the collision check a title scan cannot do.

**BL-18** — `AugmentSpec` widened from 2 fields to all 7 and gained `deny_unknown_fields`, so
`create` both accepts the full augmentation shape and rejects typos instead of discarding them. The
advertised schema and `tracker_design`'s Final step now say the same thing the code does.

One lesson worth carrying: **BL-5's first regression test was wrong in a way that would have shipped
the bug.** Written against an empty catalog it read 10,396 bytes; the same code against a full
catalog read 17,456. `existing_trackers` is empty in a bare fixture and populated in production, so
the test would have gone green while every real call still overflowed. A size assertion has to be
made against the shape that ships — the same *wrong population* error TU-5 was corrected for.

### 2026-08-16 — BL-1 fixed; BL-19 filed; the queue was not actually in git

**BL-1** — `[*]` now parses and projects, the recovery hint is derived from the payload's shape
instead of the constant `$.field`, and both rejection hints plus
`get_guide("progressive-disclosure")` advertise the grammar. Verified live: the exact call that used
to be rejected, `read_file("@tool_…", json_path="$.augmentation.params.tasks[*].id")`, returned all
18 BL ids from a buffered handle.

**BL-19** — filed from a complaint about the fix's own output. The hint is now correct, but the
*envelope* still costs a whole call to return nothing: `artifact(get)` answers with `output_id`, a
byte count and a hint, and nothing about the artifact. The librarian adapter's `format_compact` has
exactly one case — a body-truncation warning — so every other response falls through to the generic
"Result stored in …". Fixing the hint makes the second call land; it does not make it unnecessary.

**And this file was not what it appeared to be.** The BL rows are `params`, and params live in the
librarian catalog under `~/.local/share/`, **not in the repo**. The markdown carried frontmatter and
prose only — so the queue existed on this machine and nowhere else, which is the opposite of why a
tracker was chosen over Claude Code's per-profile memory in the first place. The rendered snapshot
above fixes that. Worth knowing when creating any augmented tracker: writing a good body does not
make its live state durable, and the file will not look wrong.

**BL-20** — filed from an own goal committed while writing the line above. Flipping BL-1 to `done`
via `patch={params:{tasks:[one row]}}` deleted BL-2..BL-19: merge-patch replaces arrays wholesale,
and the call answered `updated: true`. The rows survived only because the snapshot had been written
minutes earlier, for an unrelated reason.

That near-miss is the argument for the snapshot, independent of git: **params have no shrink guard,
no `force` gate, no report of what a write destroyed, and no version control** — all four of which
the body surface has. The less-protected surface is the one with no backup. Keep a rendered table in
the body of every entry-bearing tracker.


### 2026-08-16 — six fixed and archived; the queue's own tooling was most of the work

**Shipped, all on `experiments`, all fast-forward (so each SHA *is* the master SHA):**

| BL | what | SHA |
|---|---|---|
| BL-1 | `[*]` projection + payload-derived hint (+ depth walk) | `7c91cdf7`, `336d3b04` |
| BL-20 | `update_entry` — entry-grain patching; always-on entry counts | `02a87a83` |
| BL-21 | librarian guard hoisted into `read_edit_target` (all 3 write paths) | `47abcb6d` |
| BL-22 | `move` grafts history onto the new id instead of stranding it | `2d8c7f39` |
| BL-26 | `librarian-runtime` guide corrected + guard test over every guide | `6018b7ad` |
| BL-27 | `entry`-param guard fires whenever `entry` is present | `6018b7ad` |

Plus `61ab520a`: **14 `fixed`-but-unarchived bug files archived**, and three deliberately
left open because their Resumes describe real undone work —
`audit-doc-refs-gate-hides-its-own-cause`, `edit-code-remove-ast-repair-over-deletes`,
`workspace-toml-mis-rooted`. Those three were indistinguishable from the other fourteen by
status, path or age; only reading the Resumes separated them.

**BL-22 is the one to understand before touching the catalog.** `move` used to preserve an
artifact's id while changing its path, which breaks the invariant `doctor.rs` states twice
(`id == artifact_id_from_abs(abs_path)`). The next `reindex` then re-keyed the row and
`upsert`'s abs_path pre-clean cascade-deleted its events. Measured: one reindex took the
catalog from **1845 to 1834 events while reporting `removed: 0`**, and archived bug files
carried 0.02 events/row against 0.65 for live ones. Fixing it is what made the 14-file
sweep above safe to run at all.

**Two guards were written to the reproduction rather than the condition**, hours apart:
`edit_file`'s covered 1 write path of 3, and `update_entry`'s read
`entry.is_some() && fields.is_none()` so sending both dropped `entry` again. Both were
caught by the other session. The test shape that catches this class is a table containing a
row that was **green before the fix** — it proves the table discriminates rather than
refusing everything.

### Resume — state at compaction, 2026-08-16

**18 of 30 rows open.** Phase 1 remaining: BL-2, BL-3, BL-4, BL-6, BL-7, BL-19, BL-25.
BL-29/BL-30 are the other session's.

**BL-2 has fresh evidence and is the natural next.** It fired live this session and produced
a **false claim in a committed artifact**: `grep(…, limit=12)` answered *"Showing 12 of 12
matches"*, which is byte-identical to what a complete result prints, and the homogeneous
capped sample was written up as a finding. Corrected in
`docs/issues/archive/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md`
§ *And a second one, which was wrong*; the evidence and a design note are in BL-2's own
file (`4059035cf39e6aab`). Note the design constraint recorded there: a truthful denominator
is **not** available — `cap_grouped` never counts past the cap — so the fix is an explicit
incompleteness marker, not "12 of 847".

**Working practices this session settled, worth keeping:**

- **Verify a fix is live by invoking it**, never by inspecting the binary or `codescout_sha`.
  Three signals disagreed within one minute; only the tool call interrogates the process
  serving the call. (T-20; the dirty-build half is BL-24.)
- **Archive with `artifact(action="move")` and read `id_changed`** — the id changes, so
  re-point prose citing the old one in the same commit, ids as well as paths.
- **Patch tracker rows with `update_entry`**, never `patch={params:…}` — the latter replaces
  the collection.
- **A concurrent session shares this working tree and index.** Commit by pathspec; check
  `git status` before staging. Their surfaces right now: `reconnaissance-patterns.md`
  (R-N — leave alone), `archive-cadence-policy.md`, the benchmark trackers.
