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
| BL-2 | 1 | grep: stop printing a self-refuting "Showing N of N" when collection hit the cap | **done, archived** | `8e665c2d041ebb04` |
| BL-3 | 1 | Tool schemas: stop advertising conditionally-required params as optional | **done, archived** | `02d2d9d8a7eeec2e` |
| BL-4 | 1 | usage.db: derive the backfill gate from the taxonomy, not a hand-maintained integer | open | `dbebda84901961c0` |
| BL-5 | 1 | librarian: split `tracker_design` so its guidance arrives inline | **done** | `3f88d49c38ced0c1` |
| BL-6 | 1 | read_file: give the buffered full-read summary an incompleteness signal | open | `a9644b964edac789` |
| BL-7 | 1 | Write-scope denial should name `approve_write` | open | `0a15c81150c4cce7` |
| BL-8 | 2 | `truncate_compact` cuts from the tail, destroying the overflow signal | open | `c320b6564d1cb003` |
| BL-9 | 2 | `server_instructions` arrives truncated mid-word, dropping the guide pointers | open | `f366e93249f7babd` |
| BL-10 | 2 | `audit_doc_refs` reads bare comment markers as file paths | open | `772fff5739620581` |
| BL-11 | 2 | `context`/`workspace_state_at`/`link_scan` never dedup the worktree overlay | done | `d31233700ca979c2` |
| BL-12 | 2 | worktree divergence guard covers writes but not reads | done | `c611a3dce4f05d45` |
| BL-35 | 2 | `guard_worktree_write` is dead code in production (startup cwd sets the flag it gates on) | open | `1523556488a95de2` |
| BL-13 | 3 | IL1: run subtract-and-measure on the step-3 wording | blocked | `ab0b30dc9053aa6c` |
| BL-14 | 3 | read_file: `force=true` silently discarded on whole-file reads | blocked | `ce1447504150b25b` |
| BL-15 | 3 | Read-only metadata commands (wc/ls/stat) blocked on source paths | blocked | `30365fe50974fa6b` |
| BL-16 | 3 | Worktree activation diverges memory set and sub-project topology (option 2 shipped; 1-vs-3 open) | blocked | `403e3fad0356f171` |
| BL-17 | 4 | Reconcile a bug sitting in `archive/` while still marked `status: open` | open | `897fb0fbd6eb2546` |
| BL-18 | 1 | `artifact(create)`: `augment` silently discarded five of its seven fields | **done** | `29f1ddf259562b7f` |
| BL-19 | 1 | Overflow envelopes with no compact summary waste a whole call | open | `e557d0f2c9429b5d` |
| BL-20 | 1 | params merge-patch wipes entry arrays wholesale — gave entries an update path (`update_entry`) + always-on counts | **done, archived** | `36eda0c2634dbea9` |
| BL-21 | 1 | edit_file's replace_all + batch paths write librarian-managed artifacts with no guard | **done, archived** | `e52abced30ff1dbc` |
| BL-22 | 1 | `move` broke the `id == hash(abs_path)` invariant, so the next reindex cascade-deleted the history | **done, archived** | `18a637f59289192c` |
| BL-23 | 3 | a moved artifact's frontmatter still asserts its pre-move id | **done, archived** | `61e2360408cb206b` |
| BL-24 | 2 | usage.db records a sha that need not describe the built code, and drops the dirty bit | open | `a68a76301714137f` |
| BL-25 | 1 | the 2200-byte cap evicts rules into `get_guide` topics nothing triggers — 7 of 10 guides (~46 KB) have no trigger at all | open | `cfcbee6f7d047a55` |
| BL-26 | 2 | `get_guide("librarian-runtime")` says a move preserves the id; a move mints a new one — 2d8c7f39 repaired 3 of 4 copies | **done, archived** | `5d8584d109d876ea` |
| BL-27 | 3 | `update_entry`'s entry-param guard only fires when `fields` is absent; send both and `entry` is dropped silently | **done, archived** | `d082f963f57bd76b` |
| BL-28 | 3 | a directory named `--help` holding an initialised codescout project sits untracked in the repo root | open | `ffa936075f1f03fd` |
| BL-29 | 1 | `append_entry` writes catalog-only state, so this very snapshot drifts silently — tool says success, git says clean | partial (`99aaf83f` + `6ff00eee` + `0dbfd0ee`): drift reported at write time and by `doctor`; hamsa reconciled; gate now needs majority coverage; **0 trackers adrift** | `0694a4a9946e10fe` |
| BL-30 | 2 | FRICTION: adding one entry costs four bookkeeping sub-tasks — id, workflow, row format, re-render | open | `63d36f5da3b200a7` |
| BL-31 | 2 | grep: `cap_grouped`'s file-diversity round-robin is unreachable, so overflow hints name walk-order files not hot ones | open | `5f6cfe1acdfda38d` |
| BL-32 | 3 | R-N ledger reused nine ids for unrelated lessons — split by suffix in `52fca682`; the hand-allocation cause is BL-30 | open | `cdc375f4420aad6a` |
| BL-33 | 1 | the librarian guard keys on YAML quoting, so 15 of 27 trackers (incl. this queue) are unprotected | **done, archived** | `e7353641aafe0098` |
| BL-34 | 2 | repairing a frontmatter id re-serializes the whole block, reformatting hand-authored YAML | **done, archived** | `529a6c05895cc686` |
| BL-36 | 1 | `artifact(update)` re-serializes the whole frontmatter block on a single-field patch — BL-34's mechanism at the mandated archive step | **done, archived** | `82ba248228301486` |

