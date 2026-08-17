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
verified on `experiments` — gate green (`cargo fmt`, `cargo clippy -- -D warnings`,
`cargo test`) and a regression test in place. Reaching `master` is **not** required:
`experiments` is never deleted, so an unmerged fix is not at risk of being lost, and
holding the file back only grows a pile of `fixed`-but-unarchived bugs that no query
ever surfaces (`artifact(action="find", kind="bug", status="open")` filters on
`status`, not on path).

Two things the file MUST carry when archived experiments-only, because nothing
re-reads `archive/`:

- the fix SHA, **labelled `experiments`** — an `experiments` SHA orphans on rebase,
  so an unlabelled SHA in `archive/` becomes an untraceable string;
- a `## Resume` line stating that the **master-side** SHA still has to be recorded
  after cherry-pick.

Check where a SHA actually lives with `git branch --contains <fix-sha>`.

Archive through the catalog — `artifact(action="move", id=…,
new_rel_path="docs/issues/archive/…")` — never a bare `git mv`: `id =
sha256(abs_path)`, so a hand-move orphans the catalog row.

**The move mints a new id, and reports it.** Because the id is derived from the
path, archiving necessarily re-keys the artifact; `move` grafts its events, links,
observations and augmentation onto the new id in the same call and returns
`{"id": <new>, "previous_id": <old>, "id_changed": true}`. Read the new id from the
response — the old one stops resolving immediately, and a later call with it
returns `unknown id`.

**Then re-point the citations, in the same commit as the move —
paths *and* ids.** The move changes both, and archiving is a bug file's *normal*
end state — so every citation of `docs/issues/<slug>.md`, and every citation of
that artifact's 16-hex id, is a scheduled break, and the move is the moment it
fires. Measured 2026-08-08: three archive moves left **25**
dangling refs across 15 files, and the one in `docs/manual/src/concepts/`
failed CI's `Audit Doc Refs` gate on the tip commit of a release promotion.

```
grep -rn 'docs/issues/<slug>.md' . --include='*.md' --include='*.rs' --include='.env*'
```

Fix the **live** surfaces — the manual, `CHANGELOG.md`, `README`s, active trackers,
`.env*` templates, source doc comments. **Leave `docs/issues/archive/**` and
superseded session-log rounds alone**: those are historical snapshots, and
`apply_drops`' `archive_drop` exists precisely so a retired document citing a moved
path does not gate. Rewriting them would falsify the record to satisfy a linter that
is already ignoring them.

Note the gate only catches the subset that lands in a full-severity surface, and it
does not scan `CHANGELOG.md` at all
(`docs/issues/2026-08-08-audit-doc-refs-never-scans-changelog-or-contributing.md`),
so the grep is the check — not a green CI run.

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

**Archiving a tracker:** preferred path is in-place archival via the
catalog:

```
artifact(action="update", id="<id>", patch={"status": "archived"})
```

If you must also move the file on disk, use `artifact(action="move",
id="<id>", new_rel_path="docs/trackers/archive/foo.md")` — never a bare
`git mv`, which orphans the catalog record.

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
`append_entry` there fails. The reservation high-water mark *is* catalog-local, and
that is safe only because it is re-derivable — the allocator re-reads the committed
body each time. Identity has to travel with the repo; a counter does not.

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
- **Let the server allocate.** `artifact(action="append_entry", id_prefix="R", …)`
  assigns the next id atomically. Hand-allocation races: a peer session in the same
  checkout can take the id between your scan and your write.
- If you must hand-allocate, scan **every** entry format the file uses, and re-scan
  in the same breath as the write — a max-id is a fact about an instant.

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

Why this matters: measured 2026-08-17, ~400 ambiguous citations were ~12% of the
project's total, 49 of 50 sampled were F/W, and the citers were the **durable**
ledgers — R-N alone accounted for 27 of 50. That is the permanent record losing the
links to its own evidence, not session logs cross-talking.

### One entry format, never two

Do not hand-maintain an index table *alongside* body sections. Two formats for one
entry is the defect that generates the rest: the index falls behind (13 orphaned
bodies), and ids allocated by scanning one format collide with the other (9 twice-
allocated ids). Pick one:

- the **headings are the index** — nothing else to maintain; or
- the index is **rendered from `params`** via `render_template`, and entries are
  written with `append_entry` / `update_entry`.

A hand-written index is never the answer.

### Required fields

- **`**Status:**` is not optional.** It is the disposition field, and the only thing
  that makes a fired `Promote-when` harvestable. Without it, criteria go unharvested
  indefinitely — 39 of 57 entries in one ledger, over three months.
- **When a criterion fires, update the Status line.** Recording the firing only in
  prose leaves it invisible to every field-presence sweep; three fully-adjudicated
  entries sat uncounted for exactly this reason.
- Give the tracker a `params_schema` with `required` and `enum` where the shape has
  settled. A schema is what stops each author inventing their own entry shape.

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

Stamp the catalog id into the file's frontmatter as `id: <16-hex>`. The guard that
routes writers through the artifact tools reads the file's **own text** for an `id:`
line; the catalog derives ids from the path and does not need one. So a fully
registered tracker with no `id:` line is completely unguarded — and an unguarded
ledger accumulates hand-edits in arbitrary shapes, because no surface imposes one.
The most structurally damaged tracker in this repo was precisely the one with no
`id:` line.

## Querying with the librarian

The canonical "what's live right now" query — archived rows are hidden
by the default scope:

```
artifact(action="find", kind="tracker")
```

For bugs, swap the kind. **Constrain on both non-terminal states, not just `open`** —
the vocabulary has two, and `investigating` is what this guide tells you to set while
you are actively working a bug, so `status="open"` alone hides exactly the bugs someone
is in the middle of:

```
artifact(action="find", kind="bug",
         filter={"status": {"in": ["open", "investigating"]}})
```

`status="open"` remains right when you specifically mean *not yet started*.

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
