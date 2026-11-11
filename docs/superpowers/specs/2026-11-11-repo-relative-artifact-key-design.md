# Repo-relative artifact key: the stem replaces the minted slug (design)

**Status:** draft 2026-11-11, waiting for operator approval. Source tracker:
`docs/trackers/temp/artifact-identity-refactor.md`. Scope is the slug layer only. The `id`
layer and the move/archive mechanism belong to a parallel session; § Rename re-key (interface
to the move work) names the one interface between the two.

## Intent

The librarian gives one document three identities. Prose cites an entry by **file stem**
(`<stem>:TOKEN`). The entry graph (`entry_cite`) stores a **minted slug**
(`slugify(title)`, capped at 50, `-2`/`-3` dedup). The rest of the catalog uses the path-hash
**id**. This design removes the minted slug and makes the stem, scoped by a declared repo
name, the one entry-grain key. The key must survive a move of the whole checkout to another
location, and two machines must derive the same key for the same file.

Moves are done only by agents through codescout tools, never by hand. That is an operating
constraint of this design, not a goal.

## Measurements (2026-11-11, live catalog, read-only)

Scripts are in the session scratchpad; the numbers are the evidence for the choices below.

| Fact | Number |
|---|---|
| `entry_cite` rows | 6953: 6940 `origin='scan'` (re-derived by `link_scan`), 13 `origin='write'` (`append_entry(cites=…)`) |
| Slug-form `dst_ref` (`<slug>:<local>`) | 6550 |
| Distinct slugs used by `entry_cite` (src or dst) | 273; only 20 equal the file stem |
| Prose `<stem>:TOKEN` citations | 1027 |
| Prose citations that use a slug that is not a stem | about 6 genuine (for example `open-issue-work-queue-bl-n:BL-42` in `docs/trackers/context-performance.md:69`), plus 71 repo prefixes such as `codescout:A-11` |
| Slugs a fresh machine would mint differently (codescout only) | about 52 from title changes, 3 to 7 from suffix order |
| Catalog scope | one machine-wide catalog, 7681 rows across about 12 repo roots |
| Cross-repo suffix race | `tracker-hygiene-log` exists in 7 repos with slugs `-2`..`-7`; codescout holds the bare slug only because it was indexed first |
| Duplicate stems among codescout entry endpoints | 2: `docs/research/README.md`, `docs/trackers/bistriceanu/index.md` |
| Slug stability on one machine (live vs 2026-08-31 backup) | 0 of 4246 changed |

The slug is stable on one machine and unstable across machines. It is also not what authors
write. Authors write the stem, and the filename that holds it is committed.

## Goals

- **G1.** The entry-grain key of an artifact is its **key**: the file stem, or the parent
  folder name for a folder-index file (§ Key rule). It comes from the committed path and
  nothing else.
- **G2.** Each repo declares a **repo name** in `.codescout/librarian.toml`. Onboarding
  suggests one and the user confirms it.
- **G3.** `entry_cite` and every current reader of `artifact.slug` use the key. Nothing
  mints, nothing reads the title for identity, and nothing appends a dedup suffix.
- **G4.** A move of the whole checkout to another absolute path changes no key and no
  `entry_cite` row.
- **G5.** Two machines that hold the same commit derive the same key for every file.
- **G6.** A prose citation `<key>:TOKEN` and the graph's stored reference are the same
  string. Tool output shows that string, so an agent that copies it into prose writes a
  citation that resolves.

## Non-goals

- The `artifact.id` path hash stays as it is. Changing it is the parallel session's
  question.
- The archive and move mechanism, including rewriting prose citations after a move.
- Edges across repos. A citation of another repo stays report-only (`Outcome::CrossRepo`).
- Manual moves and edits by a human.
- The worktree overlay. It keeps its own design; § Worktrees states the one change it gets.

## Anti-goals

The change fails, even if it works, if any of these is true:

- A folder reorganization that keeps the filename changes a key. This is why the key is not
  the repo-relative path.
- A key depends on the title, on index order, or on which other repos the machine holds.
- A re-key drops an edge without a report.
- A user finds a repo name in a committed file that they never saw or confirmed.

## Constraints

- **One catalog per machine, many repos.** A key is unique only inside its checkout. The
  same key in two repos is normal (`tracker-hygiene-log` in 7 repos).
- **Two checkouts of one repo can share a machine.** `librarian.toml` is tracked, so both
  checkouts carry the same repo name. Storage scope must therefore be the checkout, not the
  repo name.
