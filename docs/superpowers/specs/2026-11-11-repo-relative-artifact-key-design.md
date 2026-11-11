# Artifact identity and storage: committed files are the only store (design)

**Status:** draft v4, 2026-11-11. The operator accepted the governing principle on
2026-11-11; § Decision log records every decision. Source tracker:
`docs/trackers/temp/artifact-identity-refactor.md`. Session log:
`docs/trackers/artifact-identity-session-log.md`. The parallel tracker-organization session
(`codescout-37`) owns the folder layout and the ledger formats; this spec owns identity, the
citation grammar, and the boundary between committed files and the catalog.

**Scope rule from the operator:** the current state of trackers and ledgers does not
constrain the design. Files may be renamed, rewritten or regenerated. The goal is a correct,
scalable design, not an easy migration.

## Intent

Today the librarian gives one document three identities: a path-hash `id`
(`src/librarian/ids.rs:17-23`), a title-minted `slug` (`ensure_slug`), and the file stem
that prose cites. It also keeps durable state that exists only in each machine's catalog:
events, augmentation params, observations and entry reservations. Most identity machinery
exists to keep that local state attached when a path changes: graft on move, `fix=rehome`,
the worktree overlay (shadow rows, fork and merge events, `merge_worktree`, `reseat`),
frontmatter id repair, and slug dedup.

This design removes the cause instead of managing it:

> **Committed files are the only store. The catalog is an index derived from a commit.
> Identity is the key in the filename.**

Two clones at the same commit build the same catalog. A move, a relocation, a fresh clone,
a worktree and a second machine all reduce to "index this commit".

## Measurements (2026-11-11, read-only)

| Fact | Number | Source |
|---|---|---|
| Renames under `docs/` since January | 1217, of which 1112 into `archive/` | git log |
| Commits that re-point citations after an archive or move | 144 | git log subject grep, reproduced |
| Dangling `link_scan` citations that are 16-hex ids | 449 of 867 | `codescout-37` census |
| Bare 16-hex ids in prose (tracked `.md`) | 676 occurrences; 335 distinct already stale | `stalehist.py` |
| Prose citation forms | about 10,600 path, 2435 bare hex, 1027 `<stem>:TOKEN`, 243 `[[…]]` outside code | census, `wiki2.py` |
| Ambiguous `link_scan` citations that are bare `F-n`/`W-n` | 700 of 754 | `codescout-37` census |
| Catalog events | 11,537: 11,346 `field_patch`, 64 `worktree_fork`, 47 `worktree_merge`, 57 `note`, 10 `reviewed`, 6 `superseded_by`, 3 `status_change`, 3 `verdict`, 2 `external_signal`, 1 `intent` | catalog |
| What a codescout `field_patch` records | 6423 rows, all `field=body`; payload is byte sizes and an edit count only; no content, no commit | catalog |
| Augmentations (codescout) | 41; 17 declare an entry collection holding 540 entries; 535 KB of params | `augstate.py` |
| Params entries with no heading or row in the markdown | 38, all in `docs/trackers/provenance-subsystem.md` | `augstate.py` |
| Hosts writing this repo's audit shards | 2 (`archlinux`, `ripper`) | `.codescout/audit/` |
| Live sessions on a replaced binary | 17 of 30 on 2026-10-16 | `src/librarian/requirements.rs:8-12` |
| Reindex deletes a moved file's local history | `indexer.rs:810-854`; 7 tables cascade | bug `99a4d0be1318c3c1` |

## Goals

- **G1. One store.** Every durable fact about a document lives in committed files. The
  catalog holds nothing that cannot be rebuilt from a commit.
- **G2. One identity.** A document's identity is its **key**: its filename stem, or its
  folder's name for a folder-index file. The key is canonical and unique within its repo.
- **G3. Location-free.** Moving a file between folders, archiving it, relocating the
  checkout, opening a worktree, or cloning on another machine changes no key and loses no
  state.
- **G4. One citation grammar,** closed and parseable, in which every reference to a
  document or an entry is written. A rename can therefore rewrite every citation
  mechanically and completely.
- **G5. Identical catalogs.** Two clones at the same commit hold catalogs that answer every
  query the same way.
