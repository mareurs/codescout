# Artifact identity and storage: committed files are the only store (design)

**Status:** approved by the operator 2026-11-11 (v5). The operator accepted the governing principle on
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
  derived catalog holds nothing that cannot be rebuilt from a commit. Operational state
  (progress rows, cursors, gc settings) lives in a separate machine-state file whose loss
  costs no fact about a document.
- **G2. One identity.** A document's identity is its **key**: its filename stem, or its
  folder's name for a folder-index file. The key is canonical and unique within its repo.
- **G3. Location-free.** Moving a file between folders, archiving it, relocating the
  checkout, opening a worktree, or cloning on another machine changes no key and loses no
  state.
- **G4. One citation grammar,** closed and parseable, in which every reference to a
  document or an entry is written. A rename can therefore rewrite every citation
  mechanically and completely.
- **G5. Identical catalogs.** Two clones at the same commit hold catalogs that answer every
  query the same way. The index's inputs are the commit's files only (§ Index inputs, guard,
  machine state and entry ids).
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
- An older binary and a newer binary sharing a catalog damage each other. (The migration
  window in § Cutover is how this is avoided.)

## Constraints

- **Older binaries share the catalog.** 17 of 30 live sessions ran a replaced binary on
  2026-10-16. An older binary reruns its own migrations on open.
- **`librarian.toml` is closed to new tables.** `classify::RuleFile` is
  `#[serde(deny_unknown_fields)]` (`src/librarian/classify.rs:5-9`).
- **Concurrent writers.** Several sessions write one checkout at once. Committed files turn
  their concurrent writes into git merges, so formats must keep merges trivial.