- **The qualifier namespace is shared.** `<qualifier>:TOKEN` is read as a file key when the
  qualifier names a file in this repo, and as a repo name otherwise
  (`src/librarian/tools/link_scan/resolve.rs:334-350`). CLAUDE.md § Parsers Over a Namespace
  requires an escape and a disambiguator.
- **`dst_ref` holds two shapes in one column.** It holds either a 16-hex id or a
  `<key>:<local>` pair. `gc::apply_rehome` tells them apart by shape. `ensure_slug` refuses a
  16-hex slug for this reason (`src/librarian/catalog/artifact.rs:268-325`). The same hazard
  applies to a 16-hex stem.

## Approaches

Considered for the key:

1. **The stem is the key, scoped by repo (chosen).** One key in prose and in the graph.
   Every machine derives it. Cost: a rename changes the key, so `doc(move)` must re-key;
   duplicate stems need a rule.
2. **Commit the slug to frontmatter.** Portable and stable through a rename. Rejected: the
   file keeps two names, prose would have to change or accept both, and about 273 files
   need a stamp.
3. **Remove the slug and key `entry_cite` on the artifact id.** Rejected: tool output would
   show `<16hex>:F-3`, which nobody can read, and the id is a path hash, so it differs per
   machine and per checkout location.

Considered for the source of the repo name:

1. **A declared name in `.codescout/librarian.toml`, confirmed at onboarding (chosen).**
   It survives relocation and a folder rename. Cost: one committed line per repo.
2. **The directory basename, derived each time.** Rejected: a folder rename changes it, and
   unrelated repos with the same folder name collide.
3. **The git remote.** Rejected: a local-only repo has none, and a fork or remote rename
   changes it.

`.codescout/workspace.toml` already has `[workspace] name`, but it is not tracked in
codescout's git, and `prompt-engineering` has no such file. It can serve as the onboarding
suggestion, not as the source.

## Design

### Repo name

- Stored as `[repo] name = "<name>"` in `.codescout/librarian.toml`. The file is tracked
  and is already read by `src/librarian/classify.rs`.
- Charset: `[a-z0-9][a-z0-9-]*`, the same as a key. A name of 16 hex characters is refused.
- The name is a **label**, not a storage key. It appears in tool output and in the
  three-part citation form. Changing it later changes no stored row.

### Onboarding confirms the name

When `librarian.toml` has no `[repo] name`:

1. Onboarding computes a suggestion: the `[workspace] name` from `workspace.toml` if one
   exists, otherwise the slugified directory basename.
2. If another checkout in the catalog already uses that name, onboarding says so and names
   that checkout's root. Two cases are possible: a second checkout of the same repo (keep
   the name) or a different repo (choose another name). The user decides; onboarding does
   not refuse.
3. Onboarding asks the user to confirm or edit the suggestion. If the client supports MCP
   elicitation, onboarding asks directly, as `run_command` interactive mode does
   (`src/tools/run_command/interactive.rs:36-98`). Otherwise onboarding returns the
   suggestion with an instruction for the agent to ask the user, and accepts the confirmed
   name through a new `onboarding` parameter (for example `repo_name`).
4. Onboarding writes the confirmed name to `librarian.toml` and creates the file when it is
   absent.

Until the name is confirmed, tool output uses the suggestion and marks it unconfirmed.
Because the name is only a label, confirmation later changes no stored row. Activation of a
repo with no confirmed name reminds the agent to finish onboarding.

### Key rule

- The key is the file stem.
- **Folder-index files** take the parent folder's name: `README.md`, `index.md` and
  `SKILL.md`. Examples: `docs/trackers/bistriceanu/index.md` → `bistriceanu`,
  `docs/research/README.md` → `research`. Tracker folders already exist
  (`effort-steering/`, `ecc-integration/`, `bistriceanu/`), so this rule matches the
  reorganization that the source tracker plans.
- **Collisions are order-free.** When two or more files in one checkout would get the same
  key, every one of them falls back to its repo-relative path without `.md`. No file wins by
  index order, so G5 holds.
- `doc(create)` and `doc(move)` refuse a target whose key would collide with the key of a
  file that defines or receives entry-grain edges. The refusal names the other file. A
  collision between files with no entry edges is allowed and falls back silently.
- A key of exactly 16 hex characters is refused at create and move, and `doctor` reports
  one found on disk. Measured: none exist today.

### Storage

- A new **checkout** table holds one row per indexed checkout: a surrogate id, the current
  absolute root, and the repo name label. A relocation updates `root` in this one row. The
  existing `fix=rehome` updates it in the same transaction as the rows it already moves.
- `artifact` gains `checkout_id` and `key`, unique on `(checkout_id, key)`. The indexer
  computes `key` from the path on every upsert, so a rename that bypasses `doc(move)` still
  gets the right key. That is outside the operating constraint; it costs nothing to keep.
