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

One file per bug, copied from `docs/issues/_TEMPLATE.md`.

- **Path:** `docs/issues/YYYY-MM-DD-<slug>.md` while open;
  `docs/issues/archive/` only **after** the fix has shipped to `master`
  (verify with `git branch --contains <fix-sha>`).
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

**Archive trigger:** move the file into `docs/issues/archive/` AFTER
the fix ships to `master`, **not** when status flips to `fixed`. The
file stays in `docs/issues/` while the fix lives only on a feature
branch.

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

For bugs, swap the kind and (optionally) constrain status:

```
artifact(action="find", kind="bug", status="open")
```

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
`filter`) to `artifact(action="get")`. Prose trackers need retrofit first — see
`docs/conventions/retrofitting-trackers-for-filtering.md`.

For deeper artifact / augmentation / event mechanics see
`get_guide("librarian")`.
