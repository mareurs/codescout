---
kind: research
status: draft
title: Plan L-E design review (tracker migration)
tags:
- tracker-lifecycle
- plan-l-e
- review
topic: tracker-lifecycle
---

> Independent Opus review (reconnaissance + architecture-snow-lion, read-only) of `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`, requested by the lifecycle line on 2026-11-15. The two Critical findings were re-verified by the lifecycle session: 57 tracked markdown files name `observer-blindness.md` (53 in inline code); `cutover/rewrite.rs:572` says "A citation only inline code holds: counted, never edited"; `## R-56b` and `## R-59b` exist in `archive/reconnaissance-patterns-archived-entries.md` (lines 332, 346); `prompt-hamsa-audit-log.md` holds 10-11 `A-38 —` headings. Findings I1-I9 and M-1..M-11 are the reviewer's, checked against code by the reviewer, not re-checked one by one.

# Review: Plan L-E tracker migration design (docs/superpowers/specs/2026-11-15-tracker-migration-design.md)

Reviewer: independent subagent (Opus), 2026-11-15. Skills loaded: `codescout-companion:reconnaissance`, `buddy:architecture-snow-lion`. Read on the main checkout, `experiments` HEAD `02a19eca8` (working tree, which holds uncommitted changes of other sessions). No cargo, gate or tests were run. No file of the repo was edited.

## Verdict

**Not ready for a plan.** Two Critical gaps: the blocking check (item 6) cannot pass for the heavily cited trackers, and M1 (no record lost, no body changed) is proven by the same splitter that defines the records, so the splitter's misses cannot be seen. Nine Important gaps follow.

Counts: Critical 2, Important 9, Minor 11.

---

## Critical

### C1. Check item 6 ("no source path is left in a markdown file") cannot pass for the trackers that matter, and apply has no step that removes path mentions

- **What.** § What check proves, item 6: "no source path is left in a markdown file". Exit 0 needs items 1–6 (design L189). M3: a migration whose check is not clean is not committed. But § What apply writes rewrites only tokens and `[[old-key…]]` forms. Nothing rewrites a path mention.
- **Evidence.**
  - `git grep -lF "observer-blindness.md" -- '*.md' | wc -l` → **57**. `bug-fix-session-log.md` → **119**.
  - `` git grep -lE "`[^`]*observer-blindness\.md[^`]*`" -- '*.md' | wc -l `` → **53**. These are inline-code mentions: 27 in `docs/issues`, 13 in `docs/trackers`, plus `CLAUDE.md`, `docs/TAXONOMY.md`, `docs/RELEASE.md`, `docs/PROBES.md` and others.
  - The identity cutover's rewrite does not remove them. `src/librarian/cutover/rewrite.rs:572-573`: "A citation only inline code holds: counted, never edited".
  - It also never edits frozen files (`cutover/code_refs.rs:3-5`). That covers dot directories (`frozen()`, code_refs.rs:93-101), so `.codescout/memories/*.md` keeps its tracker paths.
- **Failing scenario.** The operator runs the loop on `observer-blindness`. Apply writes the collection and deletes the flat file. Check item 6 finds 53+ files that still name `docs/trackers/observer-blindness.md` (`CLAUDE.md` among them), so the exit is not 0. The skill stops. Neither branch works. "Fix the mapping and re-apply" does nothing, because the mapping cannot change those files. "Restore" undoes the migration. Every tracker that is cited by path in prose is blocked in the same way, and that is most of the large ones.
- **Recommended fix.** Decide what a path mention after migration is:
  - (a) Lint, consistent with v5 ("paths and 16-hex ids are lint, never edges"). Item 6 then becomes review material (item 7) with a count.
  - (b) A rewrite target. Apply then rewrites path forms to `[[key]]` across the repo and refuses frozen files. That touches history files such as `docs/issues/archive`, which may not be wanted.
  - Either way, state the scope of the item-6 scan (dot directories, other repos), and say how a frozen file such as `CLAUDE.md` is handled.

### C2. M1 is proven by the same splitter that defines a record, so records the splitter misses are dropped or duplicated with a clean check

