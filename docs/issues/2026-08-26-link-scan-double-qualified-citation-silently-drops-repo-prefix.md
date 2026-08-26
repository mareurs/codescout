---
id: '4975d27ed2aa9550'
kind: bug
status: open
title: 'BUG: a `repo:file-stem:ID` citation silently degrades to `file-stem:ID` — the repo qualifier is dropped by regex slide, not parsed'
tags:
- librarian
- link-scan
- citations
- extraction
opened: 2026-08-26
owner: marius
related:
- '0b40a6e83053c1d2'
severity: low
unverified: Only the extraction-regex slide hypothesis was confirmed by test; the exact regex source (extract.rs scan_tokens) was not read line-by-line to cite its pattern text — the mechanism was inferred from black-box test behavior plus resolve.rs's split_once(':') semantics, not from reading the regex literal itself.
---

## Summary

`get_guide("tracker-conventions")`'s own worked example distinguishes a repo-only
qualifier (`codescout:A-11`) from a file-stem qualifier (`bug-fix-session-log:F-33`), and a
bug file (`docs/issues/2026-08-26-session-log-template-cites-own-ledger-ids-bare.md`)
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
  `docs/issues/2026-08-26-session-log-template-cites-own-ledger-ids-bare.md`

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

*Not yet implemented — filed on notice per CLAUDE.md; this is resolver/extraction-layer
work, not a doc fix, and is left open rather than rushed. Two honest options, and they are
not mutually exclusive:*

1. **Extend the grammar to genuinely support `<repo>:<file-stem>:<ID>`.** Capture up to
   two qualifier segments and check the outer one against known repo names (the
   `by_stem`-adjacent structure would need a parallel `by_repo_name` or equivalent) before
   falling through to the stem-only interpretation. This is the fix that makes the sibling
   bug's originally-prescribed double-qualification form actually work as documented.
2. **Retract the three-part form from anywhere it's prescribed, and document that
   file-stem qualification alone is the ceiling** — safe against a bare token's ambiguity
   and against a same-numbered entry under an unrelated stem, but *not* safe against an
   exact stem collision across repos (a rarer, harder-to-hit case, since it requires two
   different repos to name a tracker file identically). The sibling bug's fix already took
   this path pragmatically (dropped the repo prefix, kept file-stem-only), so option 2 is
   already partially in effect in practice even though this file documents that no code
   change enforces or explains it.

Either way, `librarian(action="doctor")` or `link_scan` itself could usefully warn on a
citation containing 2+ colons that isn't a rel-path or URL — right now it is silently
reinterpreted rather than flagged, which is the sharper edge here: a well-intentioned
author following the sibling bug's (wrong) advice gets no signal that their fix did
nothing.

## Tests added

None in this file. The mechanism is exercised indirectly by
`tests/link_scan.rs::session_log_template_citations_never_bind_to_a_foreign_repos_namesakes`
(added for the sibling bug), which uses only the corrected single-qualifier form and does
not itself assert on the 3-part form's behavior. A direct regression for *this* bug — two
tests, one per candidate fix in § Fix — should land with whichever option is chosen.

## Workarounds

Use single-qualifier citations (`<file-stem>:<ID>` for a per-work-stream namespace,
`<repo>:<ID>` for a single-ledger namespace like `R-N`) and accept the residual,
lower-probability risk of an exact file-stem collision across repos. This is what
`docs/templates/session-log.md` does today.

## Resume

Pick option 1 or 2 in § Fix. If 1: extend `extract.rs`'s qualifier-capturing regex (near
`scan_tokens`, `extract.rs:432` and the pattern discussed at `extract.rs:692-727`) to
accept a second colon-delimited segment, and give `resolve.rs`'s `CrossRepoToken` arm
(`resolve.rs:233-270`) a repo-name lookup path alongside `corpus.by_stem`. If 2: grep the
repo for any other place prescribing `<repo>:<file-stem>:<ID>` (only
`docs/issues/2026-08-26-session-log-template-cites-own-ledger-ids-bare.md` at time of
filing — already corrected) and close this as `wontfix` with the rationale above, keeping
the doctor/warn suggestion as a separate, smaller follow-up.

## References

- `docs/issues/2026-08-26-session-log-template-cites-own-ledger-ids-bare.md` — the bug
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

