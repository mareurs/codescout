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

- **Cite by stable ID in prose.** Entry IDs in their ledger's namespace
  (`A-11`, `F-3`, `BUG-40`), artifact ids (16-hex) or rel_paths across
  files, `<repo>:<ID>` across repos. Prose is the ONLY write surface for
  citations — never hand-create `cites` edges.
- **`link_scan` derives the edges.** `librarian(action="link_scan")` parses
  artifact bodies, resolves citations (entry tokens by their defining
  heading; archived definers lose ties to active ones; ambiguous tokens are
  reported, never guessed), and materializes/prunes scanner-owned
  `rel="cites"` edges. `write=false` (default) reports; `write=true`
  applies. Idempotent — safe to re-run any time, and the repair path after
  moves/reindex (the catalog's abs_path pre-clean cascade-drops a moved
  artifact's links; the scan heals them).
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
