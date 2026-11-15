---
id: '1ebea8dc614e39bb'
kind: plan
status: draft
title: 'Tracker migration (Plan L-E1): the inventory, apply and check commands'
---

# Tracker migration (Plan L-E1): the inventory, apply and check commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Three CLI subcommands, `codescout trackers inventory|apply|check`, that turn one flat tracker into a lifecycle collection from a user-approved mapping, carry every record byte for byte (after one pure citation rewrite), and prove it with a partition check that does not trust the splitter.

**Architecture:** Pure library code in a new module `src/librarian/migrate/` (split, index rows, citation rewrite, mapping, dates, planner, check) on top of the existing lifecycle and identity modules; a thin CLI in `src/cli/trackers.rs` does the I/O (git blobs at `HEAD`, blame, working-tree reads, the write lock, writes and deletes). The planner is deterministic: `apply` writes what it plans, and `check` re-plans from the sources at `HEAD` and compares the plan with the files on disk. This is Plan L-E1. Plan L-E2 (the `migrating-trackers` skill in `../claude-plugins`) is written after this lands, against the JSON this plan produces.

**Tech Stack:** Rust; `git2` (blobs, blame, shallow test); `pulldown-cmark` 0.13 (through `identity::grammar` and a link-text scan); `toml` + `serde` (mapping); `serde_json` (reports); `chrono::NaiveDate`; `clap`.

**Spec:** `docs/superpowers/specs/2026-11-15-tracker-migration-design.md` (v2, `8e8730071`, approved 2026-11-15). Review it answers: `docs/research/2026-11-15-plan-le-design-review.md`. Parent: `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md`. Work-stream log: collection `context-migration` (`docs/trackers/streams/context-migration/`).

## Global Constraints

- Branch from `experiments` in a worktree: `git worktree add .worktrees/tracker-migration -b feat/tracker-migration experiments`, then `EnterWorktree` with that path. The code lands on `experiments` by fast-forward after `./scripts/gate.sh` is green on the exact merged tree (operator, 2026-11-15). No commit whose subject contains `(E4`..`(E8` rides along.
- No advertised MCP tool changes: `TOOL_SURFACE_CHAR_BUDGET` is 57_664 with headroom 0. The commands are CLI only.
- No prompt surface (`src/prompts/`, `.codescout/system-prompt.md`) changes; `ONBOARDING_VERSION` does not change.
- Operator-facing text (CLI help, reports, refusals, the manual page) uses STE: short sentences, one idea each, active voice, the current situation only. No absolute path in any printed text; paths are repo-relative.
- Read door: production code under `src/librarian` uses none of `fs::read`, `fs::read_to_string`, `File::open`, `OpenOptions::new`, `io::read_to_string`, `tokio::fs` (gate `no_librarian_production_read_bypasses_the_read_door`). Committed text comes from `derived::scan::blob_text(repo, commit, rel_path)`; a working-tree file from `ReadDoor::new(vec![git_root]).text(path)`. The CLI module `src/cli/trackers.rs` follows the same rule.
- Every write goes through `crate::librarian::util::write_catalogued_file(roots, path, text)` after `std::fs::create_dir_all(parent)`; it creates no directory. A delete is `std::fs::remove_file` on a path the plan lists.
- Cap-name gate: a constant named with `CAP`, `LIMIT`, `MAX`, `BUDGET` or `THRESHOLD` needs `// cap-class: NOT_A_CAP — <why>` on the line above (`tests/result_caps.rs`). Stage a new file before that gate runs.
- Tests are inline `#[cfg(test)] mod tests` (the read-door gate blanks them), except the CLI end-to-end file `tests/cli_trackers.rs`.
- Targeted tests: `scripts/with-slot.sh cargo test --lib <filter>`; integration: `scripts/with-slot.sh cargo test --test cli_trackers`. Implementers do not run `./scripts/gate.sh`; the controller runs it in Task 12. Format lifecycle files with `rustfmt --edition 2021 <file>` (`./scripts/fmt-mine.sh` refuses them).
- Shared-checkout commits: stage named paths only, read `git diff --cached --name-only` in its own call, then `git commit -m "..." -- <paths>` (`-m` before `--`). Never `git add <directory>`.
- Dates: `today` / `now` is always a parameter of library code. Only the CLI reads the clock (`chrono::Local::now().date_naive()`).

## Review Focus

1. **A CRLF source.** `str::lines()` strips `\r`; the partition and the body comparison must pass on a CRLF tracker exactly as on LF. Test in Task 2 and Task 10.
2. **A definition-shaped line inside a fenced block** (`bug-fix-session-log.md:2909` is the real nested-fence case). It is not a record, its lines stay inside the enclosing range, and the partition still holds. Test in Task 2.
3. **A trailing `## Template` or `## Summary` after the last record.** It is a non-record section, not part of the last record. Test in Task 2.
4. **A title that slugs to nothing** (`日本語`, punctuation only). `apply` refuses and names the token and the `slug` override; it never panics. Test in Task 8.
5. **An index row whose cell holds an escaped pipe `\|` or a code span with `|`.** The cells split correctly, so the status column is the right cell. Test in Task 3.