- **What.** A record is defined by `def_re` (`link_scan/extract.rs:319-321`: `^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+`). Its extent comes from `entry_sections` (extract.rs:67-118): it ends at the next heading of the same or higher level, so nested sections overlap (doc comment, extract.rs:121-125).
  - Check items 1–2 are token-driven ("every source token is the first heading of exactly one entry"; "each entry body equals … its source section").
  - Anything the splitter does not call a record is a "non-record section". § The mapping file then lets the user send it only to `readme` or `drop`.
  - Check therefore cannot express "a record was not recognised". The proof is common-mode with the transform.
- **Evidence.**
  1. Suffixed and colon-less record headings, which are not definitions under `def_re`:
     - `docs/trackers/archive/reconnaissance-patterns-archived-entries.md:332` `## R-56b — Hit: …`
     - the same file, line 346: `## R-59b — Miss ×3: …`
     - `archive/usage-analysis-improvements-session-log.md:414` `## F-3a (deferral) — …`
     - `system-retrospective-improvements.md:298,355` `### T-15a`, `### T-15b`
     - `2026-08-15-tool-usage-investigation.md:309,328` `### TU-11a`, `### TU-11b`
     - Command: grep of `^#{1,6}\s+(\*\*)?[A-Z]{1,4}-[0-9]+[a-z]?…` over `docs/trackers/*.md` and `archive/*.md`.
     - The design's own multi-source example is exactly `reconnaissance-patterns.md` plus `archive/reconnaissance-patterns-archived-entries.md` (design § The mapping file, `sources`).
  2. Tokens repeated inside one source. `prompt-hamsa-audit-log.md` holds `## A-38 — …` (L2114) and nine `### A-38 — <subsection>` headings (L2132-2405). All ten match `def_re`.
     - An awk scan (fence-aware, approximate) counts **5 tokens repeated over 38 headings** in that file, and 1 repeated token in `archive/codescout-lessons-2026-05-20-session-log.md`.
     - `identity/index.rs:37-40` `defined_tokens` says "a repeated id repeats".
     - The design refuses a token defined in two *sources*. It says nothing about a token defined twice in *one* source.
- **Failing scenarios.**
  - (i) Folding `reconnaissance-patterns`: R-56b and R-59b appear as non-record sections. The user can choose only `readme` (a record in the readme, which D1 forbids) or `drop`. On `drop`, check items 1–6 pass and two records are gone. That is anti-goal 1 ("a record is lost and no check reports it").
  - (ii) `prompt-hamsa-audit-log` under today's `entry_sections`:
    - `A-38` yields ten sections, and the `##` one contains all nine `###` ones, so apply writes the subsection text twice.
    - Item 1 ("exactly one entry") fails with no remedy the mapping offers.
- **Recommended fix.**
  - Make check prove a **partition** that does not depend on the splitter: the concatenation of every carried record section and every non-record section (readme or drop) equals the source file, line for line.
  - Flag any non-record section whose heading holds a token-shaped string (`[A-Z]{1,4}-\d+[a-z]?`) as a "possible record". It blocks until the mapping names it a record (with a declared token, e.g. `R-56b` → entry) or confirms the drop.
  - Define repeated tokens. Rule: a definition heading whose token equals its enclosing record's token is a subsection of that record. Otherwise refuse and name both lines.
  - See also § Answers to the author's three extra items, (2).

---

## Important

### I1. `new_entry` cannot be reused for migrated records, and nothing validates derived dates

- **Evidence.** `lifecycle/write.rs:429-516` `new_entry` uses one `today` for:
  - `status_since` and `created` (L476-477);
  - `closed` for a terminal record (L478-480);
  - the dated stem (`entry_stem(schema.key_style, today, s)`, L455);
  - the `RevisitPast` comparison (L451-452).
