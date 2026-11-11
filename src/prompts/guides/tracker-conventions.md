# Tracker Conventions

codescout keeps three long-lived markdown surfaces: **bugs** (`docs/issues/`, one file per
incident), **trackers** (`docs/trackers/`, multi-entry living state — session logs,
observation tables, ADR indexes) and **specs / plans / ADRs** (`docs/specs/`,
`docs/plans/`, `docs/adrs/`). The librarian indexes all three: find with
`doc(action="find", kind="bug" | "tracker" | …)`, read with `doc(action="get", id=...)`.
Never edit a tracker file by raw path lookup — go through the catalog.

This guide keeps each rule and its trap. The full wording, the derivations and a pointer to
every other page that states the same thing are in
`docs/conventions/tracker-conventions-derivations.md`, under the same headings.

## Bug files (docs/issues/)
<!-- serves: doc.create(path~docs/issues/) -->

One file per bug.

- **Path:** `docs/issues/YYYY-MM-DD-<slug>.md` while open;
  `docs/issues/archive/` once the fix is **verified on `experiments`** —
  reaching `master` is NOT required.
- **Slug:** short kebab-case noun-phrase (3–6 words), e.g.
  `edit-code-insert-mid-function`.

### Bug frontmatter and status vocabulary
<!-- serves: doc.create(path~docs/issues/), doc.get(path~docs/issues/), doc.find(path~docs/issues/) -->

**Frontmatter:** every bug file has `kind: bug` plus a `status:` field.
The librarian classifier auto-recognizes the file on next reindex.

| Status | Meaning |
|---|---|
| `open` | Logged, investigation not started or paused |
| `taken` | A **live** session holds this right now. Requires `claimed_by: <sessionId>`; `doctor`'s `claim_liveness` resolves it |
| `investigating` | Worked, but **no live owner** — the residue of a claim whose session ended without concluding |
| `fixed` | Root cause addressed, regression test added, verified |
| `mitigated` | Workaround in place; root cause not addressed |
| `wontfix` | Intentionally not fixing; justification in the file |
| `zombie` | No longer observed but root cause unconfirmed. Pair with `last_observed:` and a re-open trigger |

### Claiming and releasing a bug
<!-- serves: doc.get(path~docs/issues/), doc.find(path~docs/issues/) -->

**Claiming a bug.** Set `taken` with your own sessionId through the catalog — a raw
frontmatter edit does not reach it:

```
doc(action="update", id=…,
         patch={"status": "taken",
                "extra": {"claimed_by": "<sessionId>", "claimed_at": "YYYY-MM-DD"}})
```

Store the sessionId and nothing else (your scratchpad path carries it). **Release** with
the same call: `status: "investigating"` and `extra: {"claimed_by": null, "claimed_at": null}`.
`closed:` stays empty until the bug is `fixed` / `mitigated` / `wontfix`. The procedure
also lives in `docs/issues/_TEMPLATE.md`.

### The unverified caveat, made queryable
<!-- serves: doc.get(path~docs/issues/), doc.find(path~docs/issues/) -->

**`unverified:` — the caveat, made queryable.** Add it when `status` overstates what was
established. Absence means nothing outstanding, so never add it empty. A settled doubt keeps
its text under a DISCHARGE MARKER as the first word, uppercase: `CLEARED`, `REFUTED`,
`RESOLVED` or `WITHDRAWN`. Never mark an escalation: a caveat that got worse must keep
reporting. Examples: `docs/issues/_TEMPLATE.md`.

### Settled caveats — STANDING and TRACKED

`STANDING` marks a limit NO action can settle; it silences the record. `TRACKED <16-hex id>`
marks remaining WORK moved into an artifact whose status a query reads, and stays silent only
while that artifact is open. **Never write `STANDING` over work** — it deletes the only
surface still reporting it.

### Trigger rules and capture discipline
<!-- serves: doc.create(path~docs/issues/) -->

**Trigger rules — open a bug file for ANY bug noticed during work:** the moment you notice
it. The cases that do and do not qualify are listed in `docs/issues/_TEMPLATE.md`.

**Archive trigger:** move the file into `docs/issues/archive/` once the fix is verified on
`experiments` — gate green and a regression test in place. Archived experiments-only, the
file MUST carry the fix SHA labelled **`experiments`**.

### Closing a bug — patch-id and fix provenance

