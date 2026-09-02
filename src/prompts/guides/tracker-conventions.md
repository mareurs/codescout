# Tracker Conventions

codescout distinguishes three long-lived markdown surfaces. **Bugs** live
in `docs/issues/` — single-file, one-incident, opened and archived per
fix. **Trackers** live in `docs/trackers/` — multi-entry living state
(session logs, observation tables, ADR indexes) maintained across many
sessions. **Specs / plans / ADRs** live in `docs/specs/`, `docs/plans/`,
`docs/adrs/` — design artifacts. All three are indexed by the librarian:
discover them with `artifact(action="find", kind="bug" | "tracker" | …)`
and read them with `artifact(action="get", id=...)`. Never edit a
tracker file by raw path lookup — go through the catalog.

## Bug files (docs/issues/)

One file per bug.

- **Path:** `docs/issues/YYYY-MM-DD-<slug>.md` while open;
  `docs/issues/archive/` once the fix is **verified on `experiments`** —
  reaching `master` is NOT required.
- **Slug:** short kebab-case noun-phrase (3–6 words), e.g.
  `edit-code-insert-mid-function`.

**Frontmatter:** every bug file has `kind: bug` plus a `status:` field.
The librarian classifier auto-recognizes the file on next reindex.

**Status vocabulary** (`status:` field on bug files):

| Value | Meaning |
|---|---|
| `open` | Logged, investigation not started or paused |
| `investigating` | Actively being worked on this session |
| `fixed` | Root cause addressed, regression test added, verified |
| `mitigated` | Workaround in place; root cause not addressed |
| `wontfix` | Intentionally not fixing; justification in the file |
| `zombie` | No longer observed but root cause unconfirmed. Pair with `last_observed:` and a re-open trigger |

`closed:` stays empty at creation — fill in `YYYY-MM-DD` only when
status flips to `fixed` / `mitigated` / `wontfix`.

**`unverified:` — the caveat, made queryable.** Add it whenever the `status` overstates
what was actually established: no regression test, root cause not addressed, a claim not
re-checked since a rebuild, a fix applied only to a gitignored file, or a blocker that has
since become an obsolete rule. **Absence means nothing outstanding — never add it empty**,
because presence is what a query filters on.

Why it exists, measured 2026-08-19: of 16 terminal-but-unarchived bug files, **14 stated
their blocker in prose** and none of it was reachable by a query. That is how
`find(kind="bug", status="open")` — the triage query this guide and the activation
bootstrap both prescribe — came to hide a `fixed` record whose own body read *"Tests
added: None, and this is the gap worth naming rather than excusing"* and *"does not
prevent recurrence"*, for a defect class CLAUDE.md describes as redirecting every
per-project memory write into the wrong repo with no review able to catch it.

The system's scarce resource is not candour; it is **legibility**. Authors already write
the caveat. Write it where a query can read it.

**Trigger rules — open a bug file for ANY bug noticed during work:**

- ✓ User explicitly asks ("log this", "open a tracker")
- ✓ Bug blocking the current task (fix-now or parking-lot)
- ✓ Incidental bug we won't fix in the current session
- ✓ Just-fixed bug whose investigation is worth preserving
- ✓ Tool quirks / misbehaviors
- ✗ Pure typos / one-token corrections — commit message is enough
- ✗ Feature ideas / refactors — those go in `docs/trackers/` or `docs/plans/`
- ✗ Subjective dislikes that aren't bugs

**Capture discipline:** add the file the moment the bug is noticed —
don't wait until task end.

**Archive trigger:** move the file into `docs/issues/archive/` once the fix is
verified on `experiments` — gate green (the four commands in `CLAUDE.md` § *Development
Commands*, whose `cargo test --workspace` and long clippy form are each there because a
narrower one shipped a defect) and a regression test in place. Reaching `master` is **not** required:
`experiments` is never deleted, so an unmerged fix is not at risk of being lost, and
holding the file back only grows a pile of `fixed`-but-unarchived bugs that no query
ever surfaces (`artifact(action="find", kind="bug", status="open")` filters on
`status`, not on path).

The file MUST carry the fix SHA when archived experiments-only, because nothing
re-reads `archive/`: label it **`experiments`**, since an `experiments` SHA orphans on
rebase and an unlabelled one in `archive/` becomes an untraceable string.

