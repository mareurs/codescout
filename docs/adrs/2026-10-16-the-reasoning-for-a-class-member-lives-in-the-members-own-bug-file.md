---
id: c84f03cdce18e33b
kind: adr
status: proposed
title: ADR-2026-10-16 — The reasoning for a class member lives in the member's own bug file, not on a shared line
owners:
- marius
tags:
- issue-clusters
- ledger
- gate-design
- contention
topic: issue-cluster ledger design
time_scope: '2026-10-16'
---

# ADR-2026-10-16 — The reasoning for a class member lives in the member's own bug file, not on a shared line

## Status

**Proposed.** Settled with the owner on 2026-10-16: legacy entries are **frozen, not migrated**; **no
generated class view in version 1**; this ADR and its companion bug are both filed. Not yet
implemented. The measurements below are **dated snapshots**; each carries its derivation so a
reader re-checks it instead of re-deriving it.

Companion bug (the gate's text and code disagree): `docs/issues/archive/2026-10-16-the-growth-gate-accepts-a-bare-stem-while-its-refusal-demands-the-plus-one-shape.md`.
Session measurements: `docs/trackers/cluster-promotion-session-log.md` `F-9`.

## Context

Every bug file carries one `cluster/<slug>` tag. `CHECK 3` of `scripts/pre-commit-ledger-counts.py`
refuses a commit in which a class gains a member unless that class's `**Members:**` field "names"
the member. The check exists on purpose: the old count gate forced authors to write per-member
prose, and when the count was removed *"nothing asks"* (comment at `:1324-1333`; the filing that
measured it, `1b92a7de`, added 1,508 characters of non-derivable prose). The gate is therefore the
only reason the reasoning exists. That is the property worth keeping.

The **carrier** is the problem. The reasoning is written once per member, at filing, but it is
stored in a class file with the rate of change of a class definition, in a field that is one
hand-edited, newest-first, ever-growing paragraph shared by every session.

**What that costs, measured 2026-10-16:**

- *Size.* Field sizes in bytes (a field is its first line plus the following non-blank lines):
  `IC-18` 88,932; the roster's `cluster/unclassified` 75,248; `IC-14` 72,827; `IC-6` 62,533; `IC-2`
  55,301; `IC-11` 54,152. 20 of 25 fields span several lines, and `members_fields`
  (`scripts/pre-commit-ledger-counts.py:820`) reads only the first line, so **94,039 bytes** of
  continuation text lie outside what the gate reads.
- *Contention.* The gate reads the git index and the harness has no per-hunk staging, so one
  session's unstaged edit to a class file blocks every other session's filing in that class. On
  2026-09-02 this blocked five sessions for about four hours
  (`docs/issues/archive/2026-09-02-one-ledger-file-serializes-every-class-edit.md`, mitigated at
  `1b3ac36b`, residual stated there). It recurred: commit `a868f87a` (2026-10-09) is titled *"leave
  the Members line of the issue clusters to the peer's pending commit"*, and on 2026-10-13 a bug
  file stayed untracked for the same reason. `scripts/commit-mine.sh` and `tests/commit-mine.sh`
  exist for this state; the script header names *"a bug file and the `**Members:**` line naming it"*
  as the motivating coupling.
- *A member list in disguise.* The ledger spec says the registry *"never lists members"* because
  earlier lists rotted. The `+1` convention rebuilt one. For `IC-18`: 69 bug files carry the tag, 61
  are named on the field, 8 are not named anywhere, and one slug on the field is now tagged with
  a different class.
- *No single entry shape.* The gate tests substring presence, not the shape its text describes
  (the companion bug). `IC-18` holds at least three entry shapes and `IC-14` begins with a fourth.
- *Coupling surface.* Seven surfaces depend on the one line: the hook script (`CHECK 1`, `CHECK 3`),
  `tests/issue_clusters.rs`, `src/librarian/tools/mv.rs` (special handling of dateless-slug
  citations), `scripts/commit-mine.sh` with `tests/commit-mine.sh`,
  `tests/pre-commit-ledger-divergence.sh`, `docs/issues/_TEMPLATE.md` with
  `src/prompts/guides/tracker-conventions.md:318`, and `scripts/probe-backticked-count.py`.
  (`src/librarian/tools/doctor.rs` and `scripts/count-with-members.sh` match the word and are
  unrelated.)

**What the entries hold, measured on `IC-18` (48 entries scored):** the vocabulary an entry shares
with its own bug file is 0.65, against 0.14 for other bug files of the same class. 69% of the
scored sentence text is grounded in the bug file's vocabulary; **31% is not**. 29% of the text
uses class or sibling language. Read by hand for 3 entries: each restates the mechanism in one to
three sentences, then argues what the member shows about the class relative to earlier members,
then adds measurements. The class argument looks **backward** (it compares to earlier members), so
the member's own file can hold it at filing time.

**Rate of change.** Over 717 commits to the per-class files and the roster since 2026-08-31, of the
534 commits that changed a field, **86% only inserted text**. The 66 that edited existing words
were mostly member lifecycle events (the per-class split, re-pointing after an archive move, a
withdrawn duplicate, promotion rulings). The reasoning is written once.

**Reader demand, 30-day window** (`.codescout/usage.db`, a floor under the 30-day retention
sweep; this session's own calls excluded): 126 of 277 sessions touched the ledger; 89 wrote it; 16
only read it. Non-writing sessions made 11 reads of per-class files (reader-only sessions) to 16 (all non-writing sessions) and mostly read the
roster and Index. Writers made 87% of all read calls, and at least 38% of their whole reads of a
per-class file were followed by an edit to the same class (a lower bound: `doc(update)` takes an
id and cannot be attributed). The transcripts of all three profiles show no native `Read` or
`Grep` on the ledger. **Low observed demand is not low need:** the field is expensive to read
(`read_file` is refused 27 times as a managed artifact; a `doc get` of a per-class file averages
10,285 bytes and reached 69,588).

## Decision

**A bug file that gains a `cluster/<slug>` tag, by filing or by retag, must contain a `## Class
fit` section stating what this member shows about the class relative to earlier members and where
the class's boundary against its nearest neighbour lies.** It does not restate the mechanism; the
bug's own Summary holds that. `CHECK 3` reads that section from the **staged bug file** instead of
reading the class's `**Members:**` field. A class file's `**Members:**` keeps its query, and its
history is **frozen**: no new `+1` entries, no migration of the existing ones.

Specifics:

- The heading is exactly `## Class fit`, an H2 outside any code fence. The 43 class-related H2 headings in existing bug files, under 22 distinct names, are not renamed.
- `cluster/unclassified` requires it too: its section says why the bug fits no class.
- The same check runs as a **test** over tracked bug files dated on or after a cutoff date set at
  implementation, so a `--no-verify` commit is caught in CI. Today `CHECK 3` runs only in the
  local pre-commit hook; CI exercises the hook script only through fixtures (the divergence suite and the
  refusal-routing test); it does not run `CHECK 3` over the corpus.
- **No generated class view in version 1.** The roster and Index stay the reading surface.

## Alternatives considered

**1. A wrapping form for the line (the open residual,
`docs/issues/2026-09-24-residual-members-line-wrapping-and-vectorless-reembed.md`).** Rejected. It
keeps the shared write, the unbounded growth and the member list, and it touches every parser. It
answers only a retrieval-quality concern that this work did not show to be real. Its premise,
"single line", is also false for 20 of 25 fields.

**2. One note file per member under the class directory.** Rejected. It removes the contention
too, but the bug file is already the filer's own uncontended file at filing time, so this adds
about 750 files and a second artifact per bug for no gain.

**3. A frontmatter field in the bug file.** Rejected with low confidence. It parses with the
existing `frontmatter()` code, but multi-paragraph prose in YAML is hostile to read and to diff.

**4. Drop the gate.** Rejected. The prose exists only because the gate asks for it.

**5. Keep the design and re-judge the residual's severity.** The fallback if this ADR is
rejected. Cheap, but the contention incidents recur (three in seven weeks that left a record).

**6. Migrate the existing entries into their bug files.** Rejected for now. It writes history into
about 22 class files and many bug files at once, and the entries stay correct as frozen text. The
strangler approach: new members use the new form, and old lines age out.

## Consequences

- **Now easier:** parallel filing in one class (each session edits only its own new file); one
  enforced entry shape; a class file that stops growing; no commit-order coupling between a bug
  file and a shared line, which is the state `commit-mine.sh` works around; a CI-enforceable check
  that reads one file and needs no comparison against `HEAD`.
- **Now harder:** no single page shows a class's members and their reasoning. Two conventions
  coexist until the old lines are retired. The parser must handle fences (32 `## ` lines sit
  inside code fences across 15 of 1,131 bug files, measured 2026-10-16). The seven surfaces named under Context must be re-read, and most will change
  (the hook script, its Rust mirror tests, the two procedure documents, `commit-mine.sh`, the divergence fixtures and the probe); `mv.rs` should not need to.
- **Not changed:** the presence check stays satisfiable by a placeholder, as the current gate is.
  Any form that lands needs mutation tests: an empty section, a heading inside a code fence and a
  placeholder body must each be refused.

## Change scenarios absorbed

- **Two sessions file bugs in one class.** Today they contend on one line; under this decision
  each edits only its own new file.
- **A class grows to hundreds of members.** Today the field grows without a ceiling (82 KB at 69
  members); under this decision the class file is constant.
- **A member is retagged or archived.** For new members the reasoning travels with the file; no
  stale slug is left on a shared line. Old lines are frozen and still carry theirs.
- **A class is split.** Neutral: a split already edits every member's tag, so the member files are
  touched either way.

## Revisit-when

- A class split needs edits to member files beyond the tag.
- Readers need a per-class overview. The signal is non-writing sessions reading per-class text, or
  a request for one; then the generated view is built from the `## Class fit` sections by a tag
  query.
- `## Class fit` sections turn out empty or placeholder in practice.
- The check cannot read the staged bug blob reliably in the hook.

**Explicitly not a trigger:** the old lines continuing to exist. They are frozen by design.

## Confidence

**Medium-high** for moving the check off the shared line; **medium** for the exact content
requirement and for how much sibling context a writer needs.

**What is given up:** the one-page class overview, until the view is built; and cheap enforcement
of quality, which neither design has.

**Not established:**

- 62% of writers' whole reads of per-class files were not followed by an attributable edit. I did
  not determine whether they study sibling entries, which a `## Class fit` writer would also need.
- The grounding measurement is a **vocabulary-overlap proxy** on one class (`IC-18`, 48 entries),
  not a claim-by-claim comparison, and only 3 entries were read by hand.
- The usage window is 30 days and `usage.db` records only codescout calls; the transcript check
  found no native `Read` or `Grep`, and about 116 read-like native `Bash` calls, mostly greps of
  the Index.
- No prototype of the new check exists.

## How each number was derived

The scratch scripts for these measurements are not kept in the repository. The methods:

- *Field size:* `**Members:**` line plus following non-blank lines to the first blank line, bytes via `wc -c`. (A first version used `awk length()`, which counts characters; it read about 0.6% low.)
- *Rate of change:* for every commit touching `docs/trackers/issue-clusters/` or the roster, compare the field at the commit with the field at its parent; a commit is a pure insertion when the old text equals the new text with one contiguous block removed (common prefix plus common suffix cover the old text).
- *Named members:* tagged files (frontmatter, parsed as the text before the closing `---`) whose slug occurs backticked in the field.
- *Grounding:* per sentence, the share of distinct content words (five or more letters, not in more than 25% of bug files) that also occur in the entry's own bug file, minus the mean share for 8 other bug files of the same class; a sentence is "grounded" at a difference of 0.25 or more.
- *Reader demand:* `tool_calls` rows whose `input_json` names the ledger, classified by tool and action; a session is a writer if any call writes the ledger.
- *Fence hazard:* count `## ` lines inside fenced blocks across `docs/issues/` and `docs/issues/archive/`.