- **The contradiction.** § Dates needs historical `created`, `status_since` and `closed`. It also needs `RevisitPast` against the real date (design L141). One parameter cannot be both.
- **Nothing checks the derived dates.** `entry::validate_entry` (entry.rs:264-297) has no date-order rule. `closed < created`, `status_since < created` and a future `status_since` all pass, so a wrong derivation is silent in check items 3–4.
- **`created` can be too young.** § Dates takes `created` from "a date the record states (… a dated `**Valid:**`)". A `**Valid:** dated` line is re-stamped on re-verification, so `created` can come out younger than the record. That breaks the stated "A later bulk sweep cannot make this date younger". The precedence among stated dates is also unspecified.
- **Failing scenario.** A record re-validated in November with `**Valid:** dated 2026-11-10` gets `created` = `status_since` = 2026-11-10. It is not stale in the hot view although it has been open since August. That is the anti-goal "a migrated open record looks fresher than it is".
- **Fix.**
  - A renderer that takes explicit `EntryDates {created, status_since, closed}`, plus a separate `now` for revisit rules, and builds the stem from `created`.
  - `created` = the minimum of every stated date and the earliest blame date.
  - Apply refuses `status_since < created`, `closed < created` and any date after `now`, and check re-verifies those rules.

### I2. Refusing a past `revisit_by` makes migrated blocked or deferred records look in good standing

- **Evidence.**
  - Design L141: "a past `revisit_by` is refused and the user sets a new one".
  - The hot view would show such a record honestly as "revisit due": `hot_view.rs:142-152` with `entry::revisit_due` (entry.rs:241-250, strictly before today).
  - `entry_invalid` does not fire on a past `revisit_by`, only on a missing one (entry.rs:283-285).
- **Failing scenario.** A record parked in September with "revisit by 2026-10-01" is migrated in December. Apply refuses. The user types a date in January, and the record disappears from the hot view until January. The truthful state was "overdue since October".
- **Fix.** For migration, carry the stated past `revisit_by` unchanged. `RevisitPast` is a rule for new writes. Keep refusing only a missing date.

### I3. The precondition list and `CodeRefs` miss real readers and writers of tracker paths

- **JSON is skipped, so the tc-suites pins are missed.** `cutover/code_refs.rs:137` `DATA_EXTENSIONS = ["jsonl","txt","json","csv","log"]`, and `is_code_evidence` (L148-154) skips them. `scripts/tc-suites/artifact-entries.json` names 10 tracker paths: `bug-fix-session-log`, `observer-blindness`, `windows-platform-support`, `structural-debt-refactor`, `reconnaissance-patterns`, `skill-frictions`, `tracker-hygiene-log`, `open-issue-work-queue`, `issue-clusters/IC-16-…`, `prompt-surface-measurement-session-log` (grep output). The design's own Non-goals name "the tc-suites pins" as a consumer that must block.
- **An escaped path is missed.** `.pre-commit-config.yaml:170` holds `files: ^(docs/trackers/issue-clusters\.md|…)`. `note_markdown_paths` (code_refs.rs:174-206) needs path bytes immediately before `.md`. Here a `\` precedes it, and `is_path_byte` (L169-171) excludes `\`, so the mention is not indexed.
- **Writers in other repos are not gated.** `claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md:74,84` tells agents to `cp` the template to `docs/trackers/<topic>-session-log.md` and `append_entry` there. `references/patterns-tracker.md:14,24` writes `docs/trackers/reconnaissance-patterns.md`. `tracker-hygiene/SKILL.md:153,338,457` writes `tracker-hygiene-log.md`. The design gates only the hygiene *banner* (`session-start.mjs:235`, which builds the path with `join(cwd,'docs','trackers','tracker-hygiene-log.md')`, a form a literal scan would also miss).
- **Failing scenario.** After `bug-fix-session-log` (or any `*-session-log`) is migrated, the next reconnaissance run recreates `docs/trackers/bug-fix-session-log.md` from the template. That file is a second definer of the collection's key, the case the parent's call 1 deletes files to avoid. Separately, migrating `observer-blindness` passes the preconditions and the tc-suite `expect_path` breaks.
- **Fix.** Precondition: no skill or hook of the installed plugins writes the tracker's path or path pattern. Concretely, every `*-session-log` and `reconnaissance-patterns` waits for the parent § Reconnaissance skill change. Also scan JSON (as a reader class) and escaped-regex forms, or have the inventory run a plain `git grep -F <stem>` beside `CodeRefs` and show the difference.

### I4. Apply has no lock, its re-run contradicts the clean-tree precondition, and a restore can destroy a peer's edits

- **The re-run is refused.** Precondition: "The working tree has no uncommitted change under `docs/trackers/`" (design L200). § What apply writes promises "a re-run over the same mapping writes nothing … and reports `already applied`" (L131). After any partial or complete apply, `docs/trackers/` holds uncommitted changes, so the re-run is refused before the idempotence check is reached. Re-deriving also needs the sources from `HEAD`, because they are deleted from the tree. This is not stated.
- **A restore can wipe a peer's work.** When the key changes, apply rewrites "every other markdown file of the repo" (L127). Those files are outside the clean-tree precondition. On this shared checkout `git status` shows many modified `docs/` files of other sessions. The design's restore, `git checkout` of the named paths (L102), discards a peer's uncommitted edits in those files.
- **No write lock.** The precedent `cutover export` takes the project write lock (`src/cli/cutover.rs:24` `use crate::cli::write_lock::{lock_root_for, take_at}`; module doc L7-9). The design takes none. A peer's `append_entry` to the flat tracker between inventory and the delete is lost. A peer write after the delete fails or recreates the file.
- **No path for late records.** A worktree branch that appended `F-241` to the flat file merges with a modify/delete conflict, and the design has no path for a record that arrives after migration.
- **Fix.**
  - Take the project write lock for apply.
  - Let the precondition accept exactly the paths a previous run of the same mapping wrote.
  - Refuse when any file apply would rewrite outside `docs/trackers/` is dirty.
  - Add a "late record" procedure: re-run inventory on the merged flat file, and append only tokens the collection lacks.

### I5. The 26 non-record trackers have no mechanism and no M1 proof

- **Evidence.** § Folders and finished streams and the § Non-record paragraph (L107) propose three outcomes: move out, "a collection whose entries are its `##` sections (`key_style: name`)", or "one entry of an existing collection". But:
  - `source_kind` has only `file` (records are *definition-heading* sections) and `files` (L114).
  - `target` is always a collection to create, written under its key (L115).
  - Check items 1–2 are driven by tokens, so for a source with zero tokens they hold vacuously.
