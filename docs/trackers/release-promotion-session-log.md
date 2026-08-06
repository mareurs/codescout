---
id: e6697aea0ee3ea37
kind: tracker
status: active
title: Release Promotion Session Log
tags:
- session-log
- release-promotion
- reconnaissance
---

> Per-work-stream friction/win log for the `experiments` -> `master` promotion
> (79-commit fast-forward, local range `eca9902e..339cea47`). Copied from
> `docs/templates/session-log.md`; see that file for the full status vocabulary
> and entry templates.

> Per-work-stream friction/win log for the recurring `experiments` -> `master`
> promotion. Copied from `docs/templates/session-log.md`; see that file for the
> full status vocabulary and entry templates.
>
> **Round 1 — 2026-07-02.** 79-commit fast-forward, local range
> `eca9902e..339cea47`. Shipped. Produced F-1.
> **Round 2 — 2026-08-06.** 397-commit fast-forward. **Not shipped** — see the
> Resume block below. Produced F-2, F-3, W-1.

## Resume — round 2, written 2026-08-06 for session compaction

> **SUPERSEDED in part, 2026-08-06 — item 1 ("push, then re-read CI") is DONE.** Pushed
> `d7988aca..99695a10` (22 commits) to `origin/experiments`. CI now runs on current HEAD
> for the first time since 2026-08-03; every verdict before this described code 21
> commits stale, which is why the old "11 of 15 red" picture was unactionable.
>
> **CI state now: 10 green / 5 red** (run `31091169757` on `99695a10`), from a baseline of
> **4 green / 11 red** (run `30852803569`, 2026-08-03).
>
> | Cleared | Still red |
> |---|---|
> | Tool Docs Sync | Audit Doc Refs — `docs/issues/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md` |
> | ubuntu/no-features | Test (windows-latest / default) — WIN-28 |
> | ubuntu/local-embed | Test (windows-latest / no-features) — WIN-28 |
> | macos/no-features | Test (windows-latest / local-embed) — WIN-28 |
> | macos/local-embed | Windows-gnu cross (MinGW + wine) — WIN-29 |
> | Clippy | |
>
> **The state change that matters for the merge decision: there is no longer an
> undiagnosed red cell.** All five have an open bug file, and three of the five are one
> bug (WIN-28) wearing three hats. Previously six of the reds were feature-gate rot,
> invisible to a default-features build by construction.
>
> **Clippy needed a fix nobody could have predicted locally.** It stayed red on run
> `31089996833` against code nobody had touched: the log's help URLs name
> `rust-clippy/rust-1.97.0` while this machine runs `clippy 0.1.95`. Three pre-existing
> patterns (`question_mark` ×2 in `src/tools/symbol/call_edges/resolver.rs`, `for_kv_map`
> in `src/tools/symbol/call_graph/mod.rs`) are lints 1.97 emits and 1.95 does not. Fixed
> in `99695a10`; Clippy green on the next run.
>
> **Standing hazard this exposed, unresolved and needing a policy call:** there is no
> `rust-toolchain.toml`, and CI uses `dtolnay/rust-toolchain@stable`, so CI re-resolves
> `stable` on every run and can acquire new lints with **zero commits**. Clippy therefore
> breaks on the calendar, and a locally-green `cargo clippy -- -D warnings` is not a
> predictor of the CI job. Two options, both the maintainer's to pick: pin a toolchain in
> the repo (local == CI, upgrades become deliberate), or keep floating and accept periodic
> lint debt. Nothing was imposed here.
>
> Items 2-5 of the Resume below stand unchanged.

Wipe and rewrite this block each session. Everything below is verified, not assumed.

### Git state

- Branch `experiments` at **`553e618e`**; 10 commits from this session
  (`ca442498..553e618e`).
- `git rev-list --left-right --count master...experiments` → **`0  397`**. Master is a
  strict ancestor, so the promotion is a **fast-forward**, not a cherry-pick. Re-check
  this before merging — a non-zero left number means master diverged.