---

## File map

- Modify `src/librarian/lifecycle/write.rs`: `EntryDates`, `RenderRules`, `render_entry`, new `WriteProblem` variants; `new_entry` delegates (Task 1).
- Modify `src/librarian/identity/grammar.rs`: `in_url_word` becomes `pub(crate)` (Task 4).
- Create `src/librarian/migrate/{mod,records,index_rows,rewrite,mapping,dates,plan,check}.rs` (Tasks 2–10); modify `src/librarian/mod.rs` (`pub mod migrate;`).
- Create `src/cli/trackers.rs`; modify `src/cli/mod.rs`, `src/main.rs` (`Commands::Trackers`) (Tasks 6, 9, 10).
- Create `tests/cli_trackers.rs` (Task 11); `docs/manual/src/concepts/tracker-migration.md`, modify `docs/manual/src/SUMMARY.md` (Task 12).

---

### Task 1: A date-explicit entry renderer

**Files:** Modify `src/librarian/lifecycle/write.rs` (+ its `write/tests.rs`).

**Interfaces:**
- Produces:
  ```rust
  pub struct EntryDates { pub created: NaiveDate, pub status_since: NaiveDate, pub closed: Option<NaiveDate> }
  pub struct RenderRules { pub refuse_past_revisit: bool }
  pub fn render_entry(schema: &CollectionSchema, collection_dir: &str, template: Option<&str>,
                      input: &NewEntry<'_>, dates: &EntryDates, now: NaiveDate, rules: RenderRules)
                      -> Result<EntryFile, Vec<WriteProblem>>
  // new WriteProblem variants:
  DateOrder { field: &'static str, date: NaiveDate, created: NaiveDate }   // status_since or closed before created
  DateInFuture { field: &'static str, date: NaiveDate }                     // any of the three after `now`
  ClosedMismatch { terminal: bool }                                         // closed present iff class is terminal
  ```
- `new_entry(schema, dir, template, input, today)` becomes `render_entry(.., &EntryDates { created: today, status_since: today, closed: terminal.then_some(today) }, today, RenderRules { refuse_past_revisit: true })`. Its behaviour and every existing test stay unchanged.

- [ ] **Step 1: Write the failing tests** in `write/tests.rs`:
  - `render_entry_writes_the_three_dates_it_is_given`: created 2026-08-01, status_since 2026-09-01, terminal, closed 2026-10-01, now 2026-11-15 → frontmatter `created: 2026-08-01`, `status_since: 2026-09-01`, `closed: 2026-10-01`.
  - `render_entry_builds_the_dated_stem_from_created`: `rel_path` ends `/2026-08-01-<slug>.md`, not the `now` date.
  - `render_entry_refuses_dates_out_of_order`: status_since 2026-07-01 < created 2026-08-01 → `Err` contains `DateOrder { field: "status_since", .. }`; closed before created → `DateOrder { field: "closed", .. }`.
  - `render_entry_refuses_a_date_after_now`: `DateInFuture { field: "created", .. }`.
  - `render_entry_refuses_closed_on_an_open_status_and_missing_closed_on_a_terminal_one`: `ClosedMismatch { terminal: false }` and `{ terminal: true }`.
  - `render_entry_keeps_a_past_revisit_by_when_the_rule_is_off`: deferred status, revisit_by 2026-10-01, now 2026-11-15, `refuse_past_revisit: false` → `Ok`, `revisit_by: 2026-10-01` in the text; with `true` → `RevisitPast`.
- [ ] **Step 2:** Run `scripts/with-slot.sh cargo test --lib lifecycle::write` — Expected: FAIL (`render_entry` not found).
- [ ] **Step 3:** Implement `render_entry` by moving `new_entry`'s body into it: the stem from `dates.created`, the frontmatter dates from `dates`, the `RevisitPast` test against `now` only when `rules.refuse_past_revisit`, and the three new date rules checked with the existing pre-checks so all problems return together. Keep the final `reread`.
- [ ] **Step 4:** Run `scripts/with-slot.sh cargo test --lib lifecycle::write` — Expected: PASS, including every pre-existing `new_entry` test.
- [ ] **Step 5:** Commit `src/librarian/lifecycle/write.rs src/librarian/lifecycle/write/tests.rs` — `feat(lifecycle): render_entry takes explicit dates; new_entry delegates (L-E1 Task 1)`.

### Task 2: Split a source into records and sections

**Files:** Create `src/librarian/migrate/mod.rs`, `src/librarian/migrate/records.rs`; modify `src/librarian/mod.rs`.