- **Failing scenario.** `sdd-ruling-log.md` is converted into `##` entries. No command can split on non-token headings. If done by hand, check items 1–2 pass trivially, and a dropped `##` section is never noticed.
- **Fix.** Add `source_kind = "sections"` with a heading level. Make the partition check from C2 the M1 proof for every kind. Add a `target.existing = <key>` mode for "one entry of an existing collection", or declare those two outcomes manual, as `frictions/` is.

### I6. Sidecars, `id:` and the catalog-only facts are not mentioned

- **Evidence.**
  - `ls docs/augmentations` lists **24** sidecars named for flat trackers (`docs-trackers-reconnaissance-patterns.yaml`, `…-tracker-hygiene-log.yaml`, …).
  - The one read holds the standing instructions ("To add an entry — ONE call: `doc(action="append_entry", id="5696563f06b2c222", …)`").
  - Parent call 9: "legacy augmentations live in their sidecars until their collection migrates", so migration is the moment to decide.
  - The inventory reads only the tracker file, so `readme_body` is drafted without the sidecar's text.
  - `collection_readme` writes no `id:` (collection.rs:247-249).
  - The identity cutover's purpose is catalog-only facts: events, observations, manual links, augmentation shapes and params (`cutover/mod.rs:4-6`). The author's replacement precondition (extra item 3) tests text only.
- **Failing scenario.** `reconnaissance-patterns` is migrated. Its sidecar stays, names a deleted path, and still tells agents to `append_entry` on id `5696563f06b2c222`. The standing instructions never reach the readme.
- **Fix.**
  - The inventory reports the sidecar, and its prompt is a `readme_body` input.
  - Apply deletes or retires the sidecar in the same run, and check item 6 includes it.
  - State that the cutover precondition is about catalog-only facts, and that a text test cannot replace it (§ Answers, (3)).

### I7. Status and index-row cases with no rule