- **17 commits unpushed.** This matters: the last CI run (`30852803569`, 2026-08-03) is
  from *before* this session, so its verdicts are stale for 6 of the jobs.
- Procedure to follow: `docs/RELEASE.md` § *Large-Cohort Promotion (Fast-Forward)*.
  Do **not** use the Standard Ship Sequence — it cherry-picks single commits and would
  mint 397 new SHAs, orphaning every SHA citation in `docs/issues/` and the trackers.

### Gate state — green locally, all six steps exit 0 at `553e618e`

```bash
cargo fmt --check
cargo clippy -- -D warnings                              # CI's exact invocation
cargo test                                               # 3344 lib tests
cargo check --no-default-features --all-targets           # warning-free
cargo test --no-default-features                         # 2477 tests
cargo test --features local-embed --no-default-features
```

The last three are the ones that matter and the ones CLAUDE.md's three-command line
omits — they are where this session found five defects. See F-3, T-16.

### What still blocks a green merge (the "remaining issues")

Ranked by what to do first.

1. **Push, then re-read CI.** Cheapest possible move and it may close two items for
   free. The stale run predates `7938d68b` (feature-gate compile fixes) and `be75e705`
   (three behaviour-gated tests). Expected to clear: `Clippy`, `Tool Docs Sync`, and the
   four non-Windows `no-features` / `local-embed` cells. Expected to stay red: the three
   Windows cells, `Audit Doc Refs`, and possibly `windows-gnu`.
2. **`docs/issues/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md`**
   (open, high; WIN-28). Nine tests fail on `windows-latest`, all in code this cohort
   added — 7 catalog `rehome`/`prune_missing`, the `like_escape` idiom guard, the index
   lock. Linux and macOS pass the same config, so path semantics not logic. **Blocked on
   a Windows runner**: the per-test panic output was never captured, and guessing at path
   normalisation risks fixing the tests rather than the code. Its Resume names
   `validate_rehome_gates` as the narrowest starting point.
