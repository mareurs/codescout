---
id: '6d16009652c88e57'
kind: spec
status: active
title: 'Tracker lifecycle: entries as files, collections as folders, three lifecycle folders'
tags:
- trackers
- lifecycle
- librarian
- design
---

# Tracker lifecycle: entries as files, collections as folders, three lifecycle folders (design)

**Status:** active since 2026-11-12. Plans L-A (core) and L-B (core completion) are merged into
`experiments`; Plan L-B2 (tools) waits for identity Plan C, and Plan L-E (migration) is designed in
`docs/superpowers/specs/2026-11-15-tracker-migration-design.md` (approved 2026-11-15), which
amends G8 and § Repo scope: an agent-run skill with three CLI commands, not one server command. Drafted 2026-11-11. Approach and decisions D1–D5 approved by the operator in the
brainstorm; leaf archives and three lifecycle folders approved after the builder check;
repo scope (§ Repo scope) taken as option a, the recommended one. Work-stream log:
`docs/trackers/tracker-organization-session-log.md`. Measurements:
`docs/trackers/tracker-hygiene-log.md` HY-34 (temp/ policy, parked) and HY-35 (why trackers go
stale). Identity layer this design depends on:
`docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` (v5, `2fbffdca1`),
owned by a parallel session.

## Intent

`docs/trackers/` has grown into 76 live files (80,236 lines) with no uniform lifecycle.
Trackers go stale, grow too long to maintain, need hand-maintained id counters, and have no
place for temporary work. The operator wants **one integrated, uniform system**, taught by
`get_guide` to every repo that runs codescout, with archiving as simple as in `docs/issues/`.

Scope is a correct, scalable design, not an easy refactor. The current trackers may be
rewritten from scratch. What must survive is listed in § Consumers that read trackers today.

## Measurements (2026-11-11)

| Fact | Number | Source |
|---|---|---|
| Live top-level trackers | 76 files, 80,236 lines; 17 ≥ 1000 lines; 34 ≥ 500 | inventory script |
| Largest | `bug-fix-session-log` 18,343 lines / 392 entries; `reconnaissance-patterns` 8,849 / 140 | same |
| Trackers with no commit since before 2026-10-01 | 32 | HY-35 |
| Of those: ended but not closed / dormant / live but lagging / correct by design | 18 / 13 / 10 / 5 (pure + mixed) | HY-35 |
| Last commit on most of the 32 | a bulk sweep (citation repoint, tool-name retirement), not content | HY-35 cause 1 |
| Files citing live trackers by path / 16-hex id / stem | 908 / 399 / 420 | inventory script |
| Repair commits after archive moves (path + id citations) | 144 | `git log` subject regex |
| Ambiguous citations that are bare `F-n`/`W-n` | 700 of 754 | `link_scan` census |
| Commits spent on id-counter machinery | ~56; live residue now ~0 | id measurement |
| Repos on this machine with a `docs/trackers/` folder (worktrees excluded) | about 35 | `ls -d ~/work/**/docs/trackers` |

## Goals

- **G1.** Every record is one entry file. No record lives as a section of a long file.
- **G2.** `docs/trackers/` has exactly three lifecycle folders, defined by the server:
  `ledgers/`, `streams/`, `temp/`. Layer 2 is one folder per collection.
- **G3.** Whether a record or a collection is stale is computed from declared fields, never
  from git commit dates.
- **G4.** Archiving is one rule for entries and collections: move into the nearest sibling
  `archive/` folder, keeping the filename.
- **G5.** No sequential id counters for new entries. A new entry needs no allocation, so a
  worktree session can add one, and git merges it.
- **G6.** `get_guide` teaches the model repo-neutrally, and passes the guide gates in
  `src/prompts/mod.rs`.
- **G7.** Every consumer listed below keeps working, on a contract that names the new layout.
- **G8.** Every repo can migrate with one skill that drives three CLI commands, one tracker at a
  time, with the user's approval per tracker (amended 2026-11-15 by
  `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`; it was "one server command");
  none is forced to by a breaking
  release before it has.