- **A record with no status.** The inventory reports the source as "none" (L84), but `status_map` maps only raw values (L120).
  - Count in `bug-fix-session-log`: 399 definition headings against 344 `**Status:**` lines (`grep -oE '^\*\*Status:\*\* *[a-z-]+' | uniq -c`: validated 144, fixed-verified 99, open 48, mitigated 28, promoted-to-bug-tracker 18, …).
  - So records with no body status exist. The mapping cannot express their status except by per-record `overrides` or `constant`.
- **An index row with no heading.** "becomes an entry whose body is only the row line" (L157). There are 3 such tokens in `bug-fix-session-log` (`comm` of heading tokens against index-row tokens). The design gives no title, key, status or heading for them.
  - With no heading, the token is defined nowhere, so `[[bug-fix-session-log:F-n]]` stops resolving.
  - Check item 1 does not see it, because the token is not a "source token" (`defined_tokens`).
- **The `**Index row:**` line.** Is it "carried verbatim" (L155) or passed through `rewrite_record_citations` (L145-147)? Its bare tokens make the difference, and the check expectation depends on it.
- **Fix.**
  - A `status_none = <word>` rule that must be set when the inventory reports any "none".
  - Synthesise `## <TOKEN> — <summary cell>` for row-only records, and list them.
  - State that the row line is verbatim and is exempt from the rewrite, or that it is rewritten.

### I8. The amendment is incomplete: the parent still says "one migration command" in five places

- **Evidence** (parent `2026-11-11-tracker-lifecycle-design.md`).
  - § Repo scope (L409-414) still reads "**A migration command** converts one repo: it creates the collections, writes the entries, … It runs per repo … and is re-runnable."
  - § Tools: "**Migration command:** § Repo scope."
  - § Migration step 2: "… legacy mode, the migration command."
  - § `get_guide`: "… the doctor checks and the migration command."
  - § What is left to the builder: "The migration command's name and flags."
  - Only the status line and G8 were edited.
  - The design's own § Amendment says § Repo scope "become[s]" the skill form.
- **Failing scenario.** A planner reading the parent builds one repo-wide re-runnable command, or a guide author teaches "the migration command".
- **Fix.** Edit those five places, or add one sentence at § Repo scope pointing at the amendment.

### I9. Two design parts that the design treats as settled are left unspecified

- **Citations inside link text and URLs.** `extract_spanned` returns bare tokens inside ordinary link text (`[see F-1](b.md)`) and inside bare URLs. Its doc says "a rewrite of the link must drop it" and "a rewrite must test it with `in_url_word`" (grammar.rs:114-127, 196-202). The design says only "It skips code spans and fences as the grammar already does" (L146). Rewriting `[see F-1](b.md)` to `[see [[k:F-1]]](b.md)` nests a wiki link inside a link.
- **How the skill finds the binary.** It is not specified. The hook uses `resolveCodescoutBinary(cwd)` (`session-start.mjs:362-368`). On another repo or an old installed binary, `codescout trackers …` is a clap error: exit 2 with empty stdout, per the hot-view contract in memory `collection-hot-view-state`.
- **Fix.** Name both rules in the design: the link-text and URL exclusions, and binary resolution plus a version probe (`codescout trackers --help` exits 0).

---

## Minor

