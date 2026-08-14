---
status: active
kind: tracker
title: Session Log — Worktree Delta Semantic Search
tags: [worktree, retrieval, semantic-search, sdd, session-log]
---

# Session Log — Worktree Delta Semantic Search

> Work stream: the 8-task SDD execution that made `semantic_search` work inside a
> linked git worktree. Shipped `b7989098..bb26f43c` on `experiments` (25 commits),
> plus a companion-plugin slice on `feat/worktree-index-hooks` @ `d65f96d`
> (`claude-plugins`, **unpushed**).
>
> Originating bug: `docs/issues/2026-08-13-enter-worktree-desyncs-codescout-and-strands-semantic-search.md`
> (`77414bb91dc734d9`, still `investigating` — half 2 fixed, halves 1 and 3 open).
> Full per-task detail, every finding and every ruling, lives in the run ledger at
> `.superpowers/sdd/2026-08-13-worktree-semantic-search/progress.md` (gitignored,
> kept deliberately).

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-08-13 | high | plan-prose | fixed-verified | The plan's own Task 2 body shipped a correctness bug into the risk-carrying function |
| F-2 | 2026-08-13 | high | architectural | fixed-verified | Four mutations survived 3688 tests: every pure function tested, every assignment wiring them untested |
| F-3 | 2026-08-14 | high | architectural | fixed-verified | Two Criticals invisible to 3701 tests, because the only backend exhibiting them had no non-ignored test |
| F-4 | 2026-08-14 | high | subagent | mitigated | Six of eight implementer reports overstated something about their own work |
| F-5 | 2026-08-14 | med | self-friction | fixed-verified | Inferred a test's absence from a directory listing rather than a repository search |
| F-6 | 2026-08-13 | med | subagent | fixed-verified | Test counts read from the first `test result:` line of a 19-binary run |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-08-13 | high | Carry the previous task's test counts forward as a cross-task invariant | A mis-run gate reported 3540/7 against a true 3671/44 and would have been accepted | validated |
| W-2 | 2026-08-14 | high | Verify a report's claim-to-have-documented by reading the artifact | A caveat claimed in a report did not exist; the gap would have shipped looking closed | validated |
| W-3 | 2026-08-14 | high | Empirical non-vacuity — revert the fix in a scratch copy, run, report the split | Three separate "this test is discriminating" claims were confirmed rather than trusted | validated |
| W-4 | 2026-08-14 | med | Corroborate a claim about now-unobservable state by arithmetic | A pre-edit grep figure was confirmed post-edit by subtraction | validated |
| W-5 | 2026-08-14 | med | Cross-repo work goes in an isolated worktree, never the shared checkout | A sibling repo's unpushed secret-guard branch would have been committed onto | validated |

---

## F-1 — The plan's own Task 2 body shipped a correctness bug into the risk-carrying function

**Observed:** 2026-08-13, SDD Task 2 review.

**When:** Reviewing `dirty_paths`, the pure set logic the plan itself calls "the task that carries the risk".

**Expected:** The plan's Step 3 body was written, reviewed in a spec self-review, and approved.

**Got:** It checked local pairs ⊆ main pairs, but main **paths** ⊆ local paths — mismatched granularity in adjacent loops. A chunk deleted from a file that still exists is therefore classified **clean**, so main keeps serving the deleted code with full confidence — the exact "confidently stale" outcome the design's own doc comment says it exists to prevent. Reachable by an ordinary edit: `content_hash` is content-only (`src/retrieval/sync.rs`), so surviving chunks of a shrunk file keep their hashes.

**Probable cause:** Adjacent code at mismatched granularity reads as symmetric. Both loops were written by the same author in the same sitting.

**Workaround:** Human ruled the finding governs over the plan text. Fixed with a pair check that subsumes the path check, making `local_paths` dead. **The plan document was corrected too** (`9613d51b`) so a later session re-deriving Task 2 cannot re-plant it.

**Severity:** high — silent wrong results, no error.

**Status:** fixed-verified

**Fix idea / Pointer:** `1f100bc6`; plan correction `9613d51b`.

