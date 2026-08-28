---
kind: convention
status: active
title: Cross-Machine Catalog Resume
owners: [marius]
tags:
  - librarian
  - catalog
  - onboarding
  - cross-machine
---

# Cross-Machine Catalog Resume

**Run this when you pull codescout onto a machine that has not been building it,
or after any catalog loss.** It is a normal, expected process — not an incident.
A clone is *supposed* to arrive with three layers missing; this page is how you
put them back and how you tell the difference between "missing" and "broken".

First measured end-to-end 2026-08-28, pulling 437 commits onto a laptop after a
desktop work stream. Every number below came from that pass.

## Why a clone is never enough

codescout splits its state across two stores with **opposite** durability:

| Layer | Lives in | Travels with `git pull`? |
|---|---|---|
| Markdown bodies, frontmatter, `entry_prefix`, `entry_high_water_*` | the repo | **yes** |
| Semantic index (code chunks + memory vectors) | Qdrant / local store | **no** |
| `cites` edges (the link graph) | `catalog.db` | **no** |
| Artifact **augmentations** — `prompt`, `params`, `params_schema`, `render_template`, `entry_collection` | `catalog.db` | **no** |
| Event log, observations | `catalog.db` | **no** |

`~/.local/share/librarian/catalog.db` is machine-local and gitignored, and it is
**machine-global** — one DB spanning every repo on the host, not one per project.
Both facts matter below.

**Each missing layer is silent, and silent in a different way.** That is the whole
reason this page exists:

- `reindex` **preserves** augmentation keyed by id rather than regenerating it — so
  it reports success and repairs nothing.
- `artifact(get)` returns `augmentation: null` with no comment.
- A missing `cites` edge is indistinguishable from an artifact that cites nothing.
- Memory points missing from the store make `recall` return *fewer* results, never
  an error.

Nothing fails. You just quietly get less.

## The sequence

Run in this order. Later steps read state the earlier ones write.

### 1. Pull, and confirm it was a fast-forward

```
git fetch --all --prune && git status
git pull --ff-only
```

`--ff-only` is the point: if the branch diverged you want to know before any
repair runs against a tree you did not expect.

### 2. Reindex the catalog

```
librarian(action="reindex")
```

Adds rows for files the catalog has never seen. **Do this before any `artifact(find)`
query** — a `find` against a stale catalog returns a confidently wrong empty set. The
response's `unindexed_hint` on a prior call is the tell you skipped it.

*2026-08-28: 99 added, 67 updated, 23 removed, 165 embedded.*

### 3. Repair memory vectors

```
index(action="verify")     # read-only; reports memories on disk with no point
./target/release/codescout migrate-memories --in-place
```

`verify` names the gap; `migrate-memories --in-place` reads the memories from disk
**server-side** and re-embeds them. Check `skipped` in the JSON result — a non-zero
`skipped` means those rows kept old vectors and are still on the previous embedding
convention.

*2026-08-28: 21 of 23 memories were on disk with no point — invisible to `recall`,
with no error anywhere. After: 23 upserted, 0 skipped, 7 anchors attached.*

### 4. Rebuild the semantic index

```
index(action="build")      # returns immediately; runs in background
index(action="status")     # poll — file_count and chunk_count climb
```

`git_sync.behind_commits` in the `verify`/`status` output is the honest measure of
how stale it was.

*2026-08-28: 437 commits behind, 116 files missing, 23 orphans.*

### 5. Rebuild the link graph

```
librarian(action="link_scan")              # report only
librarian(action="link_scan", write=true)  # apply
librarian(action="link_scan")              # MUST now show edges_missing[0]
```

**The third call is not optional.** `link_scan` is idempotent, so a second scan
reaching a fixpoint (`edges_missing[0]`, `edges_stale[0]`) is what proves the write
landed. A scan that still finds work means it did not.

*2026-08-28: 697 of 1117 edges missing → 697 added, 9 pruned, 1540 entry edges
written, fixpoint confirmed.*