> **Params and body reconciled again** (2026-08-16, second pass — 31 rows). The
> previous reconciliation held for status but not for **ids**: BL-26 and BL-27 were
> archived, and `artifact(action="move")` re-keys, so params carried the new ids while
> this snapshot still cited `db02045fdbaaf860` / `ea21099f9d39f734` — neither of which
> resolves any more. Three rows had drifted (BL-2, BL-26, BL-27) and BL-31 was missing
> entirely.
>
> A **duplicate BL-31 row** also had to be removed here: two sessions hand-rendered the
> same new params entry into this table within minutes of each other, one of them with
> `—` placeholders for the fields it could not see. `append_entry` prevented the *id*
> collision server-side (the concurrent session's entry took BL-32, mine BL-33, with no
> coordination) — but nothing protects the hand-maintained snapshot from the same race.
> That gap between the two is BL-30's cost stated precisely.
>
> That is **BL-29** demonstrated, not a lapse: `update_entry` and `move` both write
> catalog-only state, so every entry-grain write silently ages this table. The check that
> catches it is `artifact(get, entry_filter=…)` against the live params before trusting
> any row here — an id that returns `count: 0` is archived, not deleted.
>
> Earlier note, still true: BL-1, BL-20 and BL-22 were flipped with
> `artifact(action="update_entry", …)`. The note that used to sit here said the flip was
> unsafe because there was no entry-grain update; that was BL-20, and it is now fixed.
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

### 2026-08-16 — BL-2 fixed; the filed root cause was understating it

| BL | What shipped | SHA |
|----|--------------|-----|
| BL-2 | grep stops printing a denominator the walk never counted | `4b77dff5` + `358f1ced` |
| BL-31 | filed — found while fixing BL-2 | (open) |

**The filing was wrong about scope, and the reconnaissance is what caught it.** BL-2 was
filed as a corner case: "N of N" appears *when* `budget >= total`. Tracing `max` from its
single binding showed it serves as **both** the collection break threshold and
`cap_grouped`'s display budget, so `visible.len() == total` unconditionally and the second
disjunct of `truncated = hit_cap || total > visible.len()` is **dead code**. The hint had
never printed anything else. Had the fix been written to the filing, it would have
branched on a condition that cannot occur.

**Three surfaces, not one.** `format_overflow` already rendered the honest `… showing
first N` when `shown == total`; grep's own hint then re-asserted `Showing N of N` right
after it, under a header stating the count as fact. The one true clause was the shortest
and sat between the two that overrode it. Fixing only the hint would have left the header
— the line a reader anchors on — still wrong.

