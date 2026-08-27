---
id: '9f7de1c3c092095d'
kind: bug
status: fixed
title: 'BUG: two byte-identical headings make both sections permanently unaddressable, and the error''s prescribed remedy names parameters that do not exist on the tool it names'
tags:
- edit-markdown
- librarian
- error-hints
- closed-loop
closed: 2026-08-27
opened: 2026-08-27
owner: marius
related:
- docs/issues/archive/2026-08-27-edit-code-remove-cannot-remove-an-impl-block.md
severity: medium
unverified: 'Fix step 4 (the file-relative vs body-relative line-number frame in the ambiguity error) was deliberately NOT done and remains open. A second, unrelated defect is left unrepaired by choice: open-issue-work-queue.md defines ### BL-43 twice, and choosing the authoritative copy is a content judgement about another work stream.'
---

# BUG: two byte-identical headings make both sections permanently unaddressable

## Summary

When a markdown file contains the same heading text twice, every heading-addressed
surface refuses to act on either occurrence — correctly, since the query is genuinely
ambiguous. The defect is that **no disambiguator exists**, and the error's hint
prescribes one that does not: it names `start_line`/`end_line` parameters that are
absent from `edit_file`'s schema, on a tool that is gated off `.md` files entirely.
For a librarian-managed artifact the loop is fully closed — `edit_markdown` and
`edit_file` both refuse by guard, and `artifact(update, patch={body_edits})` routes
through the same resolver and hits the same ambiguity. The only remaining path is a
whole-body rewrite.

## Symptom (Effect)

Given a file with `## Fix` at two places:

```
edit_markdown(path=…, heading="## Fix", action="replace", content="TOUCHED")
→ heading '## Fix' found 2 times (lines 3, 11) — hint: Provide a more specific
  heading or use edit_file with start_line/end_line to target a specific occurrence.
```

Following that hint:

```
edit_file(path=…, new_string="TOUCHED")
→ Use edit_markdown for markdown files
```

Same ambiguity through the managed-artifact surface:

```
artifact(action="update", id="2cdc5815808a6634",
         patch={body_edits: [{heading: "## Fix", action: "edit", …}]})
→ body_edits[0]: heading '## Fix' found 2 times (lines 86, 106) — hint: Provide a
  more specific heading or use edit_file with start_line/end_line …
```

## Reproduction

`git rev-parse HEAD` → `7da4c0ed` (branch `experiments`).

```
create_file("/tmp/probe.md", "# Probe\n\n## Fix\n\nFIRST\n\n## Middle\n\nm\n\n## Fix\n\nSECOND\n")
edit_markdown("/tmp/probe.md", heading="## Fix", action="replace", content="TOUCHED")
edit_file("/tmp/probe.md", new_string="TOUCHED")
```

The managed-artifact half reproduces against any bug file carrying a repeated heading —
`docs/issues/archive/2026-08-27-unregistered-memory-tool-structs-read-as-the-live-tool.md`
has `## Fix` at file lines 86 and 106.

## Environment

Linux, branch `experiments`, codescout MCP over stdio, project `codescout`.

## Root cause

**One resolver, four surfaces.** `resolve_section_range`
(`src/tools/file_summary/file_summary.rs:229-325`) is the only heading resolver in the
tree. `references` gives its consumers: `edit_markdown` (three call sites —
`src/tools/markdown/edit_markdown.rs:89`, `:363`, `:749`, the last being the
`body_edits` path used by `artifact(update)`), `read_file`'s heading param
(`src/tools/read_file.rs:956`), and `file_summary.rs:331`. Every heading-addressed
operation in codescout inherits its behaviour.

**Its 4-tier cascade cannot disambiguate identical strings, by construction.** Tier 1
is exact-raw and tier 2 is exact-stripped; each returns `dup_error` the moment it
matches more than once, *before* tier 3 (prefix) and tier 4 (substring) run. Since the
two headings are byte-identical, no query string exists that matches one and not the
other — the later, fuzzier tiers could not help even if they were reached. There is no
`occurrence` parameter anywhere on the path.

