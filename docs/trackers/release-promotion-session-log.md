---
id: e6697aea0ee3ea37
kind: tracker
status: draft
title: Release Promotion Session Log
owners: []
tags:
- session-log
- release-promotion
- reconnaissance
topic: null
time_scope: null
---

> Per-work-stream friction/win log for the `experiments` -> `master` promotion
> (79-commit fast-forward, local range `eca9902e..339cea47`). Copied from
> `docs/templates/session-log.md`; see that file for the full status vocabulary
> and entry templates.

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