**The test shape, third instance this session.** Two rows over one corpus, capped and
complete; the complete row was **green before the fix**. That is the only shape that can
catch a defect whose symptom is that two renderings are byte-identical — asserting on the
capped string alone passes against the bug. Same shape as the `edit_file` guard (1 write
path of 3) and the `update_entry` guard (1 input shape of 2).

**Verified by invoking, not by inspecting** (T-20). `cargo test` proved `Grep::call` →
`format_compact`; only the live MCP call proved the running server serves it:

```
grep(pattern="^use serde_json::json;", glob="src/**/*.rs", limit=5)
5 matches (capped) in 5 files
  … showing first 5 — Collection stopped at limit=5, so the true total is unknown …
```

**BL-31 fell out of the same reading.** Because collection stops at `max` in walk order,
`cap_grouped`'s round-robin never runs from grep. Live proof, same session: a capped
search offered three files holding **one match each** as ways to narrow a result already
capped at five — advice that is not merely unranked but inert. Two capped searches before
it returned `across 1 files`, so the common capped result is one file's worth, which is
also why BL-2's new `(capped)` header marker is invisible in the common case. The two
defects interact; fixing BL-31 restores the marker.

**Shared-tree hazard #2.** `cargo fmt` formats the whole crate, and it was run while the
concurrent session had uncommitted edits under `src/librarian/tools/audit_doc_refs/`. Any
reformatting of their in-progress files landed in *their* diff. Pathspec-scoped commits
protect the index; they do not protect against a whole-crate formatter. Prefer
`cargo fmt -- <file>` on a shared tree.

### 2026-08-16 — BL-33: the convention was hiding the answer

| BL | What shipped | SHA |
|----|--------------|-----|
| BL-33 | guard keys on what is actually managed, not on YAML quoting | `29f0c015` |

**Filed as a quoting bug; fixed as a predicate bug.** `is_librarian_artifact` tested the
frontmatter text for a 16-lowercase-hex `id:`, and a quoted `'…'` is 18 characters, so
protection depended on which serialiser last wrote the file — 12 guarded trackers, 15 not,
the queue among the unguarded. The quoting fix alone closes 86 files repo-wide.

**The user's course-correction is the entry worth keeping.** My first pass concluded the
id-less trackers were *not* a defect: the stamped id looked like an intentional opt-in
marker, and a catalog-backed guard would refuse `edit_markdown` on
`docs/trackers/skill-frictions.md`, which CLAUDE.md documents. That reasoning deferred to a
convention instead of testing it — *"lets correct it properly not follow a rule written 10
years ago"*. Two queries then settled the design:

```
docs/RELEASE.md, CONTRIBUTING.md, PROGRESSIVE_DISCOVERABILITY.md, TAXONOMY.md, ROADMAP.md
  -> ALL catalog rows                       (so membership is the WRONG predicate)
artifact(find, augmented=true, scope="repo")
  -> count: 16, all trackers                (so augmentation is the RIGHT one)
```

Augmentation is exactly the set where state lives *outside* the file. It keeps
`skill-frictions.md` editable — but now because that is correct, not because it is written
down — and it catches `artifact-augmentation-followups.md`, augmented with **no** frontmatter
id, which no amount of string-parsing could ever reach.

**Cost of the naive route, measured before rejecting it:** the core `ToolContext`
(`src/tools/core/types.rs`) has no catalog handle, and adding a field means editing **124
construction sites across 21 files**. A trait object installed once at server construction
costs one line in `server.rs`. `guard_with_oracle` takes the oracle explicitly so no test
installs into the `OnceLock` and none can poison another in the same binary.

**Verified live on four cases**, the two negatives mattering as much as the positives:
augmented-with-no-id refused, augmented-with-quoted-id refused, prose tracker readable,
`docs/RELEASE.md` readable.

**A closing loop.** This queue is now guarded against the direct `read_markdown` that
T-22 caught me making — the observation and the fix landed in the same session, and
`artifact(get, entry_filter=…)` is now the only way in.

### 2026-08-16 — BL-34 fixed, and the corpus proved something the unit tests could not