**The hint names a capability that does not exist.** `dup_error`
(`src/tools/file_summary/file_summary.rs:269`) emits *"use edit_file with
start_line/end_line"*. `EditFile::input_schema`
(`src/tools/edit_file/mod.rs:377-403`) declares exactly: `path`, `file_path`,
`old_string`, `new_string`, `replace_all`, `insert`, `edits`. **No `start_line`, no
`end_line`** — measured 2026-08-27 by reading the schema body.

**And the named tool is gated off the file type regardless.** IL-5 refuses `edit_file`
on `.md` before schema validation, permitting only `insert=prepend|append` or
`replace_all=true`. So the hint would be unusable even with the parameters it invents.
`replace_all` is the opposite of a disambiguator — it hits every occurrence.

**For managed artifacts every branch is closed.** `edit_file` refuses on every path
(librarian guard), `edit_markdown` refuses (managed), and `body_edits` shares the
resolver. Remaining: `patch={body: …}`, a whole-body rewrite, itself gated by the 50%
shrink guard.

## Evidence

### Scope — measured 2026-08-27

`git ls-files 'docs/**/*.md' 'docs/*.md'`, per-file `grep '^##* ' | sort | uniq -d`:

```
files scanned: 1149   files with >=1 duplicate heading: 52
```

Four are **live, guarded** trackers whose only edit surface is `body_edits`
(frontmatter checked for the guard's three conditions — augmented, `id:`-stamped,
or `entry_prefix` ledger):

| tracker | dup headings | guard reason |
|---|---|---|
| `docs/trackers/release-promotion-session-log.md` | 6 | `id:` stamped |
| `docs/trackers/capability-proposals.md` | 5 | `id:` + `entry_prefix: CAP` |
| `docs/trackers/artifact-augmentation-followups.md` | 4 | `entry_prefix: AA` |
| `docs/trackers/open-issue-work-queue.md` | 1 | `id:` stamped |

### A ledger entry defined twice — consequence NOT established

`docs/trackers/open-issue-work-queue.md` defines `### BL-43 — complete BL-41's
coverage: an undeclared, undefined ledger is still invisible` **verbatim at both line
295 and line 301**, with six in-file references (lines 83, 86, 470, 472, 473, 485,
495). Two definers for one entry token in one active artifact.

**Whether this actually breaks citation resolution is unconfirmed.** A
`librarian(action="link_scan", write=false)` run over 1176 artifacts reported
`ambiguous: 539` / `dangling: 587` with `truncated: {ambiguous: true, dangling:
true}` — both buckets capped at 50 entries. `BL-43` does not appear in the visible
sample, but absence from a truncated list is not evidence of absence, and
`prefix_conflicts` covers cross-artifact prefix collisions rather than a repeated
heading inside one artifact. The duplicate itself is measured; its effect on the link
graph is not. Do not repeat the effect claim without re-measuring it.

What the duplicate *does* establish is the trap closing on itself: the heading cannot
be repaired through `body_edits`, because addressing it is exactly the ambiguous
operation.

### Coordinate mismatch between the error and the reader

For the same two headings in the same file:

- `artifact(action="get")`'s heading map reports lines **68** and **88**
- the `body_edits` ambiguity error reports lines **86** and **106**

Measured: `grep -n '^## Fix'` gives file lines 86 and 106; the file's frontmatter ends
at line 17 and the file is 211 lines against the body's 193. The heading map is
body-relative, the error is file-relative, and the constant 18-line difference is the
frontmatter. Both are internally consistent; neither says which frame it is in, so a
caller who reads `get` and then the error sees two different line numbers for one
heading.

## Hypotheses tried