## Non-goals

- The identity model (keys, the citation grammar, the derived catalog). This design uses it.
- Moving `docs/issues/`. It stays where it is and already conforms to the entry model: one
  date+slug file per bug, with an `archive/` folder inside the collection.
- The `temp/` cleanup policy. HY-34 measures it over two hygiene sweeps first.
- Deciding, per existing tracker, which content is worth carrying; § Migration gives the
  method, not the verdicts.
- The augmentation machinery for non-tracker artifacts.

## Anti-goals

The design fails, even if it works, if any of these is true:

- A committed index table can disagree with the entries it lists.
- A bulk sweep that touches many files makes stale records look fresh.
- A record's status lives only in prose.
- A collection's lifecycle is declared in two places that can disagree.
- A guide names a path that exists only in codescout's checkout.
- One collection has more than one archive location.
- Two sessions that add entries at the same time get a git conflict.

## Constraints

- **Guide gates** (`src/prompts/mod.rs`): `no_guide_names_a_path_or_field_only_this_repo_has`
  refuses `docs/templates/`, `docs/conventions/`, `docs/issues` and others in every guide; only
  paths the server defines (`docs/trackers/`, `docs/augmentations`) are allowed.
  `tracker_conventions_names_no_repo_specific_artifact_kind` refuses `docs/issues`,
  `_TEMPLATE`, `kind: bug` and "bug file" in `tracker-conventions`. The guide was cut to 22 KB
  so a direct `get_guide` arrives inline (`24904379f`); new guide text must keep that.
- **Committed files are the only store** (identity v5). The catalog is a derived index. No
  fact of this design may exist only in the catalog.
- **Identity v5 terms this design relies on:**
  - the key is the filename stem in lowercase, unique per repo; `doc(create)`/`doc(move)`
    refuse a collision;
  - the folder-index file is `readme.md`, matched case-insensitively; its key is the folder
    name;
  - the closed citation grammar `[[key]]`, `[[key:TOKEN]]`, `[[repo/key]]`,
    `[[repo/key:TOKEN]]`; a bare `TOKEN` resolves only inside its own document; paths and
    16-hex ids are lint, never edges;
  - `[[key:TOKEN]]` resolves for a token defined in any descendant whose nearest folder-index
    ancestor is `key`'s readme; a folder without a readme owns nothing and passes its files up;
  - appends go only into single-writer documents;
  - a move that keeps the filename needs no catalog step and no citation repair; a rename
    rewrites every `[[old-key…]]` in the repo.
  That spec's cutover must land before step 4 of § Migration.
- **Two skills live in another repo** (`../claude-plugins/codescout-companion/`):
  `tracker-hygiene` and `reconnaissance`. Their changes ship there.
- **CLAUDE.md byte budget** (`claude_md_stays_within_its_byte_budget`).

## Approaches

1. **Entry = file, collection = folder, lifecycle = layer-1 folder (chosen).** Records are
   files with structured frontmatter; a collection is a folder whose `readme.md` declares its
   schema; the server defines the lifecycle folders and checks them.
2. **Entries in the catalog, files rendered from it** (today's `entry_collection` params made
   universal). Rejected: the catalog is derived and machine-local; it cannot be the store
   (identity v5), and its rows cannot be merged by git.
