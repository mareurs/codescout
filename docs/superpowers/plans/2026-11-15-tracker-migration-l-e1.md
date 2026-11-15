---
id: '1ebea8dc614e39bb'
kind: plan
status: draft
title: 'Tracker migration (Plan L-E1): the inventory, apply and check commands'
---

# Tracker migration (Plan L-E1): the inventory, apply and check commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Three CLI subcommands, `codescout trackers inventory|apply|check`, that turn one flat tracker into a lifecycle collection from a user-approved mapping, carry every record byte for byte (after one pure citation rewrite), and prove it with a check that recomputes everything from the source text at `HEAD` and the files on disk.

**Architecture:** Pure library code in a new module `src/librarian/tracker_migration/` (split, index rows, citation rewrite, mapping, inventory, dates, planner, check) on top of the lifecycle and identity modules. A thin async CLI in `src/cli/trackers.rs` does the I/O: git blobs and blame at `HEAD`, working-tree reads, the write lock, writes and deletes. The planner is deterministic and `apply` writes what it plans. `check` does **not** trust the plan's texts: it takes the line ranges from a fresh plan and recomputes every expected body from the source text at `HEAD`, then compares with the files on disk. This is Plan L-E1. Plan L-E2 (the `migrating-trackers` skill in `../claude-plugins`) is written after this lands, against the JSON this plan fixes.

**Tech Stack:** Rust; `git2` (blobs, blame, shallow test); `pulldown-cmark` 0.13 (through `identity::grammar` and a link-text scan); `toml` + `serde` (mapping); `serde_json` (reports, schema values); `chrono::NaiveDate`; `clap`; `tokio` (the CLI `run` is async because `take_at` is).

**Spec:** `docs/superpowers/specs/2026-11-15-tracker-migration-design.md` (v2, `8e8730071`, approved 2026-11-15). Design review it answers: `docs/research/2026-11-15-plan-le-design-review.md`. Parent: `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md`. Work-stream log: collection `context-migration`. Plan v1 (`461b98816`) had an independent Opus review (2 Critical, 9 Important, 10 Minor); v2 settles all of them (§ Plan review findings).

## Global Constraints

- Branch from `experiments` in a worktree: `git worktree add .worktrees/tracker-migration -b feat/tracker-migration experiments`, then `EnterWorktree` with that path. The code lands on `experiments` by fast-forward after `./scripts/gate.sh` is green on the exact merged tree (operator, 2026-11-15). No commit whose subject contains `(E4`..`(E8` rides along.
- The module is `crate::librarian::tracker_migration`, not `migrate`: `crate::migrate` already exists (`src/lib.rs:57`).
- No advertised MCP tool changes: `TOOL_SURFACE_CHAR_BUDGET` is 57_664 with headroom 0. No prompt surface (`src/prompts/`, `.codescout/system-prompt.md`) changes.
- Operator-facing text (CLI help, reports, refusals, the manual page) uses STE and the current situation only. No absolute path in any printed text; paths are repo-relative.
- Read door: production code under `src/librarian` uses none of `fs::read`, `fs::read_to_string`, `File::open`, `OpenOptions::new`, `io::read_to_string`, `tokio::fs` (gate `no_librarian_production_read_bypasses_the_read_door`). Committed text: `derived::scan::blob_text(repo, commit, rel_path) -> Result<Option<String>>`. Working tree: `ReadDoor::new(vec![git_root]).text(path)`. `src/cli/trackers.rs` follows the same rule.
- Every write goes through `crate::librarian::util::write_catalogued_file(roots, path, text)` after `std::fs::create_dir_all(parent)`. A delete calls the same two guards the write door uses (`util::refuse_write_through_symlink`, `util::refuse_catalogued_path_outside_repo`) and then `std::fs::remove_file`, on a path the plan lists.
- Cap-name gate: a constant whose name contains `CAP`, `LIMIT`, `MAX`, `BUDGET` or `THRESHOLD` needs `// cap-class: NOT_A_CAP — <why>` on the line above (`tests/result_caps.rs`). Stage a new file before that gate runs.
- Tests are inline `#[cfg(test)] mod tests`, except `tests/cli_trackers.rs`.
- Test commands: one filter is `scripts/with-slot.sh cargo test --lib <filter>`; several filters go after `--`: `scripts/with-slot.sh cargo test --lib -- <f1> <f2>` (`cargo test --lib a b` is an error). Integration: `scripts/with-slot.sh cargo test --test cli_trackers`. Implementers do not run `./scripts/gate.sh`; the controller runs it in Task 12. Format lifecycle files with `rustfmt --edition 2021 <file>`.
- Shared-checkout commits: stage named paths only, read `git diff --cached --name-only` in its own call, then `git commit -m "..." -- <paths>`. Never `git add <directory>`.
- Clock: library code takes `now` as a parameter. The CLI passes `now` = the later of the local date and the UTC date, so a blame date taken in the commit's own offset is never after `now` (§ Task 7).

## Review Focus

1. **The check shares the planner's inputs.** A splitter or render bug that drops a line must fail `check`. `check_migration` recomputes expected bodies from source text and ranges; unit tests feed it hand-built faulty plans (Task 10).
2. **The key-change rewrite of other files.** Other markdown files are rewritten with an empty token set, so only `[[old-key…]]` citations change and their bare tokens stay (Task 8).
3. **A re-run against its own earlier output.** Untracked entries from a first run are not key collisions, and an equal file is `already applied` (Tasks 8, 9).
4. **A possible record nested inside a record** (`system-retrospective-improvements.md:298` `### T-15a` inside `## SRI-15`). It is reported and blocks until the mapping decides (Tasks 2, 5).
5. **A CRLF source and a definition-shaped line inside a fence.** Both split and check exactly like the LF, unfenced case (Tasks 2, 10).

---

## File map