1. **Hypothesis:** a more specific heading query disambiguates (as the hint's first
   clause suggests). **Test:** read the 4-tier cascade at
   `src/tools/file_summary/file_summary.rs:229-325`. **Verdict:** rejected — tiers 1–2
   return `dup_error` before the fuzzy tiers run, and identical strings admit no
   distinguishing query anyway.
2. **Hypothesis:** `edit_file` with a line range is the escape, per the hint.
   **Test:** read `EditFile::input_schema`; then call `edit_file` on a `.md` probe.
   **Verdict:** rejected twice over — the parameters are absent from the schema, and
   IL-5 refuses `edit_file` on `.md` before schema validation.
3. **Hypothesis:** managed artifacts escape via `body_edits`. **Test:** live call
   against `2cdc5815808a6634`. **Verdict:** rejected — `edit_markdown.rs:749` calls
   the same resolver; identical error.
4. **Hypothesis:** this is a rare shape not worth addressing. **Test:** enumerate the
   class across `docs/`. **Verdict:** rejected — 52 of 1149 files, four of them live
   guarded trackers.

## Fix

**Shipped 2026-08-27** — `164c8bd6` (`experiments`), patch-id
`a3681488286644bf7c0a182740d26065a1230d44`. Steps 1–3 below landed; **step 4 did not**
(see *Still open*).

The threading used `HeadingQuery { text, occurrence }` with a `From<&str>` impl and
`impl Into<HeadingQuery>` parameters rather than a new positional argument. That choice
was load-bearing for the size of the diff: every existing `&str` caller compiles
unchanged, so the 12 test call sites in `src/tools/file_summary/tests.rs`,
`src/tools/read_file.rs:956`, and `file_summary.rs`'s own consumer needed **zero**
edits. `plan_section_edit` already carried `#[allow(clippy::too_many_arguments)]`, so a
ninth argument would have made that worse; changing the query's *type* did not.

One finding the plan did not predict: `src/librarian/tools/update.rs` has its **own**
`apply_body_edits` loop and does not route through `plan_batch`, so the managed-artifact
path needed wiring separately from `edit_markdown`'s. That is the branch with no
alternative, so missing it would have left the actual defect in place behind a passing
`edit_markdown` test.

Add an **`occurrence`** disambiguator (1-indexed) to the heading-addressed surfaces,
and correct the hint to name it.

1. Introduce `resolve_section_range_nth(content, heading_query, occurrence:
   Option<usize>)` in `src/tools/file_summary/file_summary.rs`, keeping
   `resolve_section_range` as a delegating wrapper passing `None`. This avoids churn at
   the 12 test call sites and at `read_file.rs:956`, which have no ambiguity to
   resolve. When `occurrence` is `Some(n)` and a tier matches `k` times, select the
   n-th (1-indexed) and error if `n > k`, naming `k`.
2. Thread `occurrence` through `edit_markdown`'s three call sites
   (`src/tools/markdown/edit_markdown.rs:89`, `:363`, `:749`), exposing it on the
   `edit_markdown` schema and inside the `body_edits[]` entry shape so
   `artifact(update)` reaches it.
3. Rewrite `dup_error`'s hint (`file_summary.rs:269`) to name `occurrence` — and stop
   naming `edit_file`, which is closed for `.md` on every path.
4. Decide the frame for the reported line numbers and label it. Either report
   body-relative numbers on the `body_edits` path so they match `artifact(get)`'s
   heading map, or keep file-relative and say `file line N` in the message.

**Not the fix: auto-selecting the first occurrence.** Silent selection is how a caller
edits the plan while believing they edited the shipped record — strictly worse than
today's loud refusal.

## Tests added

Eight regression tests, each with its mutation control named before the run.

`src/tools/file_summary/tests.rs`:

- `occurrence_selects_among_identical_headings` — occurrence 1 and 2 resolve to
  different heading lines.
