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

### 2026-08-15 — SD-1 closed: the gate can now see citations in code

`experiments:450880c7` (not-yet-on-master). The scan went **947 → 1390 files**,
**50,206 → 51,310 refs**, exit 0.

**The abstraction was not built, because it already existed.** `get_ts_language`
is documented in-tree as the single source of truth for tree-sitter language
resolution, shared by the AST parser and the embedding chunker — this is its
**third** consumer. A trait with one impl per language would have been a fourth
language mapping to keep in sync, which is the one-implementor bureaucracy
`tool-registration-rule-of-three` exists to prevent. There is therefore **no
per-language code**: one predicate, `kind().contains("comment")`, covers all
nine grammars.

That predicate was the load-bearing unknown — confirmed from working code for
only five of nine. `every_supported_language_yields_its_doc_text` now proves it
for all ten language keys and fails loudly if a grammar upgrade renames a node.
That guard earns its keep because **a scanner that finds nothing is
indistinguishable from a codebase with nothing to find.**

**Python needed a second shape and would otherwise have been silently
half-covered.** It documents in *string literals* — a docstring is
`expression_statement > string`, carrying no comment node — so a comment-only
extractor returns its `#` notes and drops every docstring. One explicit,
language-gated special case, guarded from both sides: docstrings extracted;
assigned and non-leading strings NOT treated as documentation; and the rule
proven not to leak into TypeScript, where a leading bare string is a directive.

Two integration notes worth keeping. The discriminator is *"has a grammar"*,
not *"is a source file"* — `detect_language` answers `Some("markdown")` for
`.md`, so the obvious test would have routed every markdown file into the
comment extractor and found nothing anywhere. And dispatching inside the
existing loop incidentally fixed the `--paths 'src/**/*.rs'` footgun that
produced 33,105 bogus refs earlier today; restricting to documentation nodes
is a measured **~30×** noise reduction (443 source files → 1,104 refs, against
33,105 from 275 files parsed whole).

**The cap is tested rather than assumed, and the reason is the lesson.** The
first live scan produced **zero** `code_comment_capped` findings — good news
about the corpus, and no evidence at all that the cap works. Exit 0 there means
nothing needed capping, not that capping happens. So the policy moved into a
pure function beside `cap_released_history` and is pinned by a discriminating
pair: a no-op fails the High→Med assertion, a blanket always-Med fails the
reasons-survive assertion.

The three citations that resolve to nothing are split out as **SD-8** — no rule
can fix them, so they were deliberately left out of the sweep.

Baseline: **3835 / 0 / 50** (from 3818, +17 tests), fmt and clippy clean.

### 2026-08-15 — correction: SD-1b saw a fifth of what it was built for, and my "cap never fired" claim was wrong

Two errors from the same root, both mine, both worth keeping because the shape
recurred three times in one session.

**The cap claim was false.** The entry above said the first live scan produced
zero `code_comment_capped` findings, and reasoned from that to "the cap is
untested." It fires. I had run that scan as `… | head -8`, so the buffer held
**eight lines**, and I grepped those and read the empty result as a fact about a
1390-file corpus. A second attempt failed the same way for a different reason:
run bare, the findings array itself reports `"shown": 50, "total": 51833` — a
0.1% sample. Production evidence exists and is narrow and sufficient:
`src/fs/mod.rs` returns three findings, two carrying
`severity_reason=code_comment_capped`.

That is the third instance today of **measuring the instrument and reporting it
as the subject** — after the markdown auditor pointed at Rust, and the
default-mode grep capped at 50 while its footer read "Showing 50 of 50". The
parallel session filed the grep one as its own bug file. The tests written on
the false premise are still correct and stay: a policy deserves a test whatever
prompted it.

**SD-1b covered 20% of its target.** `parse_refs` extracts from exactly three
places — inline code spans, fenced blocks, link targets — and of 699
`docs/*.md` citations in this repo's `.rs` files only **140** are backticked.
Fixed in `experiments:63b279ed` with a prose branch that only the source-comment
path calls, leaving markdown byte-identical.

The first working version of that was not shippable, which is the part worth
remembering. Prose carries no backticks to signal "this is a path", so
`classify` admits any slash-bearing word: one file went **2 → 106 refs**, 97 of
them junk — the comment markers `//` and `///` themselves, plus slash-joined
English like `overview` / `read` and `generated` / `vendored` (written apart
here on purpose: quoted as one token they trip the markdown scanner, which has
no extension guard — writing about a false positive produced two).
Stripping the marker and requiring a file extension
brings it to **3 refs, 0 unknowns**. The extension rule is deliberately strict
and misses citations written without one; that is a malformed citation, better
fixed in the comment than accommodated by a fuzzier matcher.

**The noise came from where I wasn't looking.** Unknown verdicts went
24,932 → 25,333 with the code-span branch (**+401**) and → 25,349 with prose
(**+16**). The branch I was scrutinising contributed 4%; the one already shipped
contributed 96% — backticked table-dot-column refs (`commits` `git_root`,
`artifact` `slug`) that the prose guard rejects but never reaches. Proportionate
on a 25k base, non-gating, left alone deliberately.

Repo-wide: **51,833 refs, 16,398 resolved, exit 0.** Baseline **3842 / 0 / 50**
(from 3818, +24 tests).

### 2026-08-15 — SD-8 closed by git history; SD-9 names what the new surface costs

`experiments:c692f901`. The three citations left out of the sweep on purpose —
no rule could fix them — each turned out to have a different answer, and
**`git log` supplied all three**.