**Interfaces:**
- Consumes: `tools::link_scan::extract::entry_sections` (definition headings, fence- and frontmatter-aware) and `preview::headings::parse` (every ATX heading with level and 1-indexed line).
- Produces:
  ```rust
  pub type Lines = std::ops::RangeInclusive<u32>;           // 1-indexed lines of `text.lines()`
  pub enum SourceKind { File, Sections { level: usize }, Whole }
  pub struct ExtraRecord { pub heading_line: u32, pub token: String }   // from the mapping's records_extra
  pub struct Record { pub token: Option<String>, pub heading_line: u32, pub level: usize, pub title: String,
                      pub lines: Lines, pub text: String }
  pub struct Section { pub lines: Lines, pub first_line: String, pub possible_record: Option<String> }
  pub struct Repeat { pub token: String, pub first_line: u32, pub other_line: u32, pub nested: bool }
  pub struct Split { pub frontmatter: Option<Lines>, pub records: Vec<Record>, pub sections: Vec<Section>,
                     pub repeats: Vec<Repeat>, pub line_count: u32 }
  pub fn split_source(text: &str, kind: &SourceKind, extra: &[ExtraRecord]) -> Split
  pub fn partition_problems(line_count: u32, owned: &[Lines]) -> Vec<PartitionProblem>
  pub enum PartitionProblem { Uncovered(Lines), Overlap { a: Lines, b: Lines } }
  ```
- Rules (spec § What a record is): a record heading is a definition heading (`File`), an `extra` heading, or any heading of `level` (`Sections`); `Whole` is one record of every line after the frontmatter, `token: None`. A record ends before the earlier of the next heading of the same or higher level and the next record heading whose token differs. A definition heading whose token equals its enclosing record's token is inside that record (`Repeat { nested: true }`); any other repeated token is `Repeat { nested: false }`. Every line not in the frontmatter or a record is in exactly one `Section`; a section starts at a heading or at the first uncovered line, and ends where the next record or section starts. `possible_record` holds the first token-shaped string (`[A-Z]{1,4}-\d+[a-z]?`) of a section's first heading line when that heading is not a record heading.

