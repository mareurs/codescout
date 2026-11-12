---
id: '5f4d250e4bc826b5'
kind: plan
status: done
title: Tracker Lifecycle Plan L-A — Lifecycle Core
tags:
- trackers
- lifecycle
- librarian
- plan
---

# Tracker Lifecycle Plan L-A — Lifecycle Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the pure core of the tracker lifecycle model — where a file sits in the lifecycle tree, the collection schema, entry metadata, relation lines, entry naming, the eight lifecycle checks and the rendered entry table — all tested, with no change to any live tool.

**Architecture:** Plan L-A of five (L-A core, L-B tools, L-C consumers, L-D guide and skills, L-E migration). New code under `src/librarian/lifecycle/`, beside the old system (strangler), built on identity Plan A's pure core (`src/librarian/identity/`). Every function takes text and dates and returns values; nothing reads the catalog, the filesystem or git, except the ignored census test in Task 7.

**Tech Stack:** Rust 2021, `chrono` 0.4 (`NaiveDate`), `serde_json` (frontmatter `extra` values), `crate::librarian::frontmatter::parse`, `crate::librarian::statements::parse_validity`, identity Plan A (`identity::key`, `identity::grammar`).

**Spec:** `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md` (approved, `42bc25753`). Read § D1–D4 before starting. Identity spec: `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` (v5).

**Precondition:** identity Plan A (`docs/superpowers/plans/2026-11-11-artifact-identity-plan-a-identity-and-grammar.md`) Task 2 (`repo_name`), Task 4 (`key`) and Task 5 (`grammar`) are committed, and this plan's branch is created from the Task 5 commit (which also carries Task 1, the removal of the `librarian` cargo feature). This plan changes none of Task 1's files. It uses `Key`, `KeyError`, `key_of`, `is_folder_index`, `Reference` and `extract` exactly as that plan's Interfaces blocks define them.

## Global Constraints

- Tracker root: `docs/trackers`. Lifecycle folders directly under it: `ledgers`, `streams`, `temp` — no others. Archive folder name: `archive`.
- A collection is a folder directly under a lifecycle folder (or under `<lifecycle>/archive/`) holding a folder-index file `readme.md`, matched case-insensitively (identity Plan A `is_folder_index`). Its key is the folder name.
- An entry is any other `.md` file below a collection folder. Grouping subfolders (for example `reports/`) are allowed; a subfolder that holds its own `readme.md` is a nested collection and is refused. A file under `archive/` at any depth below the collection is archived. Non-`.md` files are not documents and are ignored.
- Key charset (identity): `^[a-z0-9]+(-[a-z0-9]+)*$`; a 16-hex value is refused.
- Status classes, exactly four: `open`, `blocked`, `deferred`, `terminal`. `blocked` and `deferred` require `revisit_by`; `terminal` requires `closed`; a non-terminal status must not carry `closed`.
- `exit_when` is required under `streams` and `temp` and refused under `ledgers`.
- Entry frontmatter holds scalars and lists of scalars only. Relations are body lines of the form `**<Label>:** … [[…]] …`.
- Dates are ISO `YYYY-MM-DD`, checked by shape AND by `chrono::NaiveDate` (the rule in `src/librarian/statements.rs` `parse_validity`: `dated 2026-02-30` is an error, not a value).
- No check reads a git commit date. Every check reads frontmatter dates and statuses only.
- No existing tool, catalog table, indexer path, `doctor` check or guide changes in this plan.
- Before completing any task: `./scripts/gate.sh` green (CLAUDE.md § Development Commands). Targeted runs: `scripts/with-slot.sh cargo test --lib <path>`.
- Commit by pathspec with `git commit -F <msgfile> -- <paths>`, following `docs/conventions/shared-checkout-commit-sequence.md`; never a bare `git commit`.

## Review Focus

1. **A collection readme uses an uppercase name (`README.md`) or sits in an archived stream (`streams/archive/x/readme.md`)** — expected: recognized as that collection's readme, archived in the second case. Test in Task 1.
2. **Dates written unquoted (`created: 2026-11-11`), quoted, or impossible (`2026-02-30`)** — expected: the first two parse to the same date; the third is an `EntryError::BadDate` naming the field. Test in Task 3.
3. **An entry reopened by hand: `status: open` with `closed:` still set** — expected: `EntryProblem::ClosedOnNonTerminal`, never silently treated as terminal by a check. Test in Task 3.
4. **A title with no ASCII letters or digits (`???`, `Ünïcode`)** — expected: `slug` returns `None`, and `entry_stem` refuses; no empty or `-` key is produced. Test in Task 5.
5. **A relation line whose link sits inside inline code (`**Rests on:** \`[[x]]\``)** — expected: no relation (code is never parsed, identity grammar). Test in Task 4.

---

### Task 1: Lifecycle layout and placement

**Files:**
- Create: `src/librarian/lifecycle/mod.rs`, `src/librarian/lifecycle/layout.rs`
- Modify: `src/librarian/mod.rs` (add `pub mod lifecycle;`)
- Test: `src/librarian/lifecycle/layout.rs`