**And record a `patch-id` next to it, because the sentence above describes a wound with
no remedy.** `git show <sha> | git patch-id --stable` is a content hash of the commit's
diff: it is invariant under rebase and cherry-pick, so it still finds the change after the
SHA dies. Measured 2026-08-19 — **10 of 63** archived bug files had already lost their fix
pointer (objects absent from the object DB, not merely unreferenced; subject-keyword
recovery returned 2–153 ambiguous candidates), and 37 more were one rebase away. Across
3594 commits there were **zero genuine patch-id collisions**; all 104 duplicates were the
same change on two branches, which is the anchor working. Resolve one with redirects, not
pipes — Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep <first-12-of-patch-id> /tmp/patch-ids.txt
```

**A merge commit has no patch-id, and the tool reports that by staying silent.**
`git show <merge>` emits the message with no diff — a merge has more than one parent, so
"the diff" is ambiguous — and `git patch-id` given no patch prints **nothing and exits
0**. The prescribed pipeline therefore returns empty, with no error, exactly where the
rule matters most. Cite the merged branch's **constituent commits** by SHA + patch-id
instead; each has an ordinary patch-id that survives rebase normally. Never record an
empty patch-id field, and never substitute the SHA for it.

Do not reach for `git diff <first-parent>..<merge>` to manufacture one: it hashes, but
not to the object a cherry-pick would reproduce, so it is a plausible value in a field
that means something else.

Already the practice in this repo before it was a rule —
`docs/issues/archive/2026-08-13-url-silently-overrides-local-dir-model.md` reads *"Fixed
at `38e0980b`, merged to `experiments` at `e6484b16`"*: the fix commit carries the
citation, the merge is named separately as an event.

**There is no promotion path to check and no pending-master-SHA `## Resume` line.**
Recording the SHA *and* its patch-id replaces both. Whichever way the fix reaches
`master` — fast-forward or cherry-pick — the pair stays resolvable, so nothing is owed
later and no session has to come back and reconcile.

Check where a SHA currently lives with `git branch --contains <fix-sha>`.

(Historical note: many archived bug files carry an older form that promises a master-side
SHA "still to be recorded", written when that follow-up was the rule. **They are stale
instructions, not open debt** — the SHA in each was correct when written. Do not sweep
them. Measured 2026-08-19: 10 of 63 had nonetheless lost their SHA to a rebase, which is
why the patch-id now rides alongside.)

Archive through the catalog — `artifact(action="move", id=…,
new_rel_path="docs/issues/archive/…")` — never a bare `git mv`: `id =
sha256(abs_path)`, so a hand-move orphans the catalog row.

**The move mints a new id, and reports it.** Because the id is derived from the
path, archiving necessarily re-keys the artifact; `move` grafts its events, links,
observations and augmentation onto the new id in the same call and returns
`{"id": <new>, "previous_id": <old>, "id_changed": true}`. Read the new id from the
response — the old one stops resolving immediately, and a later call with it
returns `unknown id`.

**And stage BOTH halves of the move together — `git add -- <old> <new>`.** At the
catalog layer the move is atomic; on disk it is a tracked *deletion* plus an untracked
*addition*, so every selector defined over index entries — `git add -u`, `git commit
-a` — enumerates only the deletion and commits an archive that silently un-happened.
There is no count of what was missed, because an untracked file is never a skipped
candidate: it is never enumerated. `git status --short` showing one `R` rename line per
pair is the positive confirmation; a ` D` plus a `??` is half-staged. The `move`
response carries `stage_together` and `stage_hint` saying exactly this — both paths
were always reported, and reporting them was measured insufficient: one session read
them six times across six moves and still had to be told the action.
`docs/issues/2026-09-02-tracked-only-staging-commits-half-an-archive-move.md`

**Then re-point the citations, in the same commit as the move —
paths *and* ids.** The move changes both, and archiving is a bug file's *normal*
end state — so every citation of `docs/issues/<slug>.md`, and every citation of
that artifact's 16-hex id, is a scheduled break, and the move is the moment it
fires. Measured 2026-08-08: three archive moves left **25**
dangling refs across 15 files, and the one in `docs/manual/src/concepts/`
failed CI's `Audit Doc Refs` gate on the tip commit of a release promotion.

```
grep -rn 'docs/issues/<slug>.md' . \
  --include='*.md' --include='*.rs' --include='*.sh' --include='*.py' \
  --include='*.yml' --include='*.toml' --include='.env*'
```

**The `--include` list is itself a hypothesis about where citations live, and it is the
part that fails silently** — a filter that misses a file type returns a clean zero, which
reads exactly like "no citations to fix". This list was `*.md`, `*.rs`, `.env*` until
2026-08-26, when an archive move left a dangling citation in `scripts/build-windows.sh`
and the sweep that ran reported nothing. Measured on this repo the same day: **6 live
non-`.md`/`.rs` surfaces cite `docs/issues/`** — `.github/workflows/ci.yml`,
`docker-compose.yml`, `scripts/build-windows.sh`, `scripts/fetch-models.sh`,
`scripts/friction-probe.py`, and a `tests/fixtures` `.toml`. If you are unsure, drop the
filters entirely and read the extension histogram; `.jsonl` and `.diff` hits are session
artefacts under `.buddy/` and `.superpowers/` and are not surfaces.

Fix the **live** surfaces — the manual, `CHANGELOG.md`, `README`s, active trackers, CI
workflows, `scripts/`, `.env*` templates, source doc comments. **Leave `docs/issues/archive/**` and
superseded session-log rounds alone**: those are historical snapshots, and
`apply_drops`' `archive_drop` exists precisely so a retired document citing a moved
path does not gate. Rewriting them would falsify the record to satisfy a linter that
is already ignoring them.