**Record a `patch-id` beside the SHA:** `git show <sha> | git patch-id --stable` hashes the
diff, so it survives rebase and cherry-pick. Redirect to a file, not a pipe (Iron Law 3);
the recipe is in `docs/RELEASE.md` § *Citing a fix: SHA + patch-id*.

**Write the pair as a `## Fix provenance` section.** `doctor`'s
`terminal_status_without_fix_anchor` reads the SHAPE and refuses a `fixed` or `mitigated`
record without it. A loose hash in prose is worse than none: the record then reads as
anchored.

````markdown
## Fix provenance

- **SHA:** `5a72304c` (`experiments`)
- **patch-id:** `e9f8df63b911`
````

- **Plural:** repeat the pair; each `- **patch-id:**` binds to the `- **SHA:**` above it.
  No tables — no check reads one.
- **Another repository:** `<repo>:<sha>`; the dead-pointer check skips it.
- **A fenced example is a quotation, not a declaration.**
- **No commit at all:** put `no_fix_commit: "<reason>"` in frontmatter. An empty value does
  not discharge it.

**Never cite a merge commit's patch-id** — it is empty for a clean merge and a hash of only
the resolution hunks for a conflicted one. Cite the merged branch's commits, and name the
merge as an event. `doc(action="update")` to `fixed` or `mitigated` without a recoverable
pair returns `fix_anchor_advisory`; archiving stays refused until the pair or
`no_fix_commit:` is present.

### Archiving a bug or tracker — through the catalog, never git mv
<!-- serves: doc.move(path~docs/issues/), doc.move(path~docs/trackers/) -->

Archive through the catalog — `doc(action="move", id=…, new_rel_path="docs/issues/archive/…")`
— never a bare `git mv`: the id is `sha256(abs_path)`, so a hand-move orphans the catalog
row. The move re-keys the artifact. Read the new id from the response (`id`, `previous_id`,
`id_changed`); the old one stops resolving.

### Archiving a bug or tracker — stage both halves of the move
<!-- serves: doc.move(path~docs/issues/), doc.move(path~docs/trackers/) -->

**And stage BOTH halves of the move together — `git add -- <old> <new>`.** `git add -u` and
`git commit -a` stage only the deletion and commit an archive that silently un-happened.
Confirm both halves are lettered in column 1 of `git status --short`: one `R`, or a `D` plus
an `A`. **Do not confirm on `R` alone** — it is a similarity verdict that can vanish, or pair
over a stale destination copy. The `move` response carries `stage_hint`.

### Archiving a bug or tracker — re-point the citations
<!-- serves: doc.move(path~docs/issues/), doc.move(path~docs/trackers/) -->

**Then re-point the citations, in the same commit as the move — paths *and* 16-hex ids.**
Every citation of `docs/issues/<slug>.md` and of its id is a scheduled break.

```
grep -rn 'docs/issues/<slug>.md' . \
  --include='*.md' --include='*.rs' --include='*.sh' --include='*.py' \
  --include='*.yml' --include='*.toml' --include='.env*'
```

A filter that misses a file type returns a clean zero that reads as "none to fix"; when unsure,
drop the filters. Fix the **live** surfaces. **Leave `docs/issues/archive/**` and superseded
session-log rounds alone** — they are historical. The grep is the check, not a green CI run.

### Verifying the citation sweep
<!-- serves: doc.move(path~docs/issues/) -->

**Verifying the sweep afterward needs a different pattern than the sweep — never the slug.**
A move changes only the directory, so a slug grep returns the same hit either way. Grep the
full old path `docs/issues/<name>.md`, and pair it with the control
`git grep -F 'docs/issues/archive/<name>.md'`, which must return non-zero. Move before you
write a citation, not after.

### Reading move's inbound_path_citations
<!-- serves: doc.move(path~docs/issues/) -->

`doc(action="move")`'s `inbound_path_citations` sees BOTH forms, and tells you which is
which. `inbound_citations_cleared` lists entries that cannot cite the old path: nothing is
owed there. An entry **not** cleared may hold a dead path citation or a surviving bare stem —
look at those. `citation_stem_preserved: false` clears nothing: the empty list then means
"not checked".

### Citing an archive path — where the file is, not where it will go

**Cite where the file IS, never where the archive flow will put it.** Archiving is a later,
manual move, and a partial fix cancels it, so a citation of `docs/issues/archive/<slug>.md`
for a bug still in `docs/issues/` is never repaired by the sweep.

