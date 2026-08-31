---
id: e7eebd21a5c0cd99
kind: bug
status: fixed
title: 'BUG: a `repo:file-stem:ID` citation silently degrades to `file-stem:ID` — the repo qualifier is dropped by regex slide, not parsed'
tags:
- librarian
- link-scan
- citations
- extraction
- cluster/addressing-without-an-escape-hatch
closed: 2026-08-27
opened: 2026-08-26
owner: marius
related:
- '08072e4a358640f0'
severity: low
---

## Summary

`get_guide("tracker-conventions")`'s own worked example distinguishes a repo-only
qualifier (`codescout:A-11`) from a file-stem qualifier (`bug-fix-session-log:F-33`), and a
bug file (`docs/issues/archive/2026-08-26-session-log-template-cites-own-ledger-ids-bare.md`)
combined the two into a THREE-part form, `<repo>:<file-stem>:<ID>`, to solve a case that
needs both — the current fix's own testing shows the resolver never sees that as three
parts. The leading `<repo>:` segment is silently dropped, and the citation resolves
exactly as if it had been written `<file-stem>:<ID>` all along.

## Symptom (Effect)

Given the citation text `codescout:statement-validity-session-log:F-2` scanned in a
foreign repo, `link_scan` resolves it identically to the two-part citation
`statement-validity-session-log:F-2` — no difference in `citations.raw`'s effect at all
(verified by writing both forms into an identical test fixture and observing byte-identical
`edges_added` / `cross_repo` output). There is no error, warning, or distinguishing report
row — the third segment is simply absent from the resolved outcome.

## Reproduction

```
git rev-parse HEAD   # 758c2e2e (experiments), post session that filed this
```

```rust
// tests/link_scan.rs — ad hoc, not committed standalone (see the actual regression
// added for the sibling bug, which exercises this indirectly):
add_artifact(&ctx, dir.path(), "docs/trackers/local-session-log.md", ID_B,
    "## F-2 — unrelated\n");
add_artifact(&ctx, dir.path(), "docs/trackers/topic-session-log.md", ID_TEMPLATE,
    "See `codescout:statement-validity-session-log:F-2` for the rule.\n");
// local-session-log.md's stem is "local-session-log", NOT
// "statement-validity-session-log" — no local file has that stem — so this citation
// should report cross_repo. It does. Now rename local-session-log.md's rel path to
// "docs/trackers/statement-validity-session-log.md" (same content) and re-run: the
// citation now produces an Edge to it, identical to what a bare
// `statement-validity-session-log:F-2` (no `codescout:` prefix at all) produces against
// the same fixture. The `codescout:` segment made no observable difference in either
// case.
```

## Environment

- codescout @ branch `experiments`, `src/librarian/tools/link_scan/{extract,resolve}.rs`
- Discovered while implementing the fix for
  `docs/issues/archive/2026-08-26-session-log-template-cites-own-ledger-ids-bare.md`

## Root cause

`resolve()`'s `CrossRepoToken` branch (`src/librarian/tools/link_scan/resolve.rs:245`)
does exactly one split: `citation.raw.split_once(':')` → `(qualifier, token)`. It was
written for a citation whose `raw` already has the shape `<one-qualifier>:<TOKEN>` — it
has no notion of a second qualifier segment at all, and nothing downstream re-attempts a
second split.

