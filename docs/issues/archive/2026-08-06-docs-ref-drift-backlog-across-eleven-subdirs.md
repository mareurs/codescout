---
kind: bug
status: archived
tags:
- docs
- audit_doc_refs
- ci
- drift
- backlog
closed: 2026-08-06
closed_sha: 6348dfad (experiments)
opened: 2026-08-06
owner: marius
related:
- '56b725405a9c36d1'
- '21f6d21b3bf82c30'
severity: medium
---

# BUG: real doc-reference drift in 11 of 16 docs/ subdirectories keeps the Audit Doc Refs gate red

## Summary

With the `audit_doc_refs` extractor false positives fixed and the 50-finding cap now
ordered most-severe-first, the `--fail-on high` gate still exits 1 — this time on
**genuine** drift. The shown window is now 50/50 high-severity, and a
per-subdirectory bisect puts surviving high findings in 11 of 16 `docs/`
subdirectories. This is a docs-hygiene backlog, not a code defect, and it is the
last thing standing between `experiments` and a green `Audit Doc Refs` job.

## Symptom (Effect)

```
$ cargo run --bin codescout -- audit-doc-refs --no-emit-tracker --fail-on high --json --project .
=== CLI EXIT: 1 ===
$ grep -c '"severity": "high"' gate2.json
50            # the full shown window, i.e. >= 50 high findings exist
```

Representative sample from the ordered window — each is a distinct drift shape:

| `raw_ref` | `verdict` | What actually happened |
|---|---|---|
| `src/server.rs::from_parts` (×5) | `symbol_missing` | symbol renamed or removed; docs still cite it |
| `refresh.rs::call` (×2) | `file_missing` | bare basename + symbol; the file moved |
| `docs/issues/archive/2026-05-18-il3-pipe-violation-subagent.md` | `missing` | archived to `docs/issues/archive/` |
| `docs/issues/archive/2026-06-14-librarian-artifact-index-port-to-qdrant.md` | `missing` | archived; cited from more than one doc |
| `docs/findings/2026-07-18-reconnaissance-mcp-eval-findings.md` | `missing` | `docs/findings/` does not exist |
| `docs/trackers/mrv-chat-watch/` | `missing` | tracker directory gone |
| `.claude/codescout-companion.json` | `missing` | config file path changed |

## Reproduction

At `184dbced`+ on `experiments`, with the extractor fixes in place:

```bash
cargo run -q --bin codescout -- audit-doc-refs --no-emit-tracker \
  --fail-on high --json --no-color --project . > gate.json; echo $?   # 1
grep -c '"severity": "high"' gate.json                                # 50
```

Per-subdirectory bisect (the practical way to work this down, since the window
caps at 50):

```bash
for d in docs/*/; do printf '%-24s ' "$d"; \
  cargo run -q --bin codescout -- audit-doc-refs --no-emit-tracker \
    --fail-on high --paths "$d**/*.md" --project . >/dev/null 2>&1; echo "exit=$?"; done
```

Measured 2026-08-06:

```
adrs=1  architecture=1  archive=0  conventions=0  evals=1  issues=0
lessons=1  manual=1  plans=1  research=1  reviews=1  spikes=0
superpowers=1  templates=0  trackers=1  usage-reports=1
CLAUDE.md=0 (fixed)   **/README.md=1
```

## Environment

Linux, Rust 1.95.0, codescout 0.15.0, branch `experiments`, default features.

## Root cause

Not a code defect. Ordinary fix-then-forget doc rot, accumulated over a
387-commit branch: files archived without updating inbound references, symbols
renamed without grepping the docs, directories retired while docs still name them.

The reason it reads as a surprise is that it was **never visible**. Two layers hid it:

1. `audit_doc_refs`'s extractor produced enough false positives that the signal was
   indistinguishable from noise (`docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`).
2. The 50-finding cap truncated in scan order, so the gate reported failure while
   showing only resolved refs
   (`docs/issues/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md`).

With both fixed, the backlog is measurable for the first time. This is the third
bookkeeping surface to leak under the same root cause named in CLAUDE.md's
verify-open cadence note — alongside zombie-open tracker entries and the bug-file
archive discipline.

## Evidence

Two findings from this class were fixed during the extractor work, both confirmed
genuine before editing, which is what establishes that the rest are worth the same
treatment rather than more lint-tuning:

- `docs/adrs/2026-07-20-artifact-vec-shared-catalog-boundary.md` cited
  `docs/issues/archive/2026-06-14-librarian-artifact-index-port-to-qdrant.md`; `ls` confirmed
  the file lives at `docs/issues/archive/…`. Fixed by correcting the path.
- `CLAUDE.md` cited `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`.
  `git log --all --follow` showed it added in `dc44ac3d` and **pruned** in
  `c6184884` ("prune stale issue/tracker dupes") — deleted deliberately, not
  renamed, so there was no correct target. Fixed by replacing the dead pointer with
  the fix SHA plus the still-open kotlin bug.

One false-positive class survives and is worth knowing before working the list:
a **deliberate statement of absence** reads as a citation. `CLAUDE.md` said
*"There is no `docs/ARCHITECTURE.md`; it was deliberately deleted"* — correct prose
the lint cannot distinguish from drift. Handled by removing the code span.

**Correction (2026-08-06, later the same day):** the sentence that used to sit here —
"the extractor only reads code spans and links" — is **wrong**, and acting on it would
mislead. `parse_refs` also walks the *text inside code blocks*
(`Event::Text(content) if in_code_block`), fenced and indented alike, which is how
`git worktree add .worktrees/my-feature` inside a ```bash block became a finding.
Un-code-spanning a path only helps in prose; inside a fence there are no backticks to
remove. Measured on the real corpus: of 38 finding sites in one top-50, **18 were in
prose, 18 fenced, 2 indented** — so the fenced half was invisible to this claim.

The `<!-- audit-doc-refs:ignore -->` convention guessed at here is now
evidence-backed rather than speculative: 11 findings in the reader-facing manual
alone are fictional teaching paths or correctly-documented user/runtime config, and
no prose edit can fix them without making the docs worse. See § Resume.

## Hypotheses tried

1. **Hypothesis:** the surviving highs are more extractor false positives.
   **Test:** sampled 5 across `CLAUDE.md`, `**/README.md` and `docs/adrs/`;
   checked each against the filesystem and git history.
   **Verdict:** rejected — 3 of 5 were genuine drift (two archived-file moves, one
   pruned file), 2 were false positives of *new* classes (negative reference,
   date-template placeholder link), both since fixed in the extractor.

## Fix
**2026-08-06, third pass — RESOLVED. `--fail-on high` reports 0 findings.**

`./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json
--project .` → `EXIT=0`. Local gate green alongside it: fmt, `clippy --all-targets -D
warnings`, 3498 tests / 0 failed. Commits `a7c1d7f6` (code) and `297e1074` (docs) on
`experiments`.

### How the residue was actually distributed — four classes, not three

The plan below had three. The fourth only appeared once the first three stopped
saturating the 50-finding window, which is the same lesson as F-6:

1. **Dated, point-in-time records** — the overwhelming majority. Resolved by
   `historical_drop`, extended to root-level `docs/review-*.md` after
   `docs/review-2026-03-05.md` turned out to hold 17 line-pinned refs into modules
   since split apart. The librarian's own classifier patterns already name that form.
2. **Stale archive citations** — 156 across 62 tracked files, 14× the sampled
   estimate, and including a *tracker* the sample had missed. Mechanised; see W-6 in
   `docs/trackers/release-promotion-session-log.md`.
3. **Fictional teaching paths and documented-but-absent files** — resolved by the
   section-scoped marker.
4. **Configuration values that merely look like paths** — not anticipated at all.
   `librarian-embedded.md` lists built-in classifier *patterns*; ROADMAP names modules
   for unbuilt features. Both docs are correct and neither reference can ever resolve.
   Marker, with the reason inline.

### The part that was genuine drift after all that

Small, and worth naming because it is what the gate exists for:

- `docs/PROGRESSIVE_DISCOVERABILITY.md`'s File References table pointed at
  `src/tools/symbol.rs` for three functions that now live in **two** modules
  (`src/symbol/query.rs`, `src/tools/symbol/symbols.rs`) — one row had to become two —
  plus `src/prompts/server_instructions.md`, renamed to `source.md` long ago.
- Five dead ROADMAP pointers. **Three had no successor anywhere in history** (the v1
  implementation plan, the MCP-elicitation note, the contributor-skills design) and are
  now stated as deleted rather than repointed at a guess. Its Kotlin-LSP pointer named
  a bug pruned in `c6184884` after the fix landed in `dc44ac3d`.
- `src/tools/run_command.rs` is a directory now.

### Guard against reading the green as "clean"

The audit still emits **8388 broken refs**, all at `med`. The bands moved; nothing was
suppressed out of the report. **If a future change drops that count too, distrust it** —
that is the signature of an extractor regression rather than a docs improvement. And
`docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md` is still open: a flap
into a `high` verdict now costs a spuriously red gate, so the non-determinism matters
*more* at zero than it did at several hundred.
**2026-08-06, second pass — substantially worked. The estimate below was wrong by ~5×,
for the same reason the original bug existed: it was measured through the 50-finding cap.**

Read this subsection before the older text under this heading; where they disagree,
this one is measured and the older one is not.

### The magnitude, corrected

The earlier plan ("~11 gitignore-aware + ~42 marker + ~8 by hand") described the
top-50 *window*, not the population. Per-directory counts with `--paths`:

| area | high findings | character |
|---|---|---|
| `docs/manual/**` | 22 → **11** | reader-facing; the half that mattered |
| `docs/research/**` | 37 | dated research notes |
| `docs/usage-reports/**` | 20 | dated analyses |
| `docs/plans/**` | ≥50 (capped) | plans, superseded once built |
| `docs/reviews/**` | ≥50 (capped) | dated reviews citing pinned line ranges |
| `docs/superpowers/**` | ≥50 (capped) | Feb–Mar plans |
| `docs/trackers/**` | ≥50 (capped) | session logs |
| `docs/{adrs,architecture,archive,conventions,evals,issues,spikes,templates}` | **0** | cleared this pass |

So the real residue is **several hundred**, and it is overwhelmingly one class:
**dated, point-in-time documents whose references were correct when written.** A code
review citing `src/tools/github.rs:680-690` is pinned to the commit it reviewed;
auditing it against HEAD is a category error, not drift.

### What landed (all verified against the filesystem or `git log`, per the rule below)

Genuine drift, fixed:

- The ADR citation this file's § Evidence reported as *already fixed* was **half-fixed** —
  line 43 carried the `archive/` prefix, line 137 did not. Two citations, one file, one
  corrected. Reporting a partial fix as complete is worse than reporting none.
- `docs/conventions/test-env-isolation.md` asserted `src/librarian/indexer.rs` "still
  carries an `EnvGuard`" and that "two `EnvGuard` uses remain". Both false — `45669701`
  removed it (added `109c1ead`); `grep` finds exactly **one** left, the exempt
  `src/agent/mod.rs`. Self-inflicted drift: the fix shipped, the convention doc did not.
- Four archived bug files repointed; the `PROGRESSIVE_DISCLOSURE` →
  `PROGRESSIVE_DISCOVERABILITY` rename applied at all five live citations.
- `docs/manual/src/concepts/librarian-mcp.md` cited `crates/librarian-mcp/CREDITS.md`.
  The crate was **dissolved** in `d48bf992` (`Cargo.toml:141` says so); the directory
  holds only a leftover `usage.db` and `.gitignore`. The page now says the file is gone
  and where attribution survives.
- `docs/manual/src/tools/semantic-search-diversity.md` documented an **inactive**
  feature: `MAX_CHUNKS_PER_FILE` exists nowhere in the tree, and
  `apply_file_diversity_cap` carries `#[allow(dead_code)] // re-wire when the stack
  search gains file-diversity capping (tracker L-15)`. Now carries a status callout.
  Path drift was the symptom; aspirational documentation was the disease.
- Two links broken **in the rendered book**, not merely in the lint: an extra `../` in
  `why-codescout.md` and in `troubleshooting.md` (×2).
- `docs/manual/src/concepts/tool-selection.md` had **stray duplicate ``` fences** at two
  places. Because ```` ```json ```` is not a valid *closing* fence, each stray opened a
  block that swallowed the following prose — two paragraphs and a JSON example rendered
  as literal code. Found only because the fence desync made a finding look like prose.

Extractor/resolver defects, fixed with tests:

- **`matches_archive` tested the literal `docs/archive/`.** The project archives *in
  place*, so `docs/trackers/archive/`, `docs/plans/archive/`, and
  `docs/superpowers/plans/archive/` got **no archive drop at all**. Now any `archive/`
  path segment counts. (This also corrects a claim in this file: `docs/issues/archive/**`
  took `issues_drop`, never `archive_drop`.)
- **`resolve_file_line` / `resolve_file_symbol` had no outside-project guard** that
  `resolve_file_path` always had. `Path::join` discards the base on an absolute argument,
  so a doc citing `/etc/passwd:12` resolved **`Resolved`** — range-checked against the
  host's real file. Now all three share `points_outside_project`.
- **Link resolution knew only one of the repo's two conventions.** `docs/manual/src/**`
  is an mdBook, where an internal link *must* be page-relative; the resolver rooted
  bare links at the repo root, reporting ~28 correct links as missing. Both conventions
  now resolve.
- **`path.md#fragment` links were stat'd whole**, fragment included — 8 findings that
  reported an existing page as a missing file. The fragment is now split off (and
  deliberately not validated: mdBook's slug rules are its own).

Three severity caps added, each one sentence to justify:

- `code_block` — a fenced or indented block is a *transcript* the reader runs, not a
  citation the author makes. Uses `RefPosition`, which the parser already populated and
  **nothing read**.
- `gitignored_path` — a gitignored path is *expected* absent (`.codescout/embeddings.db`,
  `.worktrees/my-feature`, `.claude/codescout-companion.json`), so absence carries no
  drift signal. Strictly narrower than excluding the page: a doc naming both a runtime
  file and a real tracked path still gates on the tracked one.
- markup-display spans are **skipped** rather than capped: a span whose content contains
  a backtick was written with multi-backtick delimiters to render markup literally. The
  audit's own manual page was flagging its own "Reference kinds" example column.

One bug found and filed, not fixed:
`docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md` — identical command,
identical tree, different high counts (`evals` 2→0, `conventions` 1→0). Every scan
self-reports `scan_meta.degraded: true` and still emits an authoritative exit code.

**2026-08-06 — one whole class is already enumerated and ready to fix: a guide filename that has never existed.**

docs/PROGRESSIVE_DISCLOSURE.md (deliberately un-code-spanned — see the trap note
below) is cited **6 times across 5 files** and there is no such
file. The real one is `docs/PROGRESSIVE_DISCOVERABILITY.md` (confirmed by `ls`, and it is
the name CLAUDE.md uses). Every citation is a plain rename:

```
docs/trackers/bug-fix-session-log.md:2512                                   <- GATES
docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md:53,84
docs/issues/2026-07-28-il3-gate-matches-pipes-inside-heredoc-text.md:183
docs/issues/archive/2026-07-28-memory-sections-filter-matches-h3-only.md:226
```

Only the first gates — `docs/issues/**` takes `issues_drop` and `docs/issues/archive/**`
takes `archive_drop`, both High→Med. Fix all six anyway; five of them are wrong regardless
of severity.

The sixth citation was **introduced by this session** and is already fixed, in
`docs/issues/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md` § References — along
with two more self-inflicted ones: that file cited the two `audit_doc_refs` bug files at
their *pre-archive* paths, which archiving them earlier the same day had invalidated.

**That is the generalisable warning for whoever works this list: archiving a bug file
breaks every inbound citation, and nothing tells you.** Six files were archived on
2026-08-06; their citers are part of this backlog. Check with
`grep -rl '<archived-basename>' docs/` after any `artifact(action="move")`.

**A second self-inflicted trap, worth avoiding while documenting this bug:** writing an
illustrative path inside a code span, in a file under a gating directory, *adds* a finding.
The round-3 Resume in `docs/trackers/release-promotion-session-log.md` originally
code-spanned src/services/auth.rs and src/foo.rs while describing them as false positives,
which would have added ~19 high findings to the very backlog it documents. They are
un-code-spanned there now. Prose naming a nonexistent example path must stay outside
backticks — the extractor reads only code spans and link targets.

Not implemented — deliberately out of scope for the merge-prep pass that found it.
Two reasons: the population is large enough to need its own session, and mixing
dozens of doc edits into a commit whose subject is extractor precision would make
both unreviewable.

Recommended order when worked:

1. **`symbol_missing` first** (`src/server.rs::from_parts` and friends). These are
   the highest-value findings — a doc citing a symbol that no longer exists is
   actively misleading, and `symbols(name=…)` gives the correct target in one call.
2. **Archived-file references next.** Mechanical: `ls docs/issues/archive/<name>`
   and prefix the path. Several files cite the same moved bug, so fix by target
   rather than by citing file.
3. **Retired directories last** (`docs/findings/`, `docs/trackers/mrv-chat-watch/`).
   These need a judgement call per reference: repoint, or delete the sentence.

Do **not** work this list by editing prose to satisfy the lint. Every finding gets
the same treatment the two fixed ones got: confirm against the filesystem and
`git log --all --follow` first, and if the reference is correct, the extractor is
what needs the change.

## Tests added

N/A — no code change. The regression guard is the CI gate itself: once the backlog
reaches zero, `Audit Doc Refs` stays green and any new drift fails the job with the
offending ref now visible in the output (per the cap-ordering fix).

## Workarounds

CI's `Audit Doc Refs` job stays red until this is worked down. It does not block
the `experiments` → `master` fast-forward promotion, which gates on tests, clippy
and fmt (see `docs/RELEASE.md` § Large-Cohort Promotion) — but it does mean the
doc-lint signal is unusable until then, since everything is red.

## Resume
**CLOSED 2026-08-06 — `Audit Doc Refs` is green in CI. Run 31107853410 on `6348dfad`:
15/15 jobs green, the first fully-green run on `experiments`.**

The SHA above is an **`experiments`** SHA and will be orphaned by the next
`git rebase master`. **The master-side SHA still needs recording after cherry-pick** —
recover it with `git log master --oneline --grep="gitignored-path cap"` if lost.

Verified three ways, because a local green had already lied once:

1. `--fail-on high` → `EXIT=0` on the working tree, six consecutive runs.
2. `--fail-on high` → `EXIT=0` against a fresh `git clone --depth 1` — the check that
   caught what the working tree could not see. 881 files, 46716 refs.
3. CI, 15/15.

**And the number that says it was not silenced: 8904 broken refs are still reported**, all
at `med`. If a later change drops that count too, distrust it.

The two decisions this file existed to frame were both taken — `historical_drop` and the
section-scoped marker. Rationale, alternatives, and the arguments against each are in
§ Resume below and in `docs/trackers/release-promotion-session-log.md` § *Resume — round 5*.

Still open, and deliberately not folded into this closure:
`docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`. At zero high findings
a single flap now decides green-versus-red, so that bug matters *more* after this fix than
before it.
### Ready to script — the archived-citation class is a pure prefix insertion (11/11 confirmed)

Do this before either decision below; it needs no policy call and it is the largest
mechanical chunk left. Sampled every archived-file finding in the three biggest live
trackers — `release-promotion-session-log.md`, `reconnaissance-patterns.md`,
`codescout-usage-frictions.md` — and **11 of 11** resolved to
`docs/issues/archive/<identical-basename>.md`. Not one needed a judgement call.

The rule, and it is decidable without reading the prose:

> if a ref is `docs/issues/<name>.md`, that path does not exist, **and**
> `docs/issues/archive/<name>.md` does, the citation is stale and the fix is to insert
> `archive/`.

Confirmed instances included `2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail`,
`2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files`,
`2026-08-06-ast-chunker-recursion-duplicates-leading-gap`,
`2026-07-28-audit-doc-refs-json-pointer-false-positive`,
`2026-05-28-path-annotation-spam` — i.e. mostly bug files archived *this same day*, which
is the warning in § Fix playing out at scale: **archiving a bug file breaks every inbound
citation and nothing tells you.**

**Two cautions for whoever scripts it:**

- **Route tracker writes through the librarian.** `artifact(action="update",
  patch={body_edits:[…]})` accepts a batch, and each entry takes `replace_all`. A bare
  `sed -i` is wrong for any *augmented* tracker (`tool-usage-patterns.md` is one) because
  its body is rendered from the catalog and a direct file edit is overwritten. Check
  `artifact(action="get").augmentation` before touching a file.
- **Do not generalise this into the resolver.** Teaching `resolve_file_path` to fall back
  to `archive/` would make every stale citation resolve silently — and a reader following
  a pre-archive path still gets nothing. The staleness is real; only the *fix* is
  mechanical. This is also why `try_basename_fallback` deliberately skips refs containing
  a slash.

Two decisions remain, and **both are gate-semantics calls, not code details** — which
is why they were left rather than taken. Everything that did not depend on them is
done (see § Fix, second pass).

### Decision 1 — do dated, point-in-time documents gate CI? (~250+ findings)

This is the whole remaining blocker. `docs/{plans,reviews,research,usage-reports,
superpowers}` and the live `docs/trackers/**` are documents written once, dated in the
filename, describing a moment. Their stale refs are historically correct.

The severity model **already** encodes this idea three times — `archive_drop`,
`issues_drop`, `memory_drop`. Extending it is a continuation, not an invention. Two
shapes:

- **(a) Drop severity** for those surfaces, as `issues_drop` does. Findings stay visible
  in the emitted tracker; they stop gating. Keeps one mechanism.
- **(b) Narrow the CI scan set** to reader-facing docs (manual, root `*.md`, `CLAUDE.md`,
  `**/README.md`, `architecture`, `conventions`, `adrs`) and leave the wide scan to the
  manual tracker-emitting mode. The current default — all of `docs/**` — is far broader
  than the tool's own stated purpose ("CI gates against `master`/PR docs to catch drift").

**Recommendation: (a).** It reuses the existing mechanism, keeps the findings
discoverable, and does not quietly shrink what the gate ever looks at.

**The one line worth arguing about:** live `docs/trackers/**` session logs. They are
consulted operationally, so a stale ref there costs a reader something — unlike a
February plan. Consider gating live trackers and dropping only their `archive/`
subdirectory (which this pass already fixed).

### Decision 2 — the marker convention for the last 11 manual findings

All 11 survivors in `docs/manual/**` are either **fictional teaching paths**
(`src/services/auth.rs`, `src/foo.rs`, `src/util/helpers.rs`, `src/auth.rs`,
`src/foo.py`) or **correctly-documented user/runtime files** (`.codescout/config.toml`,
`.codescout/memories/**`) or a **configuration value that looks like a path**
(`docs/ARCHITECTURE.md`, listed as a built-in classifier *pattern* — the doc is right;
the pattern need not resolve).

No prose edit fixes these without making the docs worse, so the
`<!-- audit-doc-refs:ignore -->` marker is now the answer. The open question is scope:

- **Section-scoped** (`recommended`) — suppress from the marker to the next heading.
  Matches how examples actually cluster, and works inside tables, where a line-scoped
  marker cannot go: an HTML comment between table rows breaks the table. All 11 sites
  are single leaf sections.
- **Line-scoped** — finest grain, but unusable for the two table cases.
- **File-scoped** — rejected. Those same pages also cite real codescout paths, and
  that is where drift hurts readers most.

### Do NOT re-do

- **Do not blanket-exclude `docs/manual/src/concepts/**`.** Measured: the manual
  cites real code, and this pass found two links broken in the *rendered book* plus a
  page documenting an inactive feature. Excluding it would have hidden all three.
- **Do not "fix" a finding by editing prose to satisfy the lint.** Confirm against the
  filesystem and `git log --all --follow` first; if the reference is correct, the
  extractor is what needs the change. This pass found five extractor/resolver defects
  that way and only then stopped finding them.
- **Do not trust a count read through the 50-finding cap.** It produced the wrong
  magnitude twice: once in the original bug ("18 findings") and once in this file's own
  first estimate (~61 vs. several hundred). Use `--paths` per directory.
- **Do not trust a single green run.** The gate is non-deterministic —
  `docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`.

### Trap to avoid while working this list

Writing an illustrative path **inside a code span, in a gating directory, adds a
finding**. This file's own text hit it twice. Prose naming a nonexistent example path
must stay outside backticks — and note the corrected rule in § Evidence: inside a
fenced block there are no backticks to remove, because the extractor walks block text
too.
## References

- `docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` — extractor precision (fixed).
- `docs/issues/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md` — the cap that hid this (fixed).
- `docs/RELEASE.md` § Large-Cohort Promotion (Fast-Forward) — why this does not block the merge.
- CLAUDE.md § verify-open cadence — the fix-then-forget root cause shared with the other two bookkeeping surfaces.