- **G6. Tool output shows only keys and grammar forms.** Agents copy what tools show, so
  tools never show an id, a hash or a path for a document reference.

## Non-goals

- The folder layout, the ledger formats and the entry id scheme (sequential `PREFIX-N` or
  dated per-entry files). These belong to `codescout-37`; § Contract with ledger formats
  states what this design requires of them.
- Edges across repos. A citation of another repo stays report-only.
- Rendering `[[…]]` as links on a forge. Readers on GitHub see the text form.
- Manual moves by a human.

## Anti-goals

The change fails, even if it works, if any of these is true:

- Any durable fact exists only in a catalog.
- A key depends on a title, an index order, a machine, a checkout location or another repo.
- A move between folders changes a key or breaks a citation.
- Two clones at the same commit disagree about a document's identity, state or edges.
- A tool shows an agent a form of reference that does not survive a move.
- An older binary and a newer binary sharing a catalog damage each other.

## Constraints

- **Older binaries share the catalog.** 17 of 30 live sessions ran a replaced binary on
  2026-10-16. An older binary reruns its own migrations on open.
- **`librarian.toml` is closed to new tables.** `classify::RuleFile` is
  `#[serde(deny_unknown_fields)]` (`src/librarian/classify.rs:5-9`).
- **Concurrent writers.** Several sessions write one checkout at once. Committed files turn
  their concurrent writes into git merges, so formats must keep merges trivial.
- **Search needs embeddings.** Vectors are derived data but are expensive to rebuild; they
  are a cache keyed by content hash, never state.

## Approaches

For where durable state lives:

1. **Committed files only; the catalog is a derived index (chosen).** Cost: every write
   tool writes files, history queries read git, and a one-time export moves today's
   catalog-only state into files. Gain: every location problem becomes "index this commit",
   and the worktree overlay, graft, rehome, reseat, the uid and aliases all go.
2. **v3: an internal ULID per row, durable state in the catalog, aliases for old
   references.** Rejected: each clone still diverges on catalog-only state, so moves pulled
   from another machine still need rebind rules, and the worktree overlay stays.
3. **A committed GUID in frontmatter, state in the catalog.** Rejected: two names per file,
   and the catalog-only state problem stays.

For the key: the filename stem, canonical by construction (chosen), over a committed slug
(two names per file) and the repo-relative path (a folder move changes it).

For the repo name: a declared name in `.codescout/repo.toml`, confirmed at onboarding
(operator decision), over the directory basename and the git remote.

## Design

### The store boundary

Every state class has one committed home. Nothing in the catalog lacks one.

| State today | Catalog-only today? | Committed home |
|---|---|---|
| Frontmatter (kind, status, title, tags…) | no | the document, unchanged |
| Ledger entries in augmentation params | yes, partly (38 entries have no markdown at all) | the markdown entries, in the format `codescout-37` defines; params cease to be a store |
| Augmentation prompt, schema, render template | yes | a committed sidecar per document, extending what `export_augmentations` writes today |
| `field_patch` events (11,346) | yes | none needed: git history of the file records more (the diff) |
| `status_change`, `superseded_by` events | yes | the frontmatter field, with git history as its log |
| `note`, `verdict`, `intent`, `reviewed`, `external_signal` events (73) | yes | text in the document, in a section the event writer appends to |
| `worktree_fork` / `worktree_merge` events (111) | yes | none: the worktree overlay is removed |
| Observations (57) | yes | text in the document, as for notes |
| Entry reservations / high-water marks | partly (frontmatter holds `entry_high_water_*`) | frontmatter, or none if `codescout-37` chooses per-entry files |
| Write-time `cites` | yes (13 `origin='write'` rows) | grammar text inside the entry |
| Manual links (`doc(link)`) | yes | grammar text in the source document, under a rel label |
| Catalog audit trail | yes (shards are exported) | git history with `Session-Id` trailers; the catalog audit tables are retired |

The catalog keeps only derived data: the document index, the parsed entries, the edge
graph, git-history caches, and embedding vectors keyed by content hash.

### Identity: the key