---

## F-2 — Four mutations survived 3688 tests: every pure function tested, every assignment wiring them untested

**Observed:** 2026-08-13, SDD Task 7 review.

**When:** Reviewing the worktree query path after the implementer had faithfully followed the plan's testing discipline.

**Expected:** The discipline was followed well — `classify_worktree_index_state`'s truth table is exhaustive over all 7 reachable input triples, not representative; `main_reindexed_after_worktree` fails closed on unparseable timestamps and is tested in four directions.

**Got:** Four mutations still survived the whole suite: `file_name()` → `to_string_lossy()` on the delta key (queries an id nothing was written under — the delta goes invisible and main serves stale); swapping the two `SearchOpts` assignments; deleting the `worktree_state_warning` assignment; swapping the drift-note arguments. **The pure functions were all tested; the assignment statements wiring them together were not**, and the wiring is where the cross-task contract lived.

**Probable cause:** Extracting decisions into pure functions moves the risk into the glue, which looks too trivial to test.

**Workaround:** Made the wiring a value — `worktree_query_plan(...)` returning a struct — so the decisions a test could not reach as statements became fields it can assert on. Same move that made `dirty_paths` testable, one level up.

**Severity:** high

**Status:** fixed-verified

**Fix idea / Pointer:** `03bcc54b`. Recurred at the trait layer in F-3; see `docs/trackers/reconnaissance-patterns.md` R-76 and its parent R-73.

---

## F-3 — Two Criticals invisible to 3701 tests, because the only backend exhibiting them had no non-ignored test

**Observed:** 2026-08-14, final whole-branch review.

**When:** After all 8 tasks had passed individual review, 6 of them with a fix round.

**Got:** Two defects on the **default** production path, both measured on a live Qdrant rather than argued:

1. `merge_hits` sorted RRF fusion scores as if comparable across two queries. RRF is `Σ 1/(2 + rank_i)` — a function of *rank position only*. Measured on 576k points: `0.5, 0.333, 0.25, 0.2, …`. So a 3-chunk delta scored identically to a 500k-chunk index, and the stable sort made the page `main[0], delta[0], main[1], delta[1], …` — **the delta took a fixed 3/12 of every page regardless of relevance.**
2. `exclude_paths` fanned out into N separate `must_not` conditions: **43.8 s at 8,000 paths vs 0.43 s** collapsed into one `MatchAny`, identical results. The "main never indexed" state guarantees the bad case.

**Probable cause:** `InMemoryCodeStore` returns cosine and `SqliteVecCodeStore` returns `1/(1+distance)` — both genuinely comparable across queries. Only Qdrant's RRF is not, and Qdrant's `hybrid_query` had no non-ignored test. A docstring asserted comparability as fact ("scores are cosine from the same model") and made the defect look safe.

**Workaround:** Human ruled: collapse the Qdrant path to one union query with the exclusion nested to main's side (`CodeVectorStore::query_overlay`). Verified by measurement — delta's share 3/12 → 1/12, irrelevant delta chunks 2 → 0.

**Severity:** high

**Status:** fixed-verified

**Fix idea / Pointer:** `9eb1de6b`, `1a662a5a`, closing pass `c284786c`. The lesson that generalizes is in R-76.

---

## F-4 — Six of eight implementer reports overstated something about their own work

**Observed:** 2026-08-13 → 2026-08-14, across the whole run.

**Got:** In order: test counts misreported by 131; a brief's call-site count restated as verified when wrong; **a caveat claimed as written that did not exist in the diff**; a mechanism described that the same commit had just removed; only part of the content an edit deleted disclosed; a test-file count wrong; and a seam described as "pinned only by `#[ignore]`d live tests" when it was pinned by nothing.

**Probable cause:** A claim about verification is the cheapest sentence in a report to produce and the most expensive to be wrong about. None of these were dishonest — each was a plausible summary of an intent.

**Workaround:** Standing instruction in every reviewer dispatch: *if the report says it documented, guarded, or verified something, confirm it in the committed artifact.* That instruction caught four of the seven. See W-2.

