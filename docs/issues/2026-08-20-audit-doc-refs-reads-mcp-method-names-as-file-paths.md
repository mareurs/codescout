---
status: open
opened: 2026-08-20
closed:
severity: low
owner: marius
related:
  - docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md
tags:
  - librarian
  - audit_doc_refs
  - lint-precision
kind: bug
---

> **KNOWN — same root cause as
> `docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`**
> ("reads `Type/method` and org/repo slugs as local file paths", fixed
> 2026-08-06). That fix replaced the extractor's unanchored-slash polarity with
> positive evidence of pathness, using **capitalization** as the discriminator.
> This is the shape that discriminator structurally cannot catch: an
> all-lowercase idiom that is not a path. Filed separately rather than reopened
> because the fix was correct for every class it enumerated — this is an
> uncovered shape, not a regression.

# BUG: `audit_doc_refs` reads MCP method names (`tools/list`) as file paths

## Summary

An MCP method name like `tools/list` is all-lowercase and slash-joined, so the
extractor's capitalization discriminator admits it as a relative path candidate.
It then resolves to nothing and is reported as a broken ref. Noise only — it
lands at `med`, below CI's `--fail-on high` — but it is noise in exactly the
documents that discuss the MCP surface, and it inflates `n_refs_broken`.

## Symptom (Effect)

`librarian(action="audit_doc_refs", paths=["docs/trackers/statement-validity-session-log.md"])`:

```json
{
  "md_file": "docs/trackers/statement-validity-session-log.md",
  "md_line": 496,
  "raw_ref": "tools/list",
  "ref_kind": "file_path",
  "verdict": "missing",
  "severity": "med",
  "severity_reason": "historical_drop",
  "status": "open"
}
```

## Reproduction

Commit `7c2e84eb` (branch `experiments`):

```
librarian(action="audit_doc_refs",
          paths=["docs/trackers/prompt-surface-compaction-session-log.md"],
          fail_on="med", emit_tracker=false)
```

That file (already committed, unrelated to this session) reports
`n_refs_broken: 24`, `exit_code: 1`, and carries 4 `tools/list` occurrences.

Population: `grep 'tools/list'` over `docs/**/*.md`, `CLAUDE.md` and
`**/README.md` returns **61 matches across 25 files** — a floor on how many of
these the full scan produces, not an exact count of findings, since some
occurrences sit inside code fences or URLs that the extractor may treat
differently. Not all were opened.

## Environment

Linux, codescout `v0.15.0`, branch `experiments`, project `codescout`.

## Root cause

`is_path_segment` (`src/librarian/tools/audit_doc_refs/parser.rs:666-671`)
accepts a segment consisting solely of ASCII lowercase letters, digits, and
`.`/`_`/`-`. `looks_like_path`'s unanchored-slash branch admits a token when
every segment passes it.

`tools` and `list` both pass. So does every other MCP method name the docs
discuss — `resources/read`, `resources/list`, `notifications/tools/list_changed`
— and, in principle, any all-lowercase two-segment idiom.

The function's own doc comment states the design premise:

> Capitalization is the discriminator. Real directory names are lowercase or
> kebab/snake (`docs`, `crates`, `codescout-embed`); an uppercase segment in an
> unanchored slash-joined token almost always means the token is an identifier
> idiom rather than a path.

The premise is sound in the direction it was written for — uppercase implies
*not* a path. The converse does not hold, and MCP method names are the standing
counterexample in a repo whose docs are largely *about* an MCP server.

**Measured 2026-08-20:** `is_path_segment` read at
`src/librarian/tools/audit_doc_refs/parser.rs:666-671`; the finding above
observed by running the tool. The `severity_reason: "historical_drop"` band was
observed but **not** traced to its code path — it is reported here as data, not
as an explained mechanism.

## Hypotheses tried

1. **Hypothesis:** this session's new prose introduced the finding.
   **Test:** ran the audit against `docs/trackers/prompt-surface-compaction-session-log.md`,
   an already-committed tracker untouched this session.
   **Verdict:** rejected — 24 broken refs, `exit_code: 1`, pre-existing.

## Fix

Not implemented. Options, none obviously best:

1. **Denylist the MCP method vocabulary** (`tools/list`, `tools/call`,
   `resources/list`, `resources/read`, `prompts/get`, `notifications/*`).
   Cheapest and immediately correct, but the archived sibling explicitly
   rejected "adding a tenth denylist entry" as the wrong shape of fix.
2. **Require a filesystem-plausible first segment** — a token whose leading
   segment names no directory at the repo root is not a repo-relative path.
   `tools/` does not exist at codescout's root. Generalizes past MCP names, and
   is close to the `path_evidence` / `cap_inferred_path` machinery that
   already exists from the sibling fix.
3. **Cap it lower.** If the classification is a guess, `low` is more honest than
   `med`. Does not reduce noise, only its weight.

Option 2 looks right, but note it would need to not regress refs to paths that
are legitimately absent because the file moved — which is what a doc-ref audit
is for.

## Tests added

None — not fixed. A fix should add a parser case asserting `tools/list` and
`resources/read` are not classified `file_path`, alongside the existing
`Type/method` cases.

## Workarounds

Run the audit with `fail_on="high"`, which is what CI already does
(`.github/workflows/ci.yml:370`). The findings are noise, not breakage.

## Resume

Read `looks_like_path` and `path_evidence` in
`src/librarian/tools/audit_doc_refs/parser.rs` and decide between fix options 2
and 3. Check first whether `path_evidence` already computes "root segment absent
from the repo" — the archived sibling's fix item 3 describes exactly that test
for severity capping (`librarian/catalog.db` has no `librarian/` at the root),
so the predicate may already exist and only need promoting from the severity
stage to the classification stage.

## References

- `src/librarian/tools/audit_doc_refs/parser.rs:666-671` — `is_path_segment`
- `docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`
  — the same root, fixed for uppercase idioms
- `.github/workflows/ci.yml:370` — the gate, at `--fail-on high`