### Why a green gate proves nothing about the sweep
<!-- serves: doc.move(path~docs/issues/) -->

Be precise about *why* a green run proves nothing here: `audit_doc_refs` finds the citation,
but reports it at `Med` on purpose, so `--fail-on high` passes. The gate is silent by design;
the grep is the check.

## Tracker artifacts (docs/trackers/)
<!-- serves: doc.create(path~docs/trackers/) -->

Trackers are living state — multi-entry tables, observation logs, ADR indexes — indexed by
the catalog and optionally augmented with a persistent prompt.

**Frontmatter shape** (required for new trackers; `id:` is optional):

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

| Status | Meaning | Visibility |
|---|---|---|
| `active` | Living tracker, actively appended to | visible |
| `draft` | Scoped / watching, not yet active | visible |
| `archived` | Terminal — work-stream wrapped | **hidden by default** |
| `superseded` | Replaced by a successor artifact | **hidden by default** |

**`draft` means the opposite on a plan:** in `docs/plans/` it means *shipped, deliberately
unarchived because a named residual is open*. Before reading a `draft` as "not started",
check whether a `resume-*` tracker names it.

### Archiving a tracker

**Preferred:** in-place archival through the catalog:

```
doc(action="update", id="<id>", patch={"status": "archived"})
```

To also move the file on disk, use `doc(action="move", id="<id>",
new_rel_path="docs/trackers/archive/foo.md")` — never a bare `git mv`.

### A `## Resume` states its own currency
<!-- serves: doc.get(path~docs/trackers/) -->

A tracker that accumulates review passes by *appending* leaves its oldest "start here"
section at the bottom. **A `## Resume` either dates its heading or opens with a banner naming
what superseded it:**

    ## Resume — state at compaction, <date>

    > **CLOSED <date> — merged at `<sha>`.** Read § *Integration* for the current state.

The marker makes staleness legible; it does not prevent it.

## Declaring an augmentation
<!-- serves: doc.augment(path~docs/trackers/) -->

**Before you augment: `merge=false` resets every field you omit, and under `merge=true` a
`params` array replaces the WHOLE collection** — a one-row patch deletes the other rows, and
the catalog is not in git. Change rows with `append_entry` / `update_entry`, never by
patching `params`.

If an artifact is meant to carry an augmentation, say so in frontmatter and name the
committed sidecar:

```yaml
expects_augmentation: docs/augmentations/docs-trackers-tool-usage-patterns.yaml
```

The sidecar carries the shape (`prompt`, `params_schema`, `render_template`,
`entry_collection`, `append_mode`, `history_cap`), never `params`. Write it with
`librarian(action="doctor", fix="export_augmentations")`, which exports only rows THIS catalog
holds. Restore procedure: `docs/conventions/cross-machine-catalog-resume.md` § 7.

### Declaring an augmentation — why the sidecar exists, and the check's remedy
<!-- serves: doc.augment(path~docs/trackers/) -->

The catalog is machine-local and git-ignored, so the sidecar is the only on-disk form of an
augmentation, and its loss is silent. **A stale sidecar is worse than an absent one:**
`augmentation_declared_but_absent` reports absence, while staleness restores clean. If the
declared sidecar exists, run `reindex`. If it does not, export it on the machine that holds
the rows, or re-author it with `doc(action="augment", …)`.

## Entry-level standard — the shape INSIDE a tracker
<!-- serves: doc.append_entry(path~docs/trackers/), doc.update_entry(path~docs/trackers/) -->

These rules govern a tracker's **entries** (`F-3`, `R-91`, `HY-11`, `BUG-40`). The citation
resolver enforces them; breaking them silently breaks the link graph.

### Declaring a ledger
<!-- serves: doc.create(path~docs/trackers/) -->

A **ledger** owns an id namespace; most trackers own none and stay directly editable. Declare
it in **frontmatter**, because the catalog is machine-local and git-ignored:

```yaml
entry_prefix: R          # or a sequence, for a ledger owning two namespaces:
entry_prefix:            #   a session log carries both F-N frictions and W-N wins
  - F
  - W
```

Set it with `doc(action="update", id=…, patch={extra: {"entry_prefix": "R"}})`. A ledger is
declared, never inferred: a design doc that quotes `## R-4` is not a namespace. Concept pages:
`docs/manual/src/concepts/entry-citations.md`, `link-scan.md`, `statement-validity.md`.