**Interfaces:**
- Consumes: `identity::key::{Key, KeyError, key_of, is_folder_index}`.
- Produces:
  - `pub const TRACKERS_DIR: &str = "docs/trackers";`, `pub const ARCHIVE_DIR: &str = "archive";`
  - `pub enum Lifecycle { Ledger, Stream, Temp }` — `Clone, Copy, Debug, PartialEq, Eq, Hash`; `pub fn dir_name(self) -> &'static str` (`"ledgers"`, `"streams"`, `"temp"`); `pub fn from_dir(s: &str) -> Option<Lifecycle>`; `pub fn requires_exit(self) -> bool` (true for `Stream`, `Temp`).
  - `pub enum Role { CollectionReadme, Entry }`.
  - `pub struct Placement { pub lifecycle: Lifecycle, pub collection: Key, pub collection_dir: String, pub role: Role, pub archived: bool }` — `collection_dir` is the repo-relative folder of the collection (`docs/trackers/ledgers/frictions` or `docs/trackers/streams/archive/x`); `archived` is true when the collection itself is under `<lifecycle>/archive/` or the entry is under an `archive/` folder below the collection.
  - `pub enum PlacementError { NotMarkdown(String), OutsideCollection(String), NestedCollection(String), BadKey(KeyError) }`.
  - `pub fn place(rel_path: &str) -> Option<Result<Placement, PlacementError>>` — `None` when the path is not below `docs/trackers/<lifecycle>/` (legacy flat trackers, other docs). Normalizes `\` to `/`.
  - `pub fn has_nested_readme(rel_path: &str, readme_dirs: &std::collections::BTreeSet<String>) -> bool` — true when a folder strictly between the collection folder and the file is in `readme_dirs`; `place` cannot see sibling files, so the caller (Task 6) passes the set.

- [ ] **Step 1: Write the failing tests**

```rust
fn p(path: &str) -> Placement { place(path).expect("in tree").expect("valid") }

#[test]
fn readmes_and_entries_in_each_lifecycle() {
    let r = p("docs/trackers/ledgers/frictions/readme.md");
    assert_eq!((r.lifecycle, r.collection.as_str(), r.role, r.archived), (Lifecycle::Ledger, "frictions", Role::CollectionReadme, false));
    let e = p("docs/trackers/streams/tracker-organization/2026-11-11-a-friction.md");
    assert_eq!((e.lifecycle, e.collection.as_str(), e.role), (Lifecycle::Stream, "tracker-organization", Role::Entry));
    assert_eq!(e.collection_dir, "docs/trackers/streams/tracker-organization");
    assert_eq!(p("docs/trackers/temp/x/README.md").role, Role::CollectionReadme);
    assert_eq!(p("docs/trackers/ledgers/frictions/reports/2026-01-01-r.md").role, Role::Entry);
}

#[test]
fn archived_entries_and_archived_collections() {
    assert!(p("docs/trackers/ledgers/frictions/archive/2026-01-01-old.md").archived);
    let s = p("docs/trackers/streams/archive/local-onnx/readme.md");
    assert_eq!((s.collection.as_str(), s.role, s.archived), ("local-onnx", Role::CollectionReadme, true));
    assert_eq!(s.collection_dir, "docs/trackers/streams/archive/local-onnx");
    assert!(p("docs/trackers/streams/archive/local-onnx/archive/2026-01-01-e.md").archived);
}

#[test]
fn outside_the_tree_is_none_and_misplaced_files_are_errors() {
    assert!(place("docs/trackers/bug-fix-session-log.md").is_none());
    assert!(place("docs/trackers/archive/old.md").is_none());
    assert!(place("docs/issues/2026-01-01-x.md").is_none());
    assert!(matches!(place("docs/trackers/ledgers/loose.md"), Some(Err(PlacementError::OutsideCollection(_)))));
    assert!(matches!(place("docs/trackers/ledgers/archive/loose.md"), Some(Err(PlacementError::OutsideCollection(_)))));
    assert!(matches!(place("docs/trackers/ledgers/Bad_Name/readme.md"), Some(Err(PlacementError::BadKey(_)))));
    assert!(matches!(place("docs/trackers/ledgers/x/data.json"), Some(Err(PlacementError::NotMarkdown(_)))));
    assert_eq!(p(r"docs\trackers\ledgers\frictions\readme.md").collection.as_str(), "frictions");
}