- Modify `src/librarian/lifecycle/write.rs` (+ `write/tests.rs`): `EntryDates`, `RenderRules`, `render_entry`, new `WriteProblem` variants (Task 1).
- Create `src/librarian/tracker_migration/{mod,records,index_rows,rewrite,mapping,inventory,dates,plan,check}.rs`; modify `src/librarian/mod.rs` (Tasks 2–10).
- Create `src/cli/trackers.rs`; modify `src/cli/mod.rs`, `src/main.rs` (`Commands::Trackers`) (Tasks 6, 9, 10).
- Create `tests/cli_trackers.rs` (Task 11); `docs/manual/src/concepts/tracker-migration.md`, modify `docs/manual/src/SUMMARY.md` (Task 12).

---

### Task 1: A date-explicit entry renderer

**Files:** Modify `src/librarian/lifecycle/write.rs`, `src/librarian/lifecycle/write/tests.rs`.

**Interfaces:**
- Produces:
  ```rust
  pub struct EntryDates { pub created: NaiveDate, pub status_since: NaiveDate, pub closed: Option<NaiveDate> }
  pub struct RenderRules { pub refuse_past_revisit: bool, pub fixed_stem: Option<String> }
  pub fn render_entry(schema: &CollectionSchema, collection_dir: &str, template: Option<&str>,
                      input: &NewEntry<'_>, dates: &EntryDates, now: NaiveDate, rules: &RenderRules)
                      -> Result<EntryFile, Vec<WriteProblem>>
  // new WriteProblem variants:
  DateOrder { field: &'static str, date: NaiveDate, created: NaiveDate }
  DateInFuture { field: &'static str, date: NaiveDate }
  ClosedMismatch { terminal: bool }
  ```
- The stem is `rules.fixed_stem` when set (it must pass `key_of`), else `entry_stem(schema.key_style, dates.created, slug(title))`.
- `new_entry(schema, dir, template, input, today)` becomes `render_entry(.., &EntryDates { created: today, status_since: today, closed: terminal.then_some(today) }, today, &RenderRules { refuse_past_revisit: true, fixed_stem: None })`; its behaviour and every existing test stay unchanged.

- [ ] **Step 1: Write the failing tests** in `write/tests.rs`:
  - `render_entry_writes_the_three_dates_it_is_given` (created 2026-08-01, status_since 2026-09-01, terminal, closed 2026-10-01, now 2026-11-15 → those three values in the frontmatter).
  - `render_entry_builds_the_dated_stem_from_created` (`rel_path` ends `/2026-08-01-<slug>.md`).
  - `render_entry_uses_a_fixed_stem` (`fixed_stem: Some("ic-16-declared-not-wired")` → that file name, whatever the title).
  - `render_entry_refuses_dates_out_of_order` (`DateOrder { field: "status_since" }`, `DateOrder { field: "closed" }`).
  - `render_entry_refuses_a_date_after_now` (`DateInFuture { field: "created" }`).
  - `render_entry_refuses_closed_mismatch` (`ClosedMismatch { terminal: false }` and `{ terminal: true }`).
  - `render_entry_keeps_a_past_revisit_by_when_the_rule_is_off` (deferred, revisit_by 2026-10-01, now 2026-11-15: `Ok` with the rule off; `RevisitPast` with it on).
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib lifecycle::write` — Expected: FAIL (`render_entry` not found).
- [ ] **Step 3:** Move `new_entry`'s body into `render_entry`; check the date rules with the existing pre-checks so every problem returns together; keep the final `reread`.
- [ ] **Step 4:** Same command — Expected: PASS, every pre-existing `new_entry` test included.
- [ ] **Step 5:** Commit both files — `feat(lifecycle): render_entry takes explicit dates and a fixed stem; new_entry delegates (L-E1 Task 1)`.

### Task 2: Split a source into records and sections

**Files:** Create `src/librarian/tracker_migration/mod.rs`, `.../records.rs`; modify `src/librarian/mod.rs`.

**Interfaces:**
- Consumes: `tools::link_scan::extract::entry_sections` (definition headings, fence- and frontmatter-aware) and `preview::headings::parse` (every ATX heading, level, 1-indexed line, fence-aware).
- Produces:
  ```rust
  pub type Lines = std::ops::RangeInclusive<u32>;   // 1-indexed lines of text.lines()
  pub enum SourceKind { File, Sections { level: usize }, Whole }
  pub struct ExtraRecord { pub heading_line: u32, pub token: String }
  pub struct Record { pub token: Option<String>, pub heading_line: u32, pub level: usize,
                      pub title: String, pub lines: Lines }
  pub struct Section { pub start: u32, pub lines: Lines, pub first_line: String }
  pub struct Possible { pub line: u32, pub token: String, pub enclosing: Option<String> }
  pub struct Repeat { pub token: String, pub first_line: u32, pub other_line: u32, pub nested: bool }
  pub struct Split { pub frontmatter: Option<Lines>, pub records: Vec<Record>, pub sections: Vec<Section>,
                     pub possibles: Vec<Possible>, pub repeats: Vec<Repeat>, pub line_count: u32 }
  pub fn split_source(text: &str, kind: &SourceKind, extra: &[ExtraRecord]) -> Split
  pub fn slice(text: &str, lines: &Lines) -> String      // text.lines() in range, joined with "\n"
  pub fn heading_token(line: &str) -> Option<(String, String)>   // (token, title) of a record heading
  pub fn partition_problems(line_count: u32, owned: &[Lines]) -> Vec<PartitionProblem>
  pub enum PartitionProblem { Uncovered(Lines), Overlap { a: Lines, b: Lines } }
  ```
- `Record` carries no text: every body is `slice(source, &record.lines)`, so the planner and the check read the same source lines and cannot disagree about what a record holds.
- `heading_token` matches `^#{1,6}\s+(?:\*\*)?([A-Z]{1,3}-\d+[a-z]?)(?:\*\*)?\s+[—–-]\s+(.+)$` and returns the token and the title **without** the `TOKEN — ` part. It is used for record titles, for `records_extra`, and by the check (Task 10).
- Rules (spec § What a record is):
  - A record heading is a definition heading from `entry_sections` (`File`), an `extra` heading, or any heading of `level` (`Sections`; token `None`, title = heading text). `Whole` is one record covering every line after the frontmatter, token `None`.
  - A record ends before the earlier of the next heading of the same or higher level and the next record heading whose token differs.
  - A definition heading whose token equals the enclosing record's token stays inside it (`Repeat { nested: true }`); any other repeated token is `Repeat { nested: false }`.
  - Every line outside the frontmatter and the records is in exactly one `Section`. A section starts at a heading or at the first uncovered line and ends where the next record or section starts; `start` is its first line.
  - A **possible record** is any heading (inside a section **or inside a record**) whose text starts with a token-shaped string (`heading_token` matches) and that is not a record heading. `enclosing` is the token of the record that contains it, or `None` in a section. A possible record whose token equals `enclosing` is a nested repeat, not a possible record.

