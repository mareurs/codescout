---
id: fc92227102f58d9e
kind: bug
status: fixed
title: 'BUG: no citation form resolves &quot;entry X in file Y of another repo&quot; — the three-part form is written by hand but the resolver has never supported it, and now reports identically to a redundant same-repo prefix'
owners:
- marius
tags:
- librarian
- link-scan
- citations
- cross-repo
- cluster/addressing-without-an-escape-hatch
closed: 2026-08-27
opened: 2026-08-27
owner: marius
related:
- e7eebd21a5c0cd99
- '08072e4a358640f0'
- c2a65e6e1814524b
severity: low
---

## Summary

`link_scan`'s citation grammar supports exactly two qualified forms: `<repo>:<TOKEN>`
(repo-only, report-only, cannot become an edge) and `<file-stem>:<TOKEN>` (same-repo,
file-disambiguated, can become an edge). There is no form for "this exact entry, in
this exact file, in that other repo" — but that need is real and has been written by
hand, unprompted, in at least two archived bug files' `## References` sections:
`claude-plugins:roster-audit-session-log:F-5`, `claude-plugins:repo-hygiene-session-log:F-2`,
`claude-plugins:reconnaissance-patterns:R-4`, and siblings. Before
`docs/issues/archive/2026-08-26-link-scan-double-qualified-citation-silently-drops-repo-prefix.md`'s
fix (`9a517e54`), these silently degraded via the same regex-slide mechanism that bug
fixed. After that fix, they are now correctly *visible* — but land in the same
`malformed_qualifier` bucket as a genuinely redundant same-repo prefix
(`codescout:statement-validity-session-log:F-2`, which should just be stripped), with
no way for a reader of the report to tell the two apart.

## Symptom (Effect)

`librarian(action="link_scan", write=false)`'s `malformed_qualifier` array reports
both shapes identically:

```json
{"src_id": "08072e4a358640f0", "raw": "claude-plugins:roster-audit-session-log:F-5", "kind": "MalformedQualifier", "line": 196}
{"src_id": "08072e4a358640f0", "raw": "codescout:statement-validity-session-log:F-2", "kind": "MalformedQualifier", "line": 141}
```

The first is a genuine, currently-unrepresentable need (file-level precision into a
*different* repo). The second is pure redundancy (the file already lives in the
citing repo). Nothing in the report distinguishes them — a reader triaging
`malformed_qualifier` has to manually check whether the outer segment names a real
sibling repo to know which remediation applies.

## Reproduction

```
git rev-parse HEAD   # 64c89040 (experiments), post the double-qualified-citation fix
```

```
librarian(action="link_scan", write=false)
```

Read `$.malformed_qualifier[*]` — as of this filing it holds 9 entries: 2 with a
`codescout:` outer qualifier (redundant same-repo, both intentional quoted evidence
inside the two bug files documenting the original defect) and 7 with `claude-plugins:`
or `eduplanner-ui:` outer qualifiers (genuine cross-repo+file references, all inside
`## References` sections).

## Environment

codescout @ branch `experiments`, `src/librarian/tools/link_scan/{extract,resolve}.rs`.
Discovered live-verifying `9a517e54` (the `MalformedQualifier` fix) immediately after
reconnecting the MCP session to the rebuilt binary.

## Root cause

`resolve::resolve`'s `CrossRepoToken` arm only ever does one `split_once(':')` — it has
no notion of "repo, then file-stem, then token" as a genuine three-segment shape, only
"one qualifier, then token." The `MalformedQualifier` fix added by `9a517e54`
deliberately treats every 2+-colon citation uniformly (Option 2 in the sibling bug:
retract-and-warn, not extend-the-grammar), so it cannot distinguish *why* a citation has
two qualifier segments — redundant duplication of the current repo's own name, or a
genuine attempt to name a file inside a *different* repo, which the two-segment
`<file-stem>:<TOKEN>` form cannot express (that form always resolves against the
CITING repo's own corpus, per `resolve.rs`'s `corpus.by_stem` lookup — it has no
concept of a foreign corpus at all).

`inferred from src/librarian/tools/link_scan/resolve.rs's CrossRepoToken arm — not
re-read line-by-line at filing time beyond what the double-qualified-citation bug
already established; the "no foreign corpus" claim follows from Corpus only ever being
built from the current find() scope's rows, not measured by seeding a live cross-repo
fixture.`

## Evidence

### The live report, post-fix

See § Reproduction. Command run: `librarian(action="link_scan", write=false)` against
`64c89040`. Full output buffered; the two categories were separated by reading each
entry's `src_id` and `raw` qualifier segment.

### The need predates the fix

`docs/issues/archive/2026-08-26-session-log-template-cites-own-ledger-ids-bare.md` §
References already contains three `claude-plugins:<file-stem>:<ID>` citations, written
2026-08-26 — before this gap was ever surfaced — as the natural way to point at a
specific entry in a sibling repo's tracker. The need was not invented by this bug
filing; it was already in use.

## Hypotheses tried

None — the mechanism is a design gap (no supported grammar), not a runtime mystery, and
follows directly from reading `resolve.rs`'s `CrossRepoToken` arm alongside the fix
that added `MalformedQualifier`. Confirmed by effect via the live `link_scan` run in §
Evidence, not by a separate hypothesis-test cycle.