- `entry_cite` replaces `src_slug` with `(checkout_id, src_key)`. A `dst_ref` of the entry
  form becomes `<key>:<local>` within the same checkout. The 16-hex file-grain form is
  unchanged.
- `artifact.slug`, `ensure_slug`, `upsert_and_mint_slug`, `mint_missing_slugs` and
  `doctor fix=mint_slugs` are removed.

### Citations and tool output

- `link_scan` builds its qualifier map from `key`, not from the raw stem, so the
  folder-index rule applies to prose too: `bistriceanu:B-3` resolves.
- The **three-part form** `<repo>:<key>:TOKEN` becomes valid. When `<repo>` is this
  checkout's repo name, it resolves as a within-repo edge. When it names another repo, it
  is `CrossRepo`, which is report-only. Today this shape is `MalformedQualifier`
  (`resolve.rs`, `Outcome::MalformedQualifier`).
- Disambiguator for the shared namespace: in a two-part `<q>:TOKEN`, a `q` that is a key in
  this checkout resolves as a key. If `q` is also a repo name the catalog knows, `doctor`
  reports the shadowing, and the three-part form is the escape.
- Tool output shows the key form wherever it shows a slug today: `get`'s `entry_links`
  (`src/librarian/tools/get.rs:387-410`), `context` anchors
  (`src/librarian/tools/context.rs:124`, `:369`), and `append_entry`'s `cites` echo.
- `append_entry(cites=…)` accepts `<key>:<local>` and `<repo>:<key>:<local>`. An old slug
  form that is not also a valid key is refused, and the error names the key to use.

### Rename re-key (interface to the move work)

When `doc(move)` changes a file's key (a new stem, or a move into or out of the
folder-index rule), it calls one new step, `rekey_artifact_key(checkout, old_key, new_key)`.
The step runs in the same transaction as the move:

- It updates `artifact.key`, the `entry_cite.src_key` rows, and every `dst_ref` that starts
  with `<old_key>:`.
- It follows the durable versus derived split of `src/librarian/catalog/rekey.rs`: it
  re-points durable rows (`origin='write'`) and drops derived rows (`origin='scan'`) for
  the next `link_scan` to rebuild. It deletes a duplicate that would conflict instead of
  stranding it (`rekey.rs` `drop_superseded`).
- It reports the files whose prose cites `<old_key>:TOKEN`. Rewriting them is the move
  session's decision.

An archive move keeps the stem (hygiene rule D10), so an archive needs no re-key.

### Worktrees

A worktree root is its own checkout row, so worktree shadow rows carry the same keys as
their main twins without a collision. Where `graft` hands `slug` across today
(`src/librarian/catalog/graft.rs:185-218`), it hands `(checkout_id, key)` across instead.
`merge_worktree` is otherwise unchanged.

### Migration

One catalog migration, in this order:

1. Create `checkout` rows from the distinct indexed roots, with the repo name from each
   root's `librarian.toml`, or the unconfirmed suggestion.
2. Fill `artifact.checkout_id` and `artifact.key`, with the collision fallback.
3. Re-point the 13 `origin='write'` rows: map `src_slug` to `(checkout_id, key)` through
   the old `artifact.slug`. Their `dst_ref` values are all 16-hex, so they stay.
4. Delete the 6940 `origin='scan'` rows. The next `link_scan` write pass rebuilds them
   against keys. Report the before and after edge counts; a large drop is a defect.
5. Drop `artifact.slug` and its unique index.

Committed text:

- The about 6 prose citations that use a slug form are rewritten to the key form by hand,
  in the same change. The census script lists them.
- `.codescout/audit/ripper-*.jsonl` logs hold slug strings. They are history and stay
  as they are.
- The stale doc comment at `resolve.rs:272-276` ("slugs … neither exist for most rows") is
  removed with the slug.

## What is left to the builder

- Exact table and column names, and the migration number.
- The elicitation schema and the name of the new `onboarding` parameter.
- How `get` and `context` lay out the key form in their output.
- Test design. One test per anti-goal is the minimum: a folder move keeps the key; two
  index orders give the same keys; a re-key drops no durable edge; a relocation changes no
  `entry_cite` row.
- The full list of slug readers to convert. A first survey found them in `link_scan/mod.rs`
  (`id_to_slug`, `entry_dst_ref`), `context.rs`, `get.rs`, `delete.rs`, `augmentation.rs`
  (`resolve_cite_ref`), `doctor/scope.rs`, `graft.rs`, `rekey.rs`, `audit/shard.rs` and
  `indexer.rs`. Verify the list with `references` before planning.