- **The key is the filename stem.** Filenames are canonical keys: `[a-z0-9]+(-[a-z0-9]+)*`.
  `doc(create)` and `doc(move)` refuse a non-canonical name; existing files are renamed
  once, during the cutover. There is no normalization layer: key and stem are the same
  string, byte for byte.
- **Folder-index files** take their folder's name: one fixed filename (`index.md`; the
  exact name is `codescout-37`'s to choose) makes a folder a document.
- **Unique per repo.** Two catalogued documents in one repo never share a key. `doc(create)`
  and `doc(move)` refuse a collision and name the other file; `doctor` and a pre-commit check
  catch one that arrives another way. There is no `Ambiguous` outcome for a key.
- **A key of exactly 16 hex characters is refused,** so no key is mistaken for a legacy id.
- **Repo name.** `.codescout/repo.toml` holds `name = "<name>"`, same charset as a key. It
  is a label: it qualifies cross-repo citations and appears in tool output. Onboarding
  suggests one (the `.codescout/workspace.toml` name if present, else the directory name),
  says when another checkout in the catalog uses it, and asks the user to confirm: through
  MCP elicitation when the client supports it
  (`src/tools/run_command/interactive.rs:36-98`), otherwise through the agent and a new
  `onboarding` parameter.

### The citation grammar

One closed grammar for every reference to a document or an entry:

| Form | Means |
|---|---|
| `[[key]]` | the document `key` in this repo |
| `[[key:TOKEN]]` | entry `TOKEN` owned by document `key` |
| `[[repo/key]]`, `[[repo/key:TOKEN]]` | the same in repo `repo` (report-only across repos) |
| bare `TOKEN` | an entry of **the same document** only |

- **Scoping.** A bare token resolves only inside its own document, the way a local name
  resolves in its own scope. Every other reference is qualified. The 700 ambiguous bare
  `F-n`/`W-n` citations stop being ambiguous by construction.
- **Ownership.** `[[key:TOKEN]]` resolves when the token is defined in document `key` or in
  a descendant file whose nearest folder-index ancestor is `key`'s index. A folder without an
  index owns nothing and passes its files up. This lets per-entry ledgers and their
  `archive/` subfolders resolve under the ledger's key.
- **Escape.** Code fences and inline code are never parsed, so TOML `[[rule]]` and regex
  `[[:space:]]` stay inert.
- **Paths and hex ids.** A markdown link or a bare path to a catalogued document, and a
  16-hex id, are not part of the grammar. `link_scan` reports them as lint with the grammar
  form to use; it never turns them into edges. The cutover rewrites today's resolvable ones.
- **A rel label** for a manual link is written in the grammar's text, for example
  `**Supersedes:** [[old-key]]`; the label vocabulary belongs to the link schema, which the
  builder defines.

### The catalog is derived

- The indexer builds the catalog from the files of one checkout at its current commit:
  documents by key, entries, edges from the grammar, sidecar-declared augmentation shapes,
  and caches of git history (status changes, last modification, authors).
- Storage is scoped by checkout. A checkout row holds its root and repo name; every other
  row joins on the checkout and the key. Internal row ids may be plain integers and may
  change on every rebuild, because nothing outside the catalog holds them.
- A rebuild from scratch is always correct; incremental indexing by content hash is an
  optimisation whose result must equal a rebuild (a test asserts it).
- Embedding vectors are a cache keyed by content hash, so a rebuild reuses them.

### Writes

Every write tool writes files, then reindexes what it wrote:

- `append_entry` writes the entry into the markdown, including its `cites` as grammar text.
- `event_create` appends the event's text to the document's event section.
- `doc(link)` writes the labelled grammar reference into the source document.
- `doc(update)` writes frontmatter or body; git records the change.
- `doc(augment)` writes the committed sidecar.

### Moves and renames

- **A move that keeps the filename** (any folder change, an archive) is a file rename and a
  reindex. No key, citation or state changes, so no catalog step and no citation repair
  exist.
- **A rename** changes a key. Because every reference is in the closed grammar, the rename
  tool rewrites every `[[old-key…]]` in the repo in the same change, and the result is
  complete. That tool is part of `codescout-37`'s move workflow; this spec guarantees that
  the grammar makes it complete.

### Worktrees and machines

