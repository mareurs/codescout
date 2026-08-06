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

## Resume — round 6, written 2026-08-06 for session compaction

**Read this one.** Rounds 2–5 are superseded where they disagree; round 5's *"two decisions
left"* is closed (both taken), and its CI table is extended below.

### Git state (verified, not remembered)

- `experiments` at **`1f20de99`** (this handoff commit), tree clean, nothing unpushed.
- **426 ahead / 0 behind `master`**, `merge-base --is-ancestor` confirms **fast-forward
  promotion is available**. Use `docs/RELEASE.md` § *Large-Cohort Promotion (Fast-Forward)*,
  not the Standard Ship Sequence.
- Local gate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  **3504 tests passed / 0 failed / 44 ignored**. Audit: `EXIT=0`, 0 high findings.

### CI — three confirmed green runs, two QUEUED at writing time

| run | SHA | result |
|---|---|---|
| 31107853410 | `6348dfad` | 15/15 |
| 31108238052 | `db4b1968` | 15/15 |
| 31109236437 | `de4f7ccd` | 15/15 |
| 31109795037 | `382c3344` | 15/15 |
| 31122588792 | `e58ad463` | **queued** |
| 31122862704 | `fcb6598f` | **queued** |
| — | `1f20de99` | not yet queued at writing |

**Do not report a verdict for the last two from this document — GitHub's runners were
backed up and neither had started.** `gh run list --branch experiments --limit 2`.
The three green runs predate the round-6 code changes, so the queued pair is the first CI
exposure for the bug-status default, the `degraded` fix and the kotlin NMT flag.

### What round 6 changed

Triage of the open-bug ledger first exposed that **the ledger itself was under-reporting**:
`find(kind="bug", status="open")` returned 5 where the correct filter returns **8**. Two
independent causes, both fixed:

- `artifact(create, kind="bug")` defaulted `status` to `draft` — not in the bug vocabulary
  at all (`open|investigating|fixed|mitigated|wontfix|zombie`); `create.rs` read
  `unwrap_or("draft")` with no reference to `kind`. Now resolved per kind, and an
  out-of-vocabulary bug status is refused with the six listed. Filed as
  `docs/issues/2026-08-06-artifact-create-bug-defaults-to-invalid-draft-status.md`.
- The **documented query** names one of two non-terminal states, so any bug marked
  `investigating` — what the guide instructs you to set while working it — is invisible.
  Fixed at all three prescribing sites, including
  `src/prompts/guides/project-activation-bootstrap.md`, which is auto-injected on
  activation. The two sites that merely *explain* the filter were left alone.

Then three fixes:

- **`scan_meta.degraded` de-saturated.** It was `true` on every run because
  `detect_language` returns `"unknown"` for any extension outside its six and that counted
  as degradation. Measured `true`/`["unknown"]` → **`false`/`[]`** on the same tree. This
  was the prerequisite that made the non-determinism bug's own earlier proposal ("exit
  non-zero when degraded") unusable — it would have failed all fifteen green CI jobs.
- **kotlin-lsp gets `-XX:NativeMemoryTracking=summary`** (item 3 of that bug). Inserted
  *before* `-Xmx2g` so codescout's cap stays the final one; the test asserts the **ordering**,
  because an edit appending after `-Xmx` would silently reopen the original bug.
- **`SymbolMissing` out of the gating band** (`c8efc17a`, round 5) — see round 5.

### One bug advanced with NO code change, deliberately — see W-8

The `symbols` search flake's filed hypothesis (LSP warming) is **refuted**. The tree-sitter
fallback is gated on `matches.is_empty()`, so a 0-match implies tree-sitter found nothing
either, and tree-sitter never touches the LSP. The surviving hypothesis is a `root`
resolution race at activation (`require_project_root_for`, `symbols.rs:225`) — a different
bug class, with precedent in the archived shared-server active-project race.

**If you pick that up: instrument the resolved `root` per 0-match, not LSP readiness.** The
harness as originally specified measures the wrong variable.

### Open bugs — use the CORRECT query

```
artifact(action="find", kind="bug",
         filter={"status": {"in": ["open", "investigating"]}})
```

Blocked on a **user decision or measurement**, not on work:

- reranker 42× latency — needs live-arm measurement and a product call;
- AST chunker minimum chunk size — its own precondition demands the retrieval benchmark
  first ("must be measured, not assumed");
- MCP orphans — needs an idle-timeout value and a definition of "idle";
- kotlin item 1 — cherry-pick to protected `master`;
- researcher rerank score scale — likely sibling-repo scope, unverified.

### Do NOT re-do

Everything in round 4's list still holds, plus:

- **Do not trust `find(kind="bug", status="open")`** — it hides `investigating`.
- **Do not build the symbols-flake harness around LSP readiness** — refuted, W-8.
- **Do not `replace_all` a line right after inserting a helper containing it** — U-37: it
  rewrote the helper's own body into a self-call, compiled clean, and surfaced only as a
  stack-overflow SIGABRT in one test.
- **Do not gate on `scan_meta.degraded` without checking what it contains** — that mistake
  is already recorded once in that bug file's history.

### Still owed

Unchanged: `index(force=true)` rebuild (ast-chunker moved chunk boundaries; ids are
content-addressed — ~2h, not started, nobody asked), the retrieval benchmark for the chunk
floor, reranker options 1–3, the toolchain pin-vs-float policy call, and the kotlin bug's
items 1/5/6. The merge itself is the user's to run.
## Resume — round 4, written 2026-08-06 for session compaction
> **Update, same day — round 5. Both decisions below were TAKEN, and
> `Audit Doc Refs` now reports 0 high findings locally (`EXIT=0`).** The section
> headed "The two decisions left" is superseded; read the round-5 addendum at the
> end of this Resume instead. Everything else here still holds.

**Read this one. Rounds 2 and 3 are superseded on every point of fact** — round 3's
magnitude estimate for the doc-drift backlog was wrong by ~5×, and its "one mechanism
decision" framing turned out to be two decisions plus five outright defects.

### Git state (verified, not remembered)

- `experiments` at **`8fffebb3`**, tree clean, nothing unpushed, in sync with origin.
- **412 ahead / 0 behind `master`** — still a strict ancestor, so promotion remains a
  fast-forward. `docs/RELEASE.md` § *Large-Cohort Promotion (Fast-Forward)*, not the
  Standard Ship Sequence.
- Two commits this round: `a68f412c` (audit_doc_refs code) and `8fffebb3` (docs). Split
  deliberately — the backlog bug itself warned that mixing extractor changes with dozens
  of doc edits makes both unreviewable.

### Gate state

- **Local gate green:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  and `cargo test` → **3495 passed, 0 failed, 44 ignored**.
- **CI on `8fffebb3` was still running at the time of writing** (7 green: Format, Clippy,
  Tool Docs Sync, MSRV, ubuntu no-features, ubuntu local-embed, macos local-embed;
  `Audit Doc Refs` and the platform matrix in flight). **Do not report a CI verdict from
  this document — re-query it.** `gh run list --branch experiments --limit 1`.