3. **One file per collection with server-managed sections** (today's model, made stricter).
   Rejected: keeps the 18k-line files, the counters, the `F`/`W` ambiguity and the merge
   conflicts.

For the lifecycle folders, seven kinds were considered (adding snapshots, handoffs, backlog,
event logs, archive). Rejected: none has an end mechanism of its own. A handoff is a temp
stream; a backlog is a ledger whose entries carry `revisit_by`; an event log is a ledger with
`review_every`; a snapshot is not living state and leaves `docs/trackers/` (§ D2); archive is
not a lifecycle but a place inside each collection (§ D3).

For the archive location, a top-level mirror (`docs/trackers/archive/<lifecycle>/<collection>/`)
was considered. Rejected: an archived file outside its collection folder leaves the
collection's ownership, so `[[collection:TOKEN]]` stops resolving; a readme in the mirror
would repeat the collection's key, which v5 refuses; a rename or a lifecycle change would
need two moves kept in sync; and the identity layer would need a lifecycle special case,
which breaks D5. Its advantage (no archived file in a live folder) is cosmetic: one glob,
`**/archive/**`, gives the same view.

## Design

### D1 — An entry is a file; a collection is a folder

```
docs/trackers/
  ledgers/
    observer-blindness/
      readme.md                                     ← key `observer-blindness`; prose + schema
      2026-09-02-the-dirty-check-blames-a-peer.md   ← one entry; key = its stem
      archive/                                      ← no readme: owned by the collection
        2026-08-17-....md
    issue-clusters/                                 ← key_style: name
      readme.md
      declared-not-wired.md
    archive/                                        ← retired ledgers (rare)
  streams/
    tracker-organization/
      readme.md                                     ← exit_when required
      2026-11-11-frictions-is-not-one-file-per-entry.md
    archive/
      local-onnx-embedding/                         ← a wrapped stream, moved whole
  temp/
    artifact-identity-refactor/
      readme.md
```

- **Collection.** A folder directly under a lifecycle folder, with a `readme.md`. Its key is
  the folder name, and it owns the tokens of every descendant file (v5 nearest-index rule).
  `readme.md` holds only prose (purpose, admission rules, standing instructions) and the
  schema (§ Collection schema). It holds **no entry table** and **no section that sessions
  append to**.
- **Entry.** One Markdown file inside a collection folder or its `archive/`. Its lowercase
  stem is its key, cited as `[[key]]`. The body is free text under the collection's entry
  template.
- **The index is rendered on read.** `doc(action="get", id=<collection>)` returns the entry
  table built from entry frontmatter, filterable with `entry_filter`. Nothing in git holds a
  copy of that table, so it cannot drift (anti-goal 1).
- **No companion files.** A stream's frictions and wins are entries of the stream itself
  (`entry_kind: friction` / `win`), so a log cannot outlive its stream (HY-35 cause 7).
- **Notes about a collection are entries** (`entry_kind: note`), never appends to the readme,
  so two sessions never write the same file (anti-goal 7, v5 single-writer appends). An event
  about one entry is an edit of that entry's own file.

#### Entry keys

- **Default `key_style: dated`:** stem `<created YYYY-MM-DD>-<slug>`, as in `docs/issues/`.
  For records of something that happened: frictions, wins, findings, rulings, proposals.
- **`key_style: name`:** stem `<slug>`. For registries of named concepts that are cited by
  name (issue clusters, operator rules) and for collections a tool writes, where the slug
  is derived from the target (§ Writer collections).
- Two concrete families need each style, so both exist; there is no third.
- Uniqueness is v5's: a key is unique per repo, and `doc(create)` refuses a collision.

#### Legacy tokens

An entry migrated from a `PREFIX-N` ledger keeps its old definition heading as its first
heading:

```markdown
## R-91 — the scout ran too late
```

The file lives under the ledger's collection folder, so by v5 ownership
`[[reconnaissance-patterns:R-91]]` resolves to it. The identity cutover rewrites today's
citations (qualified `<stem>:TOKEN`, and bare cross-document tokens that resolve today) into
that form. A new entry has no token and no number; it is cited only as `[[key]]`.

### D2 — Three lifecycle folders, defined by the server

| Folder | A collection here… | Its entries… |
|---|---|---|
| `ledgers/` | never ends | close one at a time (terminal status), then move to the collection's `archive/` |
| `streams/` | ends when its `exit_when` fires; then the whole folder moves to `streams/archive/` | close inside the stream; move with it |
| `temp/` | is temporary; policy parked until HY-34 has two sweeps of data | as a stream |

- The folder **is** the lifecycle declaration. There is no `lifecycle:` frontmatter key, so
  the two cannot disagree (anti-goal 4). The server holds the three names as constants beside
  `LEDGER_DIR` (`src/librarian/tools/append_entry.rs:3772`), so the guide may name them (W-1
  in the session log).
- **What leaves `docs/trackers/`:** snapshots (one-time dated records such as the
  2026-08-15 tool-usage investigation) become reports outside the tracker tree; policies
  (`archive-cadence-policy`) move to the repo's conventions. Neither is living state. The
  location of reports is per repo, so no guide names it.

#### Writer collections

A collection whose readme declares `writer: <tool>` is written by that tool only (today:
`legibility_scan`, `audit_doc_refs`). It uses `key_style: name` with a slug derived from the
target, so a re-scan finds and updates the same entry instead of creating a new one. **Only
durable facts are committed:** status, first seen, closed, the reason. Values that a re-run
recomputes (scores, counts, measurements) are derived data: the catalog holds them and
`doc(get)` renders them. A re-scan therefore changes a file only when a durable fact changes.

### D3 — Archiving is a move into the nearest sibling `archive/`

- An entry moves to `<collection>/archive/<stem>.md`.
- A collection moves to `<lifecycle>/archive/<collection>/`; its own `archive/` moves with it.
- The filename never changes, so the key never changes and no citation needs repair (v5).
- An `archive/` folder never has a readme, so the collection still owns its archived entries
  and `[[collection:TOKEN]]` keeps resolving.
- `doc(move)` refuses to archive an entry whose status is not terminal.
- `doc(move)` refuses to archive a stream that still has a non-terminal entry. Each open
  entry is first closed, or **promoted**: a new entry is created in the target collection
  (usually a ledger) with `**Promoted from:** [[old-key]]`, and the old entry gets
  `**Promoted to:** [[new-key]]` and a terminal status. Entries never move between
  collections, so ownership never changes after creation.
- One collection therefore has exactly one archive location (anti-goal 6).

### D4 — Staleness from declared fields

#### Entry frontmatter (scalars only)

```yaml
entry_kind: friction        # declared by the collection's schema
title: <one line>
status: open                # one of the collection's status words
status_since: 2026-11-11    # set by the server on every status change
created: 2026-11-11
closed:                     # set when status becomes terminal
revisit_by:                 # required when the status class is blocked or deferred
valid: dated 2026-11-11     # invariant | dated YYYY-MM-DD | conditional — <event>
tags: []
# plus any typed field the collection declares under `fields:`
```

`kind:` is set by the server (`entry`) and is not repeated by authors.

#### Relations live in the body

Relations are labelled grammar lines in the body, never frontmatter values, because
`rests_on: [[foo]]` in YAML is a nested list, not text:

```markdown
**Rests on:** [[2026-08-27-negative-results-name-their-scope]]
**Promote when:** the same refusal appears in a second tool
**Promoted to:** [[declared-not-wired]]
```

The label vocabulary is the identity spec's rel-label vocabulary, extended by the labels
above. `link_scan` derives the edges from these lines.

#### Core status classes (fleet-wide, TMR-5)

| Class | Meaning | Required field |
|---|---|---|
| `open` | live work or a live observation | — |
| `blocked` | waits on someone or something | `revisit_by` |
| `deferred` | parked on purpose | `revisit_by` |
| `terminal` | finished: done, fixed, wontfix, superseded, promoted… | `closed` |

A collection declares its own status words and maps each to a class. `doc(create)` and
`doc(update)` refuse a word the collection did not declare.

#### Collection schema (in `readme.md` frontmatter)

```yaml
title: <title>
entry_kinds: [friction, win, note]
statuses: {open: open, investigating: open, parked: deferred, fixed: terminal, wontfix: terminal}
initial_status: open        # REQUIRED: the status a new entry starts in
key_style: dated            # or name
fields:                     # optional typed extra entry fields
  binding: {type: enum, values: [hard, soft]}
  covers: {type: list}
owner: <who decides when it ends>
exit_when: <condition>      # REQUIRED under streams/ and temp/; refused under ledgers/
writer: <tool>              # optional: a tool owns this collection
review_every: 30d           # optional: for event-driven ledgers
hot_cap: 12                 # optional: the hot view (`codescout hot-view`) shows at most this many entries
stale_after: 45d            # optional: the hot view flags an open entry idle for more than this many days
```

`fields:` exists because two concrete consumers need typed per-entry fields: operator rules
(`binding`, `shape`, `covers`, `serves`, `evidence`, parsed in
`src/operator_rules/rule.rs:96-110`) and writer collections. Types are scalars, enums and
lists of scalars; a field holding a citation is a body line, not a field.

The entry template is the `## Entry template` section of `readme.md`. `doc(create)` copies it
into a new entry.

#### Doctor checks (mechanical; the hygiene skill gates any fix)

| Check | Fires when |
|---|---|
| `entry_invalid` | an entry breaks its collection's rules: a status, kind or field the readme does not declare, a field of the wrong type, a blocked or deferred entry with no `revisit_by`, a terminal entry with no `closed`, a non-terminal entry that carries `closed`, or a non-terminal entry that sits in `archive/` |
| `revisit_due` | a blocked or deferred entry's `revisit_by` has passed |
| `cited_target_closed` | a non-terminal entry has a relation line to a target whose `closed` date is after the entry's `status_since` |
| `stream_all_terminal` | every entry of a stream is terminal but the stream is not archived |
| `collection_invalid` | a collection's readme breaks the rules: a schema that does not parse or lacks a required key, `exit_when` missing under `streams/` or `temp/` or present under `ledgers/`, a nested collection, a second readme in one collection folder (the first in path order is the readme), a non-Markdown file in the collection, a file whose frontmatter block does not parse, or a file name that is no key |
| `ledger_quiet` | a ledger with `review_every` has had no entry created or closed in that period |
| `terminal_in_live_dir` | a terminal entry of a ledger has stayed outside `archive/` for more than 30 days after its `closed` date (the default grace; a stream is archived whole, see § D3, and `stream_all_terminal` covers it) |
| `orphan_entry` | a file under a lifecycle folder has no collection readme above it |

Every check reads frontmatter dates and statuses. None reads git commit dates (anti-goal 2).
`entry_invalid` exists because committed files are the only store: a hand edit, a merge or a
pull bypasses the write-time validation in `doc(update)`, and an entry that lacks `revisit_by`
or `closed` would silently switch off `revisit_due` and `terminal_in_live_dir`. It reports one
finding per problem and covers every entry, archived or not.
`collection_invalid` exists for the same reason at collection level: without it, a broken readme
would make its whole collection invisible to every other check. An entry whose frontmatter
parses but breaks the entry rules is reported by `entry_invalid`; a file whose frontmatter block
does not parse is reported by `collection_invalid`, by path. Either way it counts as non-terminal
for `stream_all_terminal`.

Only forward relation labels (`Rests on:`, `Depends on:`) feed `cited_target_closed`. A blocked or deferred entry counts: a closed target is the signal to unblock it. Backward
labels (`Promoted from:`, `Supersedes:`) record history, so a closed target there is expected
and never fires. The label vocabulary belongs to the builder; this rule fixes only which
direction counts.
`doctor` reports; it never moves or edits. Re-confirming that an open entry is still open
is a `doc(update)` that rewrites the same status, which advances `status_since` and silences
`cited_target_closed` until the target changes again.

### D5 — Lifecycle depends on identity, never the reverse

The lifecycle layer uses keys, the grammar and folder-index ownership. The identity layer
knows nothing about lifecycle folders, statuses, schemas or archives: an `archive/` folder is,
to it, only a folder without a readme.

### Tools

- **`doc(create)` for an entry:** given a collection key, an `entry_kind`, a title and a body,
  the server builds the path (`<collection>/<stem>.md` from `key_style`), fills the frontmatter
  from the schema (`status` defaults to the collection's `initial_status`, `status_since` and
  `created` to today) and the body from `## Entry template`. No allocation, so it works in a
  worktree.
- **`doc(create)` for a collection:** under a lifecycle folder, with the schema; refuses a
  stream or temp collection without `exit_when`.
- **`doc(update)`:** validates status words against the schema, sets `status_since` on every
  status write and `closed` on a terminal one, and validates `fields:` types.
- **`doc(move)`:** the D3 rules. A rename (key change) uses the identity spec's rename
  rewrite.
- **`doc(get)` on a collection:** renders the entry table, including derived values of writer
  collections; `entry_filter` filters entries.
- **`append_entry`:** has no role in the new model; § Repo scope says when it is removed.
- **Migration command:** § Repo scope.

### `get_guide`

`tracker-conventions` is rewritten around the entry model: the three lifecycle folders, entry
and collection frontmatter, relation lines, status classes, the archive rule, the grammar,
the doctor checks and the migration command. It names only server-defined paths. A short
legacy section says how a repo that has not migrated keeps working and how to migrate it.
A repo's own collections, templates and routing (which collection takes which observation)
stay in the repo: in codescout, the routing that `docs/TAXONOMY.md` holds today. Sections
about `PREFIX-N` allocation, high-water marks and prefix ownership leave the guide with the
machinery.

### Hygiene skill (other repo)

The detectors become thin wrappers over the doctor checks: D3 (stale-active) and D10
(session-log decay) are replaced by `stream_all_terminal`, `revisit_due`, `ledger_quiet` and
`cited_target_closed`. The human gate per finding stays. The sweep cadence fields
(`next-sweep-due`, `sweep-interval-days`) move to the hygiene collection's `readme.md`.

### Reconnaissance skill (other repo)

`F-N`/`W-N` entries become `entry_kind: friction` / `win` entries created in the work stream's
collection with `doc(create)`. The session-log template becomes the stream collection
template.

## Repo scope

> **Amended 2026-11-15** by `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`
> (§ Amendment to the parent spec): the migration is one skill that drives three CLI commands,
> one tracker at a time, with the user's approval per tracker. Every mention of "the migration
> command" in this spec (below, § Tools, § Migration step 2, § `get_guide`, § What is left to
> the builder) reads as that skill and its commands.

