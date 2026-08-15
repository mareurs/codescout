---
id: '38a17e4acf1f1fa1'
kind: tracker
status: draft
title: Structural Debt — post-690-commit refactor stream (SD-N)
owners:
- marius
tags:
- refactoring
- structural-debt
- code-smell
- librarian
- audit-doc-refs
topic: refactoring
---

## Why this exists

The `experiments` branch carries 690 commits ahead of `master` — 647 files,
+111,476 / −6,311; in `src/` + `crates/` alone 203 files and +44,070 / −4,966.
The mass landed in one subsystem: `src/librarian/` roughly doubled, adding
`src/librarian/catalog/gc.rs` (1244), `src/librarian/tools/merge_worktree.rs`
(970), `src/librarian/catalog/graft.rs` (753),
`src/librarian/tools/link_scan/` (~1260),
`src/librarian/tools/append_entry.rs` (401),
`src/librarian/tools/constitution_check.rs` (381),
`src/librarian/catalog/worktree.rs` (185),
`src/librarian/catalog/entry_cite.rs` (130), while
`src/librarian/tools/doctor.rs` grew by 2454 lines.

That is the youngest, least-settled code on the branch. This tracker holds the
structural findings from reading it — the ones that survive a "name the
structural defect in one sentence" test. It does **not** hold bugs (those are
`docs/issues/`), tool frictions (`U-N`), or plan-vs-reality drift (`F-N`).

**`SD-N` is work-stream-scoped, not a durable taxonomy slot.** Per
`docs/TAXONOMY.md` § *Work-stream-specific prefixes*, a new project-wide prefix
must be earned. If this stream outlives itself, promote it then; until it does,
the prefix lives here and nowhere else.

## Boundary with the sibling trackers

- **`docs/trackers/legibility-backlog.md`** (`cd886c414f6751b4`) owns
  over-budget function bodies ranked by observed usage.db cost. It is written
  by `librarian(action="legibility_scan")`, not by hand. SD entries **cite** it
  and never duplicate its rows. It was last written ~2026-05-09 and is stale;
  a `write=true` scan refreshes it.
- **`docs/issues/`** owns anything with a reproduction and a wrong output.
  An SD entry that turns out to produce wrong behaviour is promoted to a bug
  file and its SD row flips to `superseded` with the bug path recorded.

## Method — the discipline this stream runs under

One transformation per commit; tests green after every single move; behaviour
preserved, period. A new parameter, branch, or output in the diff means it
stopped being a refactor.

**Baseline, recorded before the first move** (`1911af3d`):

- `cargo test --workspace` — **3818 passed / 0 failed / 50 ignored**
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt` — clean
- `codescout audit-doc-refs --fail-on high` — exit 0 over 945 files

**The baseline's own caveat, which is load-bearing:** 3818 is the count for the
**default feature graph**, not for the repo. `src/dashboard` tests are filtered
out unless `--features dashboard`, and every `#[cfg(feature = "server-stack")]`
test is compiled by no lane at all. A refactor touching either moves code its
baseline does not cover, and the suite stays green throughout. Any SD entry in
those areas must state which lane actually built its tests.

## Findings

_Per-entry detail sections live below as `### SD-N — <title>`. The live table is
rendered from params at the top of the file._

## History

### 2026-08-15 — stream opened

Survey run on `1911af3d` after the backlog drive closed 14 of 15 bugs. Five
findings recorded (SD-1..SD-5). No code touched — the survey was read-only, and
the merge to `master` (a clean fast-forward, 0/690) is on the user's hold.

### 2026-08-15 — SD-5 and SD-6 closed; SD-6 and SD-7 discovered while working

SD-5 fixed in `experiments:121a2263` (not-yet-on-master). The memory was stale
twice over: it described the `escape_like_pattern` extraction as an owed plan,
and it named `src/librarian/filter.rs` line 230 as the canonical inline idiom
when that line is now a *call* to the helper. Rewritten into a settled Rust half
and an open SQL half, the latter routed to SD-2.

**SD-6 was found by pulling on SD-5's thread**, and is the sharper half of
SD-1. Every memory anchor sidecar citing a bug file pointed at the pre-archive
path — 3 of 3. Because `check_path_staleness` (`src/memory/anchors.rs`) tests
`!full.exists()` *before* comparing hashes, an archived bug file is reported as
`Deleted` rather than `Changed`: a strictly false verdict, and the more damaging
one, since `Changed` says "re-read this" while `Deleted` says "the thing this
memory rests on is gone." Fixed in `experiments:04891bd3` (not-yet-on-master).
Content survives `git mv` — proven byte-for-byte, as one anchor's recorded hash
matched the archived file exactly — so two repairs were path-only. The third
hash was left deliberately mismatched: that file's content really did change and
nobody has reconciled the memory, so the mismatch is a true warning, and
silencing it to make a checklist green would have been the wrong trade.