- Remember the local gate cannot predict CI (F-5): local clippy is two minor versions
  behind `dtolnay/rust-toolchain@stable`, and there is still no `rust-toolchain.toml`.

### What landed this round

Five `audit_doc_refs` defects, each with tests carrying over-match guards:

1. `resolve_file_line` / `resolve_file_symbol` had no outside-project guard that
   `resolve_file_path` always had — a doc citing `/etc/passwd:12` resolved **`Resolved`**,
   range-checked against the host's real file.
2. `ignore`'s `matched_path_or_any_parents` **panics** rather than errors on an
   out-of-root path; the first real run aborted with SIGABRT.
3. Link resolution knew one of the repo's **two** conventions. `docs/manual/src/**` is an
   mdBook (internal links must be page-relative); ~28 correct links were reported missing.
4. `path.md#fragment` links were stat'd whole — 8 findings calling existing pages missing.
5. `matches_archive` tested the literal `docs/archive/`, so `docs/trackers/archive/`,
   `docs/plans/archive/` and siblings got **no archive drop at all**.

Three severity caps: `code_block` (a block is a transcript, not a citation — reads
`RefPosition`, which the parser already populated and nothing consumed), `gitignored_path`
(absence is the normal state), and skipping markup-display spans (the audit was flagging
its own manual's example column).

Doc fixes, all confirmed against filesystem or `git log` first: ~12 genuine drift
citations; **two links broken in the rendered book**; **a rendering bug** in
`tool-selection.md` where stray duplicate fences swallowed two prose paragraphs; and two
substantive defects — `semantic-search-diversity.md` documented an **inactive** feature
(`MAX_CHUNKS_PER_FILE` exists nowhere; the cap is `#[allow(dead_code)]`), and
`librarian-mcp.md` cited a file in a crate **dissolved** in `d48bf992`.

### Measured state of the one red job

Per-directory `--paths` counts (the only trustworthy method — see F-6):

- **Zero high findings:** `docs/{adrs,architecture,archive,conventions,evals,issues,spikes,templates}`.
- `docs/manual`: **22 → 11**.
- Residue: `docs/research` 37, `docs/usage-reports` 20, and `docs/{plans,reviews,superpowers,trackers}`
  each ≥50 (capped, so unknown-but-larger). **Several hundred total**, overwhelmingly one
  class: dated point-in-time documents whose refs were correct when written.

### The two decisions left — both yours, both gate-semantics

They are written out in full, with recommendations and the arguments against, in
`docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md` § Resume.
In brief: (1) do dated documents gate CI — extend the drop policy (recommended) or
narrow the scan set; (2) what scope should the `<!-- audit-doc-refs:ignore -->` marker
have for the last 11 manual findings — section-scoped recommended, because a line-scoped
marker cannot live inside a markdown table.

**Neither was taken unilaterally**, and that is deliberate: three caps were already added
this round on defect grounds, and these two change what the gate *means* rather than
fixing what it gets wrong.

### Do NOT re-do

- **Do not blanket-exclude `docs/manual/src/concepts/**`.** Measured: this round found
  two book-breaking links, a rendering bug, and an inactive-feature page in there.
  Excluding it hides all three.
- **Do not "fix" a finding by editing prose to satisfy the lint.** Confirm at the
  filesystem and `git log --all --follow`; if the reference is correct, the extractor is
  what needs changing. That discipline is what produced all five defects above.
- **Do not trust an aggregate read through the 50-finding cap** (F-6). Use `--paths`.
- **Do not trust a single green `Audit Doc Refs` run** — the gate is non-deterministic:
  `docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`.
- **Do not trust a stale release binary.** It was two days old and predated all four
  source files of the tool under test. `find src crates -name '*.rs' -newer target/release/codescout`.

### Owed / unverified

- `index(force=true)` rebuild still owed — the ast-chunker change moved chunk boundaries
  and ids are content-addressed.
- Retrieval benchmark for the chunk floor; reranker options 1–3 still unchosen (needs a
  live-arm measurement).
- Toolchain pin vs float: still a policy call.
- MCP orphan idle-timeout value and the definition of "idle".
- The merge itself is the user's to run.
## Resume — round 5 addendum, written 2026-08-06

**`Audit Doc Refs` reports 0 high findings.** `./target/release/codescout
audit-doc-refs --no-emit-tracker --fail-on high --json --project .` → `EXIT=0`.
Branch at `297e1074`, **415 ahead / 0 behind `master`**, clean, pushed. Local gate:
fmt, `clippy --all-targets -D warnings`, **3498 tests passed / 0 failed**.

### The two decisions, as taken

1. **Dated documents no longer gate** — `historical_drop` covers
   `docs/{plans,research,reviews,spikes,superpowers,trackers,usage-reports}/**` and
   root-level `docs/review-*.md`. The argument that settled it: `issues_drop`
   already does exactly this for bug files, which are acted on constantly. What a
   ledger *is* — a dated record — is what decides the band, not how often it is
   read. Still gating, and asserted in the test: `docs/manual/**`, root
   `docs/*.md`, `CLAUDE.md`, `**/README.md`, `architecture/`, `conventions/`,
   `adrs/`, `evals/`, `templates/`.
2. **`<!-- audit-doc-refs:ignore -->` is section-scoped** — marker to the next
   heading of any level. Line scope was impossible where it was most needed (an
   HTML comment between table rows breaks the table); file scope was rejected
   because the same pages cite real modules.

### Not a silencing — check this before trusting the green

The audit still reports **8388 broken refs**, all at `med`, spread across
`archive_drop` (35 of the shown 50), `inferred_path` (10), `basename_ambiguous`
(4), `gitignored_path` (1). Every finding is still emitted; only the band moved.
If a future change makes the *count* fall too, that is the thing to distrust.

### What else landed in round 5

- **156 stale archive citations repointed** across 62 tracked files by a rule that
  needs no prose reading (`<dir>/<name>.md` absent + `<dir>/archive/<name>.md`
  present → insert `archive/`). 195 insertions / 195 deletions, a pure 1:1 swap;
  re-scan reports 0 remaining. Idempotent by construction.
- **Marked sections**, each with its reason inline — fictional teaching paths,
  correctly-documented user/runtime files, and configuration values that look
  like paths. Including the audit's own manual page, whose "Reference kinds"
  table the tool was reporting as drift.
- **Remaining genuine drift fixed**: `PROGRESSIVE_DISCOVERABILITY.md`'s File
  References table (one row had to become two — the three functions now live in
  `src/symbol/query.rs` and `src/tools/symbol/symbols.rs`), and five dead ROADMAP
  pointers, three of which were **deleted with no successor** and are now stated
  as deleted rather than repointed at a guess.

### Still owed

Unchanged from round 4: `index(force=true)` rebuild (ast-chunker moved chunk
boundaries; ids are content-addressed), the retrieval benchmark for the chunk
floor, reranker options 1–3, the toolchain pin-vs-float call, the MCP orphan
idle-timeout definition. The merge itself is still the user's to run.

**CI CONFIRMED: run 31107853410 on `6348dfad` — 15/15 jobs green.** The first fully-green
run on `experiments`; `Audit Doc Refs` had been red for weeks. `Windows-gnu cross (MinGW +
wine)` is green too.

Two commits landed after that run and are verified against a fresh clone rather than only
locally (`de4f7cc`: 0 high, 881 files, 46726 refs, 8906 broken at `med`):

- `c8efc17a` — **`SymbolMissing` no longer gates.** The flap mechanism turned out to be a
  band straddle inside one match in `resolve_file_symbol`: LSP answers and symbol absent →
  `SymbolMissing`/high; LSP does not answer → `Unknown`/low. A single unanswered request
  moved the exit code. `high` is now reserved for deterministic filesystem verdicts
  (`Missing`, `FileMissing`). Nothing had asserted the old value, so the map is now pinned
  by a test.
- `de4f7ccd` — backlog bug archived through the librarian; the three inbound citations that
  the archive broke were repointed by the same mechanical rule.

**Three consecutive fully-green CI runs, confirmed:**

| run | SHA | result |
|---|---|---|
| 31107853410 | `6348dfad` | 15/15 — trailing-slash cap fix |
| 31108238052 | `db4b1968` | 15/15 |
| 31109236437 | `de4f7ccd` | 15/15 — first run including the `SymbolMissing` band change |

`Audit Doc Refs` passed in all three, and `Windows-gnu cross (MinGW + wine)` with them. The
run for the final docs commit was still in flight at writing time — **check it rather than
trusting this line**, `gh run list --branch experiments --limit 1`.

The streak matters more than any single green, given
`docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`: that bug is still open,
and three independent runs is the strongest available evidence that the band change removed
the flap from the exit code rather than merely not tripping it once. `gh run list --branch experiments --limit 1`. And per
`docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`, do not treat a
single green `Audit Doc Refs` as proof — that bug is still open.
## Resume — round 3, written 2026-08-06 for session compaction

> **Read this one. Rounds 1 and 2 below are kept for the record but are superseded on
> every point of fact.** Round 2's item 1 ("push, then re-read CI") is done; its
> "remaining issues" list is down from five to one.

### Git state

- Branch `experiments` @ `e6d8ffa8`, **409 commits ahead of `master`, still a strict
  ancestor** (`git rev-list --left-right --count master...experiments` → `0 409`).
  Tree clean, nothing unpushed.
- Promotion is a **fast-forward**, not a cherry-pick. Use `docs/RELEASE.md`
  § *Large-Cohort Promotion (Fast-Forward)*, NOT the Standard Ship Sequence.
- The merge itself is the user's to run ("Prepare only — I run the merge").

### CI state — 14 of 15 green

Run `31098286970` on `cd643d58`. Baseline for comparison: `30852803569` (2026-08-03)
was 4 green / 11 red, measured on code 21 commits stale.

| Green (14) | Red (1) |
|---|---|
| Format, Clippy, Tool Docs Sync, MSRV (1.88) | **`Audit Doc Refs`** |
| ubuntu × default / no-features / local-embed | |
| macos × default / no-features / local-embed | |
| windows × default / no-features / local-embed | |
| Windows-gnu cross (MinGW + wine) | |

`Test (windows-latest / default)`: **3283 passed, 0 failed.**

### The one open item: `Audit Doc Refs`

Diagnosed and measured, not unknown. Tracked in
`docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md`.

After the extractor fixes and the `docs/lessons/**` exclusion, >50 high findings remain
in **three sub-classes needing three different mechanisms**:

1. **Fictional example paths** (~42 of the current window) — src/services/auth.rs ×11,
   src/foo.rs ×5, docs/trackers/my-tracker.md, ./gradlew (left un-code-spanned here on
   purpose: this file gates, and code-spanning a nonexistent example path would add it
   to the very backlog this entry describes). Almost all in
   `docs/manual/src/concepts/**`. Nothing syntactic separates "illustrative" from
   "stale", so these need an author-supplied marker
   (`<!-- audit-doc-refs:ignore-file -->` or similar).
2. **Gitignored runtime state** (~11) — `.worktrees/*` ×8, .codescout/embeddings.db
   ×3. A gitignore-aware severity cap clears these with no markers and no prose edits:
   a doc naming generated state is not citing a tracked file.
3. **Genuine drift** (~8) — including citations to the six bug files archived earlier
   today. Mechanical: repoint to `docs/issues/archive/…`. Note only the
   non-`docs/issues/` citers gate, since `docs/issues/**` gets `issues_drop` High→Med.

**Recommended order:** (2) then (1) then (3). **Mechanism choice was deliberately NOT
taken** — it is a design decision on a user-facing lint with several defensible answers,
and the user was asked.

**Do NOT blanket-exclude `docs/manual/src/concepts/**`.** The manual also cites real
codescout paths, and that is exactly where doc drift hurts readers most. Excluding it
would trade the gate's whole value for a green check.

### Do NOT re-do these — decisions made with evidence

- **`~80 ms` reranker figure in `retrieval-stack.md` stays.** Its bug file says "correct
  it regardless", but that line predates a **CORRECTED** note at the top of the same
  file which retracts the comparison it rests on. The row is labelled TEI; the 3091 ms
  was measured on llama-server. The two available TEI numbers disagree impossibly
  (tracker p50 ~150 ms vs manual p95 ~80 ms). A callout was added instead. See F-4.
- **Reranker options 1–3 stay unchosen.** Needs a measurement of the *live* arm
  (dense + sparse + rerank), which was never benchmarked.
- **MCP-orphan direction 1 ("exit on stdio EOF") is already implemented** —
  `ResilientStdin` absorbs only `WouldBlock`, so a 0-byte EOF propagates and the process
  exits. The orphans are never *sent* one. The fix is direction 2 (idle timeout), which
  needs a timeout value + a definition of idle; firing on a live-but-quiet session is
  worse than the leak. Still reproducing: 16 processes, oldest 2d22h, ~1.05 GB RSS.
  **Do not bulk-kill by pattern** — the list always includes the killer's own server.
- **`derive_dead_roots`' `is_absolute()` guard stays.** Relaxing it to green a POSIX
  fixture makes a prune `WHERE` match every absolute row. See W-3.
- **`count_dead_root_counts_rows_under_root` keeps its POSIX literals.** It calls
  `count_dead_root` directly, so it never reaches that guard; its subject is
  prefix-sibling LIKE semantics, not paths on disk.
- **The seven `⚠ Unreleased` manual callouts stay through the merge.** Removal trigger
  is the *release*, not `master` — both their claims stay true on master.
- **`symbols` Bug A is closed** (retracted; `include_body` IS honoured). Only Bug B
  (intermittent search-mode 0-match) keeps that file open, and it needs a scripted
  post-activation repro that hand-issued MCP calls cannot express.

### Owed / unverified

- **`index(force=true)` rebuild is owed.** The ast-chunker window fix changes chunk
  boundaries and ids are content-addressed, so the existing semantic index holds the old
  (duplicated) chunk set. This is a run, not a code change.
- **Retrieval benchmark for the chunk floor** — the only thing that flips
  `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` to fixed. Precondition
  stated in that file and still unmet.
- **Toolchain skew is unresolved and will recur on its own schedule.** No
  `rust-toolchain.toml` + `dtolnay/rust-toolchain@stable` ⇒ CI re-resolves `stable` every
  run and can go red with zero commits (that is how Clippy broke on 1.97 against
  unchanged code). Pin vs float is a policy call. See F-5.
- **A dedicated ordering test is owed** for the findings-cap fix — see the Resume of
  `docs/issues/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md`.
- **mdbook is not installed**, so the manual was never build-verified this session.

### Techniques worth keeping

- **Per-job CI logs while a run is still in progress:**
  `gh api --allow-escape-sequences "/repos/mareurs/codescout/actions/jobs/<id>/logs"`.
  `gh run view --log-failed` refuses until the whole run finishes, and a stalled sibling
  blocks it indefinitely (`Windows-gnu cross` sat 41+ min on "Install MinGW + wine").
- **Polling a job's conclusion:** GitHub returns `conclusion: ""` for pending, and jq's
  `//` only defaults on `null`/`false` — so `.conclusion // "pending"` never fires. Guard
  with `[ -n "$C" ]`.
- **`--all-features` is structurally invalid here** — `codescout-embed` has a
  `compile_error!` for mutually-exclusive ONNX backends.

### Where the rest of the record lives

| Surface | Holds |
|---|---|
| `docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md` | the one open CI blocker, with the bisect method |
| `docs/issues/archive/2026-08-06-windows-*.md` | WIN-28 (nine panics, three causes) + WIN-29 duplicate proof |
| `docs/trackers/windows-platform-support.md` | WIN-28/29 index rows + a History entry |
| this file, F-4 / F-5 / W-2 / W-3 / W-4 | the session's transferable lessons |
| `docs/trackers/reconnaissance-patterns.md` R-55, R-56 | recon-skill proposals |
| `docs/trackers/codescout-usage-frictions.md` U-30, U-33/34/35 | tool frictions; IL3 now ×7 in one session |

## Resume — round 2, written 2026-08-06 for session compaction

> **CI IS NOW 14/15 GREEN.** Run `31098286970` on `cd643d58` — baseline was 4 green / 11 red
> (`30852803569`, 2026-08-03, on code 21 commits stale).
>
> | Green (14) | Red (1) |
> |---|---|
> | Format, Clippy, Tool Docs Sync, MSRV (1.88) | `Audit Doc Refs` |
> | ubuntu × default / no-features / local-embed | |
> | macos × default / no-features / local-embed | |
> | **windows × default / no-features / local-embed** | |
> | **Windows-gnu cross (MinGW + wine)** | |
>
> `Test (windows-latest / default)`: **3283 passed, 0 failed.**
>
> **WIN-28 fixed and archived.** Nine failures, three root causes, **zero product defects** —
> two assertions were failing against *correct* implementations and the rest were fixtures
> encoding POSIX-only path shapes. Fixing the product to satisfy cluster A would have relaxed
> `derive_dead_roots`' absolute-path guard, whose absence makes a prune `WHERE` match every
> row. Full account in
> `docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md`.
>
> **WIN-29 confirmed a duplicate, empirically.** It was closed on an inference (identical
> failing-test sets) and has now gone green from the same nine fixture fixes, with no
> MinGW/wine-specific change. That is the prediction the duplicate hypothesis made.
>
> **The one remaining red is `Audit Doc Refs`**, and it is NOT undiagnosed. Measured population
> after the extractor fixes and the `docs/lessons/**` exclusion: >50 findings, ~42 of them
> illustrative paths inside `docs/manual/src/concepts/**` (src/services/auth.rs, src/foo.rs,
> .worktrees/my-feature, docs/trackers/my-tracker.md — un-code-spanned, see round 3). Three sub-classes needing three
> different mechanisms — see
> `docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md`:
>
> 1. **Fictional example paths** — need an author-supplied marker; nothing syntactic separates
>    "illustrative" from "stale".
> 2. **Gitignored runtime state** (`.worktrees/*` ×8, .codescout/embeddings.db ×3) — a
>    gitignore-aware severity cap handles these with no markers and no prose edits.
> 3. **Genuine drift** (~8), including citations to the six bug files archived earlier today.
>
> **Do NOT blanket-exclude `docs/manual/src/concepts/**`.** The manual also cites real codescout
> paths, and that is precisely where doc drift hurts readers most. Mechanism choice is an open
> decision, deliberately not taken unilaterally on a user-facing lint.
>
> Also unresolved and unrelated: no `rust-toolchain.toml` + `dtolnay/rust-toolchain@stable`
> means CI re-resolves `stable` every run and can acquire lints with zero commits (that is how
> Clippy went red on 1.97 against unchanged code). Pin vs float is a policy call.

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
> | Tool Docs Sync | Audit Doc Refs — `docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md` |
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
2. **`docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md`**
   (open, high; WIN-28). Nine tests fail on `windows-latest`, all in code this cohort
   added — 7 catalog `rehome`/`prune_missing`, the `like_escape` idiom guard, the index
   lock. Linux and macOS pass the same config, so path semantics not logic. **Blocked on
   a Windows runner**: the per-test panic output was never captured, and guessing at path
   normalisation risks fixing the tests rather than the code. Its Resume names
   `validate_rehome_gates` as the narrowest starting point.
3. **`docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`**
   (open, high). The `audit-doc-refs` job is a hard gate (`--fail-on high`, no
   `continue-on-error`) and all 18 high-severity findings are **extractor false
   positives**: `Type/method` (codescout's own `name_path` syntax, 8 of 18), GitHub
   `org/repo` slugs, ellipsis-elided external paths, plus an mdBook relative-link class
   at `med`. **Do NOT "fix" this by editing the three ADRs** — their prose is correct.
   Fix the extractor (start with the `name_path` shape: 18 → 10) or drop the gate and say
   so in the workflow comment, which currently claims all hi-sev findings are reconciled.
   Cross-references the earlier `2026-07-28-audit-doc-refs-json-pointer-false-positive.md`,
   which has priority.
4. **`docs/issues/archive/2026-08-06-windows-gnu-cross-job-red-undiagnosed.md`** (open, medium;
   WIN-29). Undiagnosed by choice. Leading hypothesis after a ledger query: WIN-28's nine
   failures are not in `scripts/build-windows.sh`'s wine skip-list, so this is likely
   item 2 wearing a second hat — confirm before fixing twice, and `graft` it into WIN-28's
   file if so.
5. **`docs/issues/archive/2026-08-06-ast-chunker-recursion-duplicates-leading-gap.md`** (open,
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
| F-4 | 2026-08-06 | med | process | fixed-verified | Three bug files' own `## Fix` sections carried wrong premises — stale-by-superseding, wrong severity, already-implemented |
| F-5 | 2026-08-06 | high | process | open | Local gate structurally cannot predict CI — clippy 1.95 vs 1.97, separator bugs invisible on Linux, `--all-features` unusable |
| F-6 | 2026-08-06 | med | measurement | mitigated | The 50-finding cap mis-sized the backlog plan by ~5× — third instance of a capped view corrupting a number the gate acts on |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-08-06 | med | Dump actual output of an *internal pure function*, don't derive it from source | Reasoning predicted the wrong padding chunk; the dump surfaced a whole-file duplicate + a second pre-existing bug | validated |
| W-2 | 2026-08-06 | high | If a truncated list feeds a pass/fail verdict, sort by the verdict's key before truncating | Would have "fixed all 18" and stayed red with no visible cause; the 18 was itself a windowed miscount of a >50 population | validated |
| W-3 | 2026-08-06 | high | Read the product code before believing a platform-specific red test | Relaxing `derive_dead_roots`' guard to green the test makes a prune `WHERE` match every row — data loss shipped to fix a test | validated |
| W-4 | 2026-08-06 | med | A duplicate closure states its own falsification test | `Windows-gnu cross` stayed the one red cell with no known cause; the diff collapsed 4 cells into 1 bug and predicted its green | validated |
| W-8 | 2026-08-06 | high | Read the fallback gate before building the harness a bug file asks for | Every signal pointed at LSP warming, including the source's own comment; one line (`matches.is_empty()`) proved a 0-match implies tree-sitter also found nothing, so server readiness cannot cause it — the harness would have measured the wrong variable | validated |
| W-6 | 2026-08-06 | high | Mechanise the decidable half first; the residue is the judgement, and its size is the estimate you should have had | Sample of 11 sized a class that was 156 across 62 files and included a tracker the sample missed; ~300 tool calls avoided, and a live tracker's "Active bug files" label pointing at the archive became visible only once the paths were right | validated |
| W-5 | 2026-08-06 | high | Invoke the tool under test; verify the binary is newer than its sources | Reading it had missed all five: a SIGABRT, a path escape resolving `/etc/passwd:12` as Resolved, a false premise about which half of the corpus was scanned, an unused field that already was the fix, and mdBook link semantics | validated |

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

**Promote-when criterion FIRED 2026-08-06 (second datapoint).** Same tool, same class: the extractor classifies non-local tokens as local file paths and the severity policy bands them `high` regardless of classification confidence. New tokens observed: `Type/method` (codescout's own `name_path` symbol syntax), GitHub `org/repo` slugs, and ellipsis-elided external paths (`…/rocks/v492/LOCK`). All 18 high-severity findings on `experiments` are false positives of this class, and the `audit-doc-refs` CI job is **red** on them with no `continue-on-error`. Promoted past a U-N entry straight to the bug ledger, which is the better destination: `docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` (which itself cross-references the earlier `docs/issues/archive/2026-07-28-audit-doc-refs-json-pointer-false-positive.md`). Status moved to `promoted-to-bug-tracker`.

---

## F-2 — Bug ledger never queried at the seam; reimplemented a filed fix and skipped its stated precondition

**Observed:** 2026-08-06, during `experiments` -> `master` merge preparation (394-commit fast-forward). Surfaced ~60 tool calls into the session, by accident, during an unrelated verify-open pass.

**When:** The session opened with an uncommitted +96-line change in `src/embed/ast_chunker.rs`. I scouted the *code* seam properly (read `nodes_to_chunks`, ran the module tests, dumped real chunk output) and shipped a fix with 7 mutation-verified tests as `ca442498`. What I never scouted was the *decision* seam: what the project had already decided about this code.

**Expected:** the uncommitted change was fresh work whose only gap was a missing test, and completing it was mine to do.

**Got (scouted, far too late):** `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` (`a8c0361cec54e6e2`, `status: open`, `severity: high`) already specified this exact change as **Fix candidate 1** — verbatim *"Introduce `AST_CHUNK_MIN` (~200-300 chars) and coalesce consecutive inner declarations below it into one chunk, keeping the container header."* I implemented 250. The same file carried two constraints I violated:

1. *"Any of these should be validated against the retrieval benchmark (`docs/research/2026-05-06-retrieval-stack-benchmark.md`) before landing — smaller chunks were chosen deliberately for precision, so a floor trades recall sharpness for cost and **must be measured, not assumed**."* No benchmark was run.
2. An explicit sequencing decision in its Resume: *"the throughput work lands first because it leaves vectors byte-identical and needs no score re-validation, whereas this change does."* That ordering was jumped.

Second consequence, same root cause: I filed `docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` as new when `docs/issues/archive/2026-07-28-audit-doc-refs-json-pointer-false-positive.md` already held the same extractor + severity-policy root cause.

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

**Counterfactual:** I had already reasoned from `nodes_to_chunks` that the coalesce run was padded by the container's **trailing `}`** gap chunk. The dump showed the run was padded by a **leading** gap spanning lines 1-8 — a full duplicate of every line before the container, emitted because the recursion re-derives gaps against the whole file with `prev_end` reset to 0. I had not predicted that shape. Reasoning alone would have produced a plausible fix for the metadata loss and **missed the duplication entirely**; it is now its own bug file, `docs/issues/archive/2026-08-06-ast-chunker-recursion-duplicates-leading-gap.md`. The dump also handed me the exact `metadata` strings (`src/mystore.rs :: impl MyStore ::     pub fn build(&self)`, note the preserved leading whitespace) that 7 new tests assert on, instead of guessed ones.

**Confirming data points:**
1. This session — 2 tool calls (insert dump test, run with `--nocapture`) surfaced one regression mechanism, one unrelated pre-existing bug, and the exact assertion strings for 7 tests.
2. Pending: a second case where a dump-vs-derive scout on an internal function surfaces something the source read missed.

**Impact:** med — caught a second bug for free and prevented seven tests being written against guessed strings.

**Promote-when:** at a second datapoint, promote to memory `reconnaissance` as a bounded rule: *"Before asserting what a chunker/formatter/serializer emits, print its real output once — internal purity is not a reason to skip it (W-1)."* Craft-shaped enough to also justify widening this skill's Phase 1 bullet from "tools / external APIs" to "tools, APIs, and any function whose output shape you are about to assert on."

**Status:** validated — single datapoint, drift caught and a second bug found before the commit landed. Awaiting promotion criterion.

---

## F-4 — Three bug files' own `## Fix` sections carried wrong premises; following them would have produced worse code

**Observed:** 2026-08-06, working the open-bug ledger one entry at a time.

**When:** Reading each bug's `## Fix` section as the plan before implementing it.

**Expected:** A filed bug's Fix section is the most reliable available guidance — it was written by someone holding the evidence.

**Got:** Three of the six were materially wrong, in three different ways:

1. **Stale-by-superseding.** `2026-07-28-reranker-costs-42x...` closes with *"That figure needs correcting regardless"*, but a **CORRECTED** note added later at the top of its own Summary retracts the comparison that claim rests on. The Fix section predates its own file's retraction. Following it would have replaced a possibly-stale `~80 ms` with a guess, across a runtime boundary (TEI vs llama-server) the two numbers do not share.
2. **Wrong severity assessment.** `2026-08-06-ast-chunker-recursion-duplicates-leading-gap` states the trailing-gap branch is *"harmless in size — it emits the container's closing brace"*. A failing test written before the fix showed it emits the closing brace **plus every line after the container to EOF**. A floor-only fix, which is what that framing invites, would have left half the bug.
3. **Already implemented.** `2026-07-28-mcp-servers-outlive-their-clients` ranks *"exit on stdio EOF"* as the cheapest fix and says the shutdown path is unread. Reading it showed `ResilientStdin` absorbs only `WouldBlock`, so a 0-byte EOF already propagates and the process already exits. The orphans are never *sent* an EOF, so the fix is direction 2 (idle timeout) and direction 1 should be struck.

**Probable cause:** A Fix section is written at peak context and then never re-read against later edits to the same file. Nothing links a Summary-level retraction to the Fix section it invalidates, and nothing re-checks a "not implemented" claim against the code.

**Workaround:** Treat a bug file's Fix section as a *hypothesis with a citation*, not a plan. Verify its premises the same way any other claim gets verified — read the code it names, and check whether a later dated note in the same file contradicts it. All three were caught this way; each correction is recorded in the bug file itself.

**Severity:** med — each would have produced a wrong or half fix that passed local tests. Not high only because the verification that caught them is the same reconnaissance pass already required before editing.

**Status:** fixed-verified — all three corrected in their bug files, with the correction stated as a correction rather than silently applied.

**Fix idea / Pointer:** Worth a line in `get_guide("tracker-conventions")`: when a bug file gains a dated retraction, re-read its Fix section in the same edit. See also R-56.

## F-5 — The local gate structurally cannot predict CI: three independent skews, one of them silent

**Observed:** 2026-08-06, after a locally-green six-step gate met a red CI three times in a row.

**When:** Every push this session. The local gate (`fmt`, `clippy -- -D warnings`, `cargo test`, plus three feature-gate builds) exited 0 each time.

**Expected:** A green local gate predicts a green CI, modulo the known Windows and doc-lint gaps.

**Got:** Three distinct classes of local-vs-CI divergence, none of which any amount of local testing would surface:

1. **Toolchain version skew.** Local `clippy 0.1.95`; CI's `dtolnay/rust-toolchain@stable` resolved **1.97.0**. Three pre-existing patterns (`question_mark` ×2, `for_kv_map`) are lints 1.97 emits and 1.95 does not. There is no `rust-toolchain.toml`, so CI re-resolves `stable` every run and can go red **with zero commits**. Clippy breaks on the calendar.
2. **Platform-invisible bugs.** Linux has one path separator, so a forward-slash-vs-backslash mismatch is *unobservable* there. 3488 tests passed locally before each of the three Windows pushes; the separator bug needed CI to exist at all — and one of them needed two CI round-trips, because normalising the seed moved the mismatch downstream into the test's own comparison.
3. **A gate command that cannot run locally.** `cargo clippy --all-features` fails on this workspace by construction — `codescout-embed` has a `compile_error!` for mutually-exclusive ONNX backends. Reaching for `--all-features` as a "stronger" local check produces a build failure unrelated to the code under test.

**Probable cause:** The local gate was specified as a command list (CLAUDE.md's three commands) rather than as an equivalence claim against CI. Nobody pinned the toolchain, so the strongest local check drifts out of alignment silently.

**Workaround:** None applied — recorded rather than fixed, because the remedy is a policy call (pin a toolchain so local == CI and upgrades are deliberate, vs keep floating and absorb periodic lint debt). Two operational mitigations that did work: push early to get a real verdict, and read a failing job's log via the REST endpoint rather than waiting for the whole run.

**Severity:** high — it converted "gate green, ready to merge" into three false readiness claims in one session, and the toolchain half will recur on its own schedule.

**Status:** open — the three lints are fixed and Windows is green, but the *skew* is unaddressed and needs the pinning decision.

**Fix idea / Pointer:** Add `rust-toolchain.toml` pinning the channel CI uses, or add a CI job that fails when `cargo --version` differs from a pinned expectation. Either makes the skew loud instead of silent.

## W-2 — Ordering a capped findings list by severity turned an unactionable gate into a self-explaining one

**Observed:** 2026-08-06, after fixing all 18 findings a bug file tabulated and watching the gate still exit 1.

**Pattern:** When a tool truncates a result list *and* computes a pass/fail verdict over the untruncated set, order the shown slice by whatever drives the verdict. `audit_doc_refs` did `findings.iter().take(50)` in scan order while the exit code scanned all 46 572 — and since most refs resolve, the shown 50 were almost always `resolved`/`low`. The gate reported failure and displayed nothing that caused it.

**Counterfactual:** Without this, the next step was a 16-run per-subdirectory bisect just to see *which files* had findings, repeated after every fix. Worse, the truncation had already corrupted the record: the "18 findings, all false positives" table in `2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` was the count *inside the window*, not the population. Fixing exactly those 18 and declaring victory was the live failure mode — and it is what I was about to do. After the ordering fix the shown 50 were 50/50 high, and the real population turned out to be >50 across 11 subdirectories.

**Confirming data points:**
1. Baseline run: 18 high in the window, `n_refs_broken: 10487`. Post-fix run: 0 high in the window, exit still 1 — proof the window was never representative.
2. The same defect had already been filed once as a family: `docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md`.

**Impact:** high — a gate that hides its own cause trains people to bypass it, and it silently corrupted a bug file's central measurement.

**Promote-when:** A second silent-cap instance appears in a tool that also gates. At two datapoints, promote to `docs/PROGRESSIVE_DISCOVERABILITY.md` as "if a truncated list feeds a verdict, sort by the verdict's key before truncating".

**Status:** validated — fixed in `45669701`, verified live (window went 0/50 high to 50/50 high), and filed as `docs/issues/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md`.

## W-3 — Reading the product code before believing a red test prevented turning a test failure into a data-loss bug

**Observed:** 2026-08-06, diagnosing nine Windows test failures (WIN-28).

**Pattern:** When a test fails only on one platform, read the product code it exercises before changing anything. The failure may be the test asserting something false *about a correct implementation*.

**Counterfactual:** Two of the three clusters were exactly that shape, and the obvious fix was harmful in both:

- Cluster A's four tests failed because `derive_dead_roots` skips non-absolute `abs_path` rows and `"/gone/old"` is not absolute on Windows. The tempting fix — relax the guard — is guarded by its own test and its own comment: without it the ancestor climb bottoms out at an empty `PathBuf` whose prune `WHERE` **matches every absolute row**. That is a catalog-wide data-loss bug, shipped to make a red test green.
- `lock_path_is_not_sited_in_bare_temp_dir` failed because `per_user_runtime_dir()` returns bare `temp_dir()` on Windows — deliberately, since `%LOCALAPPDATA%\Temp` is already per-user, as `lock_path`'s own doc comment states. The assertion was wrong, not the code. Relaxing the assertion would have deleted a real Unix-side guard against a symlink-truncation attack.

End state: nine failures, three root causes, **zero product changes** — and CI went 14/15 green.

**Confirming data points:**
1. WIN-28, this session: 2 of 3 clusters were correct-implementation cases.
2. Pairs with F-4 — in both cases the written artifact (a test, a bug file's Fix section) was the thing that was wrong, and the code was right.

**Impact:** high — prevented a data-loss regression and preserved a security guard.

**Promote-when:** A second platform-specific failure resolves as "assertion wrong, code right". At two datapoints, promote to CLAUDE.md as "a platform-specific test failure is a claim about the code, not a fact about it — read the implementation and its guard comments first".

**Status:** validated — `cd643d58`, CI run `31098286970`, windows/default 3283 passed 0 failed.

## W-4 — Stating a duplicate hypothesis with its own falsification test, then letting CI run it

**Observed:** 2026-08-06, closing WIN-29 (`Windows-gnu cross`) as a duplicate of WIN-28.

**Pattern:** When closing a bug as a duplicate on inference rather than proof, write the falsification test into the closure — and name the observation that would reopen it. Here: dump both jobs' failing-test sets and diff them; reopen if the cross job ever fails a test `windows-latest` passes.

**Counterfactual:** Without the diff, `Windows-gnu cross` stayed the only red cell with no known cause, which made four red cells look like two independent problems and left the merge picture ambiguous. Guessing "probably the same" without the diff would have been indistinguishable from the truth right up until it wasn't. And the prediction was checkable: the duplicate claim says the cross job goes green from fixture fixes alone, with no MinGW- or wine-specific change. It did.

**Confirming data points:**
1. Failing sets identical, nine tests byte for byte (run `31092134665`).
2. `Windows-gnu cross` green on run `31098286970` after only the nine fixture fixes.

**Impact:** med — collapsed four red cells into one bug and produced a checkable prediction instead of an assumption.

**Promote-when:** A second duplicate closure carries a falsification recipe and it later fires (or holds). At two datapoints, promote to `get_guide("tracker-conventions")` as "a duplicate closure names the observation that reopens it".

**Status:** validated — prediction made before the evidence existed, then confirmed.

## F-6 — A capped findings list corrupted a magnitude estimate for the third time, in the document written to warn about it

**Observed:** 2026-08-06, round 4, planning the doc-drift work from
`docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md`.

**Expected (the plan):** "~11 gitignore-aware + ~42 marker + ~8 by hand" ≈ **61** findings,
sized as one focused session.

**Got:** per-directory `--paths` counts totalling **several hundred**, with four
directories each hitting the 50-cap so their true counts are still unknown. The estimate
was ~5× low.

**Probable cause:** the estimate was read off the JSON `findings` array, which
`OutputGuard` truncates to 50 while the exit code is computed over all of them. So the
plan described the *window*, not the population — and after the ranked-ordering fix (W-2)
the window is the 50 most severe, which reads even more like a complete list.

**Two aggravations specific to this instance:**

- `overflow.total` reports **46683**, which is `n_refs_found` — total *references scanned*,
  not total findings. Sitting beside `shown: 50` it invites reading 46683 as the finding
  count. Neither number is the one a planner needs.
- Scan order decides which of the ≥N high findings land in the window. The first top-50
  looked manual-dominated; after the manual was fixed, an entirely different 50 appeared
  from directories that had shown nothing. Progress was invisible in the aggregate for
  three consecutive rounds of real fixes.

**Severity:** med — no wrong code, but it mis-scoped the work twice and would have led to
reporting "backlog cleared" after clearing one directory's worth.

**Status:** mitigated — the method is now pinned in the backlog bug's § Resume and in
round 4's *Do NOT re-do*: count with `--paths` per directory, never from the top-level run.

**Fix idea / Pointer:** the audit should report an uncapped `n_findings_by_severity`
alongside the capped array — three integers, no truncation risk. This is the same defect
class as `2026-08-06-audit-doc-refs-gate-hides-its-own-cause` (already fixed by ranking):
the gate's own summary does not carry the number the gate acts on. Third instance overall
— the first corrupted "18 findings" in a bug file, the second hid the gating cause, this
one mis-sized the plan. Kin R-50 ("the view is not the set").

## W-5 — Invoking the tool under test, instead of reading it, produced five defects that reading had missed

**Observed:** 2026-08-06, round 4, working the `audit_doc_refs` backlog. The plan of
record had been written by *reading* the extractor; this round ran it.

**Pattern:** when the seam is a **tool**, the scout is one real invocation whose output
you read — preceded by confirming the artifact you invoked was built from current source.

**Counterfactual, with the concrete finds:**

1. The local release binary was **two days old and predated all four source files** of the
   tool under test. Every number from it would have described the *old* extractor — the
   exact error that made three prior CI verdicts meaningless (F-5 kin). Caught by
   `find src crates -name '*.rs' -newer target/release/codescout` before any measurement.
2. Running it died with **SIGABRT** — `ignore`'s `matched_path_or_any_parents` panics
   rather than errors outside its root. Unknowable from the signature, which returns
   `Match`, not `Result`. Reading the crate docs would not have surfaced it either.
3. A test written for that panic asserted on the real verdict and caught a **pre-existing**
   escape: `/etc/passwd:12` resolved **`Resolved`**, range-checked against the host's file.
   Two ref kinds lacked a guard the third had always carried.
4. The backlog bug's own written premise — "the extractor only reads code spans and
   links" — was **false**: `parse_refs` walks code-block text too. Measured split of one
   top-50: 18 prose / 18 fenced / 2 indented, so half the population was invisible to the
   plan built on that sentence.
5. `RefPosition` was populated by the parser and **read by nothing**. The discriminator
   the fix needed already existed, unused — visible only by grepping its uses, not by
   reading the parser that writes it.

Without the invocation: a SIGABRT shipped into a CI gate, a path escape left in place, a
mechanism hand-rolled beside the unused one already there, and a plan built on a false
premise about which half of the corpus it covered.

**Impact:** high — four of the five are correctness defects in a gate, and the fifth would
have produced duplicate machinery.

**Promote-when:** already at threshold with W-1 ("dumping an internal pure function's real
output beat reasoning from its source") — same lesson, one level up: W-1 was a function,
this is a whole tool. Captured as **R-57** in `docs/trackers/reconnaissance-patterns.md`
with a proposed Phase 1 addition. Promote to the reconnaissance SKILL.md if a third
tool-behaviour seam repeats it, since the lesson is craft-shaped, not project-shaped.

**Status:** validated — five independent finds in one session, each confirmed by a
failing test or a crash, not by inference.
## W-6 — Mechanising the decidable half of a cleanup is what makes the undecidable half visible

**Observed:** 2026-08-06, round 5, clearing the stale archive-citation class.

**Pattern:** when a cleanup has a part that is decidable *without reading prose*,
mechanise exactly that part and run it over the whole corpus first. The residue is
then, by construction, the part that needed judgement — and it is small enough to read.

The decidable rule here:

> ref is `<dir>/<name>.md`, that path does not exist, `<dir>/archive/<name>.md` does
> → insert `archive/`.

**Counterfactual, concrete:** the plan of record sized this class from a sample of 11
and called it "mostly bug files archived today". Run over the corpus it was **156
citations across 62 tracked files** — 14× the sample — and it included a *tracker*
(`vdi-reliability-session-log.md`), not only bug files, which the sample had missed
entirely. Fixing 156 sites by hand at a conservative two calls each is ~300 tool
calls; the script was three.

**The part worth keeping:** the mechanical pass then surfaced something no rule could
have decided. `windows-platform-support.md` listed three files under **"Active bug
files"** — and after the repoint those three read
`docs/issues/archive/…`, i.e. "active" pointing at the archive, in a section that
already had a separate "Archived CI-Windows bugs" line beside it. The contradiction was
invisible while the paths were merely *wrong*; it became legible the moment they were
*right*. Same shape in reverse for four other sites, which on inspection were dated
`**Observed:** <date>` statements where "the only open bug" was true when written — those
were correctly left alone.

**Impact:** high — 14× scope correction, ~300 tool calls avoided, and one live tracker's
present-tense claim corrected that a hand pass would very likely have re-typed.

**Two guards the mechanisation needed, both non-obvious:**

1. **Idempotence by construction.** The replacement inserts `archive/` into the very
   substring being matched, so a second pass matches nothing. Verified rather than
   assumed: `grep -rc "archive/archive"` → 0.
2. **Shape assertion on the diff.** 195 insertions / 195 deletions across 62 files — a
   pure 1:1 line swap is what a path repoint must look like, and any other ratio would
   mean the sed had eaten something. Cheaper and stricter than reading 62 diffs.

**Promote-when:** a second cleanup where a decidable rule is separable from a judgement
call. At that point promote to the reconnaissance SKILL.md, since the lesson is
craft-shaped: *mechanise the decidable part first; the residue is the judgement, and its
size is the estimate you should have had.*

**Status:** validated — 156 sites, 0 remaining, diff shape asserted, one semantic
contradiction recovered that the mechanical rule could not have found.
## W-7 — Verify a filesystem-dependent gate against a fresh clone, not the working tree

**Observed:** 2026-08-06, round 5. `Audit Doc Refs` went red in CI on a tree where the
local gate had exited 0 six times in a row.

**Pattern:** for any gate whose verdict depends on *what exists on disk*, verify against
`git clone --depth 1 file://$PWD /tmp/x` and scan that, with the same binary. A working
tree is strictly more populated than a clean checkout — untracked runtime state, worktrees,
build output, tool caches — so every "does this path exist" check is optimistic locally,
and optimistic in the direction that produces a false pass.

**Counterfactual, and it is sharper than the usual F-5 skew:** the four CI findings were
`.git/worktrees/`, `.claude/worktrees/` and `.codescout/private-memories/`. Locally those
directories exist, so the refs **resolved** — they produced no finding *at all*. The local
run could not report them even in principle, at any severity. This is not "local is a
weaker gate"; it is "local cannot see this class". Six clean local runs were six
measurements of the wrong tree.

The clone cost one command and ~30 seconds, and it converted the next push from a guess
into proof: 881 files, 46673 refs, **0 high, EXIT=0**, with 8882 broken refs still
reported at `med`. Without it the alternative was another ~12-minute CI round-trip per
attempt, and one had already been spent.

**The second lesson, which is about the test rather than the gate.** The
`gitignored_path` cap *had* a passing test. It asserted `.worktrees/my-feature` — a file
*inside* an ignored directory. Docs name the directory itself, `.claude/worktrees/`, and
the matcher reads the final path component, which a trailing separator leaves empty. So
every directory-shaped ref escaped the cap while the suite stayed green. The fixture shape
was the blind spot, not the assertion: **when a rule is about paths, cover both `foo` and
`foo/`** — they are the same location and they are not the same string.

**Impact:** high — caught a whole silently-escaping class in the cap, and established a
verification step that removes CI round-trips from the loop for any filesystem-dependent
gate.

**Promote-when:** immediately for the project-shaped half — the fresh-clone check belongs
in `docs/RELEASE.md` beside the `Audit Doc Refs` step, since it is this repo's gate. The
craft-shaped half (`cover foo and foo/`; verify environment-dependent gates against a
clean checkout) is **R-58** in `docs/trackers/reconnaissance-patterns.md`.

**Status:** validated — one CI failure diagnosed, reproduced locally, fixed, and the fix
proven against a CI-equivalent tree before pushing.
## W-8 — Reading the fallback gate eliminated the hypothesis a whole harness was going to measure

**Observed:** 2026-08-06, round 6, working the open-bug ledger. The `symbols` search flake
(`docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`) was
filed with an explicit instruction: *"Not proposed — root cause unconfirmed. Needs a
controlled, scripted reproduction (N parallel `symbols(name=X)` calls immediately
post-activation, repeated across several projects/runs) before a fix can be targeted."*

**Pattern:** when a bug names a suspected mechanism, find the **fallback or guard that
would mask that mechanism** and read its condition *before* costing a reproduction. An
unreachable hypothesis is cheaper to eliminate than to measure.

**Counterfactual.** Everything pointed at LSP warming, and not weakly:

- the source's own comment names the exact state — *"a pathological LSP state (silent
  `workspace/symbol` on a still-indexing server …)"*;
- three degradation paths really are invisible to the agent: budget-exceeded yields
  `Ok(Vec::new())` with only a `tracing::warn!`, and both a task error and a join error are
  silently `continue`d;
- the symptom (0-match, succeeds on retry) is textbook cold-index behaviour.

One line ended it. The tree-sitter fallback is gated on `matches.is_empty()` — the
aggregate of *already-filtered* matches — so it runs whenever the final answer would be
zero, no matter which languages degraded, and non-matching LSP results never enter
`matches` and so cannot suppress it. **A 0-match therefore implies tree-sitter found
nothing either, and tree-sitter never touches the LSP.** Server readiness cannot produce
this symptom at all.

So the harness as specified — parallel calls instrumented for LSP readiness — would have
run, produced a flake, and measured the wrong variable. The surviving hypothesis is a
different bug class entirely: both paths key off one `root` from
`require_project_root_for` (`symbols.rs:225`), and a root not yet settled post-activation
makes the LSP query one tree while the walker walks the same wrong tree — both correctly
returning nothing. Kin to the archived shared-server active-project race, which is the
same class already seen and fixed here once.

**Impact:** high — avoided building a harness around a refuted mechanism, and redirected
the instrument from LSP readiness to the resolved `root`, which is a one-line log rather
than a parallel-call rig.

**The restraint is part of the win.** No code change was made. `symbols` is the most-used
tool in the server, the cause is narrowed but *not* confirmed, and the bug's own rule is
"not proposed until confirmed". Patching the disclosure now would have papered over
whichever path actually fires. What did land is the diagnosis plus the house convention to
mirror once it is confirmed (`references.rs`'s `completeness_warning`, `list_overview`'s
`"lsp": "warming"`) — so the next session inherits the answer and the pattern, not a guess.

**Promote-when:** a second bug where reading a guard refutes the filed hypothesis. Captured
as **R-59**; craft-shaped, so it graduates to the reconnaissance SKILL.md rather than to
project memory.

**Status:** validated — hypothesis refuted from code, replacement hypothesis named with its
precedent, and the experiment redesigned before any of it was built.
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