### A prefix has one owner per repository
<!-- serves: doc.create(path~docs/trackers/) -->

Two ledgers owning one prefix bind each other's citations silently, so the librarian refuses
it: `doc(action="create")`, `doc(action="update")` and `rekey_prefix` name the owner and write
nothing. `entry_prefix` and `external_prefix` both own a prefix, and archived ledgers still
own theirs. `F` and `W` are the one shared family, cited qualified by file stem. A hand-edited
frontmatter bypasses the refusal; `doctor`'s `entry_prefix_declared_twice` reports it. Use two
or three letters, **never more than three** (the grammar is `[A-Z]{1,3}-\d+`).

### Entry ids
<!-- serves: doc.append_entry(path~docs/trackers/) -->

**Never suffix an id.** `R-72b` can never be defined or cited; give a clash a **fresh** id.
**Let the server allocate and write the section:** `doc(action="append_entry", id_prefix="R",
anchor_heading="## Template for new entries", title=…, body=…, index_row="| {id} | … |")`
writes `## R-N — <title>` (the only shape that defines a citable token) and the index row in
one write. Omit `anchor_heading`, `title` or `body` and it only reserves the id. Hand-allocating:
scan **every** entry format in the file.

### Entry ids — refusals: main checkout only (prose ledgers)
<!-- serves: doc.append_entry(path~docs/trackers/) -->

`append_entry` refuses id allocation from a worktree session: write to a worktree-local file
and fold it in from the main checkout. `pre-push` catches a peer's duplicate id.

### Entry ids — where the counter lives
<!-- serves: doc.append_entry(path~docs/trackers/) -->

A ledger keeps its high-water mark in committed frontmatter, one key per namespace:

```yaml
entry_prefix: HY
entry_high_water_HY: 11
```

Never hand-edit it downward or delete it when compacting: it survives a fresh clone, a move
and compaction.

### Entry ids — when two hosts allocate anyway

Diverged clones can issue the same id, and the merge then holds two `## PREFIX-N` sections.
`doctor` reports `entry_defined_twice`. It is a detector, not a guard: establish which
definition is stale before renumbering.

### Entry headings — the definition rule
<!-- serves: doc.append_entry(path~docs/trackers/) -->

An entry is defined by a heading of exactly this shape:

```
## <ID> — <title>          token, whitespace, dash (— – -), whitespace, text
```

| Heading | Defines? |
|---|---|
| `## R-91 — the scout ran too late` | yes |
| `## R-91` (no title) | **no** |
| `### A-9 Addendum` (no dash) | **no** |
| `\| R-91 \| … \|` (table row) | **no** — rows never define |

An undefined-but-cited id is **dangling**.

### Entry headings — a prefix with no definer, or owned outside markdown

A prefix nothing defines or declares resolves as prose noise; `doctor`'s
`cited_prefix_with_no_definer` reports it above a citation-volume threshold. A prefix owned
OUTSIDE markdown (a benchmark suite, a JSON fixture) takes neither ledger remedy; declare it:

```yaml
external_prefix:
  TC: scripts/tc-suites/legacy-natural.json   # path relative to the repo root
```

The check stays silent only while that file exists and holds a `TC-<n>` id.

### Citing an entry — bare, or qualified
<!-- serves: doc.update_entry(path~docs/trackers/) -->

Cite by **bare token** when the prefix has one ledger: `R-98`, `HY-10`. Cite **qualified by
file stem** when several files share it:

```
bug-fix-session-log:F-33      → resolves to that log's F-33
F-33                          → several definers, Ambiguous, resolves to nothing
```

A qualifier naming no file in this repo is a cross-repo reference (`codescout:A-11`): reported,
never an edge. One naming a file that lacks the entry is **dangling**, not ambiguous.

### Citing an entry — a third shape, and mentioning without citing
<!-- serves: doc.update_entry(path~docs/trackers/) -->

`<repo>:<file-stem>:<TOKEN>` has no grammar: `link_scan` reports it and never resolves it.
**Mentioning an id without citing it:** `link_scan` derives a `cites` edge from any entry token
in prose, inline backticks included. Only a fenced code block is literal.

### One entry format, never two

Do not keep an index table beside body sections. **The headings are the index:** `link_scan`
derives a definition from a `## <ID> — <title>` heading and nothing else, so a row defines
nothing. `render_template` projects into `librarian(action="context")` only and writes no
body. A hand-maintained row table *in addition to* headings is fine.

