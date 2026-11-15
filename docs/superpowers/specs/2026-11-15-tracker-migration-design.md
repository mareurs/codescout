---
id: '4c4ab2c12c52d73e'
kind: spec
status: active
title: 'Tracker migration (Plan L-E): an agent-run, user-approved move of flat trackers into lifecycle collections (design)'
---

# Tracker migration (Plan L-E): an agent-run, user-approved move of flat trackers into lifecycle collections (design)

**Status:** approved by the operator on 2026-11-15 ("it's ok, go"); the builder check's seven questions were settled as calls 8–15 on the same day. Brainstormed with the operator in a claude-plugins session (it built the session-start hot-view hook, `../claude-plugins` commits `f07a421`, `a39db8d`, `8c9ece2`). Parent spec: `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md` (§ Migration, § Repo scope, § Consumers that read trackers today). This design **amends** that spec in one place (§ Amendment to the parent spec). Owner of the lifecycle line: memory `lifecycle-session-state`; this design was written outside that line and must be announced to it before a plan is written.

## Intent

Every flat tracker under `docs/trackers/` must become a lifecycle collection (parent § Migration step 4) without losing a record, in every repo that runs codescout. The operator decided in the brainstorm:

1. **Scope:** the full migration, for every repo, not a pilot of two or three trackers.
2. **Who decides:** an agent, guided by a skill, proposes how each tracker maps; the user approves; scripts do the mechanical work. The migration is not fully deterministic, because the per-tracker choices are judgment.
3. **What the user approves:** one mapping per tracker (folder, key, schema, the status-word table with record counts, `hot_cap`), then the verification report and the diff. Not each record.
4. **Who writes the files:** a codescout command writes the collection from the approved mapping. Record text never passes through the model.

## Measurements (2026-11-15, `integration/identity-lifecycle-20261114`)

| Fact | Number |
|---|---|
| Flat trackers in codescout | 81 files, 83,510 lines |
| Of those, with `PREFIX-N` definition headings | 55; the other 26 have none (handoffs, snapshots, policies, worklists, writer outputs) |
| Trackers that keep status in `**Status:**` body lines | 47 |
| Trackers that keep status in an index-table column | 35 (column position differs per tracker) |
| Status vocabulary | free text: `bug-fix-session-log` uses `fixed-verified` 119, `open` 59, `mitigated` 32, …; `observer-blindness` uses sentences ("wrong instrument — three claims survived …") |
| Legacy top-level `docs/trackers/archive/` | 66 flat files |
| Existing tracker folders | `bistriceanu/`, `ecc-integration/`, `effort-steering/`, `frictions/`, `issue-clusters/`, `mechanisms/`, `temp/` |
| Flat trackers in `../claude-plugins` | 27 |
| Collections that exist in either repo | 0 |

The free-text status is why a fixed rule set cannot map records: a record's status class decides whether its file goes into `archive/` or stays live, and a wrong guess moves it silently.

## Goals

- **M1.** Every record of a migrated tracker is carried into exactly one entry file, with its `PREFIX-N` heading kept (parent § Legacy tokens). Its body is byte for byte the source section after one stated, pure transform: the citation rewrite of § Citations inside records. Nothing is shortened unless the approved mapping says so.
- **M2.** The user approves each tracker's mapping before anything is written, and the verification report after.
- **M3.** A deterministic check proves M1 and the lifecycle rules after every tracker; a migration whose check is not clean is not committed.
- **M4.** The same skill and commands work in any repo that runs codescout.
- **M5.** Each new collection declares `hot_cap` and `stale_after`, or the mapping records why it has none (claude-plugins session log F-2), so the session-start hot view has data.
- **M6.** One tracker at a time (strangler). The repo stays usable between trackers; a flat tracker keeps working in legacy mode until it moves.

## Non-goals

- Switching code consumers (parent § Consumers: `operator_rules`, `pre-commit-ledger-counts.py`, `scripts/measure/*`, the tc-suites pins, `legibility_scan`, `audit_doc_refs`). Each is its own plan. The migration refuses a tracker that code still reads (§ Preconditions).
- Running the codescout migration itself. L-E builds the tools; running them over codescout's 81 trackers is the follow-up work the tools make possible.
- The doctor wiring of `lifecycle_findings`, `doc(move)` D3 rules, and the removal of `append_entry`.
- The `get_guide` `tracker-conventions` rewrite (parent § `get_guide`), and the reconnaissance and tracker-hygiene skill changes (parent § Hygiene skill, § Reconnaissance skill).

