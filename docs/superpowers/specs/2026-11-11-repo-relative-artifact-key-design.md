# Artifact identity: an internal ULID and a portable key (design)

**Status:** draft v2, 2026-11-11. Operator decisions recorded in § Decision log. Source
tracker: `docs/trackers/temp/artifact-identity-refactor.md`. This spec owns the whole
identity model: the internal id, the portable key and the repo name. The parallel
tracker-reorganization session (`codescout-37`) owns the archive and move workflows and the
folder layout. § Moves names the interface between the two.

## Intent

The librarian gives one document three identities. The **id** is `sha256(abs_path)[:16]`
(`src/librarian/ids.rs:17-23`), the catalog's primary key and every foreign key. The
**slug** is minted from the title with `-2`/`-3` dedup (`src/librarian/catalog/artifact.rs`
`ensure_slug`) and keys the entry graph. The **file stem** is what prose cites
(`<stem>:TOKEN`, `src/librarian/tools/link_scan/resolve.rs`). The id changes on every move,
on every relocation of the checkout, and between a main checkout and its worktree. The slug
differs between machines. Only the stem is what authors write and what git carries.

This design keeps two identities with two separate jobs:

- an **internal uid**, a random ULID that never leaves the machine's catalog and that every
  table joins on;
- a **portable key**, derived from committed files only, that every machine computes the
  same way and that is the only identity anything outside the catalog may hold.

Moves are done only by agents through codescout tools, never by hand. That is an operating
constraint, not a goal.

## Measurements (2026-11-11, read-only)

| Fact | Number | Source |
|---|---|---|
| Renames under `docs/` since January | 1217, of which 1112 into `archive/` | git log |
| Commits that re-point citations after an archive or move | 144 (127 after bug-file archives) | `git log --format=%s \| grep -ciE 're-?point.*(archiv\|mov)\|(archiv\|mov).*re-?point'`, reproduced |
| Dangling citations in `link_scan` that are 16-hex ids | 449 of 867 | `codescout-37` census |
| Bare 16-hex ids in prose | 2435 occurrences; 1843 resolve today, 592 are already stale | citation census |
| Frontmatter `id:` stamps | 1550 files; 1545 equal the current path hash | citation census |
| Prose `<stem>:TOKEN` citations | 1027; about 6 genuine slug-form citations | census |
| `entry_cite` rows | 6953: 6940 `scan`, 13 `write` | catalog |
| Distinct slugs used by `entry_cite` | 273; 20 equal the stem | catalog |
| Cross-repo slug suffix race | `tracker-hygiene-log` in 7 repos, slugs `-2`..`-7` | catalog |
| Rows that carry events or an augmentation (codescout) | 1380 | catalog |
| Git roots in `commits` | 47, of which 32 hold artifacts; 351 artifacts sit under none | builder check |
| Entry endpoints whose stem is not `[a-z0-9-]` | 55 of 273 (capitals, `_`, `.`) | builder check |
| Key collisions among tracked `.md` (folder-index rule, normalized) | 37 keys, 115 files; normalization adds 1 (`docs/SECURITY.md` / `docs/manual/src/concepts/security.md`) | `normkey.py` |
| Hosts writing audit shards for this repo | 2 (`archlinux`, `ripper`) | `.codescout/audit/` |
| Live sessions on a replaced binary | 17 of 30 on 2026-10-16 | `src/librarian/requirements.rs:8-12` |

## Goals

- **G1.** Every catalog row has an internal **uid** (ULID). Every table joins on it. It is
  never written to a committed file and never shown in tool output.
- **G2.** Every artifact has a portable **key**, computed from its repo-relative path by
  § Key rule. Two machines that hold the same commit compute the same key for every file.
- **G3.** Each repo declares a **repo name** in `.codescout/repo.toml`. Onboarding suggests
  one and the user confirms it.
- **G4.** A move of the whole checkout to another absolute path changes no uid, no key and
  no row other than the checkout's own.
- **G5.** A move of a file through codescout keeps its uid and every row that hangs off it.
  No graft, no new id, no re-key of edges.
- **G6.** Tool output shows keys, never uids or path hashes. A key copied from tool output
  into prose is a citation that resolves on every machine.
- **G7.** Existing 16-hex citations keep resolving through a committed alias map, without a
  prose rewrite.

## Non-goals

- The archive and move workflows, the folder layout, and rewriting prose citations after a
  move. These belong to `codescout-37`.
- Markdown path links. They are the most common citation form (about 10,600) and still break
  on a folder move. A key-based link form would be a separate addition.
- Edges across repos. A citation of another repo stays report-only (`Outcome::CrossRepo`).
- Syncing DB-only state (events, augmentation params, observations) between clones. That
  state never travels today, whatever the id is.
- Manual moves and edits by a human.