The new layout is the model for every repo that runs codescout (option a):

- **A migration command** converts one repo: it creates the collections, writes the entries,
  rewrites retired keys (§ Migration step 4) and archives retired files' content. It runs per
  repo, by a person or an agent, and is re-runnable.
- **Legacy mode until a repo migrates:** files directly under `docs/trackers/` (outside the
  three lifecycle folders) are still indexed as today, `append_entry` still serves them, and
  the lifecycle checks ignore them. `doctor` reports one summary line per repo: "N flat
  trackers not migrated".
- **Removal** of `append_entry`'s allocation, high-water marks, reservations and the legacy
  guide section happens in a later release, after the known repos have migrated. That
  release is a separate decision.

## Consumers that read trackers today

Each must work on the new layout before its old file is retired.

| Consumer | Contract today | Needs |
|---|---|---|
| `src/operator_rules/mod.rs:24` `LEDGER_PATH` | reads `docs/trackers/operator-rules.md` at runtime | read the `ledgers/operator-rules/` collection (`key_style: name`, `fields:` for the rule fields) |
| `src/operator_rules/mod.rs:52` `manifest_of` | `<!-- rules: OP-1, … -->` in the generated block of each profile's CLAUDE.md | the manifest lists entry keys; the four profile blocks are regenerated once |
| `src/librarian/tools/legibility_scan/mod.rs:390` | writes `legibility-backlog.md` | a `writer: legibility_scan` collection (§ Writer collections) |
| `librarian(audit_doc_refs)` default tracker | writes `doc-ref-audit` | same, `writer: audit_doc_refs` |
| `scripts/pre-commit-ledger-counts.py:92,102`, `src/librarian/requirements.rs:569` | roster `issue-clusters.md` + `issue-clusters/` | the `issue-clusters` collection; cluster slugs = entry keys |
| `scripts/measure/judge.py`, `lessons.py` | read `operator-rules.md`, `reconnaissance-patterns.md` at given SHAs | choose the layout by SHA (old SHAs keep old paths) |
| `scripts/tc-suites/artifact-entries.json` | `expect_path` + `expect_entry` (`R-170`, `HY-25`) | re-pin to the new paths; the tokens stay as legacy headings |
| `tracker-hygiene` skill + SessionStart banner | `tracker-hygiene-log` frontmatter cadence | the hygiene collection's `readme.md` |
| `reconnaissance` skill | `docs/templates/session-log.md`, `append_entry` | § Reconnaissance skill |
| `docs/TAXONOMY.md`, CLAUDE.md pointers | cite tracker paths and prefixes | rewrite to collection keys in the grammar |
| Code and test fixtures that build tracker paths (69 files under `src/` contain `docs/trackers/`) | various | builder enumerates with `references`/`grep` |