### Required fields
<!-- serves: doc.append_entry(path~docs/trackers/), doc.update_entry(path~docs/trackers/) -->

- **`**Status:**` is not optional:** it is the only thing that makes a fired `Promote-when`
  harvestable. Update it when a criterion fires.
- **`**Valid:**` declares a decay class:** `invariant`, `dated YYYY-MM-DD` or
  `conditional — <event>`, no fourth form; a malformed one is refused. Absence means
  `dated <its last commit>`. Detail: `docs/manual/src/concepts/statement-validity.md`.

### Required fields — Rests on, and detecting the lines
<!-- serves: doc.append_entry(path~docs/trackers/), doc.update_entry(path~docs/trackers/) -->

- **`**Rests on:**` is the durable route back to the proof** — an ADR, decision or principle,
  not a `path:line`. Both fields are detected **line-anchored**; a fenced line is skipped.

### Detecting these fields
<!-- serves: doc.append_entry(path~docs/trackers/), doc.update_entry(path~docs/trackers/) -->

Anchor detection on **structure** — line-start, a key prefix — never on a keyword.

### Compaction and archival

The ladder is **live body → archived section (heading kept) → nothing further.**

- **Never reduce an entry to a bare row to "compact" it.** That destroys its definition and
  dangles every citation of it.
- A unique definer resolves even when its artifact is `archived`; of two definers, the sole
  *active* one wins.
- **Archive into the ledger's existing archive artifact** — find it first with
  `doc(action="find", filter={"rel_path": {"contains": "archive/<ledger>"}},
  include_archived=true)`. A second archive splits the definitions.

### Make the tracker guarded
<!-- serves: doc.create(path~docs/trackers/) -->

**Declare `entry_prefix`.** That alone guards a ledger (`src/util/librarian_guard.rs`), because
only the server may advance the `PREFIX-N` counter. **Do NOT stamp `id: <16-hex>` into a
tracker's frontmatter to guard it:** that guards on catalog membership, which would refuse
`docs/RELEASE.md` and every ADR. An artifact that owns no id namespace needs no guard.

## Querying with the librarian
<!-- serves: doc.find(path~docs/trackers/), doc.find(path~docs/issues/), doc.get(path~docs/trackers/) -->

The canonical "what's live right now" query — archived rows are hidden by default:

```
doc(action="find", kind="tracker")
```

For bugs, constrain on all four non-terminal states, not just `open` — `status="open"` alone
hides `taken`, `investigating` and `zombie`:

```
doc(action="find", kind="bug",
         filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})
```

A `zombie` hit is a "has this recurred?" check, not a task (see § Bug frontmatter and status
vocabulary). Add `include_archived=true` to surface archived rows. Read a tracker with:

```
doc(action="get", id="<id>", full=true)
doc(action="get", id="<id>", heading="## Foo")
```

**Filterable trackers:** pass `entry_filter={…}` (same AST as `filter`) to `doc(action="get")`
once the augmentation declares an `entry_collection`. Prose trackers need retrofit first.
Deeper mechanics: `get_guide("librarian")`; the session-passover pattern:
`get_guide("librarian-runtime")` § *Trackers as cross-session behavior*.

## Cross-linking (edges are derived — cite in prose)
<!-- serves: doc.link(path~docs/trackers/), doc.link(path~docs/issues/) -->

- **Cite by ID in prose.** Entry IDs (`A-11`, `F-3`, `BUG-40`), artifact ids (16-hex) or
  rel_paths, `<repo>:<ID>` across repos. Prose is the ONLY write surface for citations — never
  hand-create `cites` edges. **Entry IDs are stable; artifact ids are not** (a 16-hex id changes
  when the file moves), so prefer an entry ID or rel_path for a target likely to be archived.
- **`link_scan` derives the edges:** `librarian(action="link_scan")` reports, `write=true`
  applies. It is idempotent and heals what bypasses `doc(action="move")`. Detail:
  `docs/manual/src/concepts/link-scan.md`.
- **Manual rels are few and deliberate:** `evidence-for`, `promoted-to`, `refutes`.
  `supersedes` flips the destination's status: archiving a tracker that has a successor
  REQUIRES a `supersedes` edge through `doc(action="link")`, never a bare status edit.