- `occurrence_past_the_match_count_errors_naming_the_count` — guards against clamping.
- `occurrence_zero_is_rejected_rather_than_read_as_the_first` — guards against a caller
  who counts from zero editing one section while naming another.
- `occurrence_on_a_unique_heading_still_resolves`
- `an_unqualified_duplicate_query_still_refuses_rather_than_guessing` — guards the
  deliberately-preserved loud refusal.
- `the_duplicate_hint_names_occurrence_and_not_edit_file` — asserts the hint names the
  remedy that exists and no longer names the closed door.

`src/librarian/tools/update.rs`:

- `body_edits_occurrence_reaches_the_second_of_two_identical_headings`
- `body_edits_without_occurrence_still_refuses_identical_headings`

**Mutation verification — both mutations produced the predicted failure:**

1. Resolver `indices[n - 1]` → `indices[0]`: `occurrence_selects_among_identical_headings`
   failed `left: 2, right: 6`, **and** the body_edits test failed printing the corrupted
   document — `the shipped record` had landed in the *first* section while the second
   still read `the plan`. The silent wrong-section edit, made visible.
2. `update.rs` passing `heading` instead of `query`: only the managed-path test failed,
   with the ambiguity error. This is the control that proves that path actually reads the
   selector rather than inheriting a pass from `edit_markdown`'s wiring.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean,
`cargo test` 4624 passed / 0 failed / 46 ignored.
## Workarounds

- **Plain `.md`, unguarded:** `edit_file(..., old_string=<text unique to the intended
  section>, new_string=…, replace_all=true)`. `replace_all` is required because IL-5
  permits only that or `insert`; a *unique* `old_string` keeps it to one site.
- **Guarded artifact:** `artifact(action="update", patch={body: <whole body>})`, which
  means re-transcribing the entire body to change one line, under the shrink guard.
- **Cheapest, where you own the file:** make the headings distinct. Even a trailing
  qualifier (`## Fix — options considered` vs `## Fix`) restores addressability for
  both.

## Resume

**Live-verified 2026-08-27, 18:11 EEST.** Freshness checked on all three axes before
trusting any probe: build (`target/release/codescout` @ 18:10:28 carries both new strings,
old hint count **0**), distribution (`~/.cargo/bin/codescout` is a symlink to exactly that
path — confirmed by grepping *through* the symlink, since `stat` on a symlink reports the
link's own June mtime and would have read as stale), process (serving pid 4095217 started
18:10:59, identified by walking the parent chain `sh → codescout → claude` rather than
guessing among 18 live codescout processes).

Probes, all on the wire:

- ambiguous query → hint now reads *"Pass occurrence=N to target one (1-indexed, here
  1..=2)"* and no longer names `edit_file`. The closed loop is open.
- `edit_markdown(heading="## Fix", occurrence=2)` → edited the second section, first
  untouched.
- control: the same `replace` on a *unique* heading with no `occurrence` produces a
  byte-identical shape, so the trailing-blank-line consumption is `replace`'s pre-existing
  semantics and not something `occurrence` introduced.
- `artifact(update, patch={body_edits: [{heading, occurrence, …}]})` against real
  librarian-managed artifacts — the branch that had no alternative — succeeded twice, and
  was used to perform both repairs recorded below.

**CORRECTION — § this section previously said "the four affected trackers are unrepaired"
and queued repairing them. That was wrong, and the framing behind it was wrong.** Three of
the four carry duplicates that are *correct document structure*, not defects: `### Do NOT
re-do` / `### Still owed` recur under each `## Resume — round N` in
`release-promotion-session-log.md`; `### Why` / `### Shape` / `### Acceptance` under each
`## Phase N` in `artifact-augmentation-followups.md`; `### The ask` / `### Open decisions`
/ `### Resume` under each `## CAP-N` in `capability-proposals.md` (verified by reading the
heading maps, 2026-08-27). Renaming them to restore addressability would have damaged the
documents. `occurrence` does not repair these files — it *reaches* them, which is the whole
point. Read the 52-file measurement the same way: it is mostly legitimate per-entry
template structure that the tool could not address, not 52 defects.