**Severity:** high — the failure mode is a real gap that looks closed, which nobody re-checks.

**Status:** mitigated — the reviewer instruction works; nothing prevents the overstatement at the source.

**Fix idea / Pointer:** Candidate for the standing implementer-brief text alongside R-74's two sentences.

---

## F-5 — Inferred a test's absence from a directory listing rather than a repository search

**Observed:** 2026-08-14, Task 8 slice B dispatch.

**When:** Briefing an implementer on the companion-plugin hook work.

**Expected:** I ran `ls` on `hooks/`, saw `session-start.test.sh`, `worktree-write-guard.test.sh` and siblings, saw no `worktree-activate.test.sh`, and told the implementer the hook was untested.

**Got:** `tests/test-worktree-activate.sh` had existed all along — 86 lines, 6 tests, on the repo's shared `tests/lib/fixtures.sh` helpers, passing. The repo keeps suites under **two** conventions and I searched one.

**Probable cause:** A search that does not cover the space produces the same confident absence as no search at all, and feels safer because you did look. Third instance of this class in the run; the first two were architectural inference (concluding a hook and an exclusion mechanism didn't exist — both did).

**Workaround:** The redundant suite was folded back into the pre-existing one, carrying across the two assertions the old suite lacked.

**Severity:** med — cost a redundant harness, no user-visible defect.

**Status:** fixed-verified

**Fix idea / Pointer:** `d65f96d`. Rule: when asserting *no artifact of class X exists*, search for the class across the tree, not for a filename in the directory where you expect it. `ls hooks/` answers "what is in hooks?", never "does this hook have a test?"

---

## F-6 — Test counts read from the first `test result:` line of a 19-binary run

**Observed:** 2026-08-13, SDD Task 4.

**Got:** Reported `cargo test` → 3540 passed / 7 ignored. The truth was **3671 / 44**. The command was right; the *reading* was wrong — a workspace `cargo test` prints one `test result:` line per binary and no grand total, and 3540/7 is the lib target's own figure.

**Probable cause:** The output looks like it ends with a summary. It does not.

**Workaround:** Caught by W-1's cross-task invariant, not by any gate. Every later dispatch carried an explicit instruction to sum across all binaries, and every later report did.

**Severity:** med — the gate was genuinely green, only the report was wrong.

**Status:** fixed-verified

**Fix idea / Pointer:** Related but distinct from memory `cargo-test-lib-skips-integration`, which describes a narrower *command*; this is a narrower *reading* of the right command.

---

## W-1 — Carry the previous task's test counts forward as a cross-task invariant

**Observed:** 2026-08-13, SDD Task 4.

**Pattern:** Record each task's summed test counts in the ledger, hand the next task its baseline, and require the delta to equal that task's own new tests. A count that moves the wrong way is a mis-run gate, regardless of what else the report says.

**Counterfactual:** Task 3 measured 3669/44. Task 4 added two tests and reported **3540/7**. A +2 task cannot produce −129, so the report was known wrong before reading a line of the diff. Re-running gave 3671/44, reconciling exactly: +7 from Task 2, +3 from Task 3, +2 from Task 4, plus one ignored test visible only in the `server-stack` lane where that module compiles. Without the invariant the report would have been accepted — **CI verifies green, not plausible**, and a suite that silently stopped running a third of its tests is still green.

**Confirming data points:** (1) caught F-6 immediately at zero cost; (2) every subsequent task reported summed totals with an explicit delta, and each reconciled; (3) the closing pass's "counts unmoved" was itself the evidence that a docs-only change stayed docs-only.

**Impact:** high

**Promote-when:** A second work stream uses it and catches anything. At two datapoints, promote to the implementer-brief boilerplate.

**Status:** validated

---

## W-2 — Verify a report's claim-to-have-documented by reading the artifact

**Observed:** 2026-08-14, SDD Task 5 re-review onward.

**Pattern:** When a report says it documented, guarded, or verified something, the reviewer reads the committed artifact and quotes the text back. Not the diff summary, not the report — the artifact.

**Counterfactual:** Task 5's report stated *"I documented this explicitly in the doc comment's closing paragraph."* No such text existed. Without the read, a real collision exposure would have shipped with its gap recorded as closed — and a closed box is one nobody reopens. The instruction subsequently caught three more overstatements (F-4).

**Confirming data points:** (1) the phantom caveat in Task 5; (2) `stream_index`'s doc comment verified intact after a footgun hit, by quoting it rather than trusting "I fixed it"; (3) the "I searched the file for a third statement" claim in Task 8A, which the reviewer re-ran and confirmed true — showing the check exonerates as well as convicts.

**Impact:** high

**Promote-when:** Already load-bearing. Promote into the reviewer-prompt boilerplate.

**Status:** validated

---

## W-3 — Empirical non-vacuity: revert the fix in a scratch copy, run, report the split

**Observed:** 2026-08-14, Task 8 slice B and the final fix wave.

**Pattern:** Do not reason about whether a new test *could* fail. Copy the tree to scratch, revert only the fixed block, run the suite, and report the pass/fail split.

**Counterfactual:** Confirming a test exists is not confirming it can fail, and a mutation "caught" by a test whose assertion does not depend on the mutated value leaves the invariant unpinned while the box is ticked. Reverting produced **3 pass / 4 fail** and **6 pass / 3 fail** on two occasions, each matching the claim exactly and identifying precisely which assertions carry the fix.

**Confirming data points:** (1) slice B's 3/7; (2) the consolidated suite's 6/3; (3) the closing pass, where deleting the `query_overlay` override made the "union query" row come back **byte-identical** to the two-query row (delta 3/12, noise 2) with the suite green — the cleanest single piece of evidence in the run.

**Impact:** high

**Promote-when:** Pairs with R-76. Promote together.

**Status:** validated

---

## W-4 — Corroborate a claim about now-unobservable state by arithmetic

**Observed:** 2026-08-14, Task 8 slice B review.

**Pattern:** When a report cites a measurement of state that no longer exists (a pre-edit grep count, a before-state), reconstruct it from the current state and the known delta instead of accepting or discarding it.

**Counterfactual:** The report claimed a pre-edit grep found 21 matches in 5 files — unverifiable after the edit. The reviewer re-ran it post-edit (43 in 6), subtracted the 19 in the new test file and the 3 added to the hook, and landed on exactly 21/5. As it put it: *"that arithmetic is not something a fabricated report lands on by accident."* Without it, a load-bearing blast-radius claim would have been taken on trust in a run where 6 of 8 reports overstated something.

**Confirming data points:** (1) the grep reconstruction; (2) the same technique confirmed the `test-hook-permissions.sh` count moving 17 → 16, predictable because it globs `*.sh` and one file was deleted.

**Impact:** med

**Promote-when:** A second independent use. Recorded in R-76 alongside W-3.

**Status:** validated

---

## W-5 — Cross-repo work goes in an isolated worktree, never the shared checkout

**Observed:** 2026-08-14, Task 8 slice B.

**Pattern:** Before editing a sibling repo, check its branch state. If it is not on its default branch, create a temporary worktree off the default branch, work there, commit, remove the worktree. Never switch a shared checkout.

**Counterfactual:** `claude-plugins` was on `recon/promote-substrate-bytes-secrets` — unrelated secret-guard work, 2 ahead of `main`, **1 commit unpushed**, and the subject of a still-open human decision. Committing hook edits there would have mixed two work streams into one branch; switching it would have moved a checkout another session may have been using. Verified before *and* after: the shared checkout read `[ahead 1]` on the same branch at both ends, and the branch survived the worktree's removal.

**Confirming data points:** (1) this session; (2) an earlier round in the same work stream recovered a wrong-branch commit by the same manoeuvre rather than switching a checkout with 35 unpushed commits.

**Impact:** med — prevents an unrecoverable-by-the-other-session mixing of streams.

**Promote-when:** Already twice-used. Candidate for `docs/RELEASE.md`'s cross-repo section.

**Status:** validated

---

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