- [ ] **Step 1: Write the failing tests** (inline `mod tests` in `records.rs`):
  - `records_and_sections_tile_the_source`: frontmatter, preamble, `## Index`, `## R-1 — a`, `### notes`, `## R-2 — b`, `## Template` → records R-1 (with its `###`) and R-2; sections preamble, Index, Template; `partition_problems(split.line_count, all ranges)` is empty.
  - `a_nested_child_ends_its_parent`: `## R-1 — p` / text / `### R-2 — c` / text / `## R-3 — s` → R-1 ends the line before R-2; no line in two records.
  - `a_repeated_token_under_its_own_record_nests`: `## A-38 — x` then `### A-38 — sub` → one record A-38 covering both, `Repeat { nested: true }`.
  - `a_repeated_token_elsewhere_is_reported`: two `## F-1 — …` → `Repeat { nested: false, first_line, other_line }`.
  - `a_suffixed_heading_is_a_possible_record`: `## R-56b — Hit: …` → a `Section` with `possible_record == Some("R-56b")`; with `extra = [ExtraRecord { heading_line, token: "R-56b" }]` → a `Record` with that token.
  - `a_fenced_definition_is_not_a_record` (Review Focus 2) and `a_trailing_template_is_a_section` (Review Focus 3).
  - `a_crlf_source_splits_like_lf` (Review Focus 1): the same text with `\r\n` gives the same ranges and texts.
  - `sections_kind_splits_on_the_level` and `whole_kind_is_one_record`.
  - `partition_problems_reports_a_gap_and_an_overlap`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib migrate::records` — Expected: FAIL (module missing).
- [ ] **Step 3:** Implement `split_source` and `partition_problems` in `records.rs`; register `pub mod migrate;` and `pub mod records;`.
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit the three files — `feat(migrate): split a tracker into records and sections that tile it (L-E1 Task 2)`.

### Task 3: Index tables and record facts

**Files:** Create `src/librarian/migrate/index_rows.rs`; extend `records.rs`.

**Interfaces:**
- Produces:
  ```rust
  pub fn table_cells(line: &str) -> Vec<String>     // splits on `|` outside code spans; `\|` is a literal pipe; cells trimmed
  pub struct IndexRow { pub token: String, pub line: u32, pub raw: String, pub cells: BTreeMap<String, String> }
  pub struct IndexTable { pub lines: Lines, pub header: Vec<String>, pub rows: Vec<IndexRow> }
  pub fn index_tables(text: &str) -> Vec<IndexTable>  // a table whose data rows start with a token cell
  pub fn normalise_status(raw: &str) -> Option<String>
  pub struct Facts { pub raw_status_body: Option<String>, pub stated_dates: Vec<NaiveDate>,
                     pub status_date: Option<NaiveDate>, pub closed_date: Option<NaiveDate>,
                     pub revisit_by: Option<NaiveDate> }
  pub fn record_facts(record: &Record) -> Facts
  ```
- `normalise_status`: strip `**` and backticks, trim, lowercase, cut at the first of ` — `, ` – `, ` (`, `,`, `;`, `:`, trim again; empty → `None`. The header key of `cells` is the header text lowercased and trimmed.
- `record_facts`: the body status is the first line matching `^\s*(?:[-*]\s+)?\*\*Status:?\*\*:?\s*(.*)$`. Labelled lines are bold labels `Date`, `Created`, `Opened`, `Valid`, `Status`, `Closed`, `Fixed`, `Revisit by`, `Revisit`. `stated_dates` = every ISO date (`\b\d{4}-\d{2}-\d{2}\b`, valid calendar date) on the heading line and on labelled lines; `status_date` = the first date on the status line; `closed_date` = the first date on a `Closed` or `Fixed` line; `revisit_by` = the first date on a `Revisit by` or `Revisit` line.

- [ ] **Step 1: Write the failing tests**:
  - `cells_respect_escaped_pipes_and_code_spans` (Review Focus 5): ``| F-1 | a \| b | `x|y` | open |`` → `["F-1", "a | b", "`x|y`", "open"]`.
  - `an_index_table_maps_rows_by_token`: header `| ID | Date | Status | Title |`, rows F-1, F-2 → two rows, `cells["status"] == "open"`, `raw` is the exact line.
  - `a_table_without_token_rows_is_not_an_index`.
  - `status_is_normalised`: `"**Fixed-verified** (2026-10-02)"` → `fixed-verified`; `"wrong instrument — three claims…"` → `wrong instrument`; `""` → `None`.
  - `facts_take_dates_from_heading_and_labelled_lines_only`: a date in free prose is not stated; `**Valid:** dated 2026-11-10` and `**Date:** 2026-08-02` are; `stated_dates.min() == 2026-08-02`.
  - `facts_read_status_closed_and_revisit_dates`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib migrate::index_rows migrate::records` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit — `feat(migrate): index tables, status normalisation and record facts (L-E1 Task 3)`.

### Task 4: The citation rewrite

**Files:** Create `src/librarian/migrate/rewrite.rs`; modify `src/librarian/identity/grammar.rs` (`in_url_word` → `pub(crate)`).

**Interfaces:**
- Consumes: `identity::grammar::{extract_spanned, SpannedCitation, Reference, in_url_word}`.
- Produces:
  ```rust
  pub struct RewriteCounts { pub rewritten: usize, pub skipped_link_text: usize, pub skipped_url: usize }
  pub fn rewrite_record_citations(text: &str, defined: &BTreeSet<String>, old_key: &str, new_key: &str)
      -> (String, RewriteCounts)
  ```
- A `Reference::Bare { token }` with `token ∈ defined` becomes `[[<new_key>:TOKEN]]`, unless its range lies inside the text of an ordinary link (ranges from `pulldown_cmark` `Tag::Link` events with `into_offset_iter`, excluding wiki links) or `in_url_word` holds. When `old_key != new_key`, `Reference::Entry { repo: None, key == old_key, token }` and `Reference::File { repo: None, key == old_key }` get `new_key`. Code spans and fences need no rule: `extract_spanned` yields nothing there. Edits apply from the end of the text to the start.

- [ ] **Step 1: Write the failing tests**:
  - `a_defined_bare_token_becomes_a_qualified_citation`: `"see R-34."`, defined {R-34}, key `observer-blindness` → `"see [[observer-blindness:R-34]]."`, `rewritten == 1`.
  - `an_undefined_bare_token_is_left`: `"see Q-9"` unchanged.
  - `tokens_in_code_link_text_and_urls_are_left`: `` `R-34` ``, `[see R-34](b.md)`, `https://x.test/R-34` unchanged; counts `skipped_link_text == 1`, `skipped_url == 1`.
  - `a_key_change_rewrites_old_key_citations_only`: `[[old:R-1]] [[old]] [[other:R-1]]` with old→new → `[[new:R-1]] [[new]] [[other:R-1]]`.
  - `the_rewrite_is_idempotent`: applying it twice equals applying it once.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib migrate::rewrite` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command — Expected: PASS. Also `scripts/with-slot.sh cargo test --lib identity::grammar` — PASS.
- [ ] **Step 5:** Commit both files — `feat(migrate): rewrite_record_citations, with link-text and URL skips (L-E1 Task 4)`.

### Task 5: The mapping file

**Files:** Create `src/librarian/migrate/mapping.rs`, `src/librarian/migrate/inventory.rs` (types only).

**Interfaces:**
- Produces serde structs that read the spec's § The mapping file in TOML, and:
  ```rust
  pub struct Mapping { pub sources: Vec<SourceSpec>, pub target: Target, pub schema: Option<toml::Table>,
      pub hot_view_none: Option<String>, pub readme_body: Option<String>, pub entry_kind: EntryKindRule,
      pub status_source: StatusSource, pub status_map: BTreeMap<String, String>, pub status_none: Option<String>,
      pub columns: BTreeMap<String, String>, pub row_title_column: Option<String>,
      pub records_extra: Vec<ExtraRecordSpec>, pub overrides: BTreeMap<String, Override>,
      pub sections: Vec<SectionSpec>, pub code_mentions: BTreeMap<String, MentionClass>,
      pub sidecar: Option<SidecarSpec>, pub shorten: BTreeMap<String, String> }
  pub struct SourceSpec { pub path: String, pub kind: String /* file|sections|whole|files */, pub level: Option<usize> }
  pub struct Target { pub lifecycle: String, pub key: Option<String>, pub archived: bool, pub existing: bool }
  pub enum StatusSource { BodyLabel, IndexColumn(String), Constant(String) }
  pub struct Override { pub status: Option<String>, pub created: Option<NaiveDate>, pub closed: Option<NaiveDate>,
                        pub revisit_by: Option<NaiveDate>, pub slug: Option<String> }
  pub enum MentionClass { Reader, Comment, Fixture }
  pub fn parse_mapping(text: &str) -> Result<Mapping, Vec<String>>   // unknown keys refused (deny_unknown_fields)
  pub fn mapping_problems(m: &Mapping, inv: &Inventory) -> Vec<String>
  ```
- `mapping_problems` refuses: an inventoried raw status value missing from `status_map`; records with no status and no `status_none`; row-only records and no `row_title_column`; a possible record neither in `records_extra` nor in `sections` with `confirm = true`; a non-record section with no `sections` entry; a code mention with no class; a sidecar with no `sidecar` entry; `schema` present together with `existing = true`, or absent without it; neither `hot_cap` in `schema` nor `hot_view_none` (unless `existing`). Each message names the item and the key that settles it.

- [ ] **Step 1: Write the failing tests**: `a_full_mapping_parses`; `an_unknown_key_is_refused`; one test per refusal above, each asserting the message names the item (e.g. `"status value \"mitigated\" (32 records) is not in status_map"`).
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib migrate::mapping` — Expected: FAIL.
- [ ] **Step 3:** Implement. Task 5 also creates `src/librarian/migrate/inventory.rs` with the `Inventory` types of Task 6's Interfaces block (types only, no builder), so `mapping_problems` compiles; Task 6 adds `build_inventory` and the CLI.
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit — `feat(migrate): the mapping file and its completeness checks (L-E1 Task 5)`.

### Task 6: The inventory command

**Files:** Create `src/librarian/migrate/inventory.rs` (types + pure builder), `src/cli/trackers.rs`; modify `src/cli/mod.rs`, `src/main.rs`.

**Interfaces:**
- Produces:
  ```rust
  pub struct Inventory { pub sources: Vec<SourceInventory>, pub mentions: Mentions, pub sidecar: Option<SidecarInfo> }
  pub struct SourceInventory { pub path: String, pub line_count: u32, pub records: Vec<RecordRow>,
      pub sections: Vec<SectionRow>, pub repeats: Vec<Repeat>, pub status_values: BTreeMap<String, usize>,
      pub no_status: Vec<String>, pub row_only: Vec<String>, pub no_row: Vec<String>,
      pub disagreements: Vec<Disagreement> }
  pub struct Mentions { pub citations: Vec<Hit>, pub paths: Vec<PathHit /* file, line, in_code */>,
      pub ids: Vec<Hit>, pub code: Vec<CodeHit /* file, line, source: "code_refs"|"grep" */> }
  pub fn build_inventory(sources: &[(String, String, SourceKind)], mapping_status: &StatusSource,
      other_markdown: &BTreeMap<String, String>, code_hits: Vec<CodeHit>, sidecar: Option<SidecarInfo>) -> Inventory
  ```
- CLI: `codescout trackers inventory <path>... [--kind file|sections|whole|files] [--level N] [--status-column <header>] [--project <dir>] [--json]`. It reads every source and every other tracked `.md` at `HEAD` (`blob_text`; the list from `git ls-files` through `lifecycle::scan::list_checkout_paths`), runs `CodeRefs::load(repo, commit, space).mentions(<path>)` and a plain `git grep -F -n <stem> HEAD -- ':!*.md'` (JSON and YAML included) and merges both with their origin, and reads `docs/augmentations/docs-trackers-<stem>.yaml` at `HEAD` if present. Text output is a short STE summary; `--json` prints `Inventory`. Exit 0 on success, 2 when it cannot run (not a git repo, a source missing at `HEAD`), with a one-line reason.

- [ ] **Step 1: Write the failing tests**: in `inventory.rs`, `inventory_counts_status_values_and_lists_gaps` (body labels plus one record without status plus one row-only token) and `mentions_separate_inline_code_paths_from_prose_paths`; in `src/cli/trackers.rs` (`mod tests`, a temporary git repo as in `cli/hot_view.rs` tests), `inventory_json_lists_a_json_reader_that_code_refs_misses` (a `scripts/x.json` naming the tracker path appears with `source: "grep"`).
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib migrate::inventory cli::trackers` — Expected: FAIL.
- [ ] **Step 3:** Implement; add `pub mod trackers;` and `Commands::Trackers(codescout::cli::trackers::TrackersArgs)` with `TrackersAction::{Inventory, Apply, Check}` (`Apply` and `Check` print "not built yet" until Tasks 9 and 10).
- [ ] **Step 4:** Same command — Expected: PASS. `scripts/with-slot.sh cargo test --lib no_librarian_production_read_bypasses_the_read_door` — PASS.
- [ ] **Step 5:** Commit — `feat(trackers): codescout trackers inventory (L-E1 Task 6)`.

### Task 7: Derived dates

**Files:** Create `src/librarian/migrate/dates.rs`.

**Interfaces:**
- Produces:
  ```rust
  pub fn blame_dates(repo: &git2::Repository, commit: &git2::Commit, rel_path: &str) -> anyhow::Result<Vec<NaiveDate>>  // index i = line i+1
  pub struct DateInputs<'a> { pub facts: &'a Facts, pub row_created: Option<NaiveDate>, pub blame: &'a [NaiveDate],
                              pub lines: &'a Lines, pub terminal: bool, pub needs_revisit: bool, pub ov: Option<&'a Override> }
  pub struct Derived { pub dates: EntryDates, pub revisit_by: Option<NaiveDate>, pub derived: Vec<&'static str> }
  pub fn derive_dates(i: &DateInputs, now: NaiveDate) -> Result<Derived, Vec<String>>
  ```
- Rules (spec § Dates): `created` = override, else the minimum of `facts.stated_dates`, `row_created` and the earliest blame date of `lines`; `status_since` = override-free `facts.status_date`, else `created`; `closed` (terminal) = override, else `facts.closed_date`, else the latest blame date of `lines`; `revisit_by` (blocked or deferred) = override, else `facts.revisit_by`, carried even when past; missing → error naming the token. `derived` lists each field that came from blame or a default. `blame_dates` refuses a shallow repository (`repo.is_shallow()`) with "this clone is shallow; blame dates would be wrong".

- [ ] **Step 1: Write the failing tests**: `created_is_the_minimum_of_stated_row_and_blame` (a re-stamped `Valid` 2026-11-10 and blame 2026-08-03 → 2026-08-03); `status_since_defaults_to_created`; `a_past_revisit_by_is_carried`; `a_missing_revisit_by_is_an_error`; `closed_falls_back_to_the_latest_blame_date`; `blame_dates_refuses_a_shallow_clone` (a temporary repo made with `git clone --depth 1 file://…`).
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib migrate::dates` — Expected: FAIL.
- [ ] **Step 3:** Implement (`git2::Repository::blame_file` with `BlameOptions::newest_commit(commit.id())`, the hunk's `final_signature().when()` converted to a UTC date).
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit — `feat(migrate): derived dates from stated dates and blame (L-E1 Task 7)`.

### Task 8: The planner

**Files:** Create `src/librarian/migrate/plan.rs`.

**Interfaces:**
- Consumes: Tasks 1–7; `lifecycle::collection::{collection_readme, LoadedCollection}`; `lifecycle::naming::{slug, entry_stem, entry_rel_path, archive_rel_path}`; `lifecycle::layout::{place, Lifecycle}`; `identity::key::key_of`.
- Produces:
  ```rust
  pub struct SourceText { pub path: String, pub kind: SourceKind, pub text: String, pub blame: Vec<NaiveDate> }
  pub struct MigrationInput<'a> { pub mapping: &'a Mapping, pub sources: Vec<SourceText>,
      pub other_markdown: &'a BTreeMap<String, String>, pub existing_keys: &'a BTreeSet<String>,
      pub existing: Option<&'a LoadedCollection>, pub now: NaiveDate }
  pub enum Owner { Record { entry: String }, Readme, Drop, Frontmatter }
  pub struct OwnedRange { pub source: String, pub lines: Lines, pub owner: Owner }
  pub struct MigrationPlan { pub writes: BTreeMap<String, String>, pub deletes: BTreeSet<String>,
      pub ranges: Vec<OwnedRange>, pub expected_bodies: BTreeMap<String, String>,
      pub review: Review, pub problems: Vec<String> }
  pub fn plan_migration(input: &MigrationInput) -> MigrationPlan
  ```
- Steps: split each source (Task 2) with the mapping's `records_extra`; read row facts (Task 3); refuse a `nested: false` repeat and a token defined in two sources; map each status (source → `normalise_status` → `status_map`, `status_none`, `overrides`); derive dates (Task 7); build each entry with `render_entry` (`RenderRules { refuse_past_revisit: false }`), title from the heading (or the `row_title_column` cell for a row-only record, whose body starts with the synthesised `## <TOKEN> — <cell>`), body = `rewrite_record_citations(record.text)` then, when a row exists, a blank line and `**Index row:** <raw>`; put a terminal entry at `archive_rel_path`, or every entry under `<lifecycle>/archive/<key>/` when `archived`; refuse a key in `existing_keys` (minus the source stems) or used twice, naming the token and the `slug` override (and refuse a title that slugs to nothing the same way, Review Focus 4); build the readme with `collection_readme` (sections marked `readme` appended to `readme_body` in source order, after the rewrite) unless `existing`; for `existing`, refuse a token whose entry exists with a different body and skip one whose body is equal; when the key changes, rewrite `other_markdown` and add each changed file to `writes`; add every source and, for `sidecar = retire`, the sidecar to `deletes`; record every range in `ranges`, and `partition_problems` per source into `problems`. `expected_bodies` maps each entry path to its expected body text (for Task 10).
- `problems` empty is the only state in which `apply` may write.

- [ ] **Step 1: Write the failing tests**: `a_two_record_tracker_plans_two_entries_and_a_readme`; `a_terminal_record_goes_to_archive`; `an_archived_target_puts_everything_under_lifecycle_archive`; `an_unmapped_status_is_a_problem` (defence in depth beside Task 5); `a_dropped_possible_record_without_confirm_is_a_problem`; `a_row_only_record_gets_a_synthesised_heading`; `the_index_row_line_is_verbatim_and_not_rewritten`; `a_key_collision_names_the_slug_override`; `an_unsluggable_title_is_refused_not_a_panic` (Review Focus 4); `a_key_change_rewrites_other_markdown`; `existing_mode_adds_only_missing_tokens`; `the_plan_ranges_tile_every_source`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib migrate::plan` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit — `feat(migrate): the deterministic migration planner (L-E1 Task 8)`.

### Task 9: The apply command

**Files:** Modify `src/cli/trackers.rs`.

**Interfaces:**
- Consumes: Task 8; `cli::write_lock::{lock_root_for, take_at}`; `cli::cutover::{load_and_plan, CommonArgs}`; `util::write_catalogued_file`.
- CLI: `codescout trackers apply <mapping.toml> [--project <dir>] [--no-old-catalog] [--json]`. Order: resolve the git root; take the write lock (`take_at(&lock_root_for(&start, &git_root), Duration::from_secs(10), "trackers apply")`); read the mapping and parse it; build the inventory and run `mapping_problems`; preconditions (below); build the `MigrationInput` from `HEAD` (sources, blame, other markdown, existing keys from `key_of` over every tracked `.md` path); plan; refuse on any `problems`; write every `writes` entry whose working-tree text differs (equal → listed as `already applied`), then delete every `deletes` path that exists. Output (`--json`): `{ "written": [...], "deleted": [...], "already_applied": [...], "review": {...} }`; text: one STE line per group and the counts. Exit 0 written or already applied, 1 refused (one line per reason), 2 could not run.
- Preconditions: (a) the catalog facts: `load_and_plan(&CommonArgs { project, old_db: None, old_root: vec![], json: false }, …)`; refuse when any `plan.files` key or any `Moved` item path is a source; when no old catalog is found, refuse unless `--no-old-catalog`, and say so; (b) every `CodeHit` classified, and no `Reader`; (c) `git status --porcelain=v1 -z` shows no change under `docs/trackers/` outside the plan's `writes ∪ deletes`, and no change in any other path in `writes`; (d) not shallow (from Task 7).

- [ ] **Step 1: Write the failing tests** (temporary repos, `mod tests`): `apply_writes_the_collection_and_deletes_the_source`; `a_second_apply_reports_already_applied`; `apply_after_a_partial_run_completes` (delete one written entry, re-run → it is written again, others `already_applied`); `apply_refuses_a_dirty_unrelated_tracker`; `apply_refuses_a_dirty_file_it_would_rewrite_outside_docs_trackers`; `apply_refuses_a_reader`; `apply_without_an_old_catalog_refuses_unless_the_flag_is_given`; `apply_output_lists_exact_paths`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib cli::trackers` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command — Expected: PASS; `scripts/with-slot.sh cargo test --lib no_librarian_production_read_bypasses_the_read_door` — PASS.
- [ ] **Step 5:** Commit — `feat(trackers): codescout trackers apply, locked and re-runnable (L-E1 Task 9)`.

### Task 10: The check command

**Files:** Create `src/librarian/migrate/check.rs`; modify `src/cli/trackers.rs`.

**Interfaces:**
- Produces:
  ```rust
  pub struct CheckReport { pub blocking: Vec<CheckFinding>, pub review: Review }
  pub enum CheckFinding { Partition(PartitionProblem), TokenMissing(String), TokenExtra(String),
      BodyDiffers { entry: String, first_line: usize }, Placement(String), DateOrder(String),
      Lifecycle(String), OldKeyCitation { file: String, line: u32 }, SidecarLeft(String), HotView(String) }
  pub fn check_migration(plan: &MigrationPlan, on_disk: &BTreeMap<String, Option<String>>,
                         loaded: Result<&LoadedCollection, String>, today: NaiveDate) -> CheckReport
  ```
- `check` **re-reads the written files**: the CLI plans again from `HEAD` (as apply did) and loads each `expected_bodies` path and each `writes` path from the working tree into `on_disk`. A body is the file after its frontmatter; both sides are compared as `lines()` with `\r` stripped and leading and trailing blank lines ignored. Items: 1 partition (from `plan.ranges`, independent of the renderer); 2 tokens (the first heading of each entry file on disk); 3 bodies (`on_disk` vs `expected_bodies`, except `shorten`); 4 placement and date order from each on-disk entry's frontmatter; 5 `load_collection_dir` + `lifecycle::checks::check` over the loaded tree, blocking only `entry_invalid`, `collection_invalid`, `orphan_entry`; 6 no `[[old-key…]]` left (key change) and no retired sidecar on disk; 7 `hot_view(&loaded, today)` is `Ok` when `hot_cap` is set and the collection is live. Everything else goes to `review` (time-based findings, path mentions, derived dates, drops, skipped rewrites, row-only, no-row, disagreements).
- CLI: `codescout trackers check <mapping.toml> [--project <dir>] [--json]`; exit 0 clean, 1 not clean, 2 could not run.

- [ ] **Step 1: Write the failing tests**: `check_passes_after_apply`; `a_line_deleted_from_a_written_entry_fails_check` (the mutant: apply, remove one body line from one entry on disk, check → `BodyDiffers`); `a_record_dropped_by_the_mapping_fails_the_partition_or_tokens`; `a_terminal_entry_moved_live_fails_placement`; `check_on_a_crlf_tracker_passes` (Review Focus 1); `time_based_findings_are_review_not_blocking`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib migrate::check cli::trackers` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit — `feat(trackers): codescout trackers check re-reads what apply wrote (L-E1 Task 10)`.

### Task 11: End to end on real tracker shapes

**Files:** Create `tests/cli_trackers.rs`.

- [ ] **Step 1: Write the tests** (each builds a temporary git repo, commits fixture trackers, runs the built binary with `--json`):
  - `a_session_log_with_frictions_wins_and_an_index_round_trips`: inventory → a mapping written by the test → apply → check exit 0; the entry count equals the record count; `git status` shows only the listed paths.
  - `the_archived_split_file_folds_into_its_collection` (two sources, one with `## R-56b`, named in `records_extra`).
  - `a_repeated_subsection_token_stays_inside_its_record` (`A-38` shape).
  - `a_json_reader_blocks_apply`.
  - `a_rerun_after_an_interrupted_apply_completes`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --test cli_trackers` — Expected: PASS (the commands exist since Tasks 6–10). A test that passes before its feature exists is a finding: report it.
- [ ] **Step 3:** Commit — `test(trackers): end-to-end migration on real tracker shapes (L-E1 Task 11)`.

### Task 12: Manual page, gate, landing

**Files:** Create `docs/manual/src/concepts/tracker-migration.md`; modify `docs/manual/src/SUMMARY.md`.

- [ ] **Step 1:** Write the manual page in STE: what the three commands do, the mapping keys (one line each), the exit codes, and that the `migrating-trackers` skill (Plan L-E2) drives them. No absolute paths.
- [ ] **Step 2:** The controller merges `experiments` into the branch, runs `./scripts/gate.sh` on that exact tree, and reads `GATE EXITS` (all lanes 0).
- [ ] **Step 3:** The controller fast-forwards `experiments` from the main checkout (`git merge --ff-only feat/tracker-migration`) after checking no commit subject in the range contains `(E4`..`(E8`, and tells the lifecycle line.
- [ ] **Step 4:** Commit the manual page — `docs(manual): tracker migration commands (L-E1 Task 12)`.

---

## Rulings in this plan (the operator may overrule any)

- **P-R1** Skips of code spans are not counted: `extract_spanned` yields nothing inside code, so the count would need a second scanner. Only link-text and URL skips are counted. Cost: the review shows fewer skip lines than the spec's "each skip is counted".
- **P-R2** `files` source kind is expanded by the CLI into one `Whole` source per markdown file, key from the lowercase stem; the library has three kinds.
- **P-R3** The writer precondition (session logs, `reconnaissance-patterns`, `tracker-hygiene-log`) lives in Plan L-E2's skill, per spec § Preconditions; L-E1 commands stay generic.
- **P-R4** The catalog-facts precondition reuses `cli::cutover::load_and_plan` and filters its plan to the source paths; the cutover code is not changed.
- **P-R5** Exit codes: 0 done or clean, 1 refused or not clean, 2 could not run; every non-zero exit prints one reason line per problem.