#[test]
fn a_subfolder_with_its_own_readme_is_a_nested_collection() {
    let dirs: std::collections::BTreeSet<String> = ["docs/trackers/ledgers/frictions".into(), "docs/trackers/ledgers/frictions/reports".into()].into();
    assert!(has_nested_readme("docs/trackers/ledgers/frictions/reports/2026-01-01-r.md", &dirs));
    assert!(!has_nested_readme("docs/trackers/ledgers/frictions/2026-01-01-r.md", &dirs));
}
```

- [ ] **Step 2: Run to verify failure** — `scripts/with-slot.sh cargo test --lib librarian::lifecycle::layout` → FAIL to compile.
- [ ] **Step 3: Implement `layout.rs`.** Split the normalized path on `/`; require the prefix `docs/trackers/<lifecycle>/`; if the next segment is `archive`, the collection is the segment after it and `archived = true`; the collection key is `Key::parse(<segment>)`; any `archive` segment below the collection folder sets `archived`. `NotMarkdown` is checked before role.
- [ ] **Step 4: Run to verify pass** — same command → PASS, 4 tests.
- [ ] **Step 5: Commit** — pathspec commit of `lifecycle/mod.rs`, `lifecycle/layout.rs`, `librarian/mod.rs`: `feat(librarian): lifecycle layout and placement of tracker files`.

---

### Task 2: Collection schema

**Files:**
- Create: `src/librarian/lifecycle/schema.rs`
- Modify: `src/librarian/lifecycle/mod.rs` (add `pub mod schema;`)
- Test: `src/librarian/lifecycle/schema.rs`

**Interfaces:**
- Consumes: `layout::Lifecycle` (Task 1), `crate::librarian::frontmatter::Frontmatter`.
- Produces:
  - `pub enum StatusClass { Open, Blocked, Deferred, Terminal }` — `Clone, Copy, Debug, PartialEq, Eq, Hash`; `pub fn parse(s: &str) -> Option<StatusClass>` (`"open"`, `"blocked"`, `"deferred"`, `"terminal"`); `pub fn needs_revisit(self) -> bool`.
  - `pub enum KeyStyle { Dated, Name }` (`"dated"` default when absent, `"name"`).
  - `pub enum FieldType { Scalar, Enum(Vec<String>), List }`.
  - `pub struct CollectionSchema { pub title: String, pub entry_kinds: Vec<String>, pub statuses: std::collections::BTreeMap<String, StatusClass>, pub initial_status: String, pub key_style: KeyStyle, pub fields: std::collections::BTreeMap<String, FieldType>, pub owner: Option<String>, pub exit_when: Option<String>, pub writer: Option<String>, pub review_every_days: Option<u32> }`.
  - `pub enum SchemaError { MissingTitle, MissingKey(&'static str), Malformed { key: &'static str, detail: String }, UnknownClass { word: String, class: String }, NoTerminalStatus, InitialNotOpen(String), InitialUndeclared(String), ExitRequired, ExitRefused }`.
  - `pub fn parse_schema(lifecycle: Lifecycle, fm: &Frontmatter) -> Result<CollectionSchema, Vec<SchemaError>>` — reads `title` from the typed field and `entry_kinds`, `statuses` (a mapping word → class), `initial_status`, `key_style`, `fields` (mapping name → `{type: scalar|enum|list, values: [..]}`), `owner`, `exit_when`, `writer`, `review_every` (`"<n>d"`) from `fm.extra`. Collects every error, not only the first.

**Plan decision (not in the spec):** `initial_status` is a required schema key. The spec says a new entry's status defaults to "the first `open`-class word", but `Frontmatter::extra` is a `BTreeMap` of `serde_json::Value`, which does not keep YAML mapping order, so "first" is not observable. An explicit key is the deterministic form.

- [ ] **Step 1: Write the failing tests**

```rust
fn fm(yaml: &str) -> Frontmatter { crate::librarian::frontmatter::parse(&format!("---\n{yaml}---\n")).unwrap().0.unwrap() }
const LEDGER: &str = "title: Frictions\nentry_kinds: [friction]\nstatuses: {open: open, parked: deferred, fixed: terminal}\ninitial_status: open\nkey_style: dated\nfields:\n  severity: {type: enum, values: [low, med, high]}\n  covers: {type: list}\nreview_every: 30d\n";

#[test]
fn a_full_ledger_schema_parses() {
    let s = parse_schema(Lifecycle::Ledger, &fm(LEDGER)).unwrap();
    assert_eq!(s.statuses.get("parked"), Some(&StatusClass::Deferred));
    assert_eq!(s.initial_status, "open");
    assert_eq!(s.key_style, KeyStyle::Dated);
    assert_eq!(s.fields.get("severity"), Some(&FieldType::Enum(vec!["low".into(), "med".into(), "high".into()])));
    assert_eq!(s.fields.get("covers"), Some(&FieldType::List));
    assert_eq!(s.review_every_days, Some(30));
}

#[test]
fn exit_when_is_required_for_streams_and_temp_and_refused_for_ledgers() {
    let stream = "title: S\nentry_kinds: [friction]\nstatuses: {open: open, done: terminal}\ninitial_status: open\n";
    assert!(parse_schema(Lifecycle::Stream, &fm(stream)).unwrap_err().contains(&SchemaError::ExitRequired));
    assert!(parse_schema(Lifecycle::Temp, &fm(stream)).unwrap_err().contains(&SchemaError::ExitRequired));
    let with_exit = format!("{stream}exit_when: the design ships\n");
    assert!(parse_schema(Lifecycle::Stream, &fm(&with_exit)).is_ok());
    assert!(parse_schema(Lifecycle::Ledger, &fm(&with_exit)).unwrap_err().contains(&SchemaError::ExitRefused));
}

#[test]
fn every_error_is_reported_with_the_offending_word() {
    let bad = "entry_kinds: []\nstatuses: {open: open, later: someday}\ninitial_status: shipped\nreview_every: monthly\n";
    let errs = parse_schema(Lifecycle::Ledger, &fm(bad)).unwrap_err();
    assert!(errs.contains(&SchemaError::MissingTitle));
    assert!(errs.contains(&SchemaError::UnknownClass { word: "later".into(), class: "someday".into() }));
    assert!(errs.contains(&SchemaError::NoTerminalStatus));
    assert!(errs.contains(&SchemaError::InitialUndeclared("shipped".into())));
    assert!(errs.iter().any(|e| matches!(e, SchemaError::Malformed { key: "review_every", .. })));
    assert!(errs.iter().any(|e| matches!(e, SchemaError::Malformed { key: "entry_kinds", .. })));
}

