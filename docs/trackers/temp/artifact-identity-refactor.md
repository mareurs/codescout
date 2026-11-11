---
id: '11304cb8f5373d45'
kind: tracker
status: superseded
title: Artifact identity refactor — one stable key for move, archive and worktree
tags:
- identity
- slug
- librarian
- worktree
- lifecycle
- temp
topic: artifact identity refactor
---

# Artifact identity refactor — one stable key for move, archive and worktree

> **Temporary tracker.** It is the first file in `docs/trackers/temp/`. The cleanup policy
> for that folder is parked until measured: the tracker-hygiene sweep records `temp/` data
> per HY-34 (`tracker-hygiene-log.md`). Until a policy exists, this file leaves `temp/` by
> hand under § Exit.

## Why

Moving or archiving a tracker, and working on one from a worktree, are expensive and
error-prone today. The cause is that one document has three identities, two of them tied to
its path. Any reorganization of `docs/trackers/` into folders (the goal of the brainstorm)
pays this cost once per moved file, so the identity model should be settled first.

## Current state — three identities for one document

Verified against the code on 2026-11-11.

| Layer | Derived from | Where it lives | Changes when | Read by |
|---|---|---|---|---|
| `id` | `sha256(abs_path)`, first 16 hex — `src/librarian/ids.rs` `artifact_id_from_abs` | the catalog (machine-local, git-ignored); also stamped as `id:` in frontmatter | any move or rename; also differs between the main checkout and a worktree, because the absolute path differs | links, events, augmentations, observations, 16-hex citations in prose, file-grain `entry_cite.dst_ref` |
| `slug` | `slugify(title)`, base capped at 50, `-2`/`-3` dedup against the catalog; stem fallback — `src/librarian/catalog/artifact.rs` `ensure_slug` | the catalog only; no tracker carries it in frontmatter | never — immutable once minted; `mv` carries it forward through `graft_rows` | `entry_cite.src_slug` (FK to `artifact(slug)`), `<slug>:<local>` in `entry_cite.dst_ref` |
| file stem | the filename without `.md` | the path | any rename | qualified prose citations `<stem>:TOKEN`; `link_scan` resolves the qualifier through `Corpus::by_stem` (`src/librarian/tools/link_scan/resolve.rs`) |

`resolve.rs` states why citations use the stem and not the slug: a slug is minted from the
title with a dedup suffix, so an author cannot predict it, "and a citation an author cannot
predict is not a citation". The result is that prose cites by stem while the graph stores
the slug: two keys for one thing.

895 files under `docs/trackers`, `docs/issues`, `docs/plans` and `docs/superpowers` carry a
16-hex `id:` in frontmatter (grep, 2026-11-11). That stamped value is the path hash, so it
becomes wrong at the first move; `librarian(action="doctor")` reports it as
`frontmatter_id_mismatch` and `fix="repair_frontmatter_id"` rewrites it.

## Observed costs

1. **A move mints a new `id`.** Every 16-hex citation of the old id and every stamped
   frontmatter `id:` goes stale. `get_guide("tracker-conventions")` § *Archive — re-point the
   citations* makes the caller repoint paths and ids in the same commit, by grep.
2. **A stem-cited log cannot be renamed when it is archived.** The hygiene skill (D10, step
   5b) keeps the name for any log cited as `<stem>:TOKEN`, because a rename breaks those
   citations and `link_scan` reports them as `CrossRepoToken`, not as dangling.
3. **A worktree sees a different `id` for the same file.** That is why worktree shadow rows,
   `librarian(action="merge_worktree")` and `doctor` `fix="reseat_worktree"` exist, and why
   `append_entry` refuses to allocate an entry id from a worktree session.
4. **Risk, not verified:** the slug lives only in the machine-local catalog and its dedup
   suffix depends on mint order, so two machines might mint different slugs for one
   document. Check this before relying on it.

## Prior decisions

- [`tracker-management-redesign.md`](../tracker-management-redesign.md) TMR-4, accepted
  2026-07-17: artifact identity is decoupled from the filesystem path, and dead references
  get a lifecycle. It binds new design; it does not mandate a retrofit.

## Candidate directions — not decided

1. **A committed stable key.** Mint a key once, write it to frontmatter, and make it the
   catalog's primary key; the path becomes an attribute. Moves and worktrees keep the key.
   Cost: re-keying every catalog table, and a decision about the 895 stamped path-hash ids.
2. **Promote the slug to frontmatter.** Commit it, make it author-visible, and make prose
   citations qualify by slug instead of by stem. The path-hash `id` stays internal.
3. **Keep the model, automate the repair.** `doc(move)` rewrites every citation of the old
   path, id and stem itself. Smallest change; the worktree split remains.

## Open questions

- Which of the three layers can be removed, rather than synchronized?
- What happens to an existing `<stem>:TOKEN` citation under each direction?
- Does the stable key need to be unique across repositories, or only within one?

## Exit

This tracker leaves `temp/` when the direction is chosen and written as a spec under
`docs/superpowers/specs/` (link it with `supersedes`, then archive), or when the refactor is
dropped (archive with the reason).

## Log

### 2026-11-11 — created

Created during the tracker-organization brainstorm, as the first resident of
`docs/trackers/temp/`.


### 2026-11-11 — direction taken up by the identity spec

The direction is now the spec `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`
(draft v5, `2fbffdca1`, owned by a parallel session): the lowercase filename stem is the key,
committed files are the only store, and one closed citation grammar replaces the slug, the
path-hash id and stem citations. The operator approved that spec (v5) the same day, so the exit
fired: `doc(link, rel="supersedes")` from the spec set this tracker to `superseded`.

**Superseded by:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`

The file stays in `temp/` until the tracker-lifecycle migration moves it, because an archive
move now would land in the legacy layout. HY-34 counts it as the first `temp/` outcome.