## Migration (codescout first)

Order:

1. The identity cutover lands (lowercase keys, grammar, citation rewrite, derived catalog).
2. Server support: lifecycle constants, entry and collection schema validation, entry
   `doc(create)`, `doc(update)` status handling, D3 moves, rendered `doc(get)`, the doctor
   checks, legacy mode, the migration command.
3. Guide rewrite and the two skill changes.
4. Collections move one at a time (strangler). For each retired tracker:
   - create the collection under its lifecycle folder;
   - carry **every** record into an entry file: open records into the collection, terminal
     records into its `archive/`; a record from a `PREFIX-N` ledger keeps its heading token
     (§ Legacy tokens); a record may be shortened, never dropped;
   - rewrite every `[[old-key…]]` of the retired document to `[[collection…]]` (complete,
     because the grammar is closed); split archive files such as
     `reconnaissance-patterns-archived-entries` fold into the same collection;
   - switch its consumers;
   - delete the retired file. Git keeps its history, and keeping it would give its key and
     its headings a second definer.
5. The legacy top-level `docs/trackers/archive/` (66 flat files) is converted the same way into
   `<lifecycle>/archive/<collection>/`; snapshots and policies leave `docs/trackers/`.
6. The existing folders (`effort-steering/`, `ecc-integration/`, `bistriceanu/`, `frictions/`,
   `issue-clusters/`) are migrated like any other collection. `frictions/` defines its 16
   `FR-N` in its index today (session log F-1), so they move into entry files.

