---
id: f01327c4f92f9a72
kind: spec
status: active
title: 'Tracker lifecycle cleanup: the parked items that need no identity work (design)'
tags:
- trackers
- lifecycle
- librarian
- design
---

# Tracker lifecycle cleanup: the parked items that need no identity work (design)

**Status:** active, 2026-11-12. The operator approved the approach and the five items in the
session after Plans L-A and L-B merged. This is a small change, so the design is short.
Parent spec: `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md`. The parked list is
codescout memory `tracker-lifecycle-parked-items`.

## Intent

Plans L-A and L-B left small defects and tidy-ups in `src/librarian/lifecycle/`. None of them
needs the identity layer. Plan L-B2 (the tools) waits for identity Plan C, so this work can
start now and leaves L-B2 smaller.

## Goals

1. Two readmes in one collection folder give a finding. Today `LifecycleTree::build` keeps the
   first and drops the second with no finding.
2. `TreeProblem::Schema` loses its `collection` field. No `Finding` carries it.
3. The `CheckPolicy` rustdoc points at the measurement instead of embedding it. The ledger-scope
   test in `check_entry` becomes an exhaustive `match` over `Lifecycle`.
4. The scan tests no longer depend on git's English error text.
5. The tests of `checks.rs` move to `checks/tests.rs`.

## Non-goals

- The label vocabulary (`Promoted  from` with two spaces counts as forward): the builder's.
- `table::render_markdown` escaping: no table is written to a file yet.
- `portable_reason` and a literal `/`: display only, and Windows is not supported yet.
- Every Plan L-E item, and a default `review_every`.

## Anti-goals

- A change to the eight check names, to the serialized shape of `Finding`, or to the behaviour
  of an existing check. The one exception is the consequence named in § 1, which the operator
  must see. A pinned contract stays pinned (memory
  `tracker-lifecycle-parked-items` § Pinned contracts). A new `CollectionProblem` variant is
  additive; no code outside `src/librarian/lifecycle/` uses that enum.

## Approaches

One approach fits. The items are independent and small. They are five tasks, each its own
commit, in this order, because Task 5 moves the file that Tasks 1 and 3 edit.
Alternatives that lost: one large commit (a reviewer cannot reject one item and approve its
neighbour); fixing the items inside Plan L-B2 (L-B2 is blocked, and these do not depend on it).

## Design

### 1. A duplicate readme is `collection_invalid`

`build` sorts the input by the raw bytes of `rel_path`, so `README.md` comes before `Readme.md`
before `readme.md`. A readme is a file whose placement has role `CollectionReadme`. A collection
folder is keyed by its folder path, not by its collection key: `streams/s/` and
`streams/archive/s/` are two folders. For each folder the first readme in path order is the
collection's readme. Any later readme of that folder gives `TreeProblem::DuplicateReadme(rel_path)`
and is skipped: it is not read as a schema or an entry, so it gives no `schema` or `frontmatter`
problem of its own. Its frontmatter is still parsed once at the top of the loop, and a `closed:`
value still enters `closed_dates`, as for every other file; this plan leaves that as it is.
`check` maps the problem to
`Finding::CollectionInvalid { subject: <the skipped file>, problem: CollectionProblem::DuplicateReadme }`,
which serializes as `"duplicate_readme"`.

**A deliberate change of today's behaviour (found by the plan review, checked in `tree.rs`).**
Today, when the first readme is broken (a `frontmatter` or `schema` problem), a later good readme
still creates the collection, and its entries get every entry check. After this change the first
readme decides: the collection fails, its entries get no entry checks, and the second readme is a
duplicate. A tree that has this shape today is rare; the rule is deterministic and simple, and
that is why it wins. The test `the_first_readme_decides_even_when_it_is_broken` pins it.

### 2. `TreeProblem::Schema` without `collection`

The field is set in `build` and ignored by every reader. Remove it. Plan L-B2 may add it back
when the doctor needs it.

### 3. Rustdoc and an exhaustive match

The `CheckPolicy::terminal_grace_days` rustdoc says what the default is and points at entry
W-3 of `docs/trackers/tracker-organization-session-log.md` for the measurement. The
`else if collection.lifecycle == Lifecycle::Ledger` branch becomes
`else { match collection.lifecycle { Lifecycle::Ledger => { .. } Lifecycle::Stream | Lifecycle::Temp => {} } }`
with the same behaviour.

### 4. A hermetic locale for the scan tests

The test helper `hermetic` in `scan.rs` sets `LC_ALL=C`, so git prints its English text. The
production `git()` function is unchanged. The pin is behavioural: the test that asserts git's
`not a git repository` text asks git for German (`LANGUAGE=de`, `LANG=en_US.UTF-8`) on top of
`hermetic`. German is installed on this machine, so the test fails without the fix. On a machine
without the German catalog it falls back to English and passes, which is correct.

### 5. `checks/tests.rs`

The body of `mod tests` in `checks.rs` moves to `src/librarian/lifecycle/checks/tests.rs`
unchanged except for one level of indentation. `checks.rs` keeps `#[cfg(test)] mod tests;`. The
set of test names (`cargo test --lib -- --list`) is identical before and after.

## What is left to the builder

Names of the new tests beyond those the plan gives, and the exact wording of the
`DuplicateReadme` error text.