Note the gate only catches the subset that lands in a full-severity surface, so the grep
is the check — not a green CI run.

**Cite where the file IS, never where the archive flow will put it.** The sweep above
repairs citations that *were* correct when written; it can never reach one that was wrong
on the day it was written. A citation is authored at **fix** time; the archive move is a
separate, **later** event — `fixed` is terminal, archiving is a manual
`artifact(action="move")`, and a *partial* fix cancels it outright. So writing
`docs/issues/archive/<slug>.md` for a bug still sitting in `docs/issues/` schedules no
repair at all: no archive event ever fires, therefore no sweep is ever triggered and no
procedure owns the fix — it survives until a reader follows the link. Measured 2026-08-26:
four such citations across three files, two of them in `.rs` doc comments where
`--fail-on high` cannot gate by design, needing a dedicated repair commit (`fcb86c16`).
See `bug-fix-session-log:F-69`.

*(Corrected 2026-08-26. This sentence used to add "and it does not scan `CHANGELOG.md` at
all", citing a bug that has since been fixed and archived —
`docs/issues/archive/2026-08-08-audit-doc-refs-never-scans-changelog-or-contributing.md`.
`DEFAULT_AUDIT_GLOBS` now lists both `CHANGELOG.md` and `CONTRIBUTING.md`, and a live run
counts 127 findings in the former. What is true of `CHANGELOG.md` is narrower: refs inside
already-released sections are capped by `released_history_boundary`, so a shipped section
cannot be "corrected" into falsehood.)*

Be precise about *why* a green run proves nothing here, because the plausible reason is
the wrong one. `audit_doc_refs` **does** scan source files, shell scripts included —
`DEFAULT_AUDIT_CODE_GLOBS` in `src/librarian/tools/audit_doc_refs/mod.rs` lists `**/*.sh`,
and `bash` has a grammar, so the citation *is* found. It is reported at **`Med`**, on
purpose: `scan_code_comments` forces that severity so someone archiving a bug file cannot
break the build via a comment they never touched. `--fail-on high` then passes, correctly.
So the finding exists and the gate is silent by design — which is exactly why the grep is
the check.

## Tracker artifacts (docs/trackers/)

Trackers are living state — multi-entry tables, observation logs, ADR
indexes — that grow across many sessions. They are full librarian
artifacts: backed by markdown on disk, indexed by the catalog,
optionally augmented with a persistent prompt that refreshes their
body.

**Frontmatter shape** (required for new trackers):

```yaml
---
kind: tracker
status: active           # or draft | archived | superseded
title: <human title>
owners: []
tags:
  - <topic>
---
```

The librarian assigns `id:` on the next `librarian(action="reindex")`
if omitted.

**Status vocabulary** (frontmatter `status:` field for trackers):

| Value | Meaning | Visibility |
|---|---|---|
| `active` | Living tracker, actively appended to | visible |
| `draft` | Scoped / watching, not yet active | visible |
| `archived` | Terminal — work-stream wrapped | **hidden by default** |
| `superseded` | Replaced by a successor artifact | **hidden by default** |

`done`, `in-progress`, etc. are NOT special-cased — they appear as
active. The frontmatter status drives librarian visibility.

**`draft` means the opposite thing on a plan, and one catalog column holds both.** The
row above defines it for **trackers**: *scoped / watching, not yet active*. In
`docs/plans/` this repo uses it to mean *shipped, deliberately unarchived because a named
residual is still open* — a plan says so in its own text (*"this plan stays active (do NOT
ARCHIVE) while 4b is open"*) and the residual lives in a companion `resume-*` tracker.
Both surfaces are catalogued with the same `status` column, so `artifact(action="find")`
returns them side by side with nothing marking which vocabulary a row speaks.

Measured 2026-09-01: a triage pass sorting on `status` reported
`docs/plans/2026-05-30-per-request-workspace-pinning.md` as a design stalled for 94 days.
Its phases 0–3 and 4a had shipped and were verified at the bytes on 2026-08-28. The
disambiguator existed — a `resume-*` tracker naming the plan — but the pointer runs one
way only, tracker → plan, so the plan is the row a query returns and the tracker is the row
that would have corrected it. **Before reading a `draft` as "not started", check whether a
`resume-*` tracker names it.**

**Archiving a tracker:** preferred path is in-place archival via the
catalog:

```
artifact(action="update", id="<id>", patch={"status": "archived"})
```

If you must also move the file on disk, use `artifact(action="move",
id="<id>", new_rel_path="docs/trackers/archive/foo.md")` — never a bare
`git mv`, which orphans the catalog record.


### A `## Resume` states its own currency

A tracker that accumulates review passes by *appending* leaves its oldest navigational
section — the one saying "start here / do this next" — at the **bottom**, below every
later correction, with nothing marking which is current. A reader told to start there
starts at the stalest text in the file.