- **M-1.** § Parts says `lifecycle::checks` (`lifecycle_findings`). The function is `lifecycle::scan::lifecycle_findings` (scan.rs:228-234). It loads the whole project (`load_docs` over `git ls-files --cached --others`, scan.rs:105-112), not one collection, so check item 4 must filter by collection.
- **M-2.** § Parts names `load_collection`. § What check proves uses `load_collection_dir`. Both exist (collection.rs:99, 203). Only `_dir` loads an archived collection.
- **M-3.** `write_catalogued_file` (util.rs:131-141) is a guarded, atomic, single-file write: symlink, outside-repo and unwritable refusals, then `atomic_write`. It writes no catalog row and no audit row, it is not a delete, and it gives no multi-file atomicity. The design should state the catalog stance: rely on the watcher, reindex, or the derived catalog after the E-flip.
- **M-4.** Byte-for-byte needs a stated normalisation. `frontmatter::render_body` trims leading and trailing `\n` (frontmatter.rs:245-247). `entry_sections` builds text with `str::lines()` (extract.rs:85-91), which also drops a CRLF `\r`.
- **M-5.** Dated stems built from `created` can collide repo-wide (with `docs/issues/<date>-<slug>`, or across trackers). `new_entry` is pure and does no uniqueness check. Only the target collection key is checked (design L199). The `key-space` commit hook would catch a collision late; apply should check every entry key against the key space.
- **M-6.** `closed` = the latest blame date is often a bulk-sweep date (parent HY-35 cause 1). It feeds `cited_target_closed`: `bug-fix-session-log` holds 162 `**Rests on:**`/`**Depends on:**` lines, and `reconnaissance-patterns` holds 49. The result is review noise, not a silent error.
- **M-7.** Blame in a shallow clone gives every line the boundary date. Refuse when `git rev-parse --is-shallow-repository` is true.
- **M-8.** On day one, with `stale_after` set, almost every old open record is stale. That is honest, but the flag carries no ranking signal until records are re-confirmed. Say so in the skill.
- **M-9.** `issue-clusters/IC-*.md` stems are uppercase. `key_of` lowercases them (identity/key.rs:132-135), and the parent D1 example shows `declared-not-wired.md`. The design does not say whether `files` entries keep or rename their stem. A rename is a key change that needs the rewrite.
- **M-10.** `bistriceanu/` (jpg, jsonl, zip) and `mechanisms/` (`tools/*.py`) also fall under the manual rule (non-markdown files). List them by name, as `frictions/` and `effort-steering/` are.
- **M-11.** The § Measurements index-column count of 35 was not reproduced (see the table). The claude-plugins count of 27 includes `INDEX.md`.

---

## Cited facts (claim | result)

| Claim (design) | Result |
|---|---|
| 81 flat trackers in codescout | **verified**: `ls -1p docs/trackers \| grep -v / \| wc -l` gives 81, all `.md` |
| 83,510 lines | **verified**: `cat docs/trackers/*.md \| wc -l` gives 83510 (working tree) |
| 55 with `PREFIX-N` definition headings; 26 without | **verified**: grep `^#{1,6}\s+[A-Z]{1,3}-[0-9]+\s+[—–-]\s+` gives 55 files; 26 listed by `grep -L` |
| 47 with `**Status:**` lines | **verified**: 47 |
| 35 with an index-table status column | **not reproduced**: 23 (an exact `Status` header cell), 32 (a cell containing the word `Status`), 43 (case-insensitive, any table line). The method is unstated |
| `bug-fix-session-log` fixed-verified 119, open 59, mitigated 32 | **close**: index column 121 / 62 / 33 at `02a19eca8` (F-240 landed in `ee19a47ef`). Body lines: 99 / 48 / 28 |
| `bug-fix-session-log` 398 records | **close**: 399 definition headings now |
| `observer-blindness` status in sentences | **verified**: index rows L277-280 ("wrong instrument — …") |
| 66 legacy archive files | **verified**: 66, all `.md` |
| Existing folders `bistriceanu/`, `ecc-integration/`, `effort-steering/`, `frictions/`, `issue-clusters/`, `mechanisms/`, `temp/` | **verified** (plus `archive/`) |
| 27 flat trackers in `../claude-plugins` | **verified**: 27 `.md` files, `INDEX.md` included |
| 0 collections in either repo | **verified**: no `readme.md` under `ledgers/`, `streams/` or `temp/`; `temp/` holds one flat file |
| 38 flat trackers named under `src/`, `scripts/`, `tests/` | **verified**: 38 by basename `git grep -F` over non-`.md` files |
| ~791 / 897 / 179 bare-token mentions | **unverifiable** (the design calls it rough; not re-counted) |
| ~91 of 201 body/index disagreements | **unverifiable** (not re-counted) |
| `lifecycle::collection::collection_readme` | **verified** (collection.rs:252; re-reads with `parse_schema`; no `id:`) |
| `lifecycle::write::new_entry` "and the `today` parameter" | **exists, but behaves differently**: one `today` sets `created`, `status_since`, `closed`, the stem and `RevisitPast` (I1) |
| `archive_rel_path` | **verified** (`lifecycle/naming.rs:131`, not `write.rs`; the caller must check that the target does not exist) |
| `identity::grammar::extract_spanned` | **verified** (grammar.rs:128); link-text and URL caveats (I9) |
| `util::write_catalogued_file` | **verified** as a guarded single-file atomic write; `pub(crate)`; no catalog or audit (M-3) |
| `cutover/code_refs.rs` `CodeRefs` | **verified**; misses JSON and escaped paths (I3) |
| `lifecycle::collection::load_collection` | **verified** (collection.rs:99; live only) |
| `lifecycle::checks` (`lifecycle_findings`) | **wrong module**: `lifecycle::scan::lifecycle_findings`, whole project (M-1) |
| `lifecycle::hot_view` | **verified** (`hot_view.rs:103`; `idle_days` = today − `status_since`; stale if `> stale_after`) |
| `lifecycle::schema` `KEY_HOT_CAP`, `KEY_STALE_AFTER` | **verified** (schema.rs:163-164) |
| `TOOL_SURFACE_CHAR_BUDGET` headroom 0 | **verified** (server.rs:6619-6625: "TOTAL 57_664 (headroom 0)") |
| Bare token resolves only in its own document | consistent with the parent's Constraints; `Bare` arm not re-read |