#[test]
fn the_initial_status_must_be_open_class() {
    let y = "title: T\nentry_kinds: [k]\nstatuses: {open: open, done: terminal}\ninitial_status: done\n";
    assert!(parse_schema(Lifecycle::Ledger, &fm(y)).unwrap_err().contains(&SchemaError::InitialNotOpen("done".into())));
}
```

- [ ] **Step 2: Run to verify failure** — `scripts/with-slot.sh cargo test --lib librarian::lifecycle::schema` → FAIL to compile.
- [ ] **Step 3: Implement `schema.rs`** with the interfaces above. `SchemaError` derives `Debug, PartialEq, Eq`. An empty `entry_kinds` list is `Malformed { key: "entry_kinds", .. }`.
- [ ] **Step 4: Run to verify pass** — same command → PASS, 4 tests.
- [ ] **Step 5: Commit** — pathspec commit: `feat(librarian): collection schema with status classes and lifecycle exit rules`.

---

### Task 3: Entry metadata

**Files:**
- Create: `src/librarian/lifecycle/entry.rs`
- Modify: `src/librarian/lifecycle/mod.rs` (add `pub mod entry;`)
- Test: `src/librarian/lifecycle/entry.rs`

**Interfaces:**
- Consumes: `schema::{CollectionSchema, StatusClass, FieldType}` (Task 2), `Frontmatter`, `crate::librarian::statements::{parse_validity, Validity}`.
- Produces:
  - `pub fn parse_date(s: &str) -> Option<chrono::NaiveDate>` — shape `^\d{4}-\d{2}-\d{2}$` AND `NaiveDate::parse_from_str(s, "%Y-%m-%d")`.
  - `pub struct EntryMeta { pub entry_kind: String, pub title: String, pub status: String, pub status_since: NaiveDate, pub created: NaiveDate, pub closed: Option<NaiveDate>, pub revisit_by: Option<NaiveDate>, pub valid: Option<Validity>, pub tags: Vec<String>, pub fields: std::collections::BTreeMap<String, serde_json::Value> }` — `fields` holds every `extra` key that is not one of `entry_kind`, `status_since`, `created`, `closed`, `revisit_by`, `valid`.
  - `pub enum EntryError { Missing(&'static str), BadDate { field: &'static str, value: String }, BadValidity(String) }`.
  - `pub fn parse_entry(fm: &Frontmatter) -> Result<EntryMeta, Vec<EntryError>>` — required: `entry_kind`, `title`, `status`, `status_since`, `created`. A date may arrive as a YAML string or as any scalar `serde_json::Value`; both are read through `parse_date` on its string form. `valid` is parsed by `parse_validity(&format!("**Valid:** {v}"))`.
  - `pub enum EntryProblem { KindUndeclared(String), StatusUndeclared(String), RevisitMissing { status: String }, ClosedMissing { status: String }, ClosedOnNonTerminal { status: String }, FieldUndeclared(String), FieldType { field: String, expected: FieldType } }`.
  - `pub fn class_of(schema: &CollectionSchema, meta: &EntryMeta) -> Option<StatusClass>`.
  - `pub fn validate_entry(schema: &CollectionSchema, meta: &EntryMeta) -> Vec<EntryProblem>`.

- [ ] **Step 1: Write the failing tests**

```rust
fn fm(yaml: &str) -> Frontmatter { crate::librarian::frontmatter::parse(&format!("---\n{yaml}---\n")).unwrap().0.unwrap() }
fn schema() -> CollectionSchema {
    let y = "title: F\nentry_kinds: [friction]\nstatuses: {open: open, waiting: blocked, fixed: terminal}\ninitial_status: open\nfields:\n  severity: {type: enum, values: [low, high]}\n";
    super::schema::parse_schema(super::layout::Lifecycle::Ledger, &fm(y)).unwrap()
}
const BASE: &str = "entry_kind: friction\ntitle: T\nstatus_since: 2026-11-11\ncreated: 2026-11-10\n";

#[test]
fn quoted_and_unquoted_dates_parse_the_same_and_impossible_dates_are_errors() {
    let a = parse_entry(&fm(&format!("{BASE}status: open\n"))).unwrap();
    let b = parse_entry(&fm("entry_kind: friction\ntitle: T\nstatus: open\nstatus_since: '2026-11-11'\ncreated: \"2026-11-10\"\n")).unwrap();
    assert_eq!((a.created, a.status_since), (b.created, b.status_since));
    let bad = parse_entry(&fm("entry_kind: friction\ntitle: T\nstatus: open\nstatus_since: 2026-02-30\ncreated: 2026-11-10\n")).unwrap_err();
    assert!(bad.contains(&EntryError::BadDate { field: "status_since", value: "2026-02-30".into() }));
}

#[test]
fn missing_required_fields_are_all_reported() {
    let errs = parse_entry(&fm("title: T\n")).unwrap_err();
    for f in ["entry_kind", "status", "status_since", "created"] { assert!(errs.contains(&EntryError::Missing(f)), "{f}"); }
}

#[test]
fn class_rules_and_a_hand_reopened_entry() {
    let s = schema();
    let waiting = parse_entry(&fm(&format!("{BASE}status: waiting\n"))).unwrap();
    assert!(validate_entry(&s, &waiting).contains(&EntryProblem::RevisitMissing { status: "waiting".into() }));
    let fixed = parse_entry(&fm(&format!("{BASE}status: fixed\n"))).unwrap();
    assert!(validate_entry(&s, &fixed).contains(&EntryProblem::ClosedMissing { status: "fixed".into() }));
    let reopened = parse_entry(&fm(&format!("{BASE}status: open\nclosed: 2026-11-11\n"))).unwrap();
    assert_eq!(validate_entry(&s, &reopened), vec![EntryProblem::ClosedOnNonTerminal { status: "open".into() }]);
    assert_eq!(class_of(&s, &reopened), Some(StatusClass::Open));
}

