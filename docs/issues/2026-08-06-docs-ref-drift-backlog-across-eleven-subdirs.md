---
status: open
opened: 2026-08-06
closed:
severity: medium
owner: marius
related: ["56b725405a9c36d1", "21f6d21b3bf82c30"]
tags: [docs, audit_doc_refs, ci, drift, backlog]
kind: bug
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
| `docs/issues/2026-05-18-il3-pipe-violation-subagent.md` | `missing` | archived to `docs/issues/archive/` |
| `docs/issues/2026-06-14-librarian-artifact-index-port-to-qdrant.md` | `missing` | archived; cited from more than one doc |
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
   indistinguishable from noise (`docs/issues/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`).
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
  `docs/issues/2026-06-14-librarian-artifact-index-port-to-qdrant.md`; `ls` confirmed
  the file lives at `docs/issues/archive/…`. Fixed by correcting the path.
- `CLAUDE.md` cited `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`.
  `git log --all --follow` showed it added in `dc44ac3d` and **pruned** in
  `c6184884` ("prune stale issue/tracker dupes") — deleted deliberately, not
  renamed, so there was no correct target. Fixed by replacing the dead pointer with
  the fix SHA plus the still-open kotlin bug.

One false-positive class survives and is worth knowing before working the list:
a **deliberate statement of absence** reads as a citation. `CLAUDE.md` said
*"There is no `docs/ARCHITECTURE.md`; it was deliberately deleted"* — correct prose
the lint cannot distinguish from drift. Handled by removing the code span, since the
extractor only reads code spans and links. If this recurs often, an
`<!-- audit-doc-refs:ignore -->` convention is the principled answer.

## Hypotheses tried

1. **Hypothesis:** the surviving highs are more extractor false positives.
   **Test:** sampled 5 across `CLAUDE.md`, `**/README.md` and `docs/adrs/`;
   checked each against the filesystem and git history.
   **Verdict:** rejected — 3 of 5 were genuine drift (two archived-file moves, one
   pruned file), 2 were false positives of *new* classes (negative reference,
   date-template placeholder link), both since fixed in the extractor.

## Fix

**2026-08-06 — one whole class is already enumerated and ready to fix: a guide filename that has never existed.**

`docs/PROGRESSIVE_DISCLOSURE.md` is cited **6 times across 5 files** and there is no such
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

Run the per-subdirectory bisect above to get the current 11-subdir list, then start
with `docs/adrs/` — smallest failing set with a known-genuine finding already
confirmed in it. For each finding: `ls` the target, then
`git log --all --follow -- <path>` if absent, then fix the reference or the
extractor depending on what that shows. Re-run the subdir's scan to confirm it
flips to `exit=0` before moving on.

## References

- `docs/issues/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` — extractor precision (fixed).
- `docs/issues/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md` — the cap that hid this (fixed).
- `docs/RELEASE.md` § Large-Cohort Promotion (Fast-Forward) — why this does not block the merge.
- CLAUDE.md § verify-open cadence — the fix-then-forget root cause shared with the other two bookkeeping surfaces.