**So a `## Resume` either dates its heading, or opens with a banner naming what
superseded it and where the current state lives.** Both forms are already in use:

    ## Resume — state at compaction, 2026-08-16

    ## Resume — where this stands (2026-08-11)

    > **CLOSED 2026-08-13 — merged at `e6484b16`.** Everything below this line is the
    > historical record. Read § *Integration* for the current state.

**The marker does not prevent staleness; it makes staleness legible** — and that is the
whole of it. A dated Resume whose facts have since moved is still doing its job, because
the reader can see the date and check. That is also why this is a convention rather than
a `doctor` check: no check can separate "old and still right" from "old and wrong", and
one keyed on age would fire on every correctly-dated section while catching nothing the
marker does not already cover.

Measured 2026-09-02 across every `## Resume` under `docs/trackers/` and `docs/plans/` —
nine candidates, after excluding the 497 bug files whose Resume is a short form field
inherited from `docs/issues/_TEMPLATE.md` rather than a navigational instruction.
**0 of 5 carrying a marker were misleading; 3 of 3 without one were stale** — one of them
routing an implementer to a strategy its own file had rejected, one pointing at a peer
session whose process no longer exists. Five authors had invented the marker
independently; none had written it down.

**The device generalises past `Resume`, though only that scope was measured.** Any section a
later one falsifies has the same problem: sections carry no dependency edges, so the falsifying
edit produces no diff hunk, no broken link and no check naming the section it retired — and the
retired section keeps reading as settled prose. Same repair, applied where you notice it: a
banner naming what superseded it. Recorded as `observer-blindness:OB-12`, whose blind party is
the author adding the new section, since nothing in the act of appending points backwards.
## Declaring an augmentation

If an artifact is meant to carry an augmentation, say so in **frontmatter** — and name
the committed sidecar holding its shape:

```yaml
expects_augmentation: docs/augmentations/docs-trackers-tool-usage-patterns.yaml
```

`expects_augmentation: true` is still valid and still means "declared, shape not
recorded". Anything else — a typo, an empty value, a bare word — is **reported** as
`augmentation_declaration_unparseable` rather than read as absent, because a
declaration that silently does not count is worse than none: its author believes they
are covered.

**What travels and what does not.** The sidecar carries `prompt`, `params_schema`,
`render_template`, `entry_collection`, `append_mode`, `history_cap` — the *shape*.
`params` deliberately stay catalog-only: they are live state that churns, and
committing them recreates the params-vs-body drift class BL-29/BL-40/BL-42 closed. So
a restored tracker comes back **working and empty**, never holding another machine's
rows.

Write the sidecar with `librarian(action="doctor", fix="export_augmentations")`, which
also stamps the declaration. It can only export rows **this** catalog holds, so run it
on a machine that still has them.

**Export CREATES; it never refreshes.** It skips an artifact whose sidecar already
exists — that is what makes it idempotent — so it is not the way to update one after a
shape change. `artifact_augment` handles that itself: a call that changes `prompt`,
`params_schema`, `render_template`, `entry_collection`, `append_mode` or `history_cap`
writes through to an existing sidecar, and a params-only merge leaves the file
byte-identical. The catalog is authoritative and the sidecar is its committed
projection, so a shape change made locally is what gets published — read `git diff` on
`docs/augmentations/` before committing, as with any generated file.

That wiring exists because its absence was measured rather than imagined. Export
skipping, `reindex` attaching only when a row is *absent*, and `artifact_augment` not
touching the file were each correct alone and composed into a **one-way door**: after
the first export, no path could update a sidecar. The first live shape edit hit it
within a day — widening a tracker's `status` enum reported `exported: 0` while the
committed YAML still held the superseded seven-value list, so a fresh clone's `reindex`
would have restored the old shape and reported success. **A stale sidecar is worse than
an absent one**: absence is reported by `augmentation_declared_but_absent`, staleness
restores clean.

**Why any of this is needed.** Augmentation was the one artifact state with no on-disk
form: rows, frontmatter and body all rebuild from disk on `reindex`, but the
augmentation lived only in the catalog, which is machine-local and git-ignored. Its
absence is silent in every direction — `reindex` preserves augmentation keyed by id
rather than regenerating it, so it reports healthy after a loss and repairs nothing;
`artifact(get)` returns `augmentation: null` without comment; the documented
`append_entry` / `update_entry` / `entry_filter` calls fail only at use, one caller at
a time.

`reindex` now re-attaches a declared sidecar **whenever the row is absent, and never
overwrites a live one** — repair, not sync, because params move on independently of
the committed shape. It reports `augmentations_restored` so a run that repaired
nothing is distinguishable from one with nothing to repair.

The declaration is also what `doctor`'s `augmentation_declared_but_absent` reads. Like
`entry_prefix` it is **declared, never inferred**: measured across 4,195 catalogued
artifacts, 29 mention `[LIVE]` outside fenced code and 23 of those carry no
augmentation — guides, specs, plans and bug files *describing* the mechanism. Intent is
not recoverable from prose.