#[test]
fn undeclared_words_and_typed_fields() {
    let s = schema();
    let e = parse_entry(&fm(&format!("entry_kind: win\ntitle: T\nstatus: shipped\nstatus_since: 2026-11-11\ncreated: 2026-11-10\nseverity: medium\nowner_hint: x\n"))).unwrap();
    let probs = validate_entry(&s, &e);
    assert!(probs.contains(&EntryProblem::KindUndeclared("win".into())));
    assert!(probs.contains(&EntryProblem::StatusUndeclared("shipped".into())));
    assert!(probs.iter().any(|p| matches!(p, EntryProblem::FieldType { field, .. } if field == "severity")));
    assert!(probs.contains(&EntryProblem::FieldUndeclared("owner_hint".into())));
}
```

- [ ] **Step 2: Run to verify failure** — `scripts/with-slot.sh cargo test --lib librarian::lifecycle::entry` → FAIL to compile.
- [ ] **Step 3: Implement `entry.rs`.** `Enum` accepts a string in `values`; `List` accepts an array of scalars; `Scalar` accepts a string, number or bool. A status with no class gives `StatusUndeclared` and no class-rule problems.
- [ ] **Step 4: Run to verify pass** — same command → PASS, 4 tests.
- [ ] **Step 5: Commit** — pathspec commit: `feat(librarian): entry metadata and its validation against the collection schema`.

---

### Task 4: Relation lines

**Files:**
- Create: `src/librarian/lifecycle/relations.rs`
- Modify: `src/librarian/lifecycle/mod.rs` (add `pub mod relations;`)
- Test: `src/librarian/lifecycle/relations.rs`

**Interfaces:**
- Consumes: `identity::grammar::{extract, Citation, Reference}`.
- Produces:
  - `pub struct Relation { pub label: String, pub reference: Reference, pub line: u32 }` — `Clone, Debug, PartialEq, Eq`.
  - `pub fn relations(doc: &str) -> Vec<Relation>` — every `Reference::File` or `Reference::Entry` citation from `extract(doc)` whose source line starts (after leading whitespace) with `**<Label>:**`, where `<Label>` matches `[A-Z][A-Za-z ]*[a-z]`. The label is that text, trimmed. Citations on other lines are not relations. Line numbers are those of `extract`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn labelled_lines_give_relations_and_other_lines_do_not() {
    let doc = "---\ntitle: x\n---\n**Rests on:** [[2026-08-27-negative-results]] and [[adr-b]]\n\nSee [[not-a-relation]].\n**Promoted to:** [[declared-not-wired]]\n";
    let rs = relations(doc);
    let got: Vec<(&str, String)> = rs.iter().map(|r| (r.label.as_str(), format!("{:?}", r.reference))).collect();
    assert_eq!(rs.len(), 3, "{got:?}");
    assert_eq!(rs[0].label, "Rests on");
    assert_eq!(rs[2].label, "Promoted to");
    assert_eq!(rs[2].line, 7);
}

#[test]
fn code_and_prose_labels_are_not_relations() {
    assert!(relations("**Rests on:** `[[x-doc]]`\n").is_empty());
    assert!(relations("```\n**Rests on:** [[x-doc]]\n```\n").is_empty());
    assert!(relations("Note: **rests on:** [[x-doc]]\n").is_empty());
}
```

- [ ] **Step 2: Run to verify failure** — `scripts/with-slot.sh cargo test --lib librarian::lifecycle::relations` → FAIL to compile.
- [ ] **Step 3: Implement `relations.rs`.** Build a `line → label` map from the raw lines with the label regex, then keep the `extract` citations whose line is in it.
- [ ] **Step 4: Run to verify pass** — same command → PASS, 2 tests.
- [ ] **Step 5: Commit** — pathspec commit: `feat(librarian): relation lines read through the closed citation grammar`.

---

### Task 5: Entry naming and archive paths

**Files:**
- Create: `src/librarian/lifecycle/naming.rs`
- Modify: `src/librarian/lifecycle/mod.rs` (add `pub mod naming;`)
- Test: `src/librarian/lifecycle/naming.rs`

**Interfaces:**
- Consumes: `layout::{Placement, Role, Lifecycle, TRACKERS_DIR, ARCHIVE_DIR}` (Task 1), `schema::KeyStyle` (Task 2), `identity::key::{Key, KeyError}`.
- Produces:
  - `pub const SLUG_MAX: usize = 60;`
  - `pub fn slug(title: &str) -> Option<String>` — ASCII-lowercase; every run of characters outside `[a-z0-9]` becomes one `-`; trimmed of `-`; cut to at most `SLUG_MAX` bytes at a `-` boundary when one exists; `None` when nothing is left.
  - `pub fn entry_stem(style: KeyStyle, created: chrono::NaiveDate, slug: &str) -> Result<Key, KeyError>` — `Dated`: `<YYYY-MM-DD>-<slug>`; `Name`: `<slug>`; validated with `Key::parse`.
  - `pub fn entry_rel_path(collection_dir: &str, stem: &Key) -> String` — `<collection_dir>/<stem>.md`.
  - `pub fn archive_rel_path(rel_path: &str, placement: &Placement) -> Option<String>` — for an un-archived `Entry`: `<collection_dir>/archive/<file name>` (grouping subfolders flatten into `archive/`); for an un-archived `CollectionReadme`: the collection folder's destination `docs/trackers/<lifecycle>/archive/<collection>`; `None` when already archived.

- [ ] **Step 1: Write the failing tests**