## What is left to the builder

- The default `review_every`. The grace period for `terminal_in_live_dir` is 30 days, measured on
  `docs/issues` (2026-11-11): of 212 terminal entries outside `archive/`, 69 were older than 14
  days and 1 older than 30.
- Whether `doc(create)` for entries is a new action or a mode of `create`.
- Where the three lifecycle constants live, and the classify/indexer change that marks a
  file under a lifecycle folder as an entry or a collection.
- The type vocabulary of `fields:` beyond scalars, enums and lists of scalars.
- The per-collection choice of how much to shorten carried records.
- The location of reports in codescout.
- The migration command's name and flags.
- Test design. Minimum: one test per anti-goal (no committed table exists; a bulk commit
  changes no check's result; an undeclared status is refused; a second lifecycle declaration
  is impossible; the guide gates stay green; a second archive path is refused; two worktrees
  adding entries merge without conflict).

## Calls made after the builder check (review these)

1. Retired single-file trackers are **deleted** after every record is carried (git keeps
   history), not kept as an archive: a kept file would share its key and token headings with
   the new collection.
2. Legacy tokens survive as the entry's own heading, not as an `aliases:` field: v5 ownership
   makes `[[collection:TOKEN]]` resolve, and the identity cutover rewrites old citations.
3. A stream with open entries cannot be archived; each open entry is closed or promoted. Entries
   never move between collections.
4. Collections may declare typed `fields:`; the two concrete users are operator rules and
   writer collections.
5. Writer collections commit only durable facts; recomputed values are derived data.
6. `status_since` is a server-maintained frontmatter field, so `cited_target_closed` needs no
   git date.
7. The folder-index file is `readme.md` (GitHub/GitLab render it in the folder view).
8. The operator-rules manifest lists entry keys; the four profile blocks are regenerated once.
9. New collections use no augmentation; legacy augmentations live in their sidecars until
   their collection migrates.