---

## The design's "Calls I made"

1. **CLI, not MCP.** Agreed: headroom 0 is verified. The consequence the design does not state is that the CLI writes no catalog or audit rows (M-3) and takes no write lock, unlike `cutover export` (I4).
2. **The mapping is never committed.** Agreed. Note that the rationale for a per-record override then lives nowhere after the run. Consider putting a one-line summary in the commit message.
3. **`status_since` = `created`.** The direction is right. It is undercut by `**Valid:** dated` as a `created` source (I1) and by refusing a past `revisit_by` (I2).
4. **`closed` = latest blame.** Acceptable for archive ordering. It costs `cited_target_closed` noise (M-6).
5. **Non-record sections may be dropped, records may not.** Right in principle. The word "record" is defined by the splitter, so this needs the possible-record flag and the partition check (C2).
6. **The skill lives in codescout-companion.** Agreed.
7. **The hygiene log waits for the banner.** Too narrow: the reconnaissance and tracker-hygiene skills write tracker paths too (I3).
8. **Bare tokens a source defines are rewritten.** Agreed. It needs the link-text and URL exclusions (I9) and a ruling on the `**Index row:**` line (I7).
9. **Key reuse by default.** Agreed for `[[key…]]` citations. Path citations remain (C1).
10. **Only structural findings block.** Agreed for the time-based ones. Item 6 blocking is the problem (C1).
11. **Finished streams are written into `archive/<key>/`; check loads by directory.** Agreed; `load_collection_dir` exists (M-2).
12. **Several sources, a duplicate token is refused.** Agreed across sources. A token repeated within one source is not covered (C2: `prompt-hamsa-audit-log`, 5 tokens over 38 headings).
13. **Every index row is carried verbatim.** Agreed. It is ambiguous against the rewrite, and row-only records are not specified (I7).
14. **A code mention blocks only as `reader`.** The classification is sound. The mention list comes from `CodeRefs`, which misses JSON readers and escaped paths, so the agent is never shown them (I3).
15. **Per-record folders are `files`; table-row folders are manual.** Agreed. Also list `bistriceanu/` and `mechanisms/` (M-10), and say how `IC-*` stems are keyed (M-9).

---

## Answers to the author's three extra items (from the coordinator)

### (1) An `EntryDates` renderer: do hot view and `check_entry` then behave correctly?

- **Confirmed.** `new_entry` stamps all three dates as `today` (write.rs:476-480) and also uses `today` for the stem (L455) and `RevisitPast` (L451-452).
- **Hot view: yes, with explicit dates.** The view reads `status_since` from frontmatter, `idle_days = (today - row.status_since).num_days()`, and flags stale when `> stale_after` (hot_view.rs:127-130). A historical `status_since` is therefore flagged honestly. Revisit-due works from the frontmatter `revisit_by` (L143).
- **`check_entry` / `validate_entry`: they accept any dates.** There is no rule on date order or future dates (entry.rs:264-297). Correctness rests entirely on the derivation.
- **What the split must also do.**
  - (a) Build the dated stem from `created`, not from the run date.
  - (b) Keep `RevisitPast` on a separate `now`, or drop it for migration (I2).
  - (c) Make apply refuse `status_since < created`, `closed < created` and dates after `now`.
  - (d) Take `created` as the minimum of the stated dates and the earliest blame date, not the first stated one (I1).

