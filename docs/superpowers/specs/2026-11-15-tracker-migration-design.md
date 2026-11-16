---
id: '4c4ab2c12c52d73e'
kind: spec
status: active
title: 'Tracker migration (Plan L-E): an agent-run, user-approved move of flat trackers into lifecycle collections (design)'
---

# Tracker migration (Plan L-E): an agent-run, user-approved move of flat trackers into lifecycle collections (design)

**Status:** v2, 2026-11-15. v1 (`02a19eca8`) was approved by the operator ("it's ok, go") after a builder check (calls 8–15). The lifecycle line's independent Opus review (`docs/research/2026-11-15-plan-le-design-review.md`, `4e086aea7`) found v1 not ready for a plan: 2 Critical, 9 Important, 11 Minor. v2 settles every Critical and Important finding and the Minors that change behaviour; § Review findings and how v2 settles them maps each one. None of the operator's four decisions (§ Intent) changed. Parent spec: `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md` (§ Migration, § Repo scope, § Consumers that read trackers today), amended by this design (§ Amendment to the parent spec). Work-stream log for the migration: the stream collection `context-migration` (`docs/trackers/streams/context-migration/`). The code lands on `experiments` directly, with `./scripts/gate.sh` green on the exact tree before the fast-forward (operator, relayed by the lifecycle line 2026-11-15).

## Intent

Every flat tracker under `docs/trackers/` must become a lifecycle collection (parent § Migration step 4) without losing a record, in every repo that runs codescout. The operator decided in the brainstorm:

1. **Scope:** the full migration, for every repo, not a pilot of two or three trackers.
2. **Who decides:** an agent, guided by a skill, proposes how each tracker maps; the user approves; scripts do the mechanical work. The migration is not fully deterministic, because the per-tracker choices are judgment.
3. **What the user approves:** one mapping per tracker (folder, key, schema, the status-word table with record counts, `hot_cap`), then the verification report and the diff. Not each record.
4. **Who writes the files:** a codescout command writes the collection from the approved mapping. Record text never passes through the model.

## Measurements (2026-11-15)

| Fact | Number |
|---|---|
| Flat trackers in codescout | 81 files, 83,510 lines |
| Of those, with `PREFIX-N` definition headings | 55; the other 26 have none (handoffs, snapshots, policies, worklists, writer outputs) |
| Trackers that keep status in `**Status:**` body lines | 47 |
| Trackers with a status column in an index table | 23 to 43 by method (an exact `Status` header cell: 23); column position differs per tracker |
| Status vocabulary | free text: `bug-fix-session-log` index column `fixed-verified` 121, `open` 62, `mitigated` 33; `observer-blindness` uses sentences ("wrong instrument — three claims survived …") |
| `bug-fix-session-log` | 399 definition headings, 344 `**Status:**` lines, 3 index rows with no heading |
| Record headings that are not definitions | e.g. `## R-56b` and `## R-59b` (`archive/reconnaissance-patterns-archived-entries.md:332,346`), `### T-15a`, `### TU-11a`, `## F-3a (deferral)` |
| A token repeated over headings of one file | `prompt-hamsa-audit-log.md`: `A-38` on 11 headings (one `##`, the rest `###` subsections) |
| Markdown files that name `observer-blindness.md` | 58, of them 54 inside inline code; `bug-fix-session-log.md`: 119 |
| Sidecars in `docs/augmentations/` named for flat trackers | 24 |
| Legacy top-level `docs/trackers/archive/` | 66 flat files |
| Existing tracker folders | `bistriceanu/`, `ecc-integration/`, `effort-steering/`, `frictions/`, `issue-clusters/`, `mechanisms/`, `temp/` |
| Flat trackers in `../claude-plugins` | 26, plus `INDEX.md` |
| Collections that exist | 1: `streams/context-migration/` (created 2026-11-15 for this work) |

The free-text status is why a fixed rule set cannot map records: a record's status class decides whether its file goes into `archive/` or stays live, and a wrong guess moves it silently.