## Anti-goals

The change fails, even if it works, if any of these is true:

- A folder reorganization that keeps the filename changes a key.
- A key depends on a title, on index order, on the machine, or on other repos.
- A uid appears in a committed file or in tool output, so that agents start citing it.
- A collision silently changes an existing key.
- A user finds a repo name in a committed file that they never saw or confirmed.
- An older binary and a newer binary sharing the catalog undo each other's schema.

## Constraints

- **One catalog per machine, many repos and binaries.** The catalog is shared by every repo
  and every running binary. Older binaries rerun their own migrations on open: the v9 block
  re-adds `artifact.slug` if it is missing, and the audit triggers are recreated whenever
  their text differs (`src/librarian/catalog/audit/mod.rs:232-238`).
- **`librarian.toml` is closed.** `classify::RuleFile` is `#[serde(deny_unknown_fields)]`
  (`src/librarian/classify.rs:5-9`), so a new table there breaks older binaries. This is why
  the repo name lives in its own file, as `requirements.toml` does.
- **The qualifier namespace is shared.** `<q>:TOKEN` reads `q` as a file key when it names a
  file in this repo, and as a repo name otherwise (`resolve.rs:334-350`). CLAUDE.md
  § Parsers Over a Namespace requires an escape and a disambiguator.
- **Two checkouts of one repo can share a machine.** `repo.toml` is tracked, so both carry
  the same repo name. Storage scope is the checkout, not the repo name.

## Approaches

For the internal id:

1. **A random ULID, private to the catalog (chosen).** A move or relocation becomes an
   `UPDATE` of one row. Cost: every foreign key changes type in one migration, and anything
   that leaks the id today must stop.
2. **Keep the path hash.** No migration. Rejected: it is the measured cause of the 144
   re-point commits and the 449 dangling ids.
3. **A deterministic hash of the key.** Regenerable on any clone. Rejected: it changes on a
   rename, so it needs the re-key machinery that option 1 removes, and nothing outside the
   catalog needs it to be regenerable once the key is the external identity.

For the external key:

1. **The normalized stem, scoped by repo (chosen).** It is what authors already write.
2. **A slug committed to frontmatter.** Rejected: two names per file, about 273 stamps.
3. **The repo-relative path.** Rejected: a folder reorganization changes it.

For the repo name: a declared name in a tracked file, confirmed at onboarding (chosen), over
the directory basename (a folder rename changes it) and the git remote (absent for
local-only repos).

## Design

### Identity layers

| Layer | Value | Lives in | Changes when |
|---|---|---|---|
| uid | ULID | the catalog only | never |
| key | normalized stem, or folder name for a folder-index file | derived from the committed path | a rename that changes the stem |
| repo name | declared label | `.codescout/repo.toml`, tracked | the user edits it |
| checkout | surrogate row with the current absolute root | the catalog only | a relocation updates its root |
| legacy hex id | old `sha256(abs_path)[:16]` | committed alias map | never (frozen) |

### Repo name

- Stored as `name = "<name>"` in `.codescout/repo.toml`, a new tracked file. Older binaries
  never read it.
- Charset `[a-z0-9][a-z0-9-]*`. A name of exactly 16 hex characters is refused.
- The name is a label. It appears in tool output and in the three-part citation form.
  Changing it changes no stored row.
- `link_scan` already treats a repo's git-root folder name and the `[[roots]]` names in
  `~/.config/librarian/workspace.toml` as names of this repo
  (`src/librarian/tools/link_scan/mod.rs:564-604`). The declared name joins them; it does
  not replace them.

### Onboarding confirms the name

When `.codescout/repo.toml` is absent:

1. Onboarding computes a suggestion: the `[workspace] name` from `.codescout/workspace.toml`
   if present, otherwise the normalized directory basename.
2. If another checkout in the catalog already uses that name, onboarding names that
   checkout's root and says it is either a second checkout of this repo (keep the name) or
   a different repo (choose another). The user decides; onboarding does not refuse.
3. Onboarding asks the user to confirm or edit the suggestion: through MCP elicitation when
   the client supports it (as `run_command` interactive mode does,
   `src/tools/run_command/interactive.rs:36-98`), otherwise by returning the suggestion
   with an instruction for the agent to ask the user, and accepting the confirmed name
   through a new `onboarding` parameter.
4. Onboarding writes `repo.toml`.

Until then, tool output uses the suggestion marked unconfirmed, and activation reminds the
agent to finish onboarding. Confirmation changes no stored row.

### Key rule

- **Normalization:** lowercase, then every run of characters outside `[a-z0-9]` becomes one
  `-`, trimmed at both ends. `IC-10-authorship-unrecoverable-after-the-fact` becomes
  `ic-10-authorship-unrecoverable-after-the-fact`. The prose scanner normalizes a
  qualifier the same way before lookup, so an author may write the filename as it appears.
  The qualifier regex widens to accept what the filename contains; the builder must keep
  entry-token masking correct (`RI-001:RI-2`).
