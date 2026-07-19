---
id: fee90da3055f3e19
kind: tracker
status: draft
title: Session Log — PR Review
tags:
- reconnaissance
- pr-review
- session-log
topic: pr-review
---

# Session Log — PR Review

> Two-sided observation log for the "review open PRs" work stream (PRs #7
> and #8 against `experiments`). Captures frictions (F-N) and wins (W-N)
> from reconnaissance performed while reviewing.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-07-20 | low | plan-prose | open | PR #7 cites a bug file that doesn't exist anywhere in the repo |
| F-2 | 2026-07-20 | high | plan-prose | open | PR #8's description covers ~4 items; diff silently includes a 627-line indexer.rs rewrite + 2 more undisclosed fixes |
| F-3 | 2026-07-20 | med | codescout-tool | fixed-verified | Initial PR #7 review declared "no blocking correctness issues"; independent clippy run found one |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-07-20 | high | Diff PR's claimed file list against `gh`'s actual file list before reading hunks | Would likely have missed or badly delayed the scope-mismatch finding (F-2) by reading hunks in file order instead | validated |
| W-2 | 2026-07-20 | high | Independent fmt/clippy/test run in an isolated worktree, not trusting PR self-reports | Would have pushed a broken `clippy -D warnings` gate onto `experiments` | validated |

---

## F-1 — PR #7 cites a bug file that doesn't exist anywhere in the repo

**Observed:** 2026-07-20, reviewing PR #7 (`fix-embedder-connect-error`) via the `/review` skill.

**When:** Verifying the PR's claimed bug-fix cross-reference before writing up the review.

**Expected:** PR body + commit message cite `docs/issues/2026-07-13-semantic-search-misleading-stack-error-on-missing-env.md` as the bug being fixed — implying the file exists somewhere in the repo (open, to be archived after merge, per this project's bug-tracking convention).

**Got:** `git log --all --oneline -- '*misleading-stack-error*'` returned nothing; a filesystem search for `*misleading*` under `docs/issues/` also returned nothing. The cited bug file does not exist on any branch, tracked or untracked.

**Probable cause:** The PR was authored citing a path as if the "capture on notice" step (CLAUDE.md bug-tracking convention) had already happened, but the file was never created.

**Workaround:** Flagged in the review as a required fix before merge; no code workaround needed.

**Severity:** low — no tool call failed, nothing cascaded into other work. The cost is to this project's own bug-tracking hygiene: a "fix" with no filed bug leaves no paper trail to later archive.

**Status:** open

**Fix idea / Pointer:** PR #7 (`github.com/mareurs/codescout/pull/7`) should add the missing `docs/issues/*.md` file before merge, or correct the citation to the real path if one exists under a different name.

---

## F-2 — PR #8's description covers ~4 items; diff silently includes a 627-line indexer.rs rewrite + 2 more undisclosed fixes

**Observed:** 2026-07-20, reviewing PR #8 (`fix/symbols-overview-lsp-test-grep-glob-fixes`) via the `/review` skill.

**When:** Cross-checking `gh pr view 8 --json files` (actual changed-file list) against `gh pr view 8 --json body` (the PR's own description), before deep-diving individual diff hunks.

**Expected:** PR body describes exactly four categories of change (symbols `include_body`, Windows LSP test, grep glob tests, `edit_markdown` CRLF fallback) plus a vague "docs: add previously-untracked historical issue reports" bullet — implying the non-doc production changes are narrowly scoped to two tool files.

**Got:** the diff also rewrites `src/librarian/indexer.rs` (+627/−42): a real orphan-cleanup data-loss fix (catalog rows for still-existing, merely-ignored files were being deleted) PLUS two new features (`force_include` config knob, `force_embed`/`reembed` backfill flag) PLUS an opt-in, cross-project `artifact_vec` table drop-and-rebuild migration path — none named anywhere in the PR body. It also adds a second, independent CRLF-tolerant-match implementation in `src/tools/edit_file/mod.rs` (+97, for non-markdown files) with no corresponding `docs/issues` file, plus Windows path-normalization fixes in `src/agent/mod.rs` and `src/tools/config/mod.rs`. All of this is silently folded under the one vague "docs" bullet.

**Probable cause:** The PR body was written to describe the work as the author understood it at some earlier point in the branch's life and was never updated to reflect everything actually staged into it by the time the PR was opened.

**Workaround:** Read every changed file's diff independently rather than trusting the PR body's implied file-scope; called this out explicitly as the top finding in the review.

**Severity:** high — a reviewer (human or agent) trusting the PR body alone would approve a change with far more risk (core catalog-deletion logic, a destructive-but-opt-in shared-table migration) than advertised. Matches the rubric's "wrong code merged... hidden state change" case.

**Status:** open

**Fix idea / Pointer:** PR #8 (`github.com/mareurs/codescout/pull/8`) — update the PR description to name the indexer.rs orphan-cleanup fix, the `force_include`/`reembed` features, the `artifact_vec` migration path, and the separate `edit_file` CRLF fix, each with its own bug-file reference where one doesn't already exist.

---

## W-1 — Diffing PR's claimed file list against its actual file list caught the scope-mismatch before a hunk-by-hunk read

**Observed:** 2026-07-20, start of PR #8 review, before reading any diff hunk.

**Pattern:** Before reading a large PR's diff hunk-by-hunk, first run `gh pr view <n> --json files -q '.files[].path'` and `gh pr view <n> --json body -q .body`, and compare the raw file list against what the PR description claims to touch. A mismatch is cheap to spot at that level and expensive to notice by reading hunks in file order.

**Counterfactual:** Without this check, the review would likely have read files in diff order, treated `indexer.rs` as "just the next file in a big batch," and either missed connecting its 627-line rewrite to a scope-disclosure problem, or run out of attention/budget partway through before reaching that conclusion. The eventual finding (F-2) became the single most important point in the whole review; it was only caught because the file-list comparison happened *first*, before any hunk was read.

**Confirming data points:**
1. This session, PR #8 — comparing `gh pr view 8 --json files` against the PR body immediately surfaced 6 unmentioned files (`indexer.rs`, `edit_file/mod.rs`, `edit_file/tests.rs`, `agent/mod.rs`, `config/project.rs`, `tools/config/mod.rs`+tests) before any diff was read.
2. Pending — a second PR review (any project) that repeats this check before hunk-reading would confirm the pattern generalizes.

**Impact:** high — directly produced the review's top finding (F-2) and reframed the entire review's risk assessment.

**Promote-when:** A second PR review where this same "file-list-first" check catches an undisclosed-scope PR before deep-diving. At 2 datapoints, promote into the `/review` skill's own instructions as a required first step: "before reading diff hunks, diff the PR's claimed file list against `gh`'s actual file list."

**Status:** validated

---

## Status vocabulary

See `docs/templates/session-log.md` for the canonical definitions of the
Status columns used above (`open`, `mitigated`, `fixed-verified`,
`promoted-to-bug-tracker`, `pinned-as-eval-baseline` for frictions;
`validated`, `promoted-to-permanent-docs`, `archived` for wins).

## F-3 — Initial PR #7 review declared "no blocking correctness issues"; independent clippy run found one

**Observed:** 2026-07-20, merging PR #7 + PR #8 into `experiments` after the earlier `/review` pass.

**When:** Running `cargo clippy --all-targets -- -D warnings` on the merged worktree, as a verification step before pushing — not part of the original review.

**Expected:** The earlier review of PR #7 (this same session) concluded the diff was "solid, narrowly-scoped, well-tested" with no correctness-blocking issues — only a process/hygiene gap (missing bug file).

**Got:** `clippy::items_after_test_module` fired on `src/retrieval/embedder.rs`: PR #7's new `#[cfg(test)] mod tests { ... }` block was inserted between the `DenseEmbedder` trait and the `HttpDenseEmbedder` struct/impl — production items after a test module, denied under `-D warnings`. This was visible in the original diff hunk (I read it) but I evaluated the test's *logic* and never asked "would this specific placement pass clippy." The PR body's own claim ("cargo clippy -- -D warnings: clean") only covered PR #8, not PR #7 — PR #7's body made no such claim at all, which should have been the signal to verify it independently rather than infer it was fine.

**Probable cause:** Code review evaluated logic and test correctness but did not simulate the lint gate the diff would actually be pushed through. Reading a diff hunk is not the same as running the tool that gates it.

**Workaround:** Fixed directly during merge — relocated the test module to the end of the file (`edit_code` remove + replace) and ran `cargo fmt`. Re-verified clippy clean afterward.

**Severity:** med — would have caused a failed `cargo clippy -D warnings` gate for the next person working on `experiments` (or CI, if one existed); contained to one file, one fix, no cascade.

**Status:** fixed-verified — fix landed in commit `8389e0b4` (merged to `experiments` at `bb693446`), clippy confirmed clean after.

**Fix idea / Pointer:** For future PR reviews, don't rely on a PR body's self-reported `cargo fmt`/`clippy`/`test` claim without either (a) independently running those commands on the branch, or (b) explicitly flagging in the review when the PR body makes NO such claim at all (silence isn't evidence either way).

---

## W-2 — Independent clippy/fmt/test run (not trusting PR self-reports) caught a real merge-blocking bug

**Observed:** 2026-07-20, same merge session as F-3.

**Pattern:** Before pushing a merge of externally-authored PRs to a shared branch, build the merge in an isolated worktree and independently run `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib` — even when a PR body claims these already passed. A claim about a different commit (the PR's own branch tip) does not cover what the branch looks like after merging alongside a sibling PR, or catch what the PR body simply never asserted.

**Counterfactual:** Without this check, `gh pr merge` on both PRs (trusting PR #8's "all green" self-report and PR #7's silence on the topic) would have landed `clippy::items_after_test_module` directly on `experiments` — breaking the `cargo clippy -D warnings` pre-commit gate for the next contributor, discovered only when someone else's unrelated work failed to commit.

**Confirming data points:**
1. This session — F-3, caught before push, zero blast radius.
2. Pending — a second merge session where this same independent-verification step catches a self-report gap.

**Impact:** high — prevented landing a broken lint gate on the shared integration branch.

**Promote-when:** A second PR-merge session where independent fmt/clippy/test verification (not trusting the PR body) catches a real gap. At 2 datapoints, promote to this project's RELEASE.md / Standard Ship Sequence as a required step for any multi-PR merge, not just cherry-picks to `master`.

**Status:** validated

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     artifact(action="update", id=<this artifact's id>,
              patch={body_edits: [{heading: "## Template for new entries",
                                    action: "insert_before",
                                    content: "## F-N — title\n..."}]})
     Also update the matching Index / Wins Index table row at the top. -->