```rust
use chrono::NaiveDate;
fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

#[test]
fn slugs_are_canonical_or_absent() {
    assert_eq!(slug("frictions/ is NOT one file per entry!").as_deref(), Some("frictions-is-not-one-file-per-entry"));
    assert_eq!(slug("???"), None);
    assert_eq!(slug("Ünïcode"), Some("n-code".into()));
    assert_eq!(slug("ÜÏ"), None);
    let long = slug(&"word ".repeat(30)).unwrap();
    assert!(long.len() <= SLUG_MAX && !long.ends_with('-'));
}

#[test]
fn stems_follow_the_key_style() {
    assert_eq!(entry_stem(KeyStyle::Dated, d("2026-11-11"), "a-friction").unwrap().as_str(), "2026-11-11-a-friction");
    assert_eq!(entry_stem(KeyStyle::Name, d("2026-11-11"), "declared-not-wired").unwrap().as_str(), "declared-not-wired");
    assert!(entry_stem(KeyStyle::Name, d("2026-11-11"), "deadbeefcafe1234").is_err());
}

#[test]
fn archive_paths_are_the_nearest_sibling_archive() {
    let e = super::layout::place("docs/trackers/ledgers/frictions/reports/2026-01-01-r.md").unwrap().unwrap();
    assert_eq!(archive_rel_path("docs/trackers/ledgers/frictions/reports/2026-01-01-r.md", &e).as_deref(),
        Some("docs/trackers/ledgers/frictions/archive/2026-01-01-r.md"));
    let c = super::layout::place("docs/trackers/streams/x/readme.md").unwrap().unwrap();
    assert_eq!(archive_rel_path("docs/trackers/streams/x/readme.md", &c).as_deref(), Some("docs/trackers/streams/archive/x"));
    let done = super::layout::place("docs/trackers/streams/archive/x/readme.md").unwrap().unwrap();
    assert_eq!(archive_rel_path("docs/trackers/streams/archive/x/readme.md", &done), None);
}
```

- [ ] **Step 2: Run to verify failure** — `scripts/with-slot.sh cargo test --lib librarian::lifecycle::naming` → FAIL to compile.
- [ ] **Step 3: Implement `naming.rs`** with the interfaces above.
- [ ] **Step 4: Run to verify pass** — same command → PASS, 3 tests.
- [ ] **Step 5: Commit** — pathspec commit: `feat(librarian): entry stems from title and key style, nearest-sibling archive paths`.

---

### Task 6: The lifecycle tree and its checks

**Files:**
- Create: `src/librarian/lifecycle/tree.rs`, `src/librarian/lifecycle/checks.rs`
- Modify: `src/librarian/lifecycle/mod.rs` (add `pub mod tree; pub mod checks;`)
- Test: `src/librarian/lifecycle/checks.rs`