That means the actual defect is upstream, in **extraction**, not in this split. For input
text `codescout:statement-validity-session-log:F-2`, the extraction regex needs
`<qualifier-chars>:<TOKEN>` where `TOKEN` matches `[A-Z]{1,3}-\d+`. Starting the match at
`codescout` fails — the character after `codescout:` is `s` (lowercase), not a valid
`TOKEN` start — so the regex engine (leftmost-first, per the existing
`long_file_stem_qualifier_is_captured_whole_not_truncated_to_a_suffix` regression's own
account of this engine's matching behavior) slides its start position forward. It finds a
match starting at `statement-validity-session-log:F-2`, where the qualifier chars
(word-chars + hyphens, no colon) match `statement-validity-session-log`, and the token
`F-2` matches `TOKEN`. `codescout:` is left behind as ordinary, uncaptured prose text —
identical to how the resolver already treats a stray acronym.

`inferred from src/librarian/tools/link_scan/resolve.rs:245-250 plus black-box test
behavior — not measured against the extraction regex's literal source (see
`unverified:`).`

This is the same STRUCTURAL failure mode as
`docs/issues/archive/2026-08-18-link-scan-dangling-count-is-prefix-gated-so-a-whole-namespace-reads-as-healthy.md`'s
sibling fix, `long_file_stem_qualifier_is_captured_whole_not_truncated_to_a_suffix`
(`src/librarian/tools/link_scan/extract.rs:704`) — a SLIDE, not a non-match: the whole
point of that earlier regression is that this extraction regex silently reinterprets input
it cannot fully match by re-anchoring further right, rather than failing loudly or
capturing nothing. That fix pinned the case where a *single* qualifier segment was
truncated by a length bound. This is the same slide, one qualifier segment early — the
regex was never extended to accept (or explicitly reject) a second colon at all.

## Evidence

### The two forms are indistinguishable to the resolver

See § Reproduction. `codescout:statement-validity-session-log:F-2` and
`statement-validity-session-log:F-2` produced byte-identical `link_scan(write=true)`
output against the same fixture, on the same commit.

### The guide's own two examples are both single-qualifier

`get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified* gives exactly
two forms: `codescout:A-11` (repo-only) and `bug-fix-session-log:F-33` (stem-only). It
never prescribes a three-part form — that combination was invented in the sibling bug
file's own `## Fix` section, apparently without checking it against this resolver code.
This guide is not itself wrong; the sibling bug's prescribed remedy was.

## Hypotheses tried

1. **Hypothesis:** `resolve()`'s `split_once(':')` finds the wrong split point for a
   3-part token (e.g. splits after `codescout` and tries to look up
   `"statement-validity-session-log:F-2"` as a literal token).
   **Test:** read `resolve.rs:245-250` directly.
   **Verdict:** rejected — `split_once` behaves exactly as documented; the qualifier and
   token variables it produces are never wrong for whatever `citation.raw` it receives.
   The defect is upstream of this function.
   **Evidence link:** § Root cause.
2. **Hypothesis:** the extraction regex slides past an unmatchable qualifier prefix and
   re-anchors on a shorter valid `qualifier:TOKEN` suffix, silently dropping the prefix as
   uncaptured prose.
   **Test:** black-box — compare `citation.raw`'s effect for the 3-part vs. 2-part forms
   against an identical fixture (§ Reproduction).
   **Verdict:** confirmed by effect (both forms produce identical outcomes) — but not
   confirmed by reading the actual regex source, hence `unverified:`.
   **Evidence link:** § Reproduction, § Evidence.

## Fix

**Shipped 2026-08-27 — Option 2, extended:** rather than a bare warn tacked onto an
existing arm, a double-qualified citation now gets its own first-class report class.

A new `CitationKind::MalformedQualifier` is matched by a dedicated regex
(`double_qualified_re()`, `extract.rs`) built to match the FULL
`<qualifier>(:<qualifier>)+:<TOKEN>` span directly — so it wins the leftmost-match
race against `cross_repo_re` before that regex's own slide has a chance to claim the
tail. `scan_tokens` runs it first and masks its spans, so neither `cross_repo_re` nor
`entry_re` can re-discover the embedded fragments. `resolve::resolve` always returns
`Outcome::MalformedQualifier` for this kind, unconditionally — it never inspects
`corpus`/`index`, because the citation is malformed at the SHAPE level and no lookup
could make it valid. `link_scan`'s `call()` reports it as a new peer finding class
(`counts.malformed_qualifier`, a capped `malformed_qualifier` array, a `_by_source`
breakdown, and its own `truncated` flag) — the exact same shape `ambiguous` /
`dangling` / `cross_repo` already use.

Option 1 (extend the grammar to make `<repo>:<file-stem>:<ID>` actually resolve) was
declined: nothing in the repo depends on the three-part form resolving, since the one
bug that invented it already worked around it by dropping to the two-part form.

The archived sibling bug
(`docs/issues/archive/2026-08-26-session-log-template-cites-own-ledger-ids-bare.md`)
was re-checked at fix time and had already self-corrected its own wrongly-prescribed
3-part example, citing this very bug — no further doc correction was needed.

**Known live occurrences, deliberately left untouched:**
`docs/trackers/prompt-surface-measurement-session-log.md` (an active concurrent
session's WIP tracker at fix time) and
`docs/superpowers/plans/2026-08-25-unanchored-blast-radius-eval.md` each still use the
three-part form. Neither's *resolution* changes — they were already silently
collapsing to the two-part form before this fix — but both will now surface in
`link_scan`'s `malformed_qualifier` report the next time it runs, which is the
intended remediation path: visibility, not an unrequested edit to another session's
files.

**SHA:** `9a517e54` (`experiments`)
**patch-id:** `52abb0f1294e41587c276cafe33d61055d757b44`
## Tests added

Three, all in `src/librarian/tools/link_scan/`:

- `extract::tests::a_double_qualified_citation_is_flagged_not_silently_collapsed_to_the_inner_form`
  — the whole three-part span is captured as `MalformedQualifier`; it does NOT also
  extract as a working `CrossRepoToken`, and the embedded entry token stays masked.
- `extract::tests::citation_kind_wire_values_match_what_debug_emitted` — extended to
  cover the new variant (pre-existing test, one more case added).
- `tests::a_double_qualified_citation_is_reported_not_resolved_even_when_the_inner_form_would_resolve`
  — end-to-end through `call()`. The sharpest case: seeds a real target file so the
  inner `target:F-2` form WOULD legitimately resolve to an edge on its own, then
  asserts the double-qualified citation still reports only
  (`counts.malformed_qualifier == 1`, `counts.cross_repo == 0`, `counts.dangling == 0`)
  and `edges_missing` stays 0 even in `write=true` mode.

All 73 `link_scan` tests green, plus the full suite (4403 passed, 0 failed, 8 ignored),
`cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings`.
## Workarounds

Use single-qualifier citations (`<file-stem>:<ID>` for a per-work-stream namespace,
`<repo>:<ID>` for a single-ledger namespace like `R-N`) and accept the residual,
lower-probability risk of an exact file-stem collision across repos. This is what
`docs/templates/session-log.md` does today.

## Resume

Closed — no further action. Both options in § Fix were resolved: option 2 shipped, as
a first-class report class rather than a bare warn; option 1 was deliberately
declined. The two live three-part citations named in § Fix are for their owning
sessions to fix once `link_scan` next reports them — not this bug's scope.
## References

- `docs/issues/archive/2026-08-26-session-log-template-cites-own-ledger-ids-bare.md` — the bug
  whose fix surfaced this; its `## Fix` table originally prescribed the 3-part form this
  file shows does not work, and was corrected during implementation
- `docs/issues/archive/2026-08-18-link-scan-dangling-count-is-prefix-gated-so-a-whole-namespace-reads-as-healthy.md`
  — the sibling SLIDE-class defect (length-bound truncation) whose fix
  (`long_file_stem_qualifier_is_captured_whole_not_truncated_to_a_suffix`,
  `extract.rs:704`) documents the same regex-engine slide behavior this bug relies on
- `src/librarian/tools/link_scan/resolve.rs:233-270` — `CrossRepoToken` resolution
- `src/librarian/tools/link_scan/extract.rs:692-727` — qualifier capture + its existing
  slide regression test
- `get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified* — the
  guide's own examples, both single-qualifier, never wrong