### (2) Ending each record at the next definition heading of ANY level

- **The overlap claim is confirmed.** `entry_sections` ends a section at the next heading of the same or higher level, so a `###` definition sits inside its `##` parent (extract.rs:24-25, 104-118; the doc at L121-125 says "Sections overlap by construction").
- **"Next definition of any level", taken alone, is wrong.** It removes the same-or-higher bound, so the last record absorbs trailing `## Template` and `## Summary` sections, the measured 12.1% error documented at extract.rs:26-33.
- **The rule must be:** the earlier of the next same-or-higher heading and the next definition heading at any level. Under that rule no line is duplicated. Lines are not lost either, but they are *re-attributed*: text of the parent that follows a child (or a non-definition `###` after the child) goes to the child.
- **The real corpus case is worse.** In `prompt-hamsa-audit-log.md`, `### A-38 — …` subsections repeat the parent's token (L2132-2405). Under the proposed rule they become nine separate entries that all define `A-38`, so `[[prompt-hamsa-audit-log:A-38]]` has ten definers.
- **Rule needed:** a definition heading whose token equals the enclosing record's token stays inside that record. Back it with the partition check from C2, so that any splitter error shows as a line coverage mismatch instead of passing.

### (3) Replacing the cutover precondition with a text test (no path, no frontmatter id, no unbracketed `stem:TOKEN`)

**Not sufficient.** The cutover exists to move *catalog-only* facts into files: events, observations, manual links, augmentation shapes and params (`cutover/mod.rs:4-6`). A text test of markdown files cannot see whether those facts were exported. Deleting the flat file then strands its events and params.

Where the text test also misses:
- (a) Inline-code and frozen-file paths. These remain even after a full cutover (C1).
- (b) Non-markdown citers: the `.github/workflows/ci.yml` comments, the `scripts/tc-suites/*.json` pins, and `.pre-commit-config.yaml:170` (I3).
- (c) Other repos: claude-plugins skills cite `codescout:bug-fix-session-log:F-78` and `codescout:R-125` (reconnaissance `SKILL.md`), the cross-repo `repo:stem:TOKEN` form.
- (d) Relative-path links (`../trackers/x.md`, `x.md` from the same folder), unless the test resolves them.
- (e) A 16-hex id cited in prose or tool-call examples, such as the sidecar's `id="5696563f06b2c222"`, rather than in frontmatter.
- (f) `.codescout/memories/*.md`, if the test skips dot directories.

**Recommendation.** Keep a catalog-facts precondition. For example, `cutover plan` scoped to the source paths writes nothing. Make the text test an added check, not a replacement.

---

## What the plan must contain that the design leaves open

1. The C1 ruling on path mentions, and the item-6 scope.
2. The partition proof for M1, the possible-record flag, and the repeated-token rule (C2).
3. An `EntryDates` renderer with date-order validation (I1), and keeping past `revisit_by` (I2).
4. A precondition that gates skill and hook writers in other repos, plus JSON and escaped-path readers (I3).
5. The write lock, re-run semantics consistent with the clean-tree precondition, the dirty-file refusal outside `docs/trackers/`, and the late-record procedure (I4).
6. A `sections` source kind, or explicit manual handling, for the 26 non-record trackers (I5).
7. Sidecar handling and the catalog-facts precondition (I6).
8. Status-none, row-only and row-rewrite rules (I7).
9. The parent-spec edits (I8).
10. The link-text and URL exclusions, and binary resolution (I9).
11. Test minimums beyond the design's list:
    - a record with a non-`def_re` heading is never silently dropped;
    - a repeated token in one source is refused or nested;
    - a re-run after a partial apply completes;
    - a JSON reader blocks.

Ledger checked: none. This is a design review, so no bug search was run. Phase 0 memories `collection-hot-view-state` and `lifecycle-session-state` were read.