The most interesting was the semantic-search misleading-stack-error slug (named
without its extension here on purpose — spelled in full it is itself an
unresolvable citation, which is the third time in this stream that writing
*about* drift has produced drift). It was cited twice, and
`git log --diff-filter=A` across all refs returns **nothing**. The file was
never created. Both sites already carried prose giving the full reason, so
the citation was ornament and is dropped rather than replaced — inventing a
plausible substitute is precisely what `dont-fabricate-commit-rationale` warns
against. The second was pruned as a duplicate in `c6184884` and now cites the
`gotchas` memory that CLAUDE.md already makes canonical, naming the pruning
commit so the next reader does not go hunting. The third was never a path:
prose shorthand with a literal ellipsis.

Verified by the gate rather than by eye, which is the point of having built it —
both surviving citations report `verdict=resolved`, and `CLAUDE.md` resolves at
`src/fs/mod.rs:315` where the dead path used to sit.

**SD-9 records the price of the code-comment surface**, so a future session
asking "is this gate noisy?" gets a numerator and a denominator instead of an
impression. Two false-positive classes, both non-gating: *historical
provenance* (`//! Moved from … (Phase 6.2)` — the path is absent BECAUSE the
move happened, so the sentence is true and unresolvable by design, the same
family `cap_released_history` already exists for) and *fixture filenames in
test comments* (files the test creates in a tempdir, which never existed in the
repo). Against that: the surface found 95 real stale citations and one
genuinely dead reference.

**SD-7 was paid a third time here**, and this instance would have caused real
loss rather than friction. Closing SD-8 meant rewriting the whole `items`
array — but SD-8 had been *appended*, so it was not in the local params file,
and a naive rewrite would have deleted the very row being closed. Reconstructed
it first, then rewrote, then appended SD-9. Post-write check confirms all nine
ids survived.

### 2026-08-15 — SD-3 discharged and falsified; the duplication it was looking for is real, elsewhere, and already broken

SD-3's own `fix` field carried an instruction rather than a proposal: *"UNVERIFIED whether the four share a phase structure. Read the other three BEFORE proposing any extraction."* All four bodies were read in full at `1d22b715`. The hypothesis is **falsified**, and following the instruction anyway produced a better finding than the hypothesis would have.

**The four do not share a phase structure.** They split into two pairs, on an axis the grouping that produced them cannot see:

- **Query handlers** — `src/librarian/tools/find.rs`, `src/librarian/tools/context.rs`. Resolve scope → build filter → execute → assemble.
- **Single-artifact handlers** — `src/librarian/tools/get.rs`, `src/librarian/tools/update.rs`. Reject a removed parameter off the raw `args` before deserializing → parse → id to row → assemble a response by conditional key.

The hypothesised "apply overlay" phase is 2 of 4, not 4 of 4: `get` and `find` share the read-overlay (`shadow_main_pairs`, and both say so in comments), `update` uses a different worktree operation entirely (`resolve_write_target`, fork-on-first-write), and `context` has none. What all four genuinely share is not a phase but a **convention** — repair-and-continue, where a handler fixes or notices something and rides the notice back in the response rather than erroring. `src/librarian/tools/update.rs` and `src/librarian/tools/find.rs` both emit a `corrections` key from two completely unrelated mechanisms. That is a convention worth naming, not a function worth extracting.

**What the reading found instead (SD-10).** The scope-resolution prologue is written three times. `src/librarian/tools/find.rs:524` and `src/librarian/tools/workspace_state_at.rs:98` carry the same ~30-line block verbatim — down to the line-wrap position of the shared error string. `src/librarian/tools/context.rs:56` carries a **truncated copy**: it keeps the no-current-project fallback and drops both the `scope="all"` umbrella guard and the `All → Umbrella` alias. Since `apply_scope` maps `Scope::All` to no clause at all (`src/librarian/tools/scope.rs:75`), `context` runs unfiltered where its two siblings narrow to umbrella members.

This was **measured, not inferred**. Both calls were run against the live server: `artifact(find, scope="all")` reported `scope.applied = "umbrella"`; `librarian(context, scope="all")` reported `"all"` and returned an artifact belonging to a project outside the umbrella. The duplicated error string names the exact harm its missing copy permits — *"without one it crosses into unrelated workspace projects"* — and `find`'s own overflow hint actively recommends `scope="all"`, so the narrower meaning is what an agent learns and the wider one is what `context` delivers. Filed as `docs/issues/2026-08-15-context-scope-all-crosses-umbrella-boundary.md`.

**The instrument caveat repeats, one level up.** SD-3 already carried a warning that `legibility_scan` attributes cost too coarsely to rank *within* its group. The deeper limit is that it ranks by body size and per-symbol cost at all, so it can only ever see a law duplicated **within** one symbol. A law duplicated **across** symbols in different files is structurally invisible to it — which is why the highest-value structural finding here was invisible to the very ranking that opened the group, and why `src/librarian/tools/workspace_state_at.rs` (comfortably under the body budget, never flagged) turned out to hold one of the three copies.

SD-3 is marked `superseded`, closed by SD-10, rather than rewritten — a falsified premise that produced a good finding is worth keeping legible.

**And recording SD-10 surfaced SD-11.** Its params fields carry about ten
`path:line` citations, and none of them are in this file: a `grep` for `SD-10`
against the on-disk tracker returns only the two mentions in this prose. Params
live in the catalog's augmentation table, and `render_template` output is
produced at read time rather than written to disk — so `audit_doc_refs`, which
scans files, cannot see them. That is a third citation surface, after prose
(SD-1) and code comments (SD-1b), and it is the one the project's most heavily
cited trackers actually use. Recorded, not fixed: no params citation has been
observed stale yet, and all three candidate fixes have real costs — see SD-11.
The gap is bounded, though: only *augmented* trackers are affected.
`docs/trackers/bug-fix-session-log.md` and
`docs/trackers/reconnaissance-patterns.md` are plain markdown, verified
not-augmented this session, and are scanned in full.