- A worktree is a checkout. Its catalog view is built from its own files. Its writes are
  file writes, and git merges them. The overlay is removed: shadow rows, fork and merge
  events, `merge_worktree`, `reseat_worktree`, `worktree_registration` and the related
  `doctor` checks.
- The `append_entry` refusal in a worktree has no remaining cause once the ledger format
  merges cleanly (per-entry files, or a format `codescout-37` shows to merge).
- A second machine or a fresh clone indexes the same commit and gets the same catalog.

### Contract with ledger formats

What this design requires of any ledger format `codescout-37` defines:

1. Every entry exists as markdown that defines its token; no entry exists only in params.
2. Two sessions appending entries concurrently produce a trivial git merge.
3. An entry's citations are written in the grammar.
4. Archiving an entry keeps it under its ledger's key (an `archive/` subfolder without an
   index, or the format's own equivalent).

### Removed

`ids::artifact_id_from_abs` as identity, the frontmatter `id:` stamp and its repair and
mismatch checks, `artifact.slug` and its minting and dedup, `graft_rows` on the move path,
`fix=rehome`, the worktree overlay (`resolve_write_target`, shadow rows,
`worktree_registration`, `merge_worktree`, `reseat_worktree`), catalog-held events,
observations and augmentation params as stores, `entry_reservation`, the catalog audit
tables, and the two-shape `entry_cite.dst_ref`.

### Cutover

1. **Export.** A one-time, idempotent export writes every catalog-only fact into its
   committed home (§ The store boundary): params entries without markdown (38 today),
   augmentation shapes, notes, verdicts, intents, observations and manual links. It is
   re-runnable, so writes made by an older binary after the first run are picked up by a
   later one.
2. **Rename.** Non-canonical filenames and key collisions are renamed, with citations
   rewritten.
3. **Rewrite citations.** Resolvable `<stem>:TOKEN`, path links to catalogued documents and
   16-hex ids become grammar forms. Stale hex ids that git history recovers
   (`sha256(<root>/<old path>)[:16]`: 160 of 335 distinct) are rewritten too; the rest are
   reported.
4. **New catalog file.** The new binary builds a new catalog file from the files, so an
   older binary keeps its own file and the two never share a schema.

## Open for the operator

- **When older binaries stop.** The export is re-runnable, so no write is lost while it is
  rerun. What remains is the rule for when the old catalog file may be deleted: after every
  session has restarted onto the new binary, or after a fixed date.

## Left to the builder

- The sidecar format for augmentation shapes, and the section format for events and
  observations inside a document.
- The rel-label vocabulary for manual links.
- Table and column names of the derived catalog, and the git-history cache.
- The pre-commit check for key collisions and non-canonical names.
- Tests: one per anti-goal, at minimum: a rebuild equals the incremental index; two clones
  at one commit give identical query results; a folder move changes no key and breaks no
  citation; a rename leaves no `[[old-key…]]` anywhere; no tool output contains a hex id,
  a path or a row id as a document reference; a worktree write merges through git alone.

## Decision log

Operator, 2026-11-11:

- The stem is the key (over a committed slug and over keying on an id).
- The repo name is declared in `.codescout/repo.toml` and confirmed at onboarding.
- This spec owns the id layer.
- Moves are made by agents through codescout tools only.
- The current state of trackers and ledgers does not constrain the design; files may be
  rewritten.
- **Committed files are the only store; the catalog is a derived index.**

Designer's calls, open to correction:

- Filenames are canonical keys, so there is no normalization layer (supersedes the earlier
  operator choice to normalize, which existed only to avoid renaming files).
- Keys are unique per repo, enforced at write time; there is no collision fallback and no
  `Ambiguous` key outcome.
- One closed citation grammar; a bare token is local to its document; paths and hex ids are
  lint, not edges.
- A folder key owns its descendants' tokens; the nearest folder-index ancestor wins (from
  `codescout-37`'s review).
- The catalog audit tables are retired in favour of git history with `Session-Id` trailers.
- The cutover builds a new catalog file.

Superseded by v4: the internal ULID, `.codescout/aliases.tsv`, the normalization layer, the
collision tolerance, the rebind-by-key rules, and the legacy-id alias map (all v3).
