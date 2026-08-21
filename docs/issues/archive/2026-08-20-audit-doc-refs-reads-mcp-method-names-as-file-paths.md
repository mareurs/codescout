---
kind: bug
status: fixed
tags:
- librarian
- audit_doc_refs
- lint-precision
closed: 2026-08-21
opened: 2026-08-20
owner: marius
related:
- docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md
severity: low
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

Applied: capped the inferred-path severity from `Med` to `Low` in `severity::cap_inferred_path` (`src/librarian/tools/audit_doc_refs/severity.rs`) — Option 3 from the list below ("if the classification is a guess, low is more honest than med"), chosen as the smallest, lowest-risk change over Option 2 (promoting the filesystem check into the parser's classification stage, which would need `looks_like_path` to take repo-root context it doesn't have today).

**Two things found only by tracing the actual call chain, not visible from the symptom alone:**

1. `verdict_with_drops_for_ref` calls `apply_drops` (archive/memory/issues/historical location-based drops) BEFORE `cap_inferred_path`. The original `sev == Severity::High` guard in `cap_inferred_path` meant it only fired when NOTHING had already dropped the severity — so for any ref inside `docs/trackers/**` (which `apply_drops` already drops to `Med` via `historical_drop`), the guard silently never fired. That is exactly the bug's own reproduction shape (`docs/trackers/statement-validity-session-log.md`, `docs/trackers/prompt-surface-compaction-session-log.md`) — the originally-proposed fix would have been a no-op against the bug's own repro. Changed the guard to fire whenever evidence is `Inferred`, regardless of what already ran, flooring severity at `Low`.
2. That broader guard then collided with two more existing tests (`severity_drops_one_level_in_archive`, and a would-be conflict with any Memory/Issues drop): when a location-based drop already picked a specific reason (`archive_drop`, etc.), overwriting it with `inferred_path` loses the more informative explanation. Resolved by keeping the location-drop's reason when one applied (`reason != PolicyDefault`) and only overriding to `InferredPath` when nothing else already explained it — severity always floors at `Low`, but the reported *reason* stays whichever is more specific.

Not implemented: Options 1 (denylist) and 2 (classification-stage promotion) from below — left as-is; this fix only changes severity weighting, so `n_refs_broken`'s count is unchanged, only its confidence label.

**Commit citation is non-standard — recorded honestly rather than smoothed over.** The code was written and tested in this session's own working tree, but never reached a commit of this session's own: a concurrent Claude Code session (same author name, different model instance) ran `git commit` twice while these edits sat uncommitted in the shared checkout, sweeping them into two of its own commits alongside unrelated `docs/superpowers/specs/` work (`89eb83f0` — the test additions + first severity.rs revision; `f24c3788` — the refinement to reason-preservation). Verified via `git show <sha> -- <file>` that the landed content matches exactly what this session wrote and tested; no content was lost or altered.

- **SHAs (experiments), in landing order:** `89eb83f0eb55201beea3eb2c1d4ac0344d80f653`, then `f24c37887d6352e7eec167b205a902b6162a78cb` — neither commit is *only* this fix; both also carry the concurrent session's unrelated spec/tracker changes.
- **patch-id:** reconstructed rather than taken from a single commit, since no single commit is this fix alone — `git diff d85c1572..f24c3788 -- src/librarian/tools/audit_doc_refs/{resolver,severity}.rs | git patch-id --stable` → `3745de7e3b5bd15d5b87313dffc619120762f0ab`. This identifies the *net* change to those two files across the range, not a single commit's diff; re-deriving it after any further edit to either file requires re-picking the base and range by hand, since there's no single SHA to `git show`.
## Tests added

All in `src/librarian/tools/audit_doc_refs/resolver.rs`, same commit:

- `mcp_method_name_ref_caps_to_low` — the bug as reported (`tools/list` cited from a `docs/trackers/**` file); asserts `Low` + reason stays `historical_drop`.
- `resolver_ref_with_absent_root_segment_is_capped` (pre-existing, updated) — non-historical file, asserts `Low` + reason `inferred_path`.
- `resolver_still_missing_when_basename_not_in_index` (pre-existing, updated) — bare name, non-historical file, asserts `Low` + reason `inferred_path`.
- `severity_drops_one_level_in_archive` (pre-existing, updated) — bare name cited from `docs/archive/**`, asserts `Low` + reason stays `archive_drop` (the reason-preservation behavior).

Written RED-first: the new test and the three pre-existing ones were all run against the unmodified code first (three already-passing tests had their severity assertion changed to the new expected value, then re-run to confirm they failed against old code for the stated reason, before the fix landed). `cargo test --lib audit_doc_refs` — 157 passed, 0 failed. Full `cargo test` + `cargo clippy --all-targets -- -D warnings` clean on `experiments`.
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