## Fix

**Fixed, Option 2: report legibility only, no new resolver capability.**

- **SHA:** `dd3f08aa` (`experiments`)
- **patch-id:** `10e9be57b96c8c05acf87ce7523dcac4c557d314`

`fix(librarian): split cross-repo file-qualified citations out of malformed_qualifier`

`resolve::Outcome::MalformedQualifier` is unchanged — every 2+-qualifier-segment
citation is still retracted, still never becomes an edge, regardless of what its
segments name. What changed is purely in `mod.rs`'s finding-construction pass: a
new per-row lookup (`citing_repo_name`, via the existing `containing_root` helper
against `ctx.workspace.roots`) compares each `MalformedQualifier` citation's outer
qualifier segment against the citing artifact's OWN registered workspace root
name.

- Outer segment == citing repo's own name → stays in `malformed_qualifier`
  (redundant, should be stripped — unchanged behavior).
- Outer segment != citing repo's own name → new `cross_repo_file_qualified`
  bucket (presumably names a different repo, prose-only, nothing to fix).
- Citing repo unknown (row outside every registered root) → falls back to
  `malformed_qualifier`, the conservative default — claiming "genuinely
  cross-repo" needs positive evidence the code doesn't have in that case.

**Deliberately NOT built:** any validation that the outer segment names a REAL,
known sibling repo (an umbrella member, say). That would be Option 1. The split
here only ever answers "is this NOT a self-reference" — it does not, and cannot,
confirm the citation is genuinely resolvable elsewhere. A typo'd repo name and a
genuine sibling reference land in the same new bucket; both are equally
unresolvable today, so the report doesn't need to (and doesn't try to)
distinguish them.

Also documented the third citation shape in
`get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified*:
prose-only, permanently, not a gap waiting to be filled — so a future reader
doesn't re-discover this as a fresh gap.

**Verified:** `cargo fmt`, `cargo clippy --all-targets -- -D warnings`,
`cargo test --lib` → 4416 passed, 0 failed, 8 ignored.
## Tests added

`a_cross_repo_file_qualified_citation_is_reported_separately_from_a_redundant_same_repo_one`
(`src/librarian/tools/link_scan/mod.rs`), shipped in `dd3f08aa`.

One source file, two citations: `citer:target:F-2` (the registered workspace
root IS named "citer" — genuinely redundant self-reference) and
`claude-plugins:target:F-2` (a different name — presumed cross-repo). Both cite
a target that DOES define `F-2`, so a resolver that ignored the qualifier split
entirely would silently produce an edge — same sharpest-version-of-the-bug shape
as the sibling double-qualified-citation test. Asserts:

- `counts.malformed_qualifier == 1`, holding only the `citer:...` citation;
- `counts.cross_repo_file_qualified == 1`, holding only the `claude-plugins:...`
  one, in its own top-level array and `_by_source` map;
- `counts.cross_repo == 0` — must not ALSO land in the plain two-part bucket;
- `counts.edges_missing == 0` — neither may resolve, even though the inner
  `target:F-2` form would on its own.

**Verified red before green:** ran before the `mod.rs` split existed — both
citations landed in `malformed_qualifier` (count 2, not the expected 1/1 split),
and the new `cross_repo_file_qualified` key read `Null` against an expected `1`.

**Fixed a stale fixture as part of this:** the pre-existing
`a_double_qualified_citation_is_reported_not_resolved_even_when_the_inner_form_would_resolve`
test cited `codescout:target:F-2` but registered its workspace root under the
name `"r"` — internally inconsistent once the split existed (the test's own
citation text assumes "codescout" is the repo). Renamed the registered root to
`"codescout"` so the fixture matches what its citation text always claimed;
assertions unchanged and still pass.
## Workarounds

None needed for correctness: a citation of this shape was never an edge before or
after `9a517e54`, and still isn't — it is report-only in both the old (silently
degraded) and new (correctly flagged) behavior. The only cost today is report
legibility: a triager reading `malformed_qualifier` must check each entry's outer
segment against known sibling-repo names by hand to sort "fine as documentation" from
"should be stripped."

## Resume

Fixed and archived. Nothing further planned under Option 2. If Option 1 (real
cross-repo resolution) is ever wanted, start from `resolve.rs`'s
`CrossRepoToken` arm and the workspace umbrella/member registry — see
`get_guide("librarian")` § Worktree overlay and `.codescout/workspace.toml`'s
global umbrella registry for where sibling repo names actually live (NOT
`ctx.workspace.roots`, which is this repo's own multi-project roots and is what
this fix used instead, since it only needed to detect self-reference).
## References

- `docs/issues/archive/2026-08-26-link-scan-double-qualified-citation-silently-drops-repo-prefix.md`
  — the fix (`9a517e54`) whose live verification surfaced this
- `docs/issues/archive/2026-08-26-session-log-template-cites-own-ledger-ids-bare.md`
  — the archived bug whose `## References` section holds 5 of the 9 live hits
- `docs/issues/archive/2026-08-26-cited-prefix-with-no-definer-is-invisible.md`
  — the archived bug whose `## References` section holds the other 3
- `get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified* — the
  guide that documents the two forms that DO work; silent on this third need