## Goals

- **M1.** Every record of a migrated tracker is carried into exactly one entry file, with its heading kept (parent § Legacy tokens). Its body is the source section after one stated, pure transform (§ Citations inside records). Nothing is shortened unless the approved mapping says so.
- **M1a.** M1 is proven by a check that does not depend on the splitter: every line of each source is accounted for exactly once (§ What check proves, the partition).
- **M2.** The user approves each tracker's mapping before anything is written, and the verification report after.
- **M3.** A deterministic check proves M1, M1a and the lifecycle rules after every tracker; a migration whose check is not clean is not committed.
- **M4.** The same skill and commands work in any repo that runs codescout.
- **M5.** Each new collection declares `hot_cap` and `stale_after`, or the mapping records why it has none, so the session-start hot view has data.
- **M6.** One tracker at a time (strangler). The repo stays usable between trackers; a flat tracker keeps working in legacy mode until it moves.

## Non-goals

- Switching code consumers (parent § Consumers: `operator_rules`, `pre-commit-ledger-counts.py`, `scripts/measure/*`, the tc-suites pins, `legibility_scan`, `audit_doc_refs`). Each is its own plan. The migration refuses a tracker that code still reads (§ Preconditions).
- Running the codescout migration itself. L-E builds the tools; running them is the follow-up the tools make possible.
- The doctor wiring of `lifecycle_findings`, `doc(move)` D3 rules, and the removal of `append_entry`.
- The `get_guide` `tracker-conventions` rewrite, and the reconnaissance and tracker-hygiene skill changes (parent § Hygiene skill, § Reconnaissance skill). They gate some trackers (§ Preconditions).
- Rewriting path mentions of a tracker (§ Path mentions).

## Anti-goals

The design fails, even if it works, if:

- A record is lost, duplicated, or its body text changes, and no check reports it.
- The model retypes record text.
- A collection's lifecycle or schema is declared in two places that persist (the mapping file is input, never committed).
- An open record lands in `archive/`, or a terminal one stays live, because a status word was guessed.
- A migrated record looks fresher than it is, or a migrated overdue record looks in good standing.
- A failed run, or its restore, destroys another session's uncommitted work.
- The migration writes a commit, or pushes.

## Approaches

Considered for the overall shape:

1. **A server command with a committed manifest** (the parent spec's "one server command"). Rejected by the operator: the per-tracker choices are judgment, so a manifest is only a slower way for a person to do what an agent can propose.
2. **Agent-run with user approval, scripts for the mechanics (chosen).**
3. **Fully automatic rules.** Rejected: the status vocabulary is free text (§ Measurements).

Considered for who writes the entry files, within approach 2:

1. **A codescout command writes from the approved mapping (chosen).** Record text is copied by code; the agent's job is the mapping.
2. **The agent calls `doc(create)` per record.** Rejected: `bug-fix-session-log` alone has 399 records, and every body would pass through the model, which can change a word with no error.
3. **A plugin script writes the files.** Rejected: it would copy the Rust frontmatter and schema rules into another language; the two copies drift, and the checks then reject what the script writes.

## Design

### Parts and where they live

| Part | Repo | Reuses |
|---|---|---|
| `inventory` command | codescout | `link_scan::extract::entry_sections` and `def_re` for definition headings; `cutover/code_refs.rs` (`CodeRefs`) plus a plain `git grep -F` for readers |
| `apply` command | codescout | `lifecycle::collection::collection_readme`; a date-explicit entry renderer split out of `lifecycle::write::new_entry` (§ Dates); `lifecycle::naming::{entry_stem, archive_rel_path}`; `identity::grammar::extract_spanned`; `util::write_catalogued_file`; `cli::write_lock` |
| `check` command | codescout | `lifecycle::collection::load_collection_dir`; `lifecycle::checks::check` over the loaded collection's tree; `lifecycle::hot_view` |
| Skill `migrating-trackers` | claude-plugins (`codescout-companion/skills/`) | the three commands, `doc`, git |

The commands are CLI subcommands (like `cutover` and `hot-view`), not MCP tools: the advertised tool surface has 0 headroom (`TOOL_SURFACE_CHAR_BUDGET` 57_664), the agent runs them through `run_command`, and the operator can run them by hand. The names are the builder's; this document calls them `inventory`, `apply` and `check`.

**Catalog stance.** The commands write files only, through `write_catalogued_file` (a guarded atomic single-file write that records no catalog or audit row). The catalog learns the new files by reindex or the file watcher, and the derived catalog after identity Plan E. A new entry or readme carries no `id:`.

**Binary.** The skill finds the binary as the session-start hook does (`resolveCodescoutBinary`: `codescout` on `PATH`, then the MCP-configured command) and probes it with the `trackers` help command. A non-zero exit or empty output means "this codescout has no tracker migration": the skill stops and says which build is needed.

### The per-tracker loop (the skill)

1. **Pick.** The skill lists flat trackers with their record counts and readers, and proposes an order: trackers that no code reads and no skill writes, smallest first. The user may reorder.
2. **Inventory.** `inventory <source>…` prints, as JSON and as a short text report:
   - every record: its token, heading line and level, title, line range, its raw status value and where it came from (`**Status:**` line, an index column by header name, or none), and every date the record states;
   - every heading that looks like a record but is not a definition (a token-shaped string `[A-Z]{1,4}-\d+[a-z]?` in the heading): a **possible record**;
   - every token defined on more than one heading, and whether the repeats nest inside the first (§ What a record is);
   - the distinct raw status values with counts, and the count of records with no status;
   - index rows with no heading, and records with no index row;
   - status disagreements between the body and the index;
   - the non-record sections, by line range;
   - every mention of each source outside the source: `[[key…]]` citations, path mentions (with the file, line, and whether inline code holds it), and its frontmatter `id:` written in prose;
   - every non-markdown file that names a source (`CodeRefs`, plus a plain `git grep -F <stem>` over every tracked file, JSON and YAML included; the difference is listed. The grep skips what `CodeRefs` already treats as data: `.codescout/audit/`, paths outside the key space, and the data extensions `jsonl`, `txt`, `csv` and `log`. It keeps `json`, because a JSON file can read a tracker. Amended 2026-11-16 after the Task 6 review: on `observer-blindness.md` the plain grep returned 3077 hits in 35 files, nearly all audit and eval data);
   - the sidecar `docs/augmentations/docs-trackers-<stem>.yaml` if one exists, with its prompt text.
3. **Propose.** The agent drafts a mapping file and shows the user one screen: target, schema, the status-word table (raw value → declared status → class, with counts), `status_none`, overrides, each possible record and how it is handled, each non-record section and where it goes, the classified code mentions, the sidecar's fate, `hot_cap` and `stale_after`, and anything the agent is unsure about.
4. **Approve.** The user approves or edits. Nothing is written before this.
5. **Apply.** `apply <mapping>` writes the collection, deletes the sources and the sidecar, and rewrites citations. It prints the exact list of files it wrote and deleted. It refuses rather than guesses.
6. **Check.** `check <mapping>`. Not clean → the skill stops, shows the findings, and the user decides: fix the mapping and re-apply (§ Re-runs), or restore exactly the files apply listed.
7. **Review.** The skill shows the check report (with its review items), `git diff --stat`, the readme and two sample entries. The user approves.
8. **Commit.** On the user's word, by exact file names from apply's list: stage those names (never a directory: the commit guard refuses `git add <dir>` followed by a pathspec commit), read `git diff --cached --name-only` in a separate call, then `git commit -m … -- <the same names>`. The commit message carries a one-line summary of the overrides and drops. The skill never pushes.

**Day one.** With `stale_after` set, almost every old open record is stale after migration. That is honest: the record has not been re-confirmed since its last status. The skill says so in step 7, and a `doc(update)` that re-confirms a status clears it.

### What a record is

- **A record** is a section under a definition heading (`def_re`: `TOKEN — title`, `[A-Z]{1,3}-\d+`), or a heading the mapping names a record (`records_extra`, § The mapping file), such as `## R-56b — Hit: …`.
- **Its extent** starts at its heading and ends before the earlier of: the next heading of the same or higher level, or the next record heading of any level whose token differs. The same-or-higher bound keeps the last record from absorbing trailing `## Template` or `## Summary` sections (the 12.1% error measured at `link_scan/extract.rs:26-33`). The record-heading bound stops a parent from copying a nested child.
- **A repeated token.** A definition heading whose token equals the token of the record that encloses it is a subsection of that record and stays inside it (`prompt-hamsa-audit-log` `A-38`). Any other repeat is a refusal that names both lines.
- **Non-record sections** are every other line range: the frontmatter, the preamble, index tables, the template, the status vocabulary, free prose. The partition (§ What check proves) covers them too.
- **A possible record** blocks apply until the mapping names it a record (`records_extra`) or confirms it as a non-record section (`sections`, with `confirm = true`).

### Source kinds

| `source_kind` | A record is | Used for |
|---|---|---|
| `file` | a record per § What a record is | the 55 trackers with `PREFIX-N` records |
| `sections` | every heading of the given `level` (e.g. 2), with the same extent rule; the key comes from the heading (`key_style: name`) | a non-record tracker the user wants split (`sdd-ruling-log.md`) |
| `whole` | the whole file, one record | a handoff that becomes one entry of an existing stream |
| `files` | each markdown file of a folder | `issue-clusters/` (one file per cluster) |

- A `files` entry keeps its key (the lowercase stem); the file is written with the lowercase name, so no citation changes. Its old frontmatter is replaced by entry frontmatter; old keys map through `columns` or are listed as dropped.
- **Manual, through the skill:** folders whose records are table rows with separate evidence files (`frictions/`, 16 `FR-N` in `index.md`), and folders with non-markdown files (`effort-steering/`, `bistriceanu/`, `mechanisms/`). The skill migrates them with `doc(create)` and `doc(move)` after the user approves, and runs `check` for its structural items.
- **Move out of `docs/trackers/`:** a snapshot becomes a report, a policy goes to the repo's conventions. The skill uses `doc(move)`; the file moves intact, so no record check applies.

### The mapping file

A TOML file the agent writes outside the tracked tree (the skill picks the path; the builder may fix a default under `.codescout/`). It is input to one `apply` run and is never committed. Content:

- `sources`: one or more repo-relative paths, each with its `source_kind` (and `level` for `sections`). They fold into one collection (a live tracker plus its split archive files). A token defined in two sources is a refusal.
- `target`: lifecycle folder, collection key, `archived = true|false`, `existing = true|false`.
  - The key defaults to the stem of the first source (parent D1: `observer-blindness.md` → `ledgers/observer-blindness/`).
  - `archived = true` writes the collection into `<lifecycle>/archive/<key>/`: for a stream whose records are all terminal, and for the legacy top-level archive files.
  - `existing = true` adds entries to a collection that exists (one entry of an existing stream; late records, § Re-runs). The schema then comes from that readme, and `schema` must be absent.
- `schema`: every parent § Collection schema key, plus `hot_cap` and `stale_after`, or `hot_view_none = "<reason>"`.
- `readme_body`: the readme prose (purpose, admission rules, standing instructions, `## Entry template`). The agent drafts it from the tracker's preamble and the sidecar's prompt; the user approves it.
- `entry_kind`: one kind, or a rule by token prefix (`F` → `friction`, `W` → `win`).
- `status_source`: `body_label`, `index_column = "<header>"`, or `constant = "<status>"`.
- `status_map`: raw value → declared status. **It must cover every distinct raw value the inventory reported.**
- `status_none`: the declared status of a record with no raw value. Required when the inventory reports any.
- `columns`: index header → declared field name, `created`, or `title_alias`. Optional.
- `row_title_column`: the index header whose cell titles a row-only record (§ Index rows). Required when the inventory reports any.
- `records_extra`: possible records the user names records, by heading line, each with its token text (`R-56b`).
- `overrides`: token → status, `created`, `closed` or `revisit_by` for single records.
- `sections`: each non-record section by its first line → `readme` or `drop`; a possible record also needs `confirm = true`.
- `code_mentions`: each non-markdown file that names a source, classified `reader`, `comment` or `fixture`, approved by the user.
- `sidecar`: `retire` (delete it; its prompt has gone into `readme_body`) or `keep = "<reason>"`.
- `shorten`: empty by default. A record listed here is carried with the replacement body the mapping gives; the check reports it as shortened.

### What apply writes

- The readme from `collection_readme` (re-read through `schema::parse`), unless `existing`.
- One entry per record, from the date-explicit renderer (§ Dates): frontmatter from the schema and the record's mapped status; body = the record's section after § Citations inside records, plus the `**Index row:**` line where one exists (§ Index rows). Terminal entries go to the collection's `archive/`; others stay live. The stem is built from `created`.
- **Every entry key is checked against the repo's key space** before anything is written (a dated stem can collide with `docs/issues/<date>-<slug>` or another tracker). A collision is a refusal naming both; an override `slug` resolves it.
- The citation rewrite (§ Citations inside records): in the carried bodies and the readme always; across every other markdown file only when the key changes.
- The deletion of every source file and, when `sidecar = retire`, the sidecar.
- Every write through `write_catalogued_file`, under the project write lock (`cli::write_lock`, as `cutover export` takes it). No commit, no push.
- Its output lists every file written and deleted (JSON), so the skill stages and restores by exact names.

### Re-runs and late records

- `apply` re-derives everything from the sources **at `HEAD`**, because it deleted them from the tree.
- **Idempotence by content:** a target file that already holds the text apply would write is left alone and reported `already applied`; a target that exists with other text is a refusal naming it.
- **The clean-tree precondition accepts a previous run:** an uncommitted change under `docs/trackers/` is allowed only on a path the current plan writes or deletes. Any other dirty path there is a refusal.
- **Late records.** A worktree branch that appended to the flat file after the migration merges with a modify/delete conflict. The skill resolves it by keeping the merged flat file, running `inventory` on it, and running `apply` with `existing = true`: it writes entries only for tokens the collection lacks, refuses a token whose text differs from its entry, and deletes the file again.

### Dates

The flat trackers hold no `created`, `status_since` or `closed`, and the parent spec forbids staleness from git commit dates. Each migrated record still needs them.

- **The renderer.** `new_entry` uses one `today` for `created`, `status_since`, `closed`, the dated stem and the past-`revisit_by` refusal (`write.rs:429-516`). The migration needs a renderer that takes explicit `EntryDates { created, status_since, closed }` and a separate `now`. `new_entry` becomes a caller of it with all three dates equal to `today`, so its behaviour does not change.
- `created`: the **minimum** of every date the record states (heading, index row, `**Date:**`, `**Valid:** dated`, a dated status note) and the earliest `git blame` date of its lines. A re-stamped `**Valid:**` cannot make a record younger, and neither can a bulk sweep.
- `status_since`: a date the record states for its current status, else `created`. This errs toward stale.
- `closed` (terminal only): a date the record states for its close, else the latest `git blame` date of its lines. It may be a bulk-sweep date; it only orders archived history and can add `cited_target_closed` review noise (§ What check proves item 8).
- `revisit_by` (blocked or deferred): from the record or `overrides`, **carried unchanged even when it is past**, so the hot view shows the record as revisit due. Only a missing `revisit_by` is a refusal. The past-date refusal stays a rule for new writes.
- **Date order.** Apply refuses, and check re-verifies: `status_since < created`, `closed < created`, `closed` on a non-terminal record, and any date after `now`.
- Blame refuses a shallow clone (`git rev-parse --is-shallow-repository`), where every line would carry the boundary date.
- Every date the command derives, rather than reads, is listed in the check report.

### Citations inside records

A bare token such as `R-34` resolves only inside its own document. Once a tracker is split, each record is its own document, so a bare `R-34` in record `R-91` would stop resolving.

- `apply` rewrites, in each carried body and the readme body, each bare token **that one of the sources defines** to `[[<key>:TOKEN]]`. A bare token no source defines is left alone: it did not resolve before either.
- **Not rewritten:** a token inside code spans or fences (as the grammar already skips), inside the text of an ordinary link (`[see F-1](b.md)`; rewriting it would nest a wiki link inside a link), and inside a URL word (`grammar::in_url_word`). Each skip is counted in the report.
- When the key changes, `[[old-key:TOKEN]]` and `[[old-key]]` become `[[<key>:TOKEN]]` and `[[<key>]]` in the bodies and across every other markdown file. When the key is reused (the default), outside citations stay and resolve by v5 ownership.
- The rewrite is one pure function, `rewrite_record_citations(text, defined_tokens, old_key, new_key) -> String`, on `identity::grammar::extract_spanned`. `check` applies the same function to each source section at `HEAD` and compares.

### Index rows

- Each mapped column (`columns`) becomes that frontmatter value: a declared field, `created` (one of the dates of § Dates), or `title_alias`.
- Every record with an index row also gets the row carried **verbatim, exempt from the citation rewrite**, as its last body line `**Index row:** <the row>`. It is a historical copy of the old index, so no index data is lost even for columns nobody mapped.
- A **row-only record** (an index row with no heading; 3 in `bug-fix-session-log`) becomes an entry whose body is a synthesised heading `## <TOKEN> — <cell of row_title_column>` and the row line. So the token is defined again and `[[<key>:TOKEN]]` resolves. It is listed in the check report.
- A record with no index row is carried without the line and is listed.
- A status disagreement between body and index is listed in the inventory and shown in the proposal; `status_source` decides, `overrides` fixes single records.

### Path mentions

A path mention of a tracker (`docs/trackers/observer-blindness.md`, a relative link to it, its frontmatter id in prose) is **lint, not an edge** (identity v5: "paths and 16-hex ids are lint, never edges"). The migration does not rewrite them, and they do not block the check.

- Most of them sit in inline code (54 of 58 for `observer-blindness.md`), which the identity cutover counts and never edits (`cutover/rewrite.rs:572-573`), and in frozen files (`CLAUDE.md`, `.codescout/memories/*.md`).
- The inventory lists them; the check counts them as review material (item 8). The agent may offer to edit prose mentions in the same commit, with the user's approval, but never in frozen files.
- A `[[old-key…]]` citation is different: it is an edge, so a key change must leave none (check item 6).

### What check proves

`check <mapping>` loads the collection from its directory with `load_collection_dir` (so an archived or existing collection loads too) and compares it with the sources **at `HEAD`**. It normalises line endings first: both sides are read as lines (`\r` stripped), and leading and trailing blank lines of a body are ignored, matching `frontmatter::render_body`. Blocking items:

1. **The partition.** Every line of every source belongs to exactly one of: a carried record, a readme section, a dropped section, or the replaced frontmatter. No line is in two, none in zero. This does not rely on the splitter: it is computed from the line ranges apply used, checked against the source's line count, and it catches a missed record, a duplicated subsection and a lost tail.
2. Every record token (definitions, `records_extra`, row-only records) is the first heading of exactly one entry, and no entry has a token the sources lack.
3. Each entry body equals `rewrite_record_citations` of its source range, plus the `**Index row:**` line where one exists, except the records listed under `shorten`.
4. Every status is declared; every terminal entry is in `archive/` (or the collection is archived) and every other one is live; date order holds (§ Dates).
5. The collection loads, and `lifecycle::checks::check` over its tree reports none of `entry_invalid`, `collection_invalid`, `orphan_entry` for it.
6. When the key changed, no `[[old-key…]]` citation is left in the repo; when `sidecar = retire`, the sidecar is gone.
7. The hot view renders (exit 0) when `hot_cap` is declared and the collection is live, so a collection with problems is not shown as `0 hot`.

For review, not blocking:

8. The time-based findings (`revisit_due`, `cited_target_closed`, `ledger_quiet`, `stream_all_terminal`, `terminal_in_live_dir`), path mentions by file, derived dates, dropped sections, skipped rewrites, row-only records, records without an index row, and status disagreements.

Exit 0 only when 1–7 hold. The skill treats any other exit as "not clean" and shows item 8 every time.

### Preconditions (apply refuses, naming the cause)

- **Catalog facts.** The identity cutover has moved the sources' catalog-only facts (events, observations, manual links, augmentation params) into files: a cutover plan scoped to the source paths writes nothing (parent § Migration step 1). A text test cannot see those facts, so it does not replace this. On a machine with no old catalog the refusal says so, and the skill asks the user whether the repo ever had one.
- **Readers.** Every non-markdown file that names a source is listed in `code_mentions`. A `reader` is a refusal until that consumer's own plan has switched it. A `comment` or `fixture` does not block; the skill may edit it in the same commit with the user's approval.
- **Writers (skill-side).** The skill refuses a tracker that an installed skill or hook still writes by path: every `*-session-log` and `reconnaissance-patterns` (the reconnaissance skill copies the template to `docs/trackers/<topic>-session-log.md` and appends to `reconnaissance-patterns.md`), and `tracker-hygiene-log` (the tracker-hygiene skill and the session-start banner). They wait for the parent § Reconnaissance skill and § Hygiene skill changes. The codescout commands stay generic; the list lives in the skill, next to the skills it names.
- **Keys.** The target key and every entry key are free in the repo, except the stems of sources this run deletes.
- **Clean tree.** No uncommitted change under `docs/trackers/` except on paths the current plan writes or deletes (§ Re-runs), and no uncommitted change in any other file apply would rewrite (so a restore of apply's list cannot destroy another session's work).
- **Not shallow** (§ Dates).

### Repo scope

The skill and commands assume nothing about codescout's own trackers. A repo with no `PREFIX-N` ledgers (most of the ~35 on this machine) uses the `sections`, `whole` and move-out outcomes. Legacy mode is unchanged: a flat tracker keeps working with `append_entry` until its turn.

## Amendment to the parent spec

Parent G8 and § Repo scope ("a migration command converts one repo") become: **every repo migrates with one skill that drives three CLI commands, one tracker at a time, with the user's approval per tracker.** § Migration step 4 is unchanged in content (every record carried, citations rewritten, consumers switched, file deleted); it is executed per tracker by the skill. The non-goal "deciding, per existing tracker, which content is worth carrying" stays: the decision is the user's, made in the loop. The parent's other mentions of "the migration command" (§ Tools, § Migration step 2, § `get_guide`, § What is left to the builder) read as this skill and its commands.

## Review findings and how v2 settles them

| Finding (review `4e086aea7`) | v2 |
|---|---|
| C1 path mentions block check item 6 | § Path mentions: lint, review item 8; item 6 keeps only `[[old-key…]]` and the sidecar |
| C2 the proof shares the splitter | M1a; check item 1 (partition); possible records; § What a record is (extent, repeated token) |
| I1 `new_entry` has one `today`; no date rules | § Dates: `EntryDates` renderer, `created` = minimum, stem from `created`, date-order rules in apply and check |
| I2 past `revisit_by` refused | carried unchanged; only a missing one refuses |
| I3 readers and writers missed | `git grep -F` beside `CodeRefs`, JSON and YAML included; skill-side writer precondition |
| I4 no lock, re-run vs clean tree, restore, late records | write lock; § Re-runs; clean-tree rule names exact paths and other files apply rewrites |
| I5 non-record trackers | `sections` and `whole` source kinds; the partition covers them |
| I6 sidecars, catalog facts | `sidecar` in the mapping; inventory reads it; catalog-facts precondition kept |
| I7 no status, row-only, row rewrite | `status_none`; synthesised heading via `row_title_column`; row line verbatim and exempt |
| I8 parent amendment incomplete | parent § Repo scope gets a pointer; § Amendment names the other four mentions |
| I9 link text, URLs, binary | § Citations inside records (skips); § Parts (binary and probe) |
| M-1, M-2 module names | § Parts names `lifecycle::checks::check` and `load_collection_dir` |
| M-3 catalog stance | § Parts, catalog stance |
| M-4 normalisation | § What check proves, first paragraph |
| M-5 entry-key collisions | § What apply writes |
| M-6 `closed` noise | § Dates, accepted as review noise |
| M-7 shallow clone | § Dates, § Preconditions |
| M-8 day-one staleness | § The per-tracker loop, day one |
| M-9 `IC-*` stems | § Source kinds: key kept, file name lowercased |
| M-10 `bistriceanu/`, `mechanisms/` | § Source kinds, manual |
| M-11 measurements | § Measurements corrected |

## What is left to the builder

- The command names and flags (`codescout trackers inventory|apply|check` is one shape; `migrate` is close to the existing `migrate-memories`).
- The mapping file's exact TOML layout and default location.
- How a raw status value is normalised before lookup (trim, case, the text before ` — `), as long as the inventory reports the same normalised values the map must cover.
- The `git blame` options (`-M`, `-C`, `-w`); blame reads `HEAD`.
- How the cutover plan is scoped to the source paths.
- The JSON shape of the reports.
- Test design. Minimum: one test per anti-goal, plus: a record with a non-`def_re` heading is never silently dropped (the partition fails); a repeated token nests or is refused; a re-run after a partial apply completes; a JSON reader blocks; a past `revisit_by` is carried; a re-stamped `**Valid:**` does not make `created` younger; a token inside link text is not rewritten.

## Calls I made (review these)

1. The commands are CLI subcommands, not MCP tools: no tool-surface cost, and the operator can run them by hand.
2. The mapping file is never committed; the readme is the only lasting declaration. The commit message carries a one-line summary of overrides and drops.
3. `status_since` defaults to `created`, so the migration errs toward stale.
4. `closed` defaults to the latest blame date of the record's lines.
5. A non-record section may be dropped with the user's approval; a record may not; a possible record needs an explicit decision.
6. The skill lives in codescout-companion, next to `tracker-hygiene` and `reconnaissance`.
7. Trackers that an installed skill or hook writes wait for that skill's change; the list lives in the skill.
8. Bare tokens a source defines are rewritten to `[[<key>:TOKEN]]` inside the records, except in code, link text and URLs; check re-applies the same function.
9. The collection reuses the tracker's key by default, so outside `[[…]]` citations need no rewrite.
10. Only structural findings block; time-based ones are review material.
11. A finished stream and the legacy archive files are written straight into `<lifecycle>/archive/<key>/`.
12. Several sources may fold into one collection; a token defined in two sources is refused; a repeated token in one source nests when it repeats its enclosing record's token, else is refused.
13. Every index row is carried verbatim, exempt from the rewrite; row-only records get a synthesised heading.
14. A code mention blocks only when the user approves it as a `reader`.
15. `files`, `sections` and `whole` are source kinds; table-row folders and folders with non-markdown files are manual.
16. Path mentions are lint: not rewritten, not blocking (review finding C1, option a).
17. The partition check is the M1 proof for every source kind.
18. A past `revisit_by` is carried unchanged.
19. `created` is the minimum of stated dates and the earliest blame date.
20. The catalog-facts precondition stays; the text inventory is added beside it, not instead of it.