### 6. Scan for what is left

```
librarian(action="doctor")
```

Then **partition the violations three ways before touching anything** — they need
opposite responses, and the report does not separate them for you:

1. **Machine-local drift** — `augmentation_declared_but_absent`. This is your work.
2. **Pre-existing content debt** — `terminal_status_with_caveat`,
   `cited_prefix_with_no_definer`, `entry_dated_stale`,
   `entry_cited_from_outside_but_undeclared`. These travel in git and are
   *identical on every machine*. They are not caused by the move and are not
   yours to fix as part of a resume.
3. **Other repos** — `abs_path_outside_managed_roots`, and most `missing_file`.
   The catalog is machine-global, so it holds rows for every repo on the host.

**Never run `fix=prune_missing` as part of a resume.** Those rows may be perfectly
valid for a repo that is simply in a different state on this machine. Pruning is
destructive and the resume is not the moment to decide.

*2026-08-28: 125 violations — 22 machine-local, 65 content debt, ~23 other repos.
Only the 22 were in scope.*

### 7. Restore augmentations — tiered, not uniform

This is the only step with no automated path, because **augmentation is the one
artifact state with no on-disk form.** `expects_augmentation: true` in frontmatter
records *that* one should exist; nothing records *what it was*.

Precedent: `docs/issues/archive/2026-07-02-tool-usage-patterns-augmentation-lost.md`
restored this same class by reconstructing from body prose, and states plainly that
the restore is per-machine and must be re-run elsewhere.

#### First: measure the cost — do not restore what nothing queries

**Run the documented queries. Restore only the trackers where one fails.** An
augmentation whose absence breaks nothing is not a repair to make; filling it is
authoring.

Measured 2026-08-28 across 18 unaugmented trackers: **4** had a documented
`entry_filter` query that fails today. The other 14 had none — no observable cost,
and restoring them would have meant inventing 14 standing instructions to clear a
check.

**Proxies lie here; run the query.** Two greps were tried first, and both
misranked the list:

| method | verdict on `codescout-usage-frictions` | why it was wrong |
|---|---|---|
| line-scoped grep (id and `entry_filter` on one line) | **0** | multi-line prescriptions split the two across lines |
| file-scoped (file mentions id *and* contains `entry_filter`) | **5** | its `entry_filter` mentions document a bug against a **different** tracker's id |
| **running the query** | **no failing query → leave it** | the only method that separates "queries this" from "mentions both" |

Extract the real queries, then execute each:

```
grep -n 'entry_filter' <tracker path>              # self-documented
grep -rn '<artifact-id>' docs/ --include='*.md'    # then READ the hits
artifact(action="get", id="<id>", entry_filter=<the documented filter>)
```

A failing one returns, verbatim:

```
entry_filter set but this artifact is not augmented — declare entry_collection
on its augmentation, or retrofit it
```

That error **is** the measurement. No error — or no documented query to run —
means leave it, and say so in your report, so the next session does not re-derive
the same conclusion.

**Leaving it is the honest default for a second reason:** `expects_augmentation`
firing in `doctor` is a precise signal that something is missing. Filling every
slot with reconstruction converts that signal into a false all-clear.

#### Then: mine the archive for quoted live calls

**Do this before deriving anything from body prose.** Bug files quote the original
calls verbatim — argument names, collection names, response echoes — so the archive
is the repo's *de facto* augmentation-shape store. It was never designed as one,
which is exactly why nobody thinks to look:

```
grep -rn '<artifact-id>' docs/issues/archive/ docs/superpowers/ docs/trackers/ --include='*.md'
```

then read every hit showing an `artifact_augment` / `append_entry` / `update_entry` /
`entry_filter` call, or a response echo like `changed_fields: [...]`.

**A field name recovered from a quoted call beats one derived from body prose.**
Measured 2026-08-28 restoring `open-issue-work-queue`, where it changed the result twice:

- `entry_collection` is **`tasks`** — pinned by a quoted call in
  `docs/issues/archive/2026-08-16-update-entry-drops-entry-silently-when-fields-is-also-present.md:41`.
  A body-prose derivation would have invented a different, plausible, wrong name, and
  every documented `update_entry` call against the tracker would have kept failing.
- The original rows carried a **sixth field, `next`**, recoverable only from a
  `changed_fields: ["status","bug","next"]` echo in
  `docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md:209-210`.
  Nothing in the body mentions it.

**When a field is recoverable but its values are not, say so in three places** —
declare it in `params_schema` so future appends carry it, leave every restored row
without it, and record the loss in the augmentation `prompt`. Do not fabricate values
to fill a column. `next`'s values are gone permanently; the body itself says they
"live only in the catalog, which is machine-local and git-ignored."

#### Then: check your transcription

Re-render the reconstructed rows and diff them against the body's own table. This is
what makes *recovered* a claim rather than a hope — and it is not hypothetical:
BL-44 was specifically about params rows whose content had drifted from their body
counterparts. The 2026-08-28 `open-issue-work-queue` restore diffed 44 rows against
the snapshot table at lines 45-88 and got **0 mismatches**.

Watch for decoy tables: that same file has `| BL-` rows at six other line ranges,
all inside `## History` sections. Diff against the *live* snapshot, not the first
table that matches.

**Do not restore every tracker to full params.** Since 2026-08-18
`get_guide("tracker-conventions")` has withdrawn the params-rendered-index pattern:
no `render_template` writes a body, and a table row defines no citable token. Restore
in two tiers:

- **Tier 0 — the augmentation is CODE. Restore it verbatim; check for this first.**
  A tracker written by a codescout action carries its augmentation in the binary via
  `include_str!`, so the prompt and template are in the repo and recoverable
  byte-for-byte with zero authoring. `legibility-backlog` is the known case —
  `src/librarian/tools/legibility_scan/render_prompt.md` and `render_template.j2`,
  `entry_collection: "candidates"`. Attach those exact bytes, then re-run the action
  that owns it (`librarian(action="legibility_scan", write=true)`) to repopulate
  params from the live engine.

  **The owning action does not self-heal**, so it will not do this for you: run
  against an existing-but-unaugmented tracker, `legibility_scan` returns
  `ok: true` *with* `tracker_error: "no augmentation for artifact … — call
  artifact_augment first"`. The create-and-augment path only fires when the tracker
  does not exist.

  **Verify byte-identity rather than asserting it** — read both source files and the
  stored augmentation out of `catalog.db` and compare. That is what makes it
  restoration instead of a close paraphrase.

- **Tier A — `prompt` + `params` + `entry_collection`.** Only for trackers with a
  *documented* `entry_filter` workflow. Find them by evidence, not memory:
  ```
  grep -rn 'entry_filter' docs/ CLAUDE.md --include='*.md'
  ```
  then intersect with the `augmentation_declared_but_absent` list.
- **Tier B — `prompt` only.** Everything else. Restores the `[LIVE]` standing-instruction
  block in `librarian(action="context")` and clears the doctor check.

Follow `docs/conventions/retrofitting-trackers-for-filtering.md` for the Tier-A
mechanics. Two constraints bite every time:

- `merge=false` overwrites **all seven** caller-controlled fields. Pass `prompt`,
  `params`/`params_path`, `params_schema`, `render_template` and `entry_collection`
  in the *same* call — an omitted field silently resets to `None`.
- Params ≳9 KB **cannot** be passed inline; the MCP result buffer caps the
  round-trip, so it cannot be read back to re-emit either. Write the JSON to a file
  and pass `params_path=` (read server-side). A prose-heavy 11-row table already
  measured 8.8 KB in 2026-07.

**Say so in your report when a Tier-B prompt is newly authored rather than
recovered.** It is reconstruction from the body, not restoration of the original,
and the distinction is not visible in the result.

## Verify the resume