**Interfaces:**
- Consumes: Tasks 1–5; `identity::key::key_of`; `crate::librarian::frontmatter::parse`.
- Produces (`tree.rs`):
  - `pub struct DocInput { pub rel_path: String, pub text: String }` (same shape as identity Plan A's; a separate type so this module does not depend on `identity::index`).
  - `pub struct EntryNode { pub rel_path: String, pub key: Key, pub archived: bool, pub meta: EntryMeta, pub relations: Vec<Relation> }`.
  - `pub struct CollectionNode { pub key: Key, pub lifecycle: Lifecycle, pub dir: String, pub archived: bool, pub schema: CollectionSchema, pub entries: Vec<EntryNode> }`.
  - `pub enum TreeProblem { Placement(PlacementError), NestedCollection(String), OrphanEntry(String), Schema { collection: Key, errors: Vec<SchemaError> } *(shipped as `Schema { rel_path, errors }`; a second readme in one folder is `DuplicateReadme(String)`, see the lifecycle cleanup plan)*, Entry { rel_path: String, errors: Vec<EntryError> }, Frontmatter { rel_path: String, detail: String } }`.
  - `pub struct LifecycleTree { pub collections: Vec<CollectionNode>, pub problems: Vec<TreeProblem>, pub closed_dates: std::collections::BTreeMap<Key, chrono::NaiveDate> }` with `pub fn build(docs: impl IntoIterator<Item = DocInput>) -> LifecycleTree`. `closed_dates` holds the frontmatter `closed:` date of EVERY input document that has one (lifecycle entries and others, for example bug files), keyed by `key_of`. An entry with no collection readme above it is `OrphanEntry`; a collection whose schema fails is reported once and its entries are skipped.
- Produces (`checks.rs`):
  - `pub struct CheckPolicy { pub terminal_grace_days: u32 }` with `Default` = `14`. *(Superseded: the shipped default is 30 days, measured 2026-11-11 as W-3 in `tracker-organization-session-log`.)*
  - `pub enum Finding { EntryInvalid { entry: Key, problem: EntryProblem }, RevisitDue { entry: Key, revisit_by: NaiveDate }, CitedTargetClosed { entry: Key, target: Key, closed: NaiveDate }, StreamAllTerminal { collection: Key }, StreamWithoutExit { collection: Key }, LedgerQuiet { collection: Key, last_activity: Option<NaiveDate> }, TerminalInLiveDir { entry: Key, closed: NaiveDate }, OrphanEntry { rel_path: String } }`.
  - `pub fn check(tree: &LifecycleTree, today: NaiveDate, policy: &CheckPolicy) -> Vec<Finding>` — rules:
    - `EntryInvalid`: one finding per problem that `validate_entry` returns, for EVERY entry, archived or not (spec: `entry_invalid`, which generalises the former `status_undeclared`; ruling of 2026-11-11). `EntryProblem` must derive `Clone`.
    - `RevisitDue`: class `Blocked`/`Deferred` and `revisit_by < today`.
    - `CitedTargetClosed`: class not `Terminal`, a relation targets key `t` (File or Entry form), `closed_dates[t] > meta.status_since`.
    - `StreamAllTerminal`: un-archived `Stream`/`Temp` collection with at least one entry, every entry `Terminal`.
    - `StreamWithoutExit`: never fires for a parsed schema (Task 2 already refuses it); it is emitted from `TreeProblem::Schema` containing `ExitRequired`, so `doctor` has one finding list.
    - `LedgerQuiet`: `Ledger` with `review_every_days = n`, and the latest of all entries' `created` and `closed` is older than `today - n` days, or there are no entries.
    - `TerminalInLiveDir`: un-archived entry, class `Terminal`, `closed + grace < today`.
    - `OrphanEntry`: from `TreeProblem::OrphanEntry`.

- [ ] **Step 1: Write the failing tests**

```rust
use chrono::NaiveDate;
fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }
fn doc(p: &str, t: &str) -> DocInput { DocInput { rel_path: p.into(), text: t.into() } }
const LEDGER: &str = "---\ntitle: L\nentry_kinds: [f]\nstatuses: {open: open, waiting: blocked, fixed: terminal}\ninitial_status: open\nreview_every: 30d\n---\n";
const STREAM: &str = "---\ntitle: S\nentry_kinds: [f]\nstatuses: {open: open, done: terminal}\ninitial_status: open\nexit_when: it ships\n---\n";
fn entry(status: &str, extra: &str, body: &str) -> String {
    format!("---\nentry_kind: f\ntitle: t\nstatus: {status}\nstatus_since: 2026-10-01\ncreated: 2026-10-01\n{extra}---\n{body}")
}
fn tree() -> LifecycleTree {
    LifecycleTree::build([
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc("docs/trackers/ledgers/l/2026-10-01-cites-a-fixed-bug.md", &entry("open", "", "**Rests on:** [[2026-10-05-the-bug]]\n")),
        doc("docs/trackers/ledgers/l/2026-10-01-waits.md", &entry("waiting", "revisit_by: 2026-11-01\n", "")),
        doc("docs/trackers/ledgers/l/2026-10-01-fixed-long-ago.md", &entry("fixed", "closed: 2026-10-02\n", "")),
        doc("docs/trackers/ledgers/l/archive/2026-10-01-archived-fixed.md", &entry("fixed", "closed: 2026-10-02\n", "")),
        doc("docs/issues/2026-10-05-the-bug.md", "---\nkind: bug\nstatus: fixed\nclosed: 2026-10-06\n---\n"),
        doc("docs/trackers/streams/s/readme.md", STREAM),
        doc("docs/trackers/streams/s/2026-10-01-done.md", &entry("done", "closed: 2026-11-10\n", "")),
        doc("docs/trackers/streams/no-readme/2026-10-01-lost.md", &entry("open", "", "")),
    ])
}

#[test]
fn each_check_fires_on_its_case() {
    let f = check(&tree(), d("2026-11-11"), &CheckPolicy::default());
    assert!(f.contains(&Finding::CitedTargetClosed { entry: k("2026-10-01-cites-a-fixed-bug"), target: k("2026-10-05-the-bug"), closed: d("2026-10-06") }));
    assert!(f.contains(&Finding::RevisitDue { entry: k("2026-10-01-waits"), revisit_by: d("2026-11-01") }));
    assert!(f.contains(&Finding::TerminalInLiveDir { entry: k("2026-10-01-fixed-long-ago"), closed: d("2026-10-02") }));
    assert!(!f.iter().any(|x| matches!(x, Finding::TerminalInLiveDir { entry, .. } if entry.as_str() == "2026-10-01-archived-fixed")));
    assert!(f.contains(&Finding::StreamAllTerminal { collection: k("s") }));
    assert!(f.contains(&Finding::OrphanEntry { rel_path: "docs/trackers/streams/no-readme/2026-10-01-lost.md".into() }));
    assert!(f.contains(&Finding::LedgerQuiet { collection: k("l"), last_activity: Some(d("2026-10-02")) }));
}
fn k(s: &str) -> Key { Key::parse(s).unwrap() }

#[test]
fn reconfirming_an_open_entry_silences_cited_target_closed() {
    let mut docs: Vec<DocInput> = vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc("docs/trackers/ledgers/l/2026-10-01-e.md", &entry("open", "", "**Rests on:** [[2026-10-05-the-bug]]\n").replace("status_since: 2026-10-01", "status_since: 2026-10-07")),
        doc("docs/issues/2026-10-05-the-bug.md", "---\nkind: bug\nstatus: fixed\nclosed: 2026-10-06\n---\n"),
    ];
    let f = check(&LifecycleTree::build(docs.drain(..)), d("2026-11-11"), &CheckPolicy::default());
    assert!(!f.iter().any(|x| matches!(x, Finding::CitedTargetClosed { .. })));
}
```

Anti-goal 2 (no git date) is held by the types, not by a test: `DocInput` has no date field and `check` takes `today` as its only clock, so no code path can read a commit date. A test that touched the body and compared findings could not fail, so it is not written.

- [ ] **Step 2: Run to verify failure** — `scripts/with-slot.sh cargo test --lib librarian::lifecycle::checks` → FAIL to compile.
- [ ] **Step 3: Implement `tree.rs` and `checks.rs`** with the interfaces and rules above. `Finding` derives `Debug, PartialEq, Eq`; `check` returns findings sorted by (variant order above, key) so the output is deterministic.
- [ ] **Step 4: Run to verify pass** — same command → PASS, 2 tests.
- [ ] **Step 5: Commit** — pathspec commit: `feat(librarian): lifecycle tree and the eight field-based lifecycle checks`.

---

### Task 7: The rendered entry table, and a census of this repo

**Files:**
- Create: `src/librarian/lifecycle/table.rs`
- Modify: `src/librarian/lifecycle/mod.rs` (add `pub mod table;`)
- Test: `src/librarian/lifecycle/table.rs`

**Interfaces:**
- Consumes: Task 6 (`LifecycleTree`, `CollectionNode`, `EntryNode`), `entry::class_of`.
- Produces:
  - `pub struct EntryRow { pub key: Key, pub entry_kind: String, pub title: String, pub status: String, pub class: Option<StatusClass>, pub created: NaiveDate, pub closed: Option<NaiveDate>, pub revisit_by: Option<NaiveDate>, pub archived: bool, pub fields: BTreeMap<String, serde_json::Value> }` — `serde::Serialize`.
  - `pub fn rows(tree: &LifecycleTree, collection: &Key, include_archived: bool) -> Option<Vec<EntryRow>>` — `None` for an unknown collection; sorted by `created` descending, then key ascending.
  - `pub fn render_markdown(rows: &[EntryRow]) -> String` — a table with columns `Entry | Kind | Status | Created | Closed`, the entry cell written `[[<key>]] <title>`, so the rendered table is itself valid grammar.
  - An `#[ignore]`d test `census_of_this_repo_under_the_lifecycle_model` that walks `git ls-files '*.md'` from `env!("CARGO_MANIFEST_DIR")`, builds a `LifecycleTree`, and prints: collections per lifecycle, entries, `TreeProblem` counts by variant, and how many `docs/trackers/*.md` files sit outside the lifecycle folders (legacy). It asserts only that the walk read at least one document.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn rows_sort_newest_first_and_hide_the_archive_by_default() {
    let tree = super::tree::LifecycleTree::build([
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc("docs/trackers/ledgers/l/2026-10-01-old.md", &entry("open", "2026-10-01", "")),
        doc("docs/trackers/ledgers/l/2026-11-01-new.md", &entry("open", "2026-11-01", "")),
        doc("docs/trackers/ledgers/l/archive/2026-09-01-gone.md", &entry("fixed", "2026-09-01", "closed: 2026-09-02\n")),
    ]);
    let l = Key::parse("l").unwrap();
    let live: Vec<String> = rows(&tree, &l, false).unwrap().iter().map(|r| r.key.to_string()).collect();
    assert_eq!(live, ["2026-11-01-new", "2026-10-01-old"]);
    assert_eq!(rows(&tree, &l, true).unwrap().len(), 3);
    assert!(rows(&tree, &Key::parse("nope").unwrap(), false).is_none());
}

#[test]
fn the_markdown_table_cites_each_entry_in_the_grammar() {
    let tree = super::tree::LifecycleTree::build([
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc("docs/trackers/ledgers/l/2026-10-01-old.md", &entry("open", "2026-10-01", "")),
    ]);
    let md = render_markdown(&rows(&tree, &Key::parse("l").unwrap(), false).unwrap());
    assert!(md.starts_with("| Entry | Kind | Status | Created | Closed |"));
    assert!(md.contains("[[2026-10-01-old]] t"));
    let cites = crate::librarian::identity::grammar::extract(&md);
    assert_eq!(cites.len(), 1);
}
```

(`doc`, `LEDGER` as in Task 6; `entry(status, created, extra)` here takes the created date as its second argument.)

- [ ] **Step 2: Run to verify failure** — `scripts/with-slot.sh cargo test --lib librarian::lifecycle::table` → FAIL to compile.
- [ ] **Step 3: Implement `table.rs`** and the ignored census test.
- [ ] **Step 4: Run to verify pass** — same command → PASS, 2 tests; then `scripts/with-slot.sh cargo test --lib census_of_this_repo_under_the_lifecycle_model -- --ignored --nocapture` prints the census (today: 0 collections, about 76 legacy trackers).
- [ ] **Step 5: Record the census** in `docs/trackers/tracker-organization-session-log.md` with `doc(action="append_entry", id="ecee2aab906f9df7", id_prefix="W" or "F", anchor_heading="## Template for new entries", …)`.
- [ ] **Step 6: Commit** — pathspec commit of `table.rs`, `lifecycle/mod.rs` and the session log: `feat(librarian): rendered entry table and a census of this repo under the lifecycle model`.

---

## After the plan

- Send the identity session (`codescout-62`) the Interfaces blocks of Tasks 1, 6 and 7 (names and types only): Plan B's derived catalog will hold `LifecycleTree` rows, and Plan C's write path will call `naming::entry_stem` and `archive_rel_path`.
- Plan L-B (tools) starts once identity Plans B and C exist: `doc(create)` for entries and collections, `doc(update)` status handling with `status_since`, `doc(move)` D3 rules, `doc(get)` on a collection through `table::rows`, the eight checks in `doctor`, and legacy mode.
- Plans L-C (consumers), L-D (guide and skills) and L-E (migration, in the identity Plan E window) follow L-B.