**RESOLVED 2026-08-27 — the duplicate `### BL-43` in `docs/trackers/open-issue-work-queue.md`.**
The two copies were not a copy-paste but two successive *states* of one entry: the copy at
line 295 carries the current disposition (`dropped — handed off`), the copy at 301 the
earlier `open` state plus the substantive measurement. Both index tables and the `tasks`
params row (`status: "dropped"`) agree with the first, so the second was evidence rather
than a rival claim — deleting it would have destroyed the only place that measurement
lives. Repaired by demoting the second to a nested `#### Measurement (2026-08-18) —
recorded while BL-43 was still open`, using `occurrence: 2` to reach it: the fix's first
use on a document it was written for. Single definer restored, no content lost, and
`artifact(get, heading="### BL-43 — …")` now returns the section where it previously
reported the heading **missing**.

**And the "not established" question is now answered — in the negative.** `link_scan`
before and after the repair reports `ambiguous: 539` / `dangling: 587`, byte-identical, so
the duplicate definition contributed **zero** to citation ambiguity. The mechanism:
`link_scan` reports ambiguity when *two artifacts* define one token, because an edge needs
a single target artifact; two definitions inside **one** artifact both point at that same
artifact and resolve fine. BL-43 was an addressability and document-structure defect, not a
link-graph one. (Caveat kept: the corpus grew 1176 → 1177 between runs because this bug
file was added, so it is not a perfectly controlled A/B — both counts being *exactly*
unchanged is what carries it.)

**Still genuinely open — two items, both distinct from the above.**

1. **Fix step 4 was not done, and is now measured on two files.** The `body_edits`
   ambiguity error reports **file-relative** line numbers while `artifact(action="get")`
   reports **body-relative** ones. Measured on `capability-proposals.md`, 2026-08-27:
   `get` → `occurrences: [642, 797, 1148, 1214]`; `body_edits` → `lines 660, 815, 1166,
   1232`. Exactly **18** apart on all four, and the same offset on the memory-tool-structs
   file — both carry a 17-line frontmatter block plus a blank separator line, so body line
   1 is file line 19. (An earlier draft of this entry said the offset on
   `open-issue-work-queue.md` was 16. 16 is that file's frontmatter *length*, not a
   measured offset — the two were conflated.) Both frames are
   internally consistent and neither is labelled, so a caller who reads `get` and then the
   error sees two different numbers for one heading. Decide one frame and state it in the
   message. Untouched by `164c8bd6`.

2. **`artifact(action="get", heading=X)` reports a doubly-defined heading as `heading_missing:
   true`** — the opposite diagnosis, sending the caller to look for a heading that is
   present twice. Found while repairing BL-43 and confirmed with two positive controls
   (`### BL-44 — …`, unique, resolves; `### BL-1 — …`, unique *and* containing an
   apostrophe, resolves), so it is duplication and not a character-encoding mismatch. That
   read surface never received `occurrence`; this fix only reached the write surfaces.
   Filed separately rather than folded in here.
## References

- `src/tools/file_summary/file_summary.rs:229-325` — `resolve_section_range`, the 4-tier cascade
- `src/tools/file_summary/file_summary.rs:269` — the wrong hint
- `src/tools/edit_file/mod.rs:377-403` — `EditFile::input_schema`, no line-range params
- `src/tools/markdown/edit_markdown.rs:89,363,749` — the three consumers; `:749` is `body_edits`
- `src/tools/read_file.rs:956` — heading-param consumer, unaffected
- `docs/issues/archive/2026-08-27-edit-code-remove-cannot-remove-an-impl-block.md` — same shape in `edit_code`: an error whose prescribed escape is closed in the context that raises it