## Anti-goals

The design fails, even if it works, if:

- A record is lost, or its body text changes, and no check reports it.
- The model retypes record text.
- A collection's lifecycle or schema is declared in two places that persist (the mapping file is input, never committed).
- An open record lands in `archive/`, or a terminal one stays live, because a status word was guessed.
- A migrated open record looks fresher than it is (parent anti-goal: a bulk change must not make stale records look fresh).
- The migration writes a commit, or pushes.

## Approaches

Considered for the overall shape:

1. **A server command with a committed manifest** (the parent spec's "one server command"). Rejected by the operator: the per-tracker choices are judgment, so a manifest is only a slower way for a person to do what an agent can propose.
2. **Agent-run with user approval, scripts for the mechanics (chosen).**
3. **Fully automatic rules.** Rejected: the status vocabulary is free text (§ Measurements).

Considered for who writes the entry files, within approach 2:

1. **A codescout command writes from the approved mapping (chosen).** Record text is copied by code; the agent's job is the mapping.
2. **The agent calls `doc(create)` per record.** Rejected: `bug-fix-session-log` alone has 398 records, and every body would pass through the model, which can change a word with no error.
3. **A plugin script writes the files.** Rejected: it would copy the Rust frontmatter and schema rules into another language; the two copies drift, and the checks then reject what the script writes.

## Design

### Parts and where they live

| Part | Repo | Reuses |
|---|---|---|
| `inventory` command | codescout | link_scan definition-heading shape; `cutover/code_refs.rs` (`CodeRefs`) for code readers of a path |
| `apply` command | codescout | `lifecycle::collection::collection_readme`, `lifecycle::write::new_entry` (and the `today` parameter), `archive_rel_path`, `identity::grammar::extract_spanned`, `util::write_catalogued_file` |
| `check` command | codescout | `lifecycle::collection::load_collection`, `lifecycle::checks` (`lifecycle_findings`), `lifecycle::hot_view` |
| Skill `migrating-trackers` | claude-plugins (`codescout-companion/skills/`) | the three commands, `doc`, git |

The commands are CLI subcommands (like `cutover` and `hot-view`), not MCP tools: they are run by the agent through `run_command`, they add nothing to the advertised tool surface (`TOOL_SURFACE_CHAR_BUDGET`), and the operator can run them by hand. The names are the builder's (§ Left to the builder); this document calls them `inventory`, `apply` and `check`.

### The per-tracker loop (the skill)

1. **Pick.** The skill lists flat trackers with their record counts and code readers, and proposes an order: trackers no code reads, smallest first. The user may reorder.
2. **Inventory.** `inventory <tracker>` prints, as JSON and as a short text report:
   - every record: its token, heading level, title, line range, its raw status value and where it came from (`**Status:**` line, index column by header name, or none), and any date the record states;
   - the distinct raw status values with counts;
   - the non-record sections (preamble, index tables, template, status vocabulary, free prose);
   - every citation of the tracker in the repo, by form;
   - every code file that names the tracker's path (from `CodeRefs`).
3. **Propose.** The agent drafts a mapping file (§ The mapping file) and shows the user one screen: target folder and key, the schema, the status-word table (raw value → declared status → class, with counts), any per-record overrides, which non-record sections go to the readme and which are dropped, `hot_cap` and `stale_after`, and anything the agent is unsure about.
4. **Approve.** The user approves or edits. Nothing is written before this.
5. **Apply.** `apply <mapping>` writes the collection, deletes the flat file and rewrites citations (§ What apply writes). It refuses rather than guesses.
6. **Check.** `check <key> --source <tracker path>` (§ What check proves). Not clean → the skill stops, shows the findings, and the user decides: fix the mapping and re-apply, or restore with `git checkout` / `git clean` of the named paths.
7. **Review.** The skill shows the check report and `git diff --stat`, plus the readme and two sample entries. The user approves.
8. **Commit.** By pathspec, on the user's word (shared-checkout rule, `docs/conventions/shared-checkout-commit-sequence.md`). The skill never pushes.

Non-record trackers (the 26 with no tokens) go through the same loop with a different proposal: move out of `docs/trackers/` (a snapshot becomes a report, a policy goes to the repo's conventions, by `doc(move)`), become a collection whose entries are its `##` sections (`key_style: name`), or become one entry of an existing collection (a handoff into a `temp/` stream). The legacy `docs/trackers/archive/` files and the existing folders (`frictions/` with its 16 `FR-N` defined in its index, `issue-clusters/`, …) go through the loop too (parent § Migration steps 5 and 6).

### The mapping file

A TOML file the agent writes outside the tracked tree (the skill picks the path; the builder may fix a default under `.codescout/`). It is input to one `apply` run and is never committed. Content:

- `sources`: one or more repo-relative paths that fold into this collection (a live tracker plus its split archive files, such as `reconnaissance-patterns.md` and `archive/reconnaissance-patterns-archived-entries.md`). Each source has a `source_kind`: `file` (records are definition-heading sections) or `files` (a folder whose markdown files are one record each, such as `issue-clusters/`). A token defined in two sources is a refusal.
- `target`: lifecycle folder, collection key, and `archived = true|false`. The key defaults to the stem of the first source, so `observer-blindness.md` becomes `ledgers/observer-blindness/` (parent D1). `archived = true` writes the collection straight into `<lifecycle>/archive/<key>/`: for a stream whose records are all terminal, and for the legacy top-level archive files (parent § Migration step 5).
- `schema`: every parent § Collection schema key the collection declares, plus `hot_cap` and `stale_after` (`lifecycle::schema`, `KEY_HOT_CAP`, `KEY_STALE_AFTER`) or `hot_view_none = "<reason>"`.
- `readme_body`: the prose the readme carries (purpose, admission rules, standing instructions; the `## Entry template` section). The agent drafts it from the tracker's preamble; the user approves it.
- `entry_kind`: one kind, or a rule by token prefix (`F` → `friction`, `W` → `win`).
- `status_source`: `body_label`, `index_column = "<header>"`, or `constant = "<status>"`.
- `status_map`: raw value → declared status. **It must cover every distinct raw value the inventory reported**; apply refuses an unmapped value and names it.
- `columns`: index-table header → declared field name, or `created`, or `title_alias`. Optional (§ Index rows).
- `overrides`: token → status (or `created`, `closed`, `revisit_by`) for single records.
- `sections`: each non-record section by heading → `readme` or `drop`. Dropping is allowed only for non-record sections, and each drop is listed to the user. A record can never be dropped (M1). An index table whose rows are carried (§ Index rows) is dropped as a table, not as data.
- `code_mentions`: each code file the inventory found naming a source path, classified `reader`, `comment` or `fixture` by the agent and approved by the user (§ Preconditions).
- `shorten`: empty by default. A record listed here is carried with the replacement body the mapping gives; the check then reports it as shortened, not as equal.

### What apply writes

- The readme from `collection_readme` (re-read through `schema::parse`).
- One entry per record, rendered by the same rules as `new_entry`: frontmatter from the schema and the record's mapped status, body = the record's section text unchanged, with its definition heading as the first heading. Terminal entries go to the collection's `archive/` (`archive_rel_path`); others stay live.
- Dates (§ Dates).
- The citation rewrite of § Citations inside records: inside the carried bodies and the readme always, and across every other markdown file of the repo when the key changes. The identity session agreed its cutover emits `[[entry-key]]` for tokens that become entries (memory `tracker-lifecycle-parked-items`); the builder confirms which forms exist at build time.
- The `**Index row:**` line of § Index rows.
- The deletion of every source file (parent call 1: a kept file would be a second definer of its key and tokens).
- Every write through `write_catalogued_file`. No commit, no push.
- **Idempotence by content:** a re-run over the same mapping writes nothing when every target file already holds the text it would write, and reports `already applied`. A target file that exists with other text is a refusal naming it.

### Dates

The parent spec forbids staleness from git dates, but a migrated record needs `created`, `status_since` and (if terminal) `closed`, and the flat trackers hold no such fields.

- `created`: a date the record states (heading, index row, `**Date:**`, a dated `**Valid:**`), else the earliest `git blame` date of the record's lines. A later bulk sweep cannot make this date younger.
- `status_since`: a date the record states for its status, else `created`. This errs toward **stale**: an old open record surfaces in the hot view and `revisit_due` / `stale_after` instead of hiding. (Migration day would make every open record look fresh, which is an anti-goal.)
- `closed` (terminal only): a date the record states, else the latest `git blame` date of the record's lines. It only orders archived history, so an inflated date costs little.
- `revisit_by` (blocked or deferred classes): from the record, else from `overrides`; apply refuses a blocked or deferred record without one. `new_entry`'s past-date refusal (`RevisitPast`) is relative to its `today` argument; apply must pass the real date for that comparison, so a past `revisit_by` is refused and the user sets a new one.
- Every date the command derives, rather than reads, is listed in the check report.

### Citations inside records

A bare token such as `R-34` resolves only inside its own document (`identity/grammar.rs`, `identity/index.rs` `resolve`, the `Bare` arm). Once a tracker is split, each record is its own document, so a bare `R-34` in record `R-91` would stop resolving. Measured roughly: about 791 such mentions in `reconnaissance-patterns`, 897 in `bug-fix-session-log`, 179 in `observer-blindness`.

- `apply` rewrites, in every carried record body and in the readme body, each bare token **that one of the sources defines** to `[[<key>:TOKEN]]`. A bare token no source defines is left alone: it did not resolve before either.
- When the key changes, `[[old-key:TOKEN]]` and `[[old-key]]` become `[[<key>:TOKEN]]` and `[[<key>]]` inside the bodies and across every other markdown file of the repo. When the key is reused (the default), outside citations stay as they are and resolve by v5 ownership.
- The rewrite is one pure function, `rewrite_record_citations(text, defined_tokens, old_key, new_key) -> String`, built on `identity::grammar::extract_spanned` and the bare-token scanner. It skips code spans and fences as the grammar already does.
- `check` applies the same function to each source section at `HEAD` and compares the result with the entry body byte for byte. So M1 stays provable, and the only difference allowed is the one the function produces.

### Index rows

Some index tables carry data the body lacks (in `bug-fix-session-log`: Date, Severity, Category, a summary title; in its Wins index: Impact, Pattern, Counterfactual).

- Each column the mapping maps (`columns`) becomes that frontmatter value of the entry: a declared field, `created`, or `title_alias` (kept as a declared field when the summary differs from the heading).
- Every record with an index row also gets the original row carried verbatim, as the last body line `**Index row:** <the row>`. So no index data is lost, even for columns nobody mapped. `check` expects that line exactly.
- A record with a heading and no index row is carried without the line and is listed in the check report. An index row with no heading is a record too: it becomes an entry whose body is only the row line, and it is listed.
- When the status comes from one place but the other place disagrees (the body says `validated`, the index says `fixed-verified`: about 91 of 201 F-records in `bug-fix-session-log` by a rough comparison), the inventory lists each disagreement, and the proposal shows them to the user. The mapping's `status_source` decides; `overrides` fixes single records.

### Folders and finished streams

- A folder with one markdown file per record (`issue-clusters/`) uses `source_kind = "files"`: each file becomes an entry, its body unchanged after the citation rewrite, its own frontmatter replaced by the entry frontmatter (the old frontmatter keys go to `columns`-style mapping or are listed as dropped).
- A folder whose records are table rows with separate evidence files (`frictions/`, 16 `FR-N` in `index.md`) and a folder with non-markdown files (`effort-steering/migration-manifest.json`) are outside the commands. The skill migrates them by hand with `doc(create)` and `doc(move)`, after the user approves, and runs `check` for the structural part (items 3 to 5). The skill lists each such folder as manual.
- A stream whose records are all terminal is written with `archived = true`. Otherwise `stream_all_terminal` would fire on it the day it is written.

### What check proves

`check --mapping <file>` (or `--key <key> --source <path>…` for a run without the mapping) loads the collection from its directory with `load_collection_dir`, so an archived collection loads too, and compares it with the sources **at `HEAD`** (apply deleted them from the tree). It reports:

1. every source token is the first heading of exactly one entry, and no entry has a token the sources lack;
2. each entry body equals `rewrite_record_citations` of its source section, plus the `**Index row:**` line where one exists, byte for byte, except the records listed under `shorten`;
3. every status is declared, every terminal entry is in `archive/` (or the collection is archived) and every other one is live;
4. the collection loads and these findings are empty for it: `entry_invalid`, `collection_invalid`, `orphan_entry`;
5. the hot view renders (exit 0) when `hot_cap` is declared and the collection is live, so a collection with problems is not shown as `0 hot` (claude-plugins session log F-6);
6. when the key changed, no `[[old-key…]]` citation is left in the repo; and no source path is left in a markdown file;
7. for review, not blocking: the time-based findings (`revisit_due`, `cited_target_closed`, `ledger_quiet`, `stream_all_terminal`, `terminal_in_live_dir`), the derived dates, the dropped non-record sections, records without an index row, and status disagreements.

Exit 0 only when 1–6 hold. The skill treats any other exit as "not clean" and shows item 7 to the user every time. The time-based checks are working as designed when they fire on old records; they are review material, not migration errors.

### Preconditions (apply refuses, naming the cause)

- The repo's identity cutover is done (parent § Migration step 1). The builder picks the test; the cutover's own "a plan of `HEAD` writes nothing" is the first candidate.
- Every code file that names a source path (`CodeRefs`) is listed in `code_mentions`. A file classified `reader` is a refusal until that consumer's own plan has switched it (then the file no longer names the path). A `comment` or `fixture` mention does not block: the skill shows it to the user, who may have the agent edit it in the same commit. A rough count found 38 of codescout's flat trackers named under `src/`, `scripts/` or `tests/`, mostly in comments and fixtures.
- The tracker is not the hygiene log while the session-start banner in `../claude-plugins` still reads its frontmatter cadence (`next-sweep-due`, `sweep-interval-days`); the parent spec moves those fields to the hygiene collection's readme, and the banner must follow first.
- The target key is free in the repo, except for the stem of a source this run deletes (key reuse is the default, parent D1).
- The working tree has no uncommitted change under `docs/trackers/` (so a failed run can be restored with git).

### Repo scope

The skill and commands assume nothing about codescout's own trackers. A repo with no `PREFIX-N` ledgers (most of the ~35 on this machine) uses the non-record proposals only. Legacy mode is unchanged: a flat tracker keeps working with `append_entry` until its turn.

## Amendment to the parent spec

Parent G8 ("every repo can migrate with one server command") and § Repo scope ("a migration command converts one repo") become: **every repo migrates with one skill that drives three CLI commands, one tracker at a time, with the user's approval per tracker.** § Migration step 4 is unchanged in content (every record carried, citations rewritten, consumers switched, file deleted); it is now executed per tracker by the skill. The non-goal "deciding, per existing tracker, which content is worth carrying" stays: the decision is the user's, made in the loop. The parent's status line and G8 must be edited when this design is approved.

## What is left to the builder

- The command names and flags (`codescout trackers inventory|apply|check` is one shape; `migrate` is close to the existing `migrate-memories`).
- The mapping file's exact TOML layout and default location.
- How a raw status value is normalised before lookup (trim, case, the text before ` — `), as long as the inventory reports the same normalised values the map must cover.
- The `git blame` options for § Dates (`-M`, `-C`, `-w`), and whether blame reads the file at `HEAD`.
- The JSON shape of the inventory and check reports.
- Test design. Minimum: one test per anti-goal (a lost record fails the check; a changed body byte fails it; an unmapped status value is refused; a terminal record never lands live; a migrated open record keeps a historical `status_since`; apply leaves no commit).

## Calls I made (review these)

1. The commands are CLI subcommands, not MCP tools: no tool-surface cost, and the operator can run them by hand.
2. The mapping file is never committed; the readme is the only lasting declaration.
3. `status_since` defaults to `created`, not to migration day, so the migration errs toward showing records as stale.
4. `closed` defaults to the latest blame date of the record's lines.
5. A non-record section may be dropped with the user's approval; a record may not.
6. The skill lives in codescout-companion, next to `tracker-hygiene` and `reconnaissance`.
7. The hygiene log waits for the claude-plugins banner to read the readme.

After the builder check (2026-11-15):

8. Bare tokens a source defines are rewritten to `[[<key>:TOKEN]]` inside the records; "byte for byte" means after that one pure rewrite, which `check` re-applies to the source.
9. The collection reuses the tracker's key by default, so outside citations need no rewrite.
10. Only the structural findings (`entry_invalid`, `collection_invalid`, `orphan_entry`) block the check; the time-based ones are shown for review.
11. A finished stream and the legacy archive files are written straight into `<lifecycle>/archive/<key>/`; `check` loads by directory.
12. Several sources may fold into one collection; a token defined twice is a refusal.
13. Every index row is carried verbatim as the entry's last line `**Index row:** …`; mapped columns also become frontmatter values.
14. A code mention blocks only when the user approves it as a `reader`; comments and fixtures are shown, not blocking.
15. Folders of one file per record are a second source kind; table-row folders (`frictions/`) and folders with non-markdown files are migrated by hand through the skill.