`858f22ec` — `frontmatter::replace_id_line` splices the `^id:` line instead of round-tripping
the block through `parse` → `write`. `mv::repair_frontmatter_id` now reads through the parser
(authoritative about *whether* a repair is needed) and writes through the splice (authoritative
about *which bytes move*). When they disagree — a folded or flow-mapped `id:` the line scan
cannot see — it warns and leaves the file alone. Declining beats reformatting: a stale id is a
broken citation, a reformat is data loss.

**Verified live against the named regression corpus**, `/home/marius/work/mirela/eduplanner-ui`
(reverted on purpose 2026-08-16 precisely so it could serve as one):

| | before | after |
|---|---|---|
| files | 26 | 26 |
| diff | 402+ / 370− | **26+ / 26−** |
| lines/file | 30 | **1** |

Every hunk in `git diff -U0` is `@@ -2 +2 @@`, and every `−`/`+` line begins with `id:`. Both
ADR/FDR templates are byte-identical outside that line — `created: {YYYY-MM-DD}` intact.

**Three things worth carrying forward.**

1. **The corpus caught a class the unit tests structurally could not.** `title: "{Title}"` and
   `category: a | b | c` are shapes a *round-trip* mangles and a *splice* never sees. The fix
   is not "handle more YAML shapes correctly" — it is "stop reading shapes you have no business
   rewriting." A narrower contract needs fewer cases.

2. **Fourth instance this session of a test that exercises the mechanism but not the call
   site.** `move_rewrites_the_frontmatter_id_it_just_invalidated` was green through this entire
   defect, because it only ever asserted *that the id changed*. When a fix lands in a shared
   helper, drive at least one real call site — and mutation-verify that one, since it is the
   only test that can fail for the right reason.

3. **A read-only precondition check is cheap and buys the apply.** Before writing into a
   foreign repo, `grep -c '^id:'` over the 26 flagged files confirmed every one would take the
   splice path rather than the decline path. The four files the doctor did *not* flag
   (ADR-0023, ADR-0024, two READMEs) have no `id:` at all — the abstention branch, visible for
   free in the same result.

The 26-line repair is left **uncommitted** in `eduplanner-ui`'s working tree. It is the correct
repair now, but committing into another repo is not this session's call.
### Resume — state at compaction, 2026-08-16

**15 of 35 rows open.** Phase 1 remaining: **BL-4, BL-19** (BL-29 is phase 1 but the other
session's). Phase 2: BL-9, BL-24, BL-30, BL-31, BL-35. Phase 3: BL-13, BL-14, BL-15, BL-16,
BL-28, BL-32. Phase 4: BL-17.

BL-29/BL-30/BL-35 belong to the concurrent session. **BL-4 and BL-19 are the natural next** —
BL-19 is a progressive-disclosure sibling of the BL-2 fix (incompleteness signals on buffered
output).

**Working practices, carried forward:**

- **Read the filing as a hypothesis, not a spec.** BL-2's filed root cause was narrower
  than the defect; the fix would have been written to a branch that cannot execute.
  Trace the variable from its binding before implementing the fix a bug file prescribes.
- **The catching test contains a row that was green before the fix.** Four for four
  this session on guard/message defects — BL-34 made it a helper-vs-call-site variant.
- **When the fix lands in a shared helper, drive at least one real call site**, and
  mutation-verify that test specifically. The helper's own tests cannot prove the caller
  reaches for it.
- **Verify a fix is live by invoking it**, never by inspecting the binary or
  `codescout_sha` (T-20).
- **Archive with `artifact(action="move")` and re-point ids in the same commit** — read
  `id_changed`. An id that returns `count: 0` from `find` is archived, not deleted.
- **Check the snapshot against live params before trusting a row** (BL-29). Three rows
  had drifted here, all from catalog-only writes.
- **A concurrent session shares this working tree, index, and formatter.** Commit by
  pathspec; prefer `cargo fmt -- <file>`.
- **Before writing into a foreign repo, run the read-only precondition check first.** The
  dry-run says *which* files; a `grep` on the shape says *which code path* they will take.
  Both are free, and together they make the apply reviewable in advance (BL-34).