- **Folder-index files** (`README.md`, `index.md`, `SKILL.md`, any case) take the parent
  folder's name: `docs/trackers/bistriceanu/index.md` → `bistriceanu`.
- **A folder key owns its folder's tokens.** A qualified citation `<folder-key>:TOKEN`
  resolves when the token is defined in the folder-index file or in any file inside that
  folder. This serves per-entry ledger folders (`frictions/FR-3-<slug>.md`), which
  `codescout-37` plans.
- **Collisions never change a key.** There is no fallback key. A key shared by two files in
  one checkout is tolerated, and a two-part citation of it resolves as `Ambiguous`, as
  today. Collisions are refused only where they would hurt: `doc(create)`, `doc(move)` and
  the first entry definition or entry citation of a file (`append_entry`, `link_scan`
  write) refuse or report when the file's key is shared, and the refusal names the other
  file and asks for a rename. Only tracked files that exist on disk count; rows with
  `missing_since` and untracked files do not.
- A key of exactly 16 hex characters is refused at create and move, and `doctor` reports
  one found on disk. Measured: none exist.

### Storage

- **checkout**: one row per indexed checkout: a uid, the current absolute root, the repo
  name label. A checkout is the activated project root, which is also where `repo.toml`
  and `librarian.toml` are read.
- **artifact**: primary key `uid`; columns `checkout_uid`, `rel_path`, `key` (indexed, not
  unique), `legacy_id` (the old path hash, nullable). The absolute path is the checkout root
  joined with `rel_path`.
- Every table that references `artifact(id)` today references `artifact(uid)`:
  `artifact_augmentation`, `artifact_chunk`, `artifact_link`, `artifact_observation`,
  `entry_reservation`, `events`, `event_edges.dst_artifact_id`.
- **entry_cite** becomes `(src_uid, src_local, dst_uid, dst_local, rel, origin)`. A file-grain
  edge has a NULL `dst_local`. The two-shape `dst_ref` text column, the 16-hex guard and
  the shape test in `gc::apply_rehome` go away.
- Removed: `artifact.slug`, `ensure_slug`, `upsert_and_mint_slug`, `mint_missing_slugs`,
  `doctor fix=mint_slugs`, `fix=repair_frontmatter_id`, the `frontmatter_id_mismatch`
  check, `ids::artifact_id_from_abs` as an identity source, and `graft_rows` on the move path.

### Binding a file to its row

The indexer finds a row by `(checkout_uid, rel_path)`. When none exists:

1. **Fresh clone or new file:** mint a uid.
2. **A move that arrived by pull** (the old `rel_path` is gone, a new file has the same key
   and no row): bind the new path to the old row when exactly one missing row in the
   checkout has that key. Folder moves and archives keep the stem, so they rebind, and the
   row's local state survives.
3. **A rename that arrived by pull** (the key changed): no binding by key is possible. The
   old row is kept with `missing_since` when it has history, never hard-deleted in the same
   pass. Git rename detection between the last indexed commit and HEAD is a candidate for
   binding it; its feasibility is not verified. What happens to unbound rows belongs to
   the move session.

### Citations and tool output

- Tool output shows `<key>:<local>`, or `<repo>:<key>:<local>` where the repo is not
  obvious: `get`'s `entry_links`, `context` anchors, `doc` results, `append_entry`'s
  `cites` echo.
- **Three-part form** `<repo>:<key>:TOKEN` becomes valid. With this checkout's repo name it
  is a within-repo edge; with another repo name it is `CrossRepo`, report-only. Today this
  shape is `MalformedQualifier`. It is also the escape for bare `F-n`/`W-n` tokens, which
  are 700 of today's 754 ambiguous citations.
- **Disambiguator:** in a two-part `<q>:TOKEN`, a `q` that is a key in this checkout
  resolves as a key. If `q` is also a known repo name, `doctor` reports the shadowing.
- `append_entry(cites=…)` accepts `<key>:<local>`, `<repo>:<key>:<local>`, a rel_path, and a
  legacy 16-hex id through the alias map. A slug form that is not also a key is refused,
  and the error names the key to use.

### Legacy 16-hex ids

- The migration writes a committed alias map, `.codescout/legacy-ids.tsv`: one line per
  16-hex id that resolves at migration time, mapping it to the target's repo-relative path.
  The resolver reads it, so the 1843 live hex citations and the frozen files that cite ids
  (for example `f2ecdd76a6189efb` in `src/`, `tests/` and `docs/evals/data/`) keep
  resolving on every clone. Agents are never shown a hex id again, so the map does not grow
  after the migration, except for the `legacy_id` of rows the migration itself created.