3. **`docs/issues/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`**
   (open, high). The `audit-doc-refs` job is a hard gate (`--fail-on high`, no
   `continue-on-error`) and all 18 high-severity findings are **extractor false
   positives**: `Type/method` (codescout's own `name_path` syntax, 8 of 18), GitHub
   `org/repo` slugs, ellipsis-elided external paths, plus an mdBook relative-link class
   at `med`. **Do NOT "fix" this by editing the three ADRs** — their prose is correct.
   Fix the extractor (start with the `name_path` shape: 18 → 10) or drop the gate and say
   so in the workflow comment, which currently claims all hi-sev findings are reconciled.
   Cross-references the earlier `2026-07-28-audit-doc-refs-json-pointer-false-positive.md`,
   which has priority.
4. **`docs/issues/2026-08-06-windows-gnu-cross-job-red-undiagnosed.md`** (open, medium;
   WIN-29). Undiagnosed by choice. Leading hypothesis after a ledger query: WIN-28's nine
   failures are not in `scripts/build-windows.sh`'s wine skip-list, so this is likely
   item 2 wearing a second hat — confirm before fixing twice, and `graft` it into WIN-28's
   file if so.
5. **`docs/issues/2026-08-06-ast-chunker-recursion-duplicates-leading-gap.md`** (open,
   medium). Pre-existing, not a regression, does not block the merge. The inner-node
   recursion re-derives gaps against the whole file with `prev_end` reset to 0, emitting a
   chunk that duplicates every line before the container. Fix plan is in the file; it
   changes the emitted chunk set for every decomposed container in every language and
   invalidates existing indexes, so it wants its own change.

### Do NOT re-do these — decisions already made with evidence

- **`docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` stays `open`.** Its Fix
  candidate 1 was implemented at `ca442498` (7 tests, both behaviours mutation-verified),
  but that file requires validation against
  `docs/research/2026-05-06-retrieval-stack-benchmark.md` before landing — *"must be
  measured, not assumed"* — and **the benchmark was never run**. It also records an
  ordering decision (throughput work first, being vector-identical) that `ca442498`
  jumped. Running that benchmark is the only thing that turns it `fixed`. See F-2.
- **The seven new manual pages keep their `⚠ Unreleased` callout through the merge.**
  Removal triggers at **release**, not at merge — master is not crates.io. Mechanical:
  `grep -rl 'Unreleased — on the `experiments` branch only' docs/manual/src/`, then
  `docs/RELEASE.md` § *Release Cycle* step **1b**.
- **New subsystem docs go straight into the main manual**, not staged under
  `docs/manual/src/experimental/`. The staging-then-move flow measured 0/62 compliance
  this cohort; see the revised buddy memory `experimental-docs-lifecycle`.
- **`docs/FEATURES.md` is archived** to `docs/archive/FEATURES.md` (zero live inbound
  refs; moved through the catalog, id preserved).

### Where the rest of the record lives

| Surface | What it holds from this round |
|---|---|
| `CHANGELOG.md` `[Unreleased]` | All ten feature clusters + notable fixes; the canonical cohort list |
| `docs/RELEASE.md` | Large-Cohort Promotion procedure; Release Cycle step 1b |
| `docs/ROADMAP.md` standing backlog | The local five-step gate alias proposal |
| `docs/trackers/reconnaissance-patterns.md` | R-55 (`miss → proposal`): query the ledger by file identity, not task category |
| `docs/trackers/codescout-usage-frictions.md` | U-30 (IL3 ×4 + the orphaned deny hook), U-31 (shell-on-source blocks CI-gate repro), U-32 (`.buddy` write asymmetry) |
| `docs/trackers/codescout-usage-hookify.md` | H-1 stale flag **resolved** — deny hook orphaned by the `.sh` → `.mjs` port |
| `docs/trackers/tool-usage-patterns.md` | T-14/15/16, plus T-013 params row backfilled |
| `docs/trackers/windows-platform-support.md` | WIN-28, WIN-29 |
| `docs/RELEASE-TODO.md` | CI-gate record corrected (`audit-doc-refs` is a hard gate, and it is red) |

### Unverified / owed

- `mdbook` is not installed here, so the book was never build-verified. Cross-link
  targets were checked against `SUMMARY.md` by hand.
- The retrieval benchmark for the chunk floor (see *Do NOT re-do*).
- `docs/RELEASE-TODO.md`'s "Error message path sanitization" item could not be verified —
  `strip_project_prefix` does not exist under that name; the mechanism was not chased.

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-07-02 | med | codescout-tool | promoted-to-bug-tracker | `audit_doc_refs` flags legitimate cross-repo hook paths as `missing`/`high`, inconsistently |
| F-2 | 2026-08-06 | high | process | mitigated | Bug ledger never queried at the seam — reimplemented a filed fix and skipped its stated precondition |
| F-3 | 2026-08-06 | med | process | fixed-verified | Treated CLAUDE.md's three local commands as "the gate"; the merge gate is 15 CI jobs, red for 3 weeks |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-08-06 | med | Dump actual output of an *internal pure function*, don't derive it from source | Reasoning predicted the wrong padding chunk; the dump surfaced a whole-file duplicate + a second pre-existing bug | validated |

---

## F-1 — `audit_doc_refs` flags legitimate cross-repo hook paths as `missing`/`high`, inconsistently

**Observed:** 2026-07-02, pre-dispatch reconnaissance before trusting a fork's doc-staleness sweep of the 79-commit `experiments`->`master` promotion.

**When:** About to rely on a fork subagent's brief that told it to treat every `audit_doc_refs` finding with `verdict=missing AND severity=high` (outside a named ADR exclusion list) as real staleness, across all 58 changed markdown files, including `docs/architecture/companion-plugin.md`.

**Expected:** A `missing`/`high` finding means the referenced path genuinely doesn't exist under the active project and is real drift worth fixing.

**Got (scouted):** Ran `librarian(action="audit_doc_refs", paths=["docs/RELEASE.md","docs/architecture/companion-plugin.md"])` directly before trusting the fork. `docs/architecture/companion-plugin.md` cites five paths inside the sibling `../claude-plugins/codescout-companion/` repo (`hooks/hooks.json`, `hooks/session-start.sh`, `hooks/subagent-guidance.sh`, `hooks/pre-tool-guard.sh`, `.claude/codescout-companion.json`) — all real, correct references to a repo outside the active project root. All five came back `verdict=missing, severity=high`, no explanatory note. Meanwhile other refs to the exact same external repo on the exact same page (`../claude-plugins/codescout-companion/` itself, and unrelated example paths like `/path/to/sibling`) came back `verdict=unknown, severity=low` WITH a helpful `notes: "path outside active project; scope=umbrella required"`. Same root cause (path outside `scope=project`), inconsistent verdict/severity/notes treatment depending on `ref_kind` classification.

**Probable cause:** The classifier's "outside active project" carve-out (which downgrades to `unknown`+note) appears to fire for some `ref_kind`s (bare directory-looking refs) but not others (relative sub-paths one level under an already-flagged, out-of-scope parent directory), so those fall through to the default `missing`/`high` policy.

**Workaround:** Corrected the in-flight fork's brief via `SendMessage`: told it to also exclude any `missing`/`high` hit in `docs/architecture/companion-plugin.md` referencing the `../claude-plugins/codescout-companion/` hook paths, and to flag (not silently trust) any other file whose "high" hits are all rooted under a path that itself resolves `verdict=unknown` with the umbrella-scope note.

**Severity:** med — without the scout, the fork would have reported `docs/architecture/companion-plugin.md` (a file already read and trusted this session) as carrying 5 high-severity broken refs, a false regression entering the promotion punch list.

**Status:** promoted-to-bug-tracker (2026-08-06; was `mitigated`) — fork corrected mid-flight; `audit_doc_refs`'s own inconsistent path handling is unfixed and now tracked in the bug ledger rather than here. See the promote-when note below.

**Fix idea / Pointer:** Candidate for a U-N entry in `docs/trackers/codescout-usage-frictions.md` if this recurs on a second file/session — any doc describing `codescout-companion` internals will likely trip the same false positive. Promote once a second datapoint lands.

**Promote-when criterion FIRED 2026-08-06 (second datapoint).** Same tool, same class: the extractor classifies non-local tokens as local file paths and the severity policy bands them `high` regardless of classification confidence. New tokens observed: `Type/method` (codescout's own `name_path` symbol syntax), GitHub `org/repo` slugs, and ellipsis-elided external paths (`…/rocks/v492/LOCK`). All 18 high-severity findings on `experiments` are false positives of this class, and the `audit-doc-refs` CI job is **red** on them with no `continue-on-error`. Promoted past a U-N entry straight to the bug ledger, which is the better destination: `docs/issues/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` (which itself cross-references the earlier `docs/issues/2026-07-28-audit-doc-refs-json-pointer-false-positive.md`). Status moved to `promoted-to-bug-tracker`.

---

## F-2 — Bug ledger never queried at the seam; reimplemented a filed fix and skipped its stated precondition

**Observed:** 2026-08-06, during `experiments` -> `master` merge preparation (394-commit fast-forward). Surfaced ~60 tool calls into the session, by accident, during an unrelated verify-open pass.

**When:** The session opened with an uncommitted +96-line change in `src/embed/ast_chunker.rs`. I scouted the *code* seam properly (read `nodes_to_chunks`, ran the module tests, dumped real chunk output) and shipped a fix with 7 mutation-verified tests as `ca442498`. What I never scouted was the *decision* seam: what the project had already decided about this code.

**Expected:** the uncommitted change was fresh work whose only gap was a missing test, and completing it was mine to do.

**Got (scouted, far too late):** `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` (`a8c0361cec54e6e2`, `status: open`, `severity: high`) already specified this exact change as **Fix candidate 1** — verbatim *"Introduce `AST_CHUNK_MIN` (~200-300 chars) and coalesce consecutive inner declarations below it into one chunk, keeping the container header."* I implemented 250. The same file carried two constraints I violated:

1. *"Any of these should be validated against the retrieval benchmark (`docs/research/2026-05-06-retrieval-stack-benchmark.md`) before landing — smaller chunks were chosen deliberately for precision, so a floor trades recall sharpness for cost and **must be measured, not assumed**."* No benchmark was run.
2. An explicit sequencing decision in its Resume: *"the throughput work lands first because it leaves vectors byte-identical and needs no score re-validation, whereas this change does."* That ordering was jumped.

Second consequence, same root cause: I filed `docs/issues/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` as new when `docs/issues/2026-07-28-audit-doc-refs-json-pointer-false-positive.md` already held the same extractor + severity-policy root cause.

**Probable cause:** the `project-activation-bootstrap` guide was **auto-injected into the first tool response of the session** and states, verbatim: *"Bug or regression work: `artifact(action=\"find\", kind=\"bug\", status=\"open\")` — the known-bug ledger. Don't re-file a filed bug as new; mark a rediscovery KNOWN and cite the ledger path."* The guidance was present, positioned at Phase 0, and still missed. The reason is the trigger shape: the rule fires on **task category** ("bug or regression work"), and I had classified the session as *documentation + merge prep*. A category-triggered rule cannot fire for someone who has categorised their task differently — and "am I doing bug work?" is exactly the question a merge-prep framing answers "no" to, right up until it edits a file that has an open bug against it.

**Workaround:** `a8c0361cec54e6e2` updated in place rather than archived — records that candidate 1 is implemented at `ca442498` with a green 5-config gate, that the benchmark precondition is **unmet**, that the sequencing was jumped, and the explicit criteria to close. The two `audit_doc_refs` bugs now cite each other, mine noting the earlier filing has priority and carrying the `graft` command to fold them.

**Severity:** high — a corpus-invalidating change (chunk ids are content-addressed, so every boundary change re-embeds everything) landed out of its planned sequence with no evidence that recall held. Cherry-picking `ca442498` to `master` would force a full re-index on every consumer while the question its own bug file raises stays unanswered. The duplicate ledger entry is the minor half.

**Status:** mitigated — nothing archived falsely, both preconditions documented, close criteria written down. The benchmark run is still owed and is the only thing that turns this `fixed-verified`.

**Fix idea / Pointer:** Recon needs a **file-identity** ledger trigger, not a category one: before editing a file, `artifact(action="find", kind="bug")` constrained to that file or subsystem. Filed as R-55 `miss` + `proposal` in `docs/trackers/reconnaissance-patterns.md`.

---

## F-3 — Treated CLAUDE.md's three local commands as "the gate"; the real merge gate is 15 CI jobs, red for three weeks

**Observed:** 2026-08-06, ~40 tool calls into merge preparation, after all documentation work had already landed.

**When:** The user's task was "prepare for merging to master." Every subsequent action depended on the current state of the merge gate. I ran `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test` early, reported them green, and proceeded to documentation.

**Expected:** gate state unknown but probably fine; CLAUDE.md's *"Run `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` before completing any task"* is the gate.

**Got (scouted):** `gh run list --branch experiments` returns `"conclusion": "failure"` for **every run** back to 2026-07-13. The most recent (`30852803569`, 2026-08-03) had **11 of 15 jobs red**: Clippy, Tool Docs Sync, Audit Doc Refs, Windows-gnu cross, and 7 of 9 test-matrix cells. `.github/workflows/ci.yml` runs a 3x3 matrix (ubuntu/macos/windows x default/local-embed/no-features) plus four non-test jobs. CLAUDE.md's three commands cover **one** of nine test cells.

The two most consequential defects were invisible to the local three by construction: `tests/link_scan.rs` and `src/server.rs`'s `make_server` helper reference `codescout::librarian` / `ServerEnv.librarian` with no `#[cfg(feature = "librarian")]`, so they fail to *compile* under `--no-default-features` and `--features local-embed`. Nobody builds those configs locally, so six matrix cells were red on code that a default-features `cargo test` reports as perfectly green.

**Probable cause:** CLAUDE.md's pre-commit instruction is a *per-task hygiene* rule and reads like a complete gate. `docs/RELEASE.md`'s Standard Ship Sequence reinforced it — step 1 is "tests passing, clippy clean" with no mention of feature configs or of consulting CI at all. Nothing in either surface says "the gate is CI, and here is how to check it."

**Counterfactual:** one `gh run list` call in the first five tool calls would have reordered the entire session — CI repair first (it is what actually blocks the merge), documentation second. Instead documentation landed first, and two of the three CI defects I ended up fixing are unrelated to documentation entirely. Mid-session "fmt clean, clippy clean" claims were true but narrower than the framing implied.

**Severity:** med — no wrong work was produced and nothing had to be redone, but the sequencing inverted the user's stated goal and the gate-green claims were misleadingly scoped.

**Status:** fixed-verified — all five CI-equivalent steps now run locally and exit 0 at `be75e705` (`fmt`, `clippy -- -D warnings`, and `cargo test` in all three feature configs). `docs/RELEASE.md`'s new *Large-Cohort Promotion* section lists the full gate including the two non-default configs and puts the ancestry check first, so the next person inherits the correct definition rather than the three-command subset.

**Fix idea / Pointer:** the durable half is already promoted into `docs/RELEASE.md`. The remaining candidate is adding `cargo check --no-default-features` to the CLAUDE.md pre-commit line, or a `cargo xtask gate` alias that runs all five — a local subset that silently omits six of nine cells will keep producing this friction.

---

## W-1 — Dumping an internal pure function's real output beat reasoning from its source

**Observed:** 2026-08-06, finishing the uncommitted `AST_CHUNK_MIN` change in `src/embed/ast_chunker.rs`.

**Pattern:** When the question is *"what does this pipeline actually emit?"*, add a throwaway test that prints the real output and read it — even when the code is an internal pure function whose source is open in front of you.

The existing guidance covers the *external* case: this skill's Phase 1 says *"For tools / external APIs: read the actual response shape, not docs"*, and `get_guide("project-activation-bootstrap")` Phase 2 says *"A claim about how a TOOL behaves needs the call run once and the real output read."* Both are framed around tools and APIs. The extension this datapoint supports: the rule matters **more**, not less, for an internal pure function, because the source being right there makes deriving the answer feel authoritative.

**Counterfactual:** I had already reasoned from `nodes_to_chunks` that the coalesce run was padded by the container's **trailing `}`** gap chunk. The dump showed the run was padded by a **leading** gap spanning lines 1-8 — a full duplicate of every line before the container, emitted because the recursion re-derives gaps against the whole file with `prev_end` reset to 0. I had not predicted that shape. Reasoning alone would have produced a plausible fix for the metadata loss and **missed the duplication entirely**; it is now its own bug file, `docs/issues/2026-08-06-ast-chunker-recursion-duplicates-leading-gap.md`. The dump also handed me the exact `metadata` strings (`src/mystore.rs :: impl MyStore ::     pub fn build(&self)`, note the preserved leading whitespace) that 7 new tests assert on, instead of guessed ones.

**Confirming data points:**
1. This session — 2 tool calls (insert dump test, run with `--nocapture`) surfaced one regression mechanism, one unrelated pre-existing bug, and the exact assertion strings for 7 tests.
2. Pending: a second case where a dump-vs-derive scout on an internal function surfaces something the source read missed.

**Impact:** med — caught a second bug for free and prevented seven tests being written against guessed strings.

**Promote-when:** at a second datapoint, promote to memory `reconnaissance` as a bounded rule: *"Before asserting what a chunker/formatter/serializer emits, print its real output once — internal purity is not a reason to skip it (W-1)."* Craft-shaped enough to also justify widening this skill's Phase 1 bullet from "tools / external APIs" to "tools, APIs, and any function whose output shape you are about to assert on."

**Status:** validated — single datapoint, drift caught and a second bug found before the commit landed. Awaiting promotion criterion.

---

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