**SD-7 was found by trying to close SD-5 in this tracker.** Flipping one row's
status requires re-sending the entire `items` array, because `artifact`
dispatches `append_entry` and nothing else at entry grain. The operation a
status tracker performs most often is the one it does least safely. Worked
around here via `params_path` plus a post-write verification that all seven ids
survived; recorded rather than fixed, because an entry-grain update is a feature.

Baseline re-run after both fixes: **3818 passed / 0 failed / 50 ignored** —
unchanged, as a behaviour-preserving change requires.

### 2026-08-15 — SD-1a swept; SD-1's own measurement was wrong by an order of magnitude

`experiments:53796432` (not-yet-on-master) repointed **95** archived bug-file
citations across 86 `.rs` files. SD-1 had recorded "7 distinct stale paths, 12
occurrences." The true figure: **111 unique bug files cited across 94 files — 10
live, 95 archived-and-stale, 6 nonexistent. 86% stale.**

The method is the lesson, and it failed twice in one sitting, both times
silently:

1. The 7/12 figure came from running `audit_doc_refs` over
   `--paths 'src/**/*.rs'` — a markdown parser pointed at Rust. It reads
   `tokio::sync::Semaphore` as a link and reports 33,105 refs, so the handful of
   `docs/`-shaped findings that surface are an arbitrary fraction, not a count.
   **A tool used outside its domain does not fail loudly; it answers.**
2. The follow-up sweep nearly repeated it. A default-mode `grep` reported "50
   matches in 20 files" with a footer reading "Showing 50 of 50"; `mode="files"`
   on the identical pattern reported **266 in 94**. Building the substitution
   list from the first would have swept a fifth of the sites and reported done.

Both are the shape W-39 already tabulated — a count asserted from a method that
cannot produce it. The correction here is not a bigger number but a different
instrument: enumerate, classify against the filesystem, then re-classify
afterwards and require the residual to be zero.

Verification, because a 236-line substitution deserves it: the diff is
**symmetric** (236 insertions, 236 deletions across 86 files — a pure path
substitution cannot add or remove a line); post-sweep classification reads
`live=10 stale=0 gone=6`; a grep for a doubled archive segment returns zero; baseline
**3818 / 0 / 50** unchanged, `fmt --check` and `clippy --all-targets -D warnings`
clean.

SD-1 stays **open** on purpose. The sweep fixes today's state; only the gate
(SD-1b) stops the next archive wave from re-opening it, and that is a feature
needing a design pass rather than a sweep.

### 2026-08-15 — SD-2 closed: one spelling, and a gate that proved itself twice

`experiments:31609aa5` (not-yet-on-master). Nested-`REPLACE` occurrences across
`src` went **5 → 1**.

Two things the survey had wrong, both discovered by doing the work:

**The duplicated unit was larger than recorded.** The `|| '/%' ESCAPE` tail is
shared as well, so what sat at four sites was the whole *strict-descendant
predicate*, not merely the escape idiom. Extracting the larger concept removed
more duplication and — the actual test for whether an extraction is real — gave
the function an obvious name.

**The operand asymmetry is the reason the second implementation exists at all.**
`catalog::worktree::covering_conn` escapes a per-row **column**; the other three
escape a bound parameter. A column cannot be reached by `escape_like_pattern`,
which acts on a Rust-side value. Three separate comments each explained this
independently — the surest sign a shared concept was going unnamed.

Behaviour preservation is proven rather than argued:
`descendant_path_like_reproduces_the_pre_extraction_sql_exactly` pins the exact
pre-refactor string, whitespace included, and passed first try. A stray line
continuation in the helper's `format!` would change the query text even though
SQL tolerates it; that assertion is what would catch it. Three property tests
cover what a snapshot cannot explain — escape order, strict-descendant
anchoring, operand interpolation.

**The guard is the deliverable, not the tidiness.** The Rust-side gate could not
do this job: it greps a Rust call signature, and this idiom is SQL text. One law
with two spellings needs two gates, or the unguarded spelling is the one that
drifts. It was mutation-verified twice — once deliberately, and once by
accident, catching my own characterization test's expected literal as a genuine
second occurrence in the same commit that introduced it.

One process note for the next session, because it recurred today:
`cargo test --lib descendant_path_like` reported **4/4 green while the suite was
broken**, because the filter matches the behaviour tests but not
`sql_descendant_like_...`. That is F-48 — a name filter selects by naming
convention, not by blast radius. Only `--workspace` tells the truth.

Baseline: **3823 / 0 / 50** (from 3818, +5 tests), `fmt --check` and
`clippy --all-targets -D warnings` clean.