The check now splits by remedy. A declared sidecar that exists means run `reindex`. No
sidecar means the shape survives only in some other machine's catalog — export it
there and commit the result, or re-author with `artifact_augment(id=…, prompt=…, …)`.
## Entry-level standard — the shape INSIDE a tracker

The rules above govern the tracker *file*. These govern its **entries** (`F-3`,
`R-91`, `T-17`, `BUG-40`). They are not style preferences: the first three are
enforced by the citation resolver, and violating them silently breaks the link
graph. Every figure below was measured on this repo on 2026-08-17.

### Declaring a ledger

A **ledger** is an artifact that owns an id namespace. It is a much narrower thing
than a tracker: measured 2026-08-17, 27 unaugmented trackers under
`docs/trackers/` and only **three** were ledgers — the rest are design docs,
research notes and finished session logs, which own no ids and must stay directly
editable.

Declare one in **frontmatter**, never only in the catalog:

```yaml
entry_prefix: R          # or a sequence, for a ledger owning two namespaces:
entry_prefix:            #   a session log carries both F-N frictions and W-N wins
  - F
  - W
```

Set it with `artifact(action="update", id=…, patch={extra: {"entry_prefix": "R"}})`.

**Frontmatter, because the catalog is machine-local and git-ignored.** A
declaration stored in the augmentation is absent in a fresh clone, so every
`append_entry` there fails. **The counter has to travel with the repo too** — see
*Entry ids* below for `entry_high_water_<PREFIX>`. This guide previously argued the
opposite, that a catalog-local mark was safe because the allocator re-reads the
committed body each time. That was wrong in one specific way, and it cost a silent
defect: the body's maximum is not monotonic, because compaction moves entries out to
an archive companion. Identity *and* the counter travel; only the within-machine race
guard stays local.

A ledger is **declared, never inferred**. A design doc that quotes `## R-4` in
prose is not a namespace, and inferring one from content would make every such doc
allocatable.

### Entry ids

The resolver's token grammar is `\b[A-Z]{1,3}-\d+\b` — one to three uppercase
letters, a hyphen, digits, and **nothing else**.

- **Never suffix an id.** `R-72b`, `F-6a` are not valid tokens at all: digit→letter
  is not a word boundary, so a suffixed id can never be defined *or* cited. A
  collision-resolution scheme built on `a`/`b` suffixes produces ids the graph
  cannot represent. If two entries share a number, give the later one a **fresh**
  id.
- **Let the server allocate, and let it write the section too.**
  `artifact(action="append_entry", id_prefix="R", anchor_heading="## Template for
  new entries", title=…, body=…)` assigns the next id atomically **and** writes
  `## R-N — <title>` — the only shape that defines a citable token — in the same
  file write. Passing `anchor_heading` + `title` + `body` together is what selects
  that path; omit any of the three and the call only reserves the id, leaving the
  section yours to write and a reserved-but-unwritten id as a live failure mode.
  This holds for an **augmented** ledger too, as long as it declares no
  `entry_collection` (its entries being body sections, not params rows).
  Hand-allocation races: a peer session in the same checkout can take the id
  between your scan and your write.
- **Write the index row after, never before.** The allocator counts an id already
  claimed by an index row, so a row written ahead of its section consumes the
  number it names — which is why codescout's own `statement-validity-session-log`
  starts at `F-2`/`W-3`.
- If you must hand-allocate, scan **every** entry format the file uses, and re-scan
  in the same breath as the write — a max-id is a fact about an instant.
- **From the MAIN checkout only.** `append_entry` refuses id allocation from a
  worktree session, on the same grounds it refuses `cites`: an entry id is
  ledger-wide state and must key to the main tracker. Left unguarded, the worktree's
  shadow row is a different `artifact_id`, so both trees issue the same number — and
  nothing repairs it at merge, because the renumber only covers params rows. If a
  worktree session must record something, write it to a worktree-local file and fold
  it into the ledger from the main checkout after the merge.

**Where the counter lives.** A ledger carries its own high-water mark in committed
frontmatter, one key per namespace, written by the allocator:

```yaml
entry_prefix: HY
entry_high_water_HY: 11
```

Do not hand-edit it downward, and do not delete it when compacting. It is the only
one of the allocator's three inputs that survives a fresh clone, an
`artifact(action="move")`, and compaction — the live body's maximum *drops* when
entries move to an archive companion, and the machine-local reservation does not
travel. Without the committed mark, a compacted-and-archived ledger reissues `HY-1`,
and because the resolver binds a token to its sole **active** definer, every
historical citation silently re-points to the new entry with no dangling or ambiguous
count moving. Compaction is safe because a body edit preserves the frontmatter block
byte for byte; a hand-rewritten file does not.

### Entry headings — the definition rule

An entry is defined by a heading of exactly this shape:

```
## <ID> — <title>          token, whitespace, dash (— – -), whitespace, text
```

Anything else defines nothing:

| Heading | Defines? |
|---|---|
| `## R-91 — the scout ran too late` | yes |
| `## R-91` (no title) | **no** |
| `### A-9 Addendum` (no dash) | **no** — a section *about* A-9 |
| ``### `A-9` — title`` (code-first) | **no** |
| `\| R-91 \| … \|` (table row) | **no** — rows never define |

An undefined-but-cited id becomes a **dangling** citation. A ledger carrying 48
row-only entries produced ~30 of this project's 39 sampled dangling entry tokens;
a sibling ledger keeping headings for all its entries produced zero.

**Heading level is not the discriminator** in the table above — `### D-6 — …` and
`#### S-1 — …` both define. `### A-9 Addendum` fails on the missing dash, not the
level; two readers took it as a level rule and were wrong.

**A prefix with no definer ANYWHERE is a third state, distinct from dangling.**
Dangling means the prefix is known (something, somewhere, defines a sibling
entry or declares `entry_prefix`) but this specific id isn't. A prefix nobody
has ever defined or declared is not a citation candidate at all in the
resolver's terms, so it resolves as prose noise — the same gate that keeps
`UTF-8`/`SHA-256` silent — and is reported by neither `link_scan`'s
dangling/ambiguous buckets nor a ledger-scoped `doctor` check. `doctor`'s
`cited_prefix_with_no_definer` check exists specifically for this state: it
starts from the citation graph rather than a known ledger's claimed entries,
and fires only above a citation-volume threshold to stay quiet on incidental
prose. See `docs/issues/archive/2026-08-26-cited-prefix-with-no-definer-is-invisible.md` — the
`doctor` check, this documentation, and a `link_scan` `counts.truncated` flag per finding
array all shipped and are archived together.


### Citing an entry — bare, or qualified

Cite by **bare token** when the prefix has exactly one ledger: `R-98`, `HY-10`,
`T-17`, `CAP-5`.

Cite **qualified by file stem** when several files share a prefix. `F-N` and `W-N`
are namespaced per work stream, so each session log owns its own counter and
`F-1…F-5` are defined in *all eight* live logs:

```
bug-fix-session-log:F-33      → resolves to that log's F-33
F-33                          → eight definers, Ambiguous, resolves to nothing
```

The qualifier is the **file stem**, deliberately not `artifact.slug`: slugs are
lazily minted from `slugify(title)` with `-2` dedup, so they neither exist for most
artifacts nor can be predicted from the filename — and a citation an author cannot
predict is not a citation.

A qualifier naming no file in this repo is still a cross-repo reference
(`codescout:A-11`): reported, never turned into an edge, because edges cannot span
workspaces. A qualifier that *does* name a file which lacks that entry is
**dangling**, not ambiguous — the two need different fixes, so they are reported
separately.

**A third shape — `<repo>:<file-stem>:<TOKEN>`, naming a file inside a *different*
repo — has no supported grammar at all.** `link_scan` treats any citation with 2+
qualifier segments before the token uniformly: it is retracted (never silently
collapsed to the inner `<file-stem>:<TOKEN>` form) and reported, never resolved —
same as the two-part cross-repo form above, it can never become an edge, edges
cannot span workspaces. Write it anyway when you mean it —
`claude-plugins:roster-audit-session-log:F-5` in a `## References` section is
understood by a human reader even though the resolver will not chase it — but know
it is prose only, permanently, not a gap waiting to be filled. To keep the report
legible, these land in their own `cross_repo_file_qualified` bucket rather than
the generic `malformed_qualifier` one whenever the outer segment does not match
the citing artifact's own repo name. A citation whose outer segment *is* the
citing repo's own name (`codescout:foo:F-2` cited from inside codescout itself)
is genuinely redundant, not cross-repo — that one stays in `malformed_qualifier`;
strip the prefix instead of writing it.

Why this matters: measured 2026-08-17, ~400 ambiguous citations were ~12% of the
project's total, 49 of 50 sampled were F/W, and the citers were the **durable**
ledgers — R-N alone accounted for 27 of 50. That is the permanent record losing the
links to its own evidence, not session logs cross-talking.

### One entry format, never two

Do not hand-maintain an index table *alongside* body sections. Two formats for one
entry is the defect that generates the rest: the index falls behind (13 orphaned
bodies), and ids allocated by scanning one format collide with the other (9 twice-
allocated ids).

**The headings are the index.** That is the shape — not one of two. It is also the only
shape in which entries are citable, because `link_scan` derives a definition from a
`## <ID> — <title>` heading and from nothing else. A table row defines no token, so a
ledger whose entries live only in rows has entries that **nothing can cite, ever**.

This guide previously offered "the index is rendered from `params` via `render_template`"
as an equal second option. It was withdrawn 2026-08-18 for two measured reasons:

- **No `render_template` writes a body.** `render_params` has three callers: `context.rs`
  projects into the `librarian(action="context")` bundle, `legibility_scan` renders one
  compiled-in template keyed by file/symbol rather than by `PREFIX-N`, and a test. A
  reader who attached a template and expected a table to appear in the file got nothing —
  and hand-wrote the rows instead, which is how the option produced hand-maintained
  indexes in practice.