- **Search needs embeddings.** Vectors are derived data but are expensive to rebuild; they
  are a cache keyed by content hash, never state. (Today reuse is within one artifact only,
  and vectors die with their artifact row; the content-keyed cache is Plan C work.)

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
| `note`, `verdict`, `intent`, `reviewed`, `external_signal` events (73) | yes | in a single-writer document, a section of that document; on a shared document (a collection index, a long ledger), an entry document of its own (`entry_kind: note` in `codescout-37`'s format) |
| `worktree_fork` / `worktree_merge` events (111) | yes | none: the worktree overlay is removed |
| Observations (57) | yes | as for notes |
| Entry reservations / high-water marks | partly (frontmatter holds `entry_high_water_*`) | frontmatter, or none if `codescout-37` chooses per-entry files |
| Write-time `cites` | yes (13 `origin='write'` rows) | grammar text inside the entry |
| Manual links (`doc(link)`) | yes | grammar text in the source document, under a rel label |
| Catalog audit trail | yes (shards are exported) | git history with `Session-Id` trailers; the catalog audit tables are retired |

The catalog keeps only derived data: the document index, the parsed entries, the edge
graph, git-history caches, and embedding vectors keyed by content hash.

**Single-writer rule.** A write tool may append to a section only in a document that one
writer owns (an entry file, a session's own log). Two sessions appending at the end of the
same section of a shared document produce an add/add conflict, which breaks contract
requirement 2. Anything appended to a shared document becomes a document of its own.

**Frontmatter holds scalars and lists of scalars** (status, dates, kind, `entry_kind`, tags,
owners, a ledger's `entry_prefix`). Every relation is a
labelled grammar line in the body, for example `**Rests on:** [[key]]`. `link_scan` parses
bodies, never frontmatter, because `[[foo]]` in YAML is a nested list, not text.

### Identity: the key

- **The key is the filename stem in lowercase,** and it must match
  `[a-z0-9]+(-[a-z0-9]+)*`. Every file the librarian creates or moves already has a
  lowercase canonical name, so for those the key is the stem byte for byte. Files whose
  names other tools fix (`CLAUDE.md`, `AGENTS.md`, `README.md`, `CONTRIBUTING.md`,
  `SKILL.md`) keep their names; lowercasing is the only mapping, so `CLAUDE.md` has the key
  `claude`. `doc(create)` and `doc(move)` refuse a name whose lowercase stem is not
  canonical; other non-canonical names are renamed once, during the cutover. Lowercasing is
  deterministic and needs no collision rule beyond uniqueness.
- **Folder-index files** take their folder's name. The folder-index filename is
  `readme.md` (`codescout-37`'s choice: a forge renders a readme in the folder view), matched
  case-insensitively, so an existing `README.md` is also a folder index.
- **Unique per repo.** Two catalogued documents in one repo never share a key. `doc(create)`
  and `doc(move)` refuse a collision and name the other file; `doctor` and a pre-commit check
  catch one that arrives another way. There is no `Ambiguous` outcome for a key.
- **A key of exactly 16 hex characters is refused,** so no key is mistaken for a legacy id.
- **Repo name.** `.codescout/repo.toml`, at the git root of the project (the nearest enclosing `.git`
  ancestor; the project root when there is no git), holds `name = "<name>"`, same charset as
  a key. The git root is where repo-relative paths and keys are measured from, so every
  project in one repository shares one name. It is a label: it qualifies cross-repo
  citations and appears in tool output. Onboarding
  suggests one (the `.codescout/workspace.toml` name if present, else the directory name),
  says when another checkout uses it (below), and asks the user to confirm: through
  MCP elicitation when the client supports it
  (`src/tools/run_command/interactive.rs:36-98`), otherwise through the agent and a new
  `onboarding` parameter. A project that is already onboarded is asked on its next
  `onboarding` call, and `workspace(activate)` says when its name is unconfirmed.
  *Another checkout uses it:* onboarding scans the `.codescout/project.toml` files under the
  user's folders (bounded depth and time; build and worktree directories skipped) and reads
  `<git root>/.codescout/repo.toml` next to each. A match whose repository has the same first
  commit (`git rev-list --max-parents=0 HEAD`) is another checkout of this repository (a
  worktree or a clone) and is reported as that. A match with a different first commit is a
  name collision: onboarding warns and the user decides. Once the catalog holds checkouts (its
  checkout row carries the repo name), the catalog is the source and the scan is the fallback.

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
- `event_create` appends the event's text to the document's event section when one writer
  owns the document; on a shared document it creates an entry document of its own
  (single-writer rule).
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
- The indexer's refusal to index a linked worktree is removed; a worktree is indexed from its
  own files. Retiring the catalog audit tables is the last step, because an older binary
  re-creates the audit triggers when it opens the shared file, and the elision of escaping
  files must stay until the migration window.

### Contract with ledger formats

What this design requires of any ledger format `codescout-37` defines:

1. Every entry exists as markdown: a section that defines its token, or a document of its
   own (cited `[[key]]`). No entry exists only in params.
2. Two sessions appending entries concurrently produce a trivial git merge.
3. An entry's citations are written in the grammar.
4. Archiving an entry keeps it under its ledger's key (an `archive/` subfolder without an
   index, or the format's own equivalent).

### Index inputs, guard, machine state and entry ids

Five things the old code does that the first version of this spec did not name (found by the
catalog scout and its review; session log F-11):

- **Index inputs.** The index reads the commit's files and nothing else. Classification
  rules, the walk's ignore set and `force_include` live in committed `.codescout/` config.
  Machine-global config (`~/.config/librarian/workspace.toml`) is a default for a checkout
  that has no committed rules and is ignored for one that has them; the catalog records
  which source applied. Without this, two clones at one commit can disagree (G5).
- **Key space.** `contrib/**`, fixture trees, evidence trees and dated plan and spec pairs
  lie outside the key space by a committed scope rule (Plan B). `AGENTS.md` and
  `contrib/pi/AGENTS.md` both map to the key `agents` today, and neither can be renamed;
  the scope rule resolves it.
- **Guard.** `librarian_guard` keeps one signal: a declared `entry_prefix`, because the
  server owns that file's id counters. The `id:` stamp and the augmentation table stop
  being signals when their stores go, so a bug file or an ordinary tracker is edited like
  any markdown file.
- **Machine state.** Reindex progress rows, gc settings, the embed-error marker and the
  chunk backfill cursor move to a small machine-state file next to the derived catalog. It
  is not derived and holds no fact about a document, so losing it is harmless. Audit
  watermarks go with the audit tables.
- **Entry ids.** Until `codescout-37`'s format makes ids collision-free, `append_entry`
  takes an advisory lock on the ledger file and allocates `max(body, frontmatter
  high-water) + 1`. The `entry_reservation` table is removed.

### Tools that change meaning

- `doc(graft)`, `fix=prune_missing` and `fix=rehome` are removed: no rows exist to graft,
  prune or rehome.
- `rekey_prefix` stays as a file rewrite of one prefix's headings and `[[…]]` citations.
- `scope=repo|umbrella|all` stays, resolved over the checkouts that the name scan finds.
- Consumers that read hex ids or `catalog.db` move with the read flip: the companion
  `goal-stop-hook`, the `constitution-check` CLI and its hooks, `scripts/file-provenance.py`
  (used by `fmt-mine.sh`, a companion hook and CI), and `audit_doc_refs` with its own
  resolver.

### Removed

`ids::artifact_id_from_abs` as identity, the frontmatter `id:` stamp and its repair and
mismatch checks, `artifact.slug` and its minting and dedup, `graft_rows` on the move path,
`fix=rehome`, the worktree overlay (`resolve_write_target`, shadow rows,
`worktree_registration`, `merge_worktree`, `reseat_worktree`), catalog-held events,
observations and augmentation params as stores, `entry_reservation`, the catalog audit
tables, and the two-shape `entry_cite.dst_ref`.

### Cutover

Migration is second priority; the design is what matters. The operator's plan:

- After the design is built, the binary is released, and `codescout-37` has finished its
  side, there is one **migration window**. Every session is stopped for it, so no older
  binary runs against the catalog during the switch.
- Inside the window, recovery is **best effort**: recover as much as can be recovered from
  the old catalog and files, report what could not be recovered, then switch to the new
  setup. Nothing has to run against both setups at once.

The recovery steps, in order:

1. **Export** every catalog-only fact into its committed home (§ The store boundary):
   params entries without markdown (38 today), augmentation shapes, notes, verdicts,
   intents, observations and manual links.
2. **Rename** non-canonical filenames and key collisions.
3. **Rewrite citations.** Resolvable `<stem>:TOKEN`, path links to catalogued documents,
   16-hex ids, and bare tokens cited outside their own document become grammar forms. A
   bare cross-document token is rewritten to `[[owner-key:TOKEN]]` when it has exactly one
   owner today; one with several owners (most bare `F-n`/`W-n`) is reported, not guessed.
   An entry migrated into its own file keeps its legacy heading under the ledger folder, so
   `[[ledger-key:TOKEN]]` resolves by ownership in either migration order. Stale hex ids
   that git history recovers (`sha256(<root>/<old path>)[:16]`: 160 of 335 distinct) are
   rewritten too; the rest are reported.
4. **Build** the new catalog from the files, in a new catalog file. The old file is kept,
   read-only, as the recovery source until the operator retires it.

The export and the citation rewrite run before the read flip is activated, so the flip never
reads a half-migrated tree. The cutover tool needs a read-only open of the old file: today's
`Catalog::open` runs migrations whenever the opener's schema differs from the file's.

## Open for the operator

None. The cutover question was answered on 2026-11-11 (§ Cutover), and the amendments from
the catalog scout were approved the same day (§ Decision log).

## Left to the builder

- The sidecar format for augmentation shapes. It only has to carry legacy augmentations: a
  collection in `codescout-37`'s format keeps its schema in its readme frontmatter, its
  standing instructions in readme prose, and renders its table on read, so it needs no
  augmentation at all.
- The section format for events and observations inside a single-writer document.
- The rel-label vocabulary for manual links.
- Table and column names of the derived catalog, and the git-history cache.
- The pre-commit check for key collisions and non-canonical names.
- Tests: one per anti-goal, at minimum: a rebuild equals the incremental index; two clones
  at one commit give identical query results; a folder move changes no key and breaks no
  citation; a rename leaves no `[[old-key…]]` anywhere; no tool output contains a hex id,
  a path or a row id as a document reference; a worktree write merges through git alone.
- Plan B owns: the "another checkout uses this name" check above, the `workspace(activate)`
  nudge for an unconfirmed name, remembering a declined confirmation, and a key-space scope
  rule (which trees lie outside the key space, for example fixture and evidence trees and
  dated plan and spec pairs) that must exist before uniqueness is enforced.
- Plan order: B (key-space gate, the name checks, the derived builder in a new file, the
  grammar in shadow mode), C (store writes into files as dual-write, and the content-keyed
  vector cache), D (the cutover tool), E (the read and write flip, `doc(move)` as a rename,
  overlay removal, guard replacement, the consumers above, audit retirement and the
  deletion tail). The cutover tool runs before the flip is activated.

## Decision log

Operator, 2026-11-11:

- The stem is the key (over a committed slug and over keying on an id).
- The repo name is declared in `.codescout/repo.toml` and confirmed at onboarding.
- This spec owns the id layer.
- Moves are made by agents through codescout tools only.
- The current state of trackers and ledgers does not constrain the design; files may be
  rewritten.
- **Committed files are the only store; the catalog is a derived index.**
- Spec v5 approved. Cutover: one migration window after the build and `codescout-37`'s
  implementation, with every session stopped; best-effort recovery from the old setup,
  then the switch. Migration is second priority to the design.
- The librarian is an integral part of codescout: the `librarian` cargo feature is removed,
  and the librarian is on even when no semantic search is available (2026-11-11, during
  planning; Plan A Task 1).
- The repo name file lives at the git root of the project, falling back to the project root
  when there is no git (2026-11-11, after Plan A's final review; recommended by the
  executing session, accepted by the operator).
- Projects that are already onboarded are asked for the name on their next `onboarding`
  call (2026-11-11).
- "Another checkout uses this name" is found by scanning the `.codescout/project.toml`
  files under the user's folders; the same first commit means the same repository
  (2026-11-11; the first-commit rule is the executing session's proposal).
- The amendments from the catalog scout (session log F-11) and the plan order B, C, D
  (cutover tool), E (the flip), approved on 2026-11-11: one guard signal, a separate
  machine-state file, a lock plus the high-water mark for entry ids, committed config as
  the index's inputs, `contrib/**` outside the key space, scalars and lists of scalars in
  frontmatter, and the tool changes under § Tools that change meaning.

Designer's calls, open to correction:

- Filenames are canonical keys; the only mapping is lowercasing, for names other tools fix
  (supersedes the earlier operator choice to normalize, which existed only to avoid
  renaming files).
- Keys are unique per repo, enforced at write time; there is no collision fallback and no
  `Ambiguous` key outcome.
- One closed citation grammar; a bare token is local to its document; paths and hex ids are
  lint, not edges.
- A folder key owns its descendants' tokens; the nearest folder-index ancestor wins (from
  `codescout-37`'s review).
- The catalog audit tables are retired in favour of git history with `Session-Id` trailers.
- The key is the lowercase stem; names fixed by other tools keep their case (v5, after
  `codescout-37`'s `readme.md` choice exposed that `CLAUDE.md` cannot be renamed).
- Append sections only in single-writer documents; frontmatter holds scalars only and
  relations live in the body (from `codescout-37`'s v4 review).
- The cutover rewrites bare cross-document tokens that have one owner (from
  `codescout-37`'s v4 review).
- The cutover builds a new catalog file.
- Machine-global classification config is a default for a checkout without committed rules,
  not an addition to committed rules, so two clones still agree. This refines the operator's
  approved wording ("the machine config may only add"), which would still let clones differ;
  open to correction.

Superseded by v4: the internal ULID, `.codescout/aliases.tsv`, the normalization layer, the
collision tolerance, the rebind-by-key rules, and the legacy-id alias map (all v3).