- [ ] **Step 1: Write the failing tests** (inline `mod tests`):
  - `records_and_sections_tile_the_source` (frontmatter, preamble, `## Index`, `## R-1 — a` with a `### notes`, `## R-2 — b`, `## Template` → two records, three sections, `partition_problems` empty).
  - `a_nested_child_ends_its_parent`.
  - `a_repeated_token_under_its_own_record_nests` (`## A-38 — x` then `### A-38 — sub` → one record, `Repeat { nested: true }`).
  - `a_repeated_token_elsewhere_is_reported` (two `## F-1 — …` → `nested: false`).
  - `a_suffixed_heading_in_a_section_is_a_possible_record` (`## R-56b — Hit: …` → `Possible { token: "R-56b", enclosing: None }`; with `extra` → a `Record`).
  - `a_possible_record_inside_a_record_is_reported` (`## SRI-15 — …` then `### T-15a — …` → `Possible { token: "T-15a", enclosing: Some("SRI-15") }`, still inside SRI-15's lines).
  - `a_heading_with_utf_8_in_the_middle_is_not_possible` (`## Notes on UTF-8 handling` → none).
  - `a_fenced_definition_is_not_a_record`, `a_trailing_template_is_a_section`, `a_crlf_source_splits_like_lf`.
  - `sections_kind_splits_on_the_level`, `whole_kind_is_one_record`.
  - `heading_token_strips_the_token_from_the_title` (`## R-1 — 日本語` → `("R-1", "日本語")`).
  - `partition_problems_reports_a_gap_and_an_overlap`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib tracker_migration::records` — Expected: FAIL (module missing).
- [ ] **Step 3:** Implement; register `pub mod tracker_migration;` and `pub mod records;`.
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit the three files — `feat(tracker-migration): split a tracker into records and sections that tile it (L-E1 Task 2)`.

### Task 3: Index tables and record facts

**Files:** Create `src/librarian/tracker_migration/index_rows.rs`; extend `records.rs`.

**Interfaces:**
- Produces:
  ```rust
  pub fn table_cells(line: &str) -> Vec<String>   // split on `|` outside code spans; `\|` is a literal pipe; trimmed
  pub struct IndexRow { pub token: String, pub line: u32, pub raw: String, pub cells: BTreeMap<String, String> }
  pub struct IndexTable { pub section_start: u32, pub lines: Lines, pub header: Vec<String>, pub rows: Vec<IndexRow> }
  pub fn index_tables(text: &str, split: &Split) -> Vec<IndexTable>
  pub fn normalise_status(raw: &str) -> Option<String>
  pub struct Facts { pub raw_status_body: Option<String>, pub stated_dates: Vec<NaiveDate>,
                     pub status_date: Option<NaiveDate>, pub closed_date: Option<NaiveDate>,
                     pub revisit_by: Option<NaiveDate> }
  pub fn record_facts(source: &str, record: &Record) -> Facts
  ```
- `index_tables` scans **only non-record sections** and skips fenced blocks. A table is an index when its data rows start with a token cell (`[A-Z]{1,3}-\d+[a-z]?`). `cells` keys are header texts lowercased and trimmed.
- `normalise_status`: strip `**` and backticks, trim, lowercase, cut at the first of ` — `, ` – `, ` (`, `,`, `;`, `:`, trim again; empty → `None`.
- `record_facts`: the body status is the first line of the record matching `^\s*(?:[-*]\s+)?\*\*Status:?\*\*:?\s*(.*)$`. Labelled lines are bold labels `Date`, `Created`, `Opened`, `Valid`, `Status`, `Closed`, `Fixed`, `Revisit by`, `Revisit`. `stated_dates` = every valid ISO date (`\b\d{4}-\d{2}-\d{2}\b`) on the heading line and on labelled lines; `status_date` = the first date on the status line; `closed_date` = the first date on a `Closed` or `Fixed` line; `revisit_by` = the first date on a `Revisit by` or `Revisit` line.

- [ ] **Step 1: Write the failing tests**: `cells_respect_escaped_pipes_and_code_spans` (``| F-1 | a \| b | `x|y` | open |`` → `["F-1", "a | b", "`x|y`", "open"]`); `an_index_table_maps_rows_by_token`; `a_token_table_inside_a_record_is_not_an_index`; `a_fenced_example_row_is_not_an_index`; `status_is_normalised` (`"**Fixed-verified** (2026-10-02)"` → `fixed-verified`; `"wrong instrument — three claims…"` → `wrong instrument`); `facts_take_dates_from_heading_and_labelled_lines_only`; `facts_read_status_closed_and_revisit_dates`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib -- tracker_migration::index_rows tracker_migration::records` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit — `feat(tracker-migration): index tables, status normalisation and record facts (L-E1 Task 3)`.

### Task 4: The citation rewrite

**Files:** Create `src/librarian/tracker_migration/rewrite.rs`.

**Interfaces:**
- Consumes: `identity::grammar::{extract_spanned, SpannedCitation, Reference, in_url_word}` (`in_url_word` is already `pub(crate)`, `grammar.rs:203`).
- Produces:
  ```rust
  pub struct RewriteCounts { pub rewritten: usize, pub skipped_link_text: usize, pub skipped_url: usize }
  pub fn rewrite_record_citations(text: &str, defined: &BTreeSet<String>, old_key: &str, new_key: &str)
      -> (String, RewriteCounts)
  ```
- A `Reference::Bare { token }` with `token ∈ defined` becomes `[[<new_key>:TOKEN]]`, unless its range lies inside the text of an ordinary link (ranges from `pulldown_cmark` `Tag::Link` events with `into_offset_iter`) or `in_url_word` holds. When `old_key != new_key`, a `Reference::Entry { repo: None, key == old_key, .. }` or `Reference::File { repo: None, key == old_key }` has **only its key bytes** replaced, so a label or anything else inside the brackets is kept. Code spans and fences need no rule: `extract_spanned` yields nothing there. Edits apply from the end of the text to the start.
- **Other files** (Task 8) are always called with an **empty** `defined` set: only their `[[old-key…]]` citations change.

- [ ] **Step 1: Write the failing tests**: `a_defined_bare_token_becomes_a_qualified_citation`; `an_undefined_bare_token_is_left`; `tokens_in_code_link_text_and_urls_are_left` (counts 1 and 1); `a_key_change_rewrites_old_key_citations_only` (`[[old:R-1]] [[old]] [[other:R-1]]`); `a_key_change_keeps_a_label` (`[[old:F-1|x]]` → `[[new:F-1|x]]`, if the grammar parses a label; else the test asserts the text is unchanged and says why); `an_empty_defined_set_leaves_bare_tokens` (`"F-1 and [[old:F-1]]"` → `"F-1 and [[new:F-1]]"`); `the_rewrite_is_idempotent`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib tracker_migration::rewrite` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit — `feat(tracker-migration): rewrite_record_citations, with link-text and URL skips (L-E1 Task 4)`.

### Task 5: The mapping file and the inventory types

**Files:** Create `src/librarian/tracker_migration/mapping.rs`, `.../inventory.rs` (types only; Task 6 adds the builder).

**Interfaces:**
- Produces the inventory types (serialised as the `inventory --json` output that Plan L-E2 reads):
  ```rust
  pub struct Inventory { pub sources: Vec<SourceInventory>, pub mentions: Mentions, pub sidecar: Option<SidecarInfo> }
  pub struct SourceInventory { pub path: String, pub line_count: u32, pub records: Vec<RecordRow>,
      pub sections: Vec<SectionRow>, pub possibles: Vec<Possible>, pub repeats: Vec<Repeat>,
      pub tables: Vec<TableRow>, pub status_values: BTreeMap<String, usize>, pub no_status: Vec<String>,
      pub row_only: Vec<String>, pub no_row: Vec<String>, pub duplicate_rows: Vec<String>,
      pub disagreements: Vec<Disagreement> }
  pub struct RecordRow { pub token: Option<String>, pub heading_line: u32, pub level: usize, pub title: String,
      pub first: u32, pub last: u32, pub raw_status: Option<String>, pub status_origin: Option<String>,
      pub stated_dates: Vec<NaiveDate> }
  pub struct SectionRow { pub start: u32, pub last: u32, pub first_line: String }
  pub struct TableRow { pub section_start: u32, pub header: Vec<String>, pub rows: usize }
  pub struct Disagreement { pub token: String, pub body: String, pub index: String }
  pub struct Mentions { pub citations: Vec<Hit>, pub paths: Vec<PathHit>, pub ids: Vec<Hit>, pub code: Vec<CodeHit> }
  pub struct Hit { pub file: String, pub line: u32, pub text: String }
  pub struct PathHit { pub file: String, pub line: u32, pub in_code: bool, pub frozen: bool }
  pub struct CodeHit { pub file: String, pub line: u32, pub origin: String /* "code_refs" | "grep" */ }
  pub struct SidecarInfo { pub path: String, pub prompt: Option<String> }
  ```
  (`Possible` and `Repeat` from Task 2 derive `Serialize`.)
- Produces the mapping (spec § The mapping file), `#[serde(deny_unknown_fields)]` on every struct:
  ```rust
  pub struct Mapping { pub sources: Vec<SourceSpec>, pub target: Target, pub schema: Option<toml::Table>,
      pub hot_view_none: Option<String>, pub readme_body: Option<String>, pub entry_kind: EntryKindRule,
      pub status_source: StatusSource, pub status_map: BTreeMap<String, String>, pub status_none: Option<String>,
      pub columns: BTreeMap<String, String>, pub row_title_column: Option<String>, pub index_table: Option<u32>,
      pub records_extra: Vec<ExtraRecordSpec>, pub possibles_inside: Vec<PossibleSpec>,
      pub overrides: BTreeMap<String, Override>, pub sections: Vec<SectionSpec>,
      pub code_mentions: BTreeMap<String, MentionClass>, pub sidecar: Option<SidecarSpec>,
      pub shorten: BTreeMap<String, String> }
  pub struct SourceSpec { pub path: String, pub kind: String /* file|sections|whole|files */, pub level: Option<usize> }
  pub struct Target { pub lifecycle: String, pub key: Option<String>, pub archived: bool, pub existing: bool }
  pub struct ExtraRecordSpec { pub source: String, pub line: u32, pub token: String }
  pub struct PossibleSpec { pub source: String, pub line: u32 }        // stays inside its enclosing record
  pub struct SectionSpec { pub source: String, pub start: u32, pub to: String /* readme|drop */, pub confirm: bool }
  pub enum StatusSource { BodyLabel, IndexColumn(String), Constant(String) }
  pub struct Override { pub status: Option<String>, pub created: Option<NaiveDate>, pub closed: Option<NaiveDate>,
                        pub revisit_by: Option<NaiveDate>, pub slug: Option<String> }
  pub enum MentionClass { Reader, Comment, Fixture }
  pub fn parse_mapping(text: &str) -> Result<Mapping, Vec<String>>
  pub fn mapping_problems(m: &Mapping, inv: &Inventory) -> Vec<String>
  ```
- `mapping_problems` refuses, each message naming the item and the key that settles it: a raw status value missing from `status_map`; records with no status and no `status_none`; row-only records and no `row_title_column`; a token in two index tables and no `index_table`; a possible record in a section that is neither in `records_extra` nor in `sections` with `confirm = true`; a possible record inside a record that is neither in `records_extra` nor in `possibles_inside`; a non-record section with no `sections` entry (keyed by source and `start`); a code mention with no class; a sidecar with no `sidecar` entry; `schema` present with `existing = true`, or absent without it; a new collection without `hot_cap` **and** `stale_after` in `schema`, unless `hot_view_none` is set (M5).

- [ ] **Step 1: Write the failing tests**: `a_full_mapping_parses`; `an_unknown_key_is_refused`; one test per refusal above, each asserting the message names the item (e.g. `status value "mitigated" (32 records) is not in status_map`); `inventory_types_round_trip_json`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib tracker_migration::mapping` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit both files — `feat(tracker-migration): the mapping file, its completeness checks and the inventory types (L-E1 Task 5)`.

### Task 6: The inventory command

**Files:** Modify `src/librarian/tracker_migration/inventory.rs`; create `src/cli/trackers.rs`; modify `src/cli/mod.rs`, `src/main.rs`.

**Interfaces:**
- Produces:
  ```rust
  pub fn build_inventory(sources: &[(String, String, SourceKind)], status: &StatusSource,
      other_markdown: &BTreeMap<String, String>, frozen: &dyn Fn(&str) -> bool,
      code_hits: Vec<CodeHit>, sidecar: Option<SidecarInfo>) -> Inventory
  pub struct TrackersArgs { pub action: TrackersAction }
  pub enum TrackersAction { Inventory { .. }, Apply { .. }, Check { .. } }
  pub async fn run(args: TrackersArgs) -> anyhow::Result<()>
  ```
- CLI: `codescout trackers inventory <path>... [--kind file|sections|whole|files] [--level N] [--status-column <header>] [--project <dir>] [--json]`. It reads every source and every other tracked `.md` at `HEAD` (`blob_text`; the list from the `HEAD` tree), takes `frozen` from `CodeRefs::frozen`, runs `CodeRefs::load(repo, commit, &space).mentions(<path>)` and `git grep -n -F <stem> HEAD -- ':!*.md'` (JSON and YAML included) and merges both with their origin, and reads `docs/augmentations/docs-trackers-<stem>.yaml` at `HEAD` if present. `--kind files` expands a folder into one `Whole` source per markdown file. Text output is a short STE summary; `--json` prints `Inventory`. Exit 0 on success, 2 when it cannot run (not a git repo, a source missing at `HEAD`), with a one-line reason.

- [ ] **Step 1: Write the failing tests**: in `inventory.rs`, `inventory_counts_status_values_and_lists_gaps` and `mentions_separate_inline_code_paths_from_prose_paths`; in `src/cli/trackers.rs` (temporary git repo, as in `cli/hot_view.rs` tests), `inventory_json_lists_a_json_reader_that_code_refs_misses` (`scripts/x.json` naming the tracker path → `origin: "grep"`).
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib -- tracker_migration::inventory cli::trackers` — Expected: FAIL.
- [ ] **Step 3:** Implement; add `pub mod trackers;` and `Commands::Trackers(codescout::cli::trackers::TrackersArgs)` dispatched with `.await` (`Apply` and `Check` print a one-line "not built yet" and exit 2 until Tasks 9 and 10).
- [ ] **Step 4:** Same command — Expected: PASS. `scripts/with-slot.sh cargo test --lib no_librarian_production_read_bypasses_the_read_door` — PASS.
- [ ] **Step 5:** Commit — `feat(trackers): codescout trackers inventory (L-E1 Task 6)`.

### Task 7: Derived dates

**Files:** Create `src/librarian/tracker_migration/dates.rs`.

**Interfaces:**
- Produces:
  ```rust
  pub fn blame_dates(repo: &git2::Repository, commit: &git2::Commit, rel_path: &str) -> anyhow::Result<Vec<NaiveDate>>
  pub struct DateInputs<'a> { pub facts: &'a Facts, pub row_created: Option<NaiveDate>, pub blame: &'a [NaiveDate],
      pub lines: &'a Lines, pub terminal: bool, pub needs_revisit: bool, pub ov: Option<&'a Override>, pub token: &'a str }
  pub struct Derived { pub dates: EntryDates, pub revisit_by: Option<NaiveDate>, pub derived: Vec<&'static str> }
  pub fn derive_dates(i: &DateInputs, now: NaiveDate) -> Result<Derived, Vec<String>>
  ```
- `blame_dates`: index `i` is line `i + 1`; `Repository::blame_file` with `BlameOptions::newest_commit(commit.id())`; each date is the hunk's `final_signature().when()` in **its own offset** (`offset_minutes`), so a commit made in the evening west of UTC keeps its local date. It refuses a shallow repository (`repo.is_shallow()`): "this clone is shallow; blame dates would be wrong".
- Rules (spec § Dates): `created` = override, else the minimum of `facts.stated_dates`, `row_created` and the earliest blame date of `lines`; `status_since` = `facts.status_date`, else `created`; `closed` (terminal) = override, else `facts.closed_date`, else the latest blame date of `lines`; `revisit_by` (blocked or deferred) = override, else `facts.revisit_by`, carried even when past; missing → an error naming the token. `derived` lists each field that came from blame or a default.

- [ ] **Step 1: Write the failing tests**: `created_is_the_minimum_of_stated_row_and_blame` (a re-stamped `Valid` 2026-11-10 and blame 2026-08-03 → 2026-08-03); `status_since_defaults_to_created`; `a_past_revisit_by_is_carried`; `a_missing_revisit_by_is_an_error`; `closed_falls_back_to_the_latest_blame_date`; `blame_dates_use_the_commit_offset` (a commit dated `2026-08-03T23:30:00-0700` → 2026-08-03); `blame_dates_refuses_a_shallow_clone` (`git clone --depth 1 file://…`).
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib tracker_migration::dates` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit — `feat(tracker-migration): derived dates from stated dates and blame (L-E1 Task 7)`.

### Task 8: The planner

**Files:** Create `src/librarian/tracker_migration/plan.rs`.

**Interfaces:**
- Consumes: Tasks 1–7; `lifecycle::collection::collection_readme`; `lifecycle::schema::parse_schema`; `lifecycle::naming::{slug, archive_rel_path}`; `lifecycle::layout::{place, Lifecycle}`; `identity::key::key_of`.
- Produces:
  ```rust
  pub struct SourceText { pub path: String, pub kind: SourceKind, pub text: String, pub blame: Vec<NaiveDate>, pub fixed_stem: Option<String> }
  pub struct ExistingEntry { pub rel_path: String, pub text: String, pub token: Option<String> }
  pub struct MigrationInput<'a> { pub mapping: &'a Mapping, pub sources: Vec<SourceText>,
      pub other_markdown: &'a BTreeMap<String, String>, pub key_paths: &'a BTreeMap<String, BTreeSet<String>>,
      pub existing: Option<(&'a CollectionSchema, Option<&'a str> /* template */, Vec<ExistingEntry>)>, pub now: NaiveDate }
  pub enum Owner { Record { entry: String, token: Option<String>, row: Option<String> }, Readme, Drop, Frontmatter }
  pub struct OwnedRange { pub source: String, pub lines: Lines, pub owner: Owner }
  pub struct MigrationPlan { pub target_dir: String, pub readme_path: Option<String>,
      pub writes: BTreeMap<String, String>, pub deletes: BTreeSet<String>, pub ranges: Vec<OwnedRange>,
      pub shortened: BTreeSet<String>, pub defined: BTreeSet<String>, pub old_key: String, pub new_key: String,
      pub review: Review, pub problems: Vec<String> }
  pub fn plan_migration(input: &MigrationInput) -> MigrationPlan
  pub fn expected_body(source: &str, r: &OwnedRange, defined: &BTreeSet<String>, old: &str, new: &str) -> String
  ```
- `expected_body` = `rewrite_record_citations(slice(source, &r.lines), defined, old, new).0`, then, when `Owner::Record.row` is `Some(raw)`, a blank line and `**Index row:** <raw>`. A row-only record has an empty range body: its body is the synthesised heading `## <TOKEN> — <cell of row_title_column>` and the row line. **The planner builds every entry body with `expected_body`**, then renders the entry with `render_entry(.., &RenderRules { refuse_past_revisit: false, fixed_stem })`; a record in `shorten` gets the mapping's replacement body instead and is listed in `shortened`.
- Steps:
  1. Split each source with the mapping's `records_extra` (Task 2); read index tables (Task 3), choosing `index_table` when a token is in two.
  2. Refuse a `nested: false` repeat, a token defined in two sources, and an unsettled possible record.
  3. Map each status (source → `normalise_status` → `status_map`, `status_none`, `overrides`); derive dates (Task 7); map `columns` cells into declared fields (a string; a value the schema type refuses is a problem naming the token and the column).
  4. Title = `heading_token`'s title (without the token); key = `fixed_stem`, else the stem from `created` and the title's slug, else the override `slug`. A title that slugs to nothing, or a key whose path in `key_paths` is **not** the planned target and not a source being deleted, is a problem naming the token and the `slug` override.
  5. Place a terminal entry at `archive_rel_path`, or every entry under `docs/trackers/<lifecycle>/archive/<key>/` when `archived`.
  6. New collection: convert `schema` with `serde_json::to_value` into the `BTreeMap<String, serde_json::Value>` that `collection_readme(lifecycle, title, &extra, body)` takes; the readme body is `readme_body`, then each `readme` section's `expected_body` in source order. Parse the result back with `frontmatter::parse` + `parse_schema` for the `CollectionSchema` the entries use. `existing`: use the given schema and template; a token whose existing entry body differs is a problem; an equal one is skipped (no write).
  7. Key change: rewrite each `other_markdown` file with `rewrite_record_citations(text, &BTreeSet::new(), old, new)`; add changed files to `writes`.
  8. Add every source and, for `sidecar = retire`, the sidecar to `deletes`. Record every range in `ranges`; run `partition_problems` per source into `problems`.
- `problems` empty is the only state in which `apply` may write.

- [ ] **Step 1: Write the failing tests**: `a_two_record_tracker_plans_two_entries_and_a_readme`; `a_terminal_record_goes_to_archive`; `an_archived_target_puts_everything_under_lifecycle_archive`; `an_unmapped_status_is_a_problem`; `an_unsettled_possible_record_is_a_problem`; `a_row_only_record_gets_a_synthesised_heading`; `the_index_row_line_is_verbatim_and_not_rewritten`; `columns_become_declared_fields`; `a_shortened_record_is_listed`; `a_key_collision_names_the_slug_override`; `an_unsluggable_title_is_refused_not_a_panic` (`## R-1 — 日本語`); `a_rerun_does_not_collide_with_its_own_untracked_entries` (`key_paths` holds the planned target path → no problem); `a_key_change_rewrites_old_key_citations_in_other_files_only` (another file holds a bare `F-1` and `[[old:F-1]]` → only the second changes); `existing_mode_adds_only_missing_tokens_and_refuses_a_changed_one`; `a_files_source_keeps_its_lowercase_stem`; `the_plan_ranges_tile_every_source`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib tracker_migration::plan` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit — `feat(tracker-migration): the deterministic migration planner (L-E1 Task 8)`.

### Task 9: The apply command

**Files:** Modify `src/cli/trackers.rs`.

**Interfaces:**
- Consumes: Task 8; `cli::write_lock::{lock_root_for, take_at}` (`take_at` is async); `derived::old_catalog::resolve_old_catalog`; `cli::cutover::{load_and_plan, CommonArgs}`; `util::write_catalogued_file`.
- CLI: `codescout trackers apply <mapping.toml> [--project <dir>] [--no-old-catalog] [--json]`. Order:
  1. Resolve the git root; `take_at(&lock_root_for(&start, &git_root), Duration::from_secs(10), "trackers apply").await`.
  2. Parse the mapping; build the inventory (Task 6) and run `mapping_problems`.
  3. Preconditions: (a) catalog facts: when `resolve_old_catalog(None, env LIBRARIAN_DB, data dir)` names no existing file, refuse unless `--no-old-catalog`, and say so; otherwise `load_and_plan(&CommonArgs { project, old_db: None, old_root: vec![], json: false }, …)` and refuse when any `plan.files` key or any `Moved` item path is a source; (b) every `CodeHit` classified and none `Reader`; (c) `git status --porcelain=v1 -z` shows no change under `docs/trackers/` outside the plan's `writes ∪ deletes`, and no change in any other path in `writes`; (d) not shallow.
  4. Build `MigrationInput` from `HEAD` (sources, blame, other markdown) plus `key_paths` = `key_of` over every tracked path in the `HEAD` tree and every untracked `.md` under `docs/` (so a first run's untracked entries are seen with their paths); for `existing`, `load_collection` by key and read each entry's text from the working tree.
  5. Plan; refuse on any `problems`.
  6. For each `writes` path: absent → write; equal text → `already applied`; other text → **refusal** naming it, unless the path is an other-markdown rewrite (those are rewritten). Then delete each `deletes` path that exists.
- Output (`--json`): `{ "written": [...], "deleted": [...], "already_applied": [...], "review": {...} }`; text: one STE line per group and the counts. Exit 0 written or already applied, 1 refused (one line per reason), 2 could not run.
- **Late records:** the skill first commits the merge that brings the flat file back (so it is at `HEAD`), then runs `apply` with `existing = true`.

- [ ] **Step 1: Write the failing tests** (temporary repos, `mod tests`): `apply_writes_the_collection_and_deletes_the_source`; `a_second_apply_reports_already_applied`; `apply_after_a_partial_run_completes` (delete one written entry, re-run); `apply_refuses_a_target_that_holds_other_text`; `apply_refuses_a_dirty_unrelated_tracker`; `apply_refuses_a_dirty_file_it_would_rewrite_outside_docs_trackers`; `apply_refuses_a_reader`; `apply_without_an_old_catalog_refuses_unless_the_flag_is_given`; `a_late_record_after_a_committed_merge_is_added_in_existing_mode`; `apply_output_lists_exact_paths`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib cli::trackers` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command — Expected: PASS; `scripts/with-slot.sh cargo test --lib no_librarian_production_read_bypasses_the_read_door` — PASS.
- [ ] **Step 5:** Commit — `feat(trackers): codescout trackers apply, locked and re-runnable (L-E1 Task 9)`.

### Task 10: The check command

**Files:** Create `src/librarian/tracker_migration/check.rs`; modify `src/cli/trackers.rs`.

**Interfaces:**
- Produces:
  ```rust
  pub struct CheckInput<'a> { pub plan: &'a MigrationPlan, pub sources: &'a BTreeMap<String, String> /* text at HEAD */,
      pub collection_files: &'a BTreeMap<String, String> /* every file under target_dir, from disk */,
      pub repo_markdown: &'a BTreeMap<String, String> /* every working-tree .md */,
      pub loaded: Result<&'a LoadedCollection, String>, pub hot: Option<Result<(), String>>, pub today: NaiveDate }
  pub struct CheckReport { pub blocking: Vec<CheckFinding>, pub review: Review }
  pub enum CheckFinding { Partition { source: String, problem: PartitionProblem }, TokenMissing(String),
      TokenExtra(String), TokenTwice(String), BodyDiffers { entry: String, first_line: usize },
      ReadmeMissing { source: String, start: u32 }, Placement(String), DateOrder(String), Lifecycle(String),
      OldKeyCitation { file: String, line: u32 }, SidecarLeft(String), HotView(String) }
  pub fn check_migration(i: &CheckInput) -> CheckReport
  ```
- `check_migration` **recomputes**, it does not reuse plan texts:
  1. **Partition:** for each source, `line_count = text.lines().count()` from `sources`, and `partition_problems` over the plan's ranges for that source.
  2. **Tokens:** read the first heading of **every** markdown file in `collection_files` (readme excluded) with `heading_token`; each record token of the plan appears exactly once; a token the plan lacks is `TokenExtra`.
  3. **Bodies:** for each `Owner::Record` range, `expected_body(sources[source], range, plan.defined, plan.old_key, plan.new_key)` against the body of that entry file on disk (after its frontmatter), except `plan.shortened`. Both sides are compared as `lines()` with `\r` stripped and leading and trailing blank lines ignored.
  4. **Readme:** each `Owner::Readme` range's `expected_body` appears in the readme on disk, in source order.
  5. **Placement and dates:** from each entry's frontmatter on disk: terminal in `archive/` (or archived collection), others live; date order.
  6. **Lifecycle:** `loaded` is `Ok` and `lifecycle::checks::check` over its tree reports no `entry_invalid`, `collection_invalid`, `orphan_entry`.
  7. **Old key and sidecar:** when `old_key != new_key`, `grammar::extract` over every `repo_markdown` file finds no `[[old_key…]]`; a retired sidecar is absent.
  8. **Hot view:** `hot` is `Some(Err(_))` → `HotView`. The CLI fills `hot` for a live collection with `hot_cap`: `load_collection(root, key)` must succeed, its tree must have no problems, and `hot_view(&loaded, today)` must be `Ok`.
  Everything else goes to `review`: time-based findings, path mentions, derived dates, drops, skipped rewrites, row-only, no-row, disagreements.
- CLI: `codescout trackers check <mapping.toml> [--project <dir>] [--json]`; it re-plans from `HEAD` (as apply did), loads the inputs above, and exits 0 clean, 1 not clean, 2 could not run.

- [ ] **Step 1: Write the failing tests**:
  - `check_passes_after_apply`.
  - `a_line_deleted_from_a_written_entry_fails_check` (the disk mutant → `BodyDiffers`).
  - `a_plan_whose_write_lacks_a_line_fails_check` (hand-built faulty `MigrationPlan`: the file on disk was rendered from a body missing one line its range covers → `BodyDiffers`).
  - `a_plan_with_a_gap_in_its_ranges_fails_the_partition` (hand-built plan → `Partition`).
  - `an_extra_entry_file_fails_tokens` (`TokenExtra`).
  - `a_records_extra_token_with_a_suffix_is_found` (`R-56b`).
  - `a_readme_section_missing_from_the_readme_fails` (`ReadmeMissing`).
  - `a_terminal_entry_moved_live_fails_placement`.
  - `an_old_key_citation_left_in_another_file_fails`.
  - `an_ambiguous_collection_key_fails_the_hot_view_item` (the same key under `ledgers/` and `streams/`).
  - `check_on_a_crlf_tracker_passes`; `time_based_findings_are_review_not_blocking`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --lib -- tracker_migration::check cli::trackers` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command — Expected: PASS.
- [ ] **Step 5:** Commit — `feat(trackers): codescout trackers check recomputes from HEAD and re-reads what apply wrote (L-E1 Task 10)`.

### Task 11: End to end on real tracker shapes

**Files:** Create `tests/cli_trackers.rs`.

- [ ] **Step 1: Write the tests** (each builds a temporary git repo, commits fixture trackers, runs the built binary with `--json`):
  - `a_session_log_with_frictions_wins_and_an_index_round_trips` (inventory → a test-written mapping → apply → check exit 0; entry count equals record count; `git status` shows only the listed paths).
  - `the_archived_split_file_folds_into_its_collection` (two sources, `## R-56b` in `records_extra`).
  - `a_repeated_subsection_token_stays_inside_its_record` (`A-38` shape).
  - `a_nested_possible_record_blocks_until_settled` (`T-15a` inside `SRI-15`).
  - `a_json_reader_blocks_apply`.
  - `a_rerun_after_an_interrupted_apply_completes`.
- [ ] **Step 2:** `scripts/with-slot.sh cargo test --test cli_trackers` — Expected: PASS. A test that passes before its feature exists is a finding: report it.
- [ ] **Step 3:** Commit — `test(trackers): end-to-end migration on real tracker shapes (L-E1 Task 11)`.

### Task 12: Manual page, gate, landing

**Files:** Create `docs/manual/src/concepts/tracker-migration.md`; modify `docs/manual/src/SUMMARY.md`.

- [ ] **Step 1:** Write the manual page in STE: what the three commands do, the mapping keys (one line each), the exit codes, the late-record order, and that the `migrating-trackers` skill (Plan L-E2) drives them. No absolute paths.
- [ ] **Step 2:** The controller merges `experiments` into the branch, runs `./scripts/gate.sh` on that exact tree, and reads `GATE EXITS` (all lanes 0).
- [ ] **Step 3:** The controller checks no commit subject in the range contains `(E4`..`(E8`, fast-forwards `experiments` from the main checkout (`git merge --ff-only feat/tracker-migration`), and tells the lifecycle line.
- [ ] **Step 4:** Commit the manual page — `docs(manual): tracker migration commands (L-E1 Task 12)`.

---

## Rulings in this plan (the operator may overrule any)

- **P-R1** Skips inside code spans are not counted (`extract_spanned` yields nothing there); only link-text and URL skips are.
- **P-R2** The CLI expands a `files` source into one `Whole` source per markdown file, with `fixed_stem` = the lowercase stem; the library has three kinds.
- **P-R3** The writer precondition (session logs, `reconnaissance-patterns`, `tracker-hygiene-log`) lives in Plan L-E2's skill, per spec § Preconditions.
- **P-R4** The catalog-facts precondition reuses `cli::cutover::load_and_plan`, filtered to the source paths; "no old catalog" is detected with `resolve_old_catalog` before it.
- **P-R5** Exit codes: 0 done or clean, 1 refused or not clean, 2 could not run.
- **P-R6** `now` is the later of the local and the UTC date; blame dates use each commit's own offset.
- **P-R7** Late records need the merge committed first, so the flat file is at `HEAD` when `apply` reads it.
- **P-R8** A token in two index tables needs `index_table` in the mapping; there is no silent winner.

## Plan review findings (review of v1, `461b98816`) and how v2 settles them

| Finding | v2 |
|---|---|
| C1 check shares the planner's inputs | `Record` holds no text; `expected_body` from source lines; `check_migration` gets the sources and recomputes; faulty-plan tests (Task 10) |
| C2 other-file rewrite turns bare tokens | other files use an empty token set; test (Tasks 4, 8) |
| I1 re-run collides with its own entries; differing target overwritten | `key_paths` with paths; a differing target is refused (Tasks 8, 9) |
| I2 nested possible records; sections keyed by first line | `Possible.enclosing`, `possibles_inside`; sections keyed by (source, start) (Tasks 2, 5) |
| I3 token extraction, `TokenExtra`, item 6 scope | `heading_token` with suffix; every collection file; every working-tree `.md` (Tasks 2, 10) |
| I4 hot-view item cannot fail | `load_collection` + tree problems + `hot_view`; ambiguous-key test (Task 10) |
| I5 existing mode needs bodies; late-record order | `ExistingEntry` with text; merge committed first (Tasks 8, 9) |
| I6 `files`, `shorten`, `columns`, M5, readme ranges | `fixed_stem`; `shortened`; columns step; `stale_after` required; `ReadmeMissing` (Tasks 1, 5, 8, 10) |
| I7 invalid test commands | `cargo test --lib -- a b` |
| I8 inventory types | every field given (Task 5) |
| I9 index-table scope | non-record sections only, fences skipped, `index_table` (Tasks 3, 5) |
| Minors | `in_url_word` unchanged; title without token; schema conversion; label kept; commit-offset dates; `resolve_old_catalog`; possible-record regex anchored at heading start, 1–3 letters; delete guards; module renamed; async `run` |