- **Rows are uncitable.** Measured across the catalog: 13 ledgers in five repos had
  entries their body never defines, the largest at 64 of 68, and one ledger's entire
  namespace resolved to nothing against 117 live citations.

Keeping a rendered row table *in addition to* headings is fine — a table is a good
reading surface. Just write it knowing it is hand-maintained and that the heading, not the
row, is what makes the entry reachable.

You do not have to take this on trust. `librarian(action="doctor")` reports
`entry_without_definition` and `ledger_defines_nothing` per artifact, and
`append_entry` / `update_entry` return `undefined_in_body` for an entry they just wrote
that no heading defines.

### Required fields

- **`**Status:**` is not optional.** It is the disposition field, and the only thing
  that makes a fired `Promote-when` harvestable. Without it, criteria go unharvested
  indefinitely — 39 of 57 entries in one ledger, over three months.
- **When a criterion fires, update the Status line.** Recording the firing only in
  prose leaves it invisible to every field-presence sweep; three fully-adjudicated
  entries sat uncounted for exactly this reason.
- Give the tracker a `params_schema` with `required` and `enum` where the shape has
  settled. A schema is what stops each author inventing their own entry shape.
- **`**Valid:**` declares a decay class.** Declaring one is what makes an entry a
  *Statement* — a claim that can be true or false and owes a proof. Declaring NONE
  is **not** an exemption — absence means decay, so an undeclared entry already
  means `dated <its last commit>` by default. What actually decides whether an
  undeclared entry gets flagged today is exposure:
  `entry_cited_from_outside_but_undeclared` only fires above the citation
  threshold, so an uncited entry is left alone because nothing rests on it. A
  section the server writes is stamped `**Valid:** dated <today>` unless the
  caller passes a class. Three forms, no fourth: `invariant` (a law), `dated
  YYYY-MM-DD` (true of an instant), `conditional — <event>` (true until that
  fires). A bare `conditional` with no event, an unknown value, or a
  calendar-invalid date (`dated 2026-02-30`) is refused — `conditionally
  speaking` does not parse as the class it starts with, on a word-boundary
  check — and caught by `librarian(action="doctor")`'s `validity_unparseable`
  check if one already landed. The first declaration in a section wins if there
  is more than one.
- **`**Rests on:**` is the durable route back to the proof.** Code rots and
  `path:line` rots with it; an ADR, a decision, or a principle does not. Three of
  codescout's seven ADRs already converge on this shape by practice — `Decision` is
  the claim, `Confidence` the evidence, `Revisit-when` the condition, and
  `Sites (initial)` labelling the rotting pointers as rotting in its own heading. It
  is parsed today; nothing consumes it yet — that lands with a later layer.
- Both fields are detected **line-anchored**, and a line inside a fenced code block
  is skipped — a worked example teaching the syntax is never mistaken for a
  declaration, including the examples on this page. `librarian(action="doctor")`
  reports `entry_conditional_past_due`, `entry_dated_stale`, and
  `entry_cited_from_outside_but_undeclared` — read-only worklists, never verdicts,
  each gated on cross-file citation exposure so an entry nothing depends on never
  generates work. The third names an entry "load-bearing and undeclared," never
  "promoted" — that judgement stays with the reader.

### Detecting these fields

Anchor detection on **structure** — line-start, a key prefix — never on a keyword.
Prose and field share a vocabulary by construction, so `grep -c 'Status:'` also
counts sentences *about* Status, and a `/fired/` probe matches "the tell that should
have fired". Both mistakes were made, in the same pass, by the same agent.

### Compaction and archival

The ladder is **live body → archived section (heading kept) → nothing further.**

- **Never reduce an entry to a bare row to "compact" it.** That destroys its
  definition and dangles every citation of it. Row-reduction is what created the
  dangling population described above.
- **Archival is safe for citations.** A unique definer resolves even when its
  artifact is `archived` — archived is not nonexistent. Where two artifacts define
  one token, the sole *active* one wins.
- **Archive into the ledger's existing archive artifact.** Check first —
  `artifact(action="find", filter={"rel_path": {"contains": "archive/<ledger>"}},
  include_archived=true)`. Forking a second archive for one ledger splits the
  definitions and creates ambiguous tokens.

### Make the tracker guarded

**Declare `entry_prefix`.** A ledger is guarded by that declaration alone — the guard
treats a declared `entry_prefix` as one of three independent reasons a file is
off-limits (`src/util/librarian_guard.rs`: augmented, stamped, or ledger), because the
`PREFIX-N` counter is state only the server may advance. Nothing else is needed, and any
YAML form works: `entry_prefix: R`, a block sequence, or `[F, W]`.

**Do NOT stamp `id: <16-hex>` into a tracker's frontmatter to guard it.** This guide
recommended exactly that until 2026-08-18, and the advice was wrong in a way worth
knowing rather than just deleting:

- It guards on **catalog membership**, the axis the guard's own pinned test rejects.
  `a_catalogued_but_unaugmented_file_stays_directly_editable` argues that membership-
  guarding would refuse `docs/RELEASE.md`, `CONTRIBUTING.md` and every ADR — all catalog
  rows — and it names a prose tracker that `CLAUDE.md` documents `edit_markdown` for.
  Stamping an id converts a file by hand into the case that test exists to forbid.
- It was tried, and it broke a documented workflow. Stamping `id:` into
  `reconnaissance-patterns.md` silently disabled `docs/TAXONOMY.md`'s `edit_markdown`
  append path for R-N; confirmed by probe, reverted in `bb9a94d7`.

The diagnosis the old advice rested on is still true — the guard reads the file's **own
text**, so a registered tracker whose frontmatter says nothing is invisible to it. The
remedy is the declaration, not the stamp. And **an artifact that owns no id namespace
needs no guard**: a convention doc, an ADR, a finished session log has nothing a hand-edit
can corrupt, and guarding it only makes routine edits ceremony.

## Querying with the librarian

The canonical "what's live right now" query — archived rows are hidden
by the default scope:

```
artifact(action="find", kind="tracker")
```

For bugs, swap the kind. **Constrain on all three non-terminal states, not just `open`**
— `status="open"` alone hides `investigating` (this guide tells you to set that while
actively working a bug) and `zombie` (recurring-but-unconfirmed, kept open in case it
comes back). `zombie` sits between "no longer observed" and "closed" precisely because
no other status fits — which is what makes it unreachable if the canonical triage query
omits it too:

```
artifact(action="find", kind="bug",
         filter={"status": {"in": ["open", "investigating", "zombie"]}})
```

A `zombie` hit in that query is a "has this recurred?" check, not a task to pick up —
most zombie records have no available work by design (see § Status vocabulary). Filter
it back out explicitly (`{"status": {"in": ["open", "investigating"]}}`) when you
specifically want only the actionable two.

`status="open"` alone remains right when you specifically mean *not yet started*.

Surface archived rows when needed:

```
artifact(action="find", kind="tracker", include_archived=true)
```

Read a tracker's full body or one section:

```
artifact(action="get", id="<id>", full=true)
artifact(action="get", id="<id>", heading="## Foo")
```

**Filterable trackers** — augmented trackers that store structured rows in a params array
can be queried at entry grain via `entry_filter`. Call `artifact_augment` with
`entry_collection="<array-key>"` to enable it, then pass `entry_filter={…}` (same AST as
`filter`) to `artifact(action="get")`. Prose trackers need retrofit first.

For deeper artifact / augmentation / event mechanics see
`get_guide("librarian")`. For how augmented trackers carry cross-session
behavior — including the session-passover pattern — see
`get_guide("librarian-runtime")` § *Trackers as cross-session behavior*.

## Cross-linking (edges are derived — cite in prose)

How artifacts reference each other, and who maintains the link graph:

- **Cite by ID in prose.** Entry IDs in their ledger's namespace
  (`A-11`, `F-3`, `BUG-40`), artifact ids (16-hex) or rel_paths across
  files, `<repo>:<ID>` across repos. Prose is the ONLY write surface for
  citations — never hand-create `cites` edges.
  **Entry IDs are stable; artifact ids are not.** A 16-hex artifact id is
  `sha256(abs_path)`, so it changes whenever the file moves — archiving one is
  the common case. Prefer an entry ID or a rel_path when the target is likely to
  be archived, and re-point 16-hex citations in the same commit as the move
  (`id_changed: true` in the move response is the signal).
- **`link_scan` derives the edges.** `librarian(action="link_scan")` parses
  artifact bodies, resolves citations (entry tokens by their defining
  heading; archived definers lose ties to active ones; ambiguous tokens are
  reported, never guessed), and materializes/prunes scanner-owned
  `rel="cites"` edges. `write=false` (default) reports; `write=true`
  applies. Idempotent — safe to re-run any time, and the repair path after
  reindex. `artifact(action="move")` now grafts an artifact's links onto its
  new id itself, so the move no longer drops them; the scan is what heals the
  cases that bypass it — a bare `git mv`, or any other route that leaves a row
  whose id does not match its path for the abs_path pre-clean to cascade-drop.
- **Manual rels are few and deliberate:** `evidence-for`, `promoted-to`,
  `refutes` via `artifact(action="link")` — use sparingly; a wrong edge
  pollutes context packing. `supersedes` is side-effectful (flips dst
  status, emits an event): archiving a tracker that has a successor
  REQUIRES a supersedes edge — created through `artifact(action="link")`,
  never a bare status edit.
- **Where links pay off:** `artifact(action="get", include_links=true)`,
  `artifact(action="graph", depth=1-3)`, and
  `librarian(action="context", anchor_id=…)` all read the graph — a
  well-cited tracker gets neighborhood packing for free.