Do not trust the steps; check the outcomes.

```
index(action="verify")                 # git_sync.status, memories.missing_count → 0
librarian(action="link_scan")          # edges_missing[0], edges_stale[0]
librarian(action="doctor")             # augmentation_declared_but_absent → 0
artifact(action="get", id="<a Tier-A tracker>", entry_filter={"status":{"eq":"open"}})
```

The last one is the real test: it is the call CLAUDE.md documents for browsing
entries, and it is the one that fails first when augmentation is missing.

## Traps measured on the 2026-08-28 pass

All three produced **plausible output**, which is what makes them dangerous — none
raised an error.

**Escaped pipes silently shift every field in a parsed table row.** Reconstructing
params from a body table by splitting on `|` breaks on any cell containing `\|`
inside prose: the row yields 11–12 cells instead of 9, and every field right of the
escape lands one column over. On the hamsa log this would have put a `confidence`
value into `outcome` — a wrong row that looks entirely well-formed.

Split on `(?<!\\)\|` and unescape, and **assert the cell count per row**. The
assertion is the part that matters: it is what turns a silent mis-parse into a
loud one, and it is what caught this. Three of 34 rows carried the escape.

**Counting bare `---` to find duplicate frontmatter reports ~22× too many.**
Session logs use `---` as an entry separator: same string, different position. A
sweep for duplicated frontmatter must count a frontmatter-only key instead:

```
for f in docs/trackers/*.md; do
  n=$(awk 'NR<=60 && /^kind: /{c++} END{print c+0}' "$f")
  [ "$n" -ge 2 ] && echo "$n $f"
done
```

*Naive version: 22 hits, all false. Correct version: 1.*

**Filtering doctor output by an absolute project path matches nothing.** Response
paths are **relativized** — project-internal paths render relative
(`docs/trackers/<name>.md`) while foreign repos stay absolute. So
`grep -v '/home/.../codescout'` excludes *nothing*, which reads exactly like "this
project has no violations." Filter on the leading `/` instead: absolute means
another repo. See `get_guide("progressive-disclosure")` § Path-relative annotation.


## You are probably not alone in the checkout

A resume pass shares two things with any concurrent session: the **working tree**
and the **catalog**. Both bit during the 2026-08-28 pass.

- **Untracked files appeared mid-session** — two `resume-*.md` trackers, written by
  a peer at 12:45. `git add -A docs/` would have swept them into a commit that had
  nothing to do with them. **Stage explicit paths, never `-A`**, and re-read
  `git status --short` before every commit. The failure mode is already on record:
  `bug-fix-session-log:F-60`, *"a peer's routine commit absorbed this session's
  uncommitted `append_entry` writes."*
- **Worktrees make an unqualified write ambiguous, and the server refuses it.**
  With linked worktrees present and no explicit activation, `edit_markdown` returns
  `Write blocked: git worktrees detected but workspace(action='activate') has not
  been called`, naming them. Answer it with an explicit
  `workspace(action="activate", path="<main repo>")` rather than working around it —
  the guard is asking which tree you mean, and on a resume the answer is always the
  main checkout.
- **Subagents must not call `workspace(action="activate")`.** It mutates the
  *parent's* active project mid-turn
  (`docs/issues/2026-08-23-subagent-activate-mutates-parent-active-project.md`, open).
  Brief them to pass `workspace=<path>` per call instead. All four restore agents on
  this pass were briefed that way and none tripped it.

## Related

- `get_guide("tracker-conventions")` — augmentation declaration, `entry_prefix`, entry shape
- `get_guide("librarian")` — augmentation lifecycle, `merge` semantics, `params_path`
- `docs/conventions/retrofitting-trackers-for-filtering.md` — Tier-A mechanics
- `docs/issues/archive/2026-07-02-tool-usage-patterns-augmentation-lost.md` — the precedent restore
- `docs/trackers/bug-ledger-resume-2026-08-28.md` — the handoff this process was derived from