- Existing frontmatter `id:` stamps stay as they are and resolve through the map. No new
  file is stamped. `util/librarian_guard.rs` (`is_librarian_id`) uses the stamp to detect a
  managed file; it needs a signal that works for unstamped files.
- The 592 hex ids that are already stale stay unresolved, as today.

### Moves

`doc(move)` becomes: rename the file, `UPDATE artifact SET rel_path, key`, refile chunk
vectors. The uid and every row that hangs off it stay. A move that changes the key leaves
every edge in place, because edges hold uids; only prose that cites the old key breaks, and
the move reports those files. What the move does about them, and every other part of the
workflow, belongs to `codescout-37`.

### Worktrees

A worktree root is its own checkout row, so worktree rows have their own uids and the same
keys as their main twins. The `worktree_of` link and the fork and merge flow keep working,
on uids. The `append_entry` refusal in a worktree has two causes
(`src/librarian/tools/append_entry.rs:237-246`). Shared identity removes only the first
(the shadow misses the main reservation): the reservation can resolve through `worktree_of`
to the main twin. The second (a prose ledger cannot be merged afterwards) remains.

### Audit

- An audit `row_id` for an artifact-keyed table is the uid, which is host-local. The payload
  adds the key and repo name, so rows from two hosts can be correlated.
- The shard exporter decides a row's repo through the checkout, not through a slug join
  (`src/librarian/catalog/audit/shard.rs:282-346`).
- Pending audit rows are exported before any old column is dropped. The migration's own
  bulk rewrite is recorded as one marker row, not as about 14k exported lines.

### Migration and binary coexistence

The schema change replaces the primary key of `artifact` and every foreign key. An older
binary cannot run against that schema, and it would try to rebuild its own (constraint
above). The recommended shape is a **new catalog file** that the new binary builds from the
old one, so that older binaries keep their file and the two never touch the same schema.
Writes that older binaries make to the old file after the copy must not be lost; the builder
must choose between replaying them from `catalog_audit` and a cutover that requires every
session to restart. This is the largest open question; see § Open for the operator.

Migration content, whatever the shape:

1. Create checkout rows from the indexed roots. A row under no known root is assigned to the
   nearest ancestor with `.codescout/`, else its git top level, else reported.
2. Mint uids; fill `rel_path`, `key`, `legacy_id`; rewrite every foreign key.
3. Convert the 13 `origin='write'` `entry_cite` rows to uids through the old slug and id.
   Drop the 6940 `scan` rows; the next `link_scan` write pass rebuilds them. Report the edge
   counts before and after; a large drop is a defect.
4. Write `.codescout/legacy-ids.tsv`.
5. Rewrite the about 6 slug-form prose citations to key form by hand, in the same change.
6. Remove the stale comment at `resolve.rs:272-276`.

## Open for the operator

- **Cutover with older binaries running:** replay old-file writes, or require a restart of
  every session. Measured: most sessions run a replaced binary at any time.

## Left to the builder

- Exact table and column names, and the migration numbering.
- The elicitation schema and the new `onboarding` parameter's name.
- The widened qualifier regex and its entry-token masking.
- The managed-file signal that replaces the frontmatter stamp in `librarian_guard`.
- Test design. The minimum is one test per anti-goal: a folder move keeps uid and key; two
  index orders give the same keys; a relocation changes only the checkout row; a collision
  never changes a key; tool output contains no uid and no hex id; an older binary cannot
  damage the new catalog.
- The full list of `artifact.id`, `artifact.slug` and `src_slug` readers. Surveys found
  about 14 id mint sites in 11 files and slug readers in `link_scan/mod.rs`, `context.rs`,
  `get.rs`, `delete.rs`, `augmentation.rs`, `doctor/scope.rs`, `graft.rs`, `rekey.rs`,
  `audit/shard.rs` and `indexer.rs`. Verify with `references` before planning.

## Decision log

Operator, 2026-11-11:

- The stem is the key (over a committed slug and over keying on the id).
- The repo name is declared and confirmed at onboarding; it lives in `.codescout/repo.toml`.
- Stems are normalized, for keys and for prose qualifiers alike.
- This spec takes over the id layer: an internal ULID plus the portable key.
- Moves are agent-only, through codescout tools.

Designer's calls, open to correction:

- Folder-index files take the folder name; a folder key owns its folder's tokens (from
  `codescout-37`'s review).
- No collision fallback; collisions are tolerated and reported, and refused only at create,
  move and first entry use (from `codescout-37`'s review).
- A checkout is the activated project root.
- The declared repo name joins the existing self-names in `link_scan`.
- Legacy hex ids resolve through a committed alias map; no prose rewrite.
- A new catalog file is the recommended migration shape.
