---
id: '61b048e6957259ed'
kind: bug
status: open
title: 'BUG: no citation form resolves &quot;entry X in file Y of another repo&quot; — the three-part form is written by hand but the resolver has never supported it, and now reports identically to a redundant same-repo prefix'
owners:
- marius
tags:
- librarian
- link-scan
- citations
- cross-repo
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

*Not yet implemented — filed on notice. Two honest options:*

1. **Build real repo-name-lookup support.** Give `resolve.rs` a `by_repo_name`-style
   structure (workspace member / umbrella repo names) so `<repo>:<file-stem>:<TOKEN>`
   can genuinely resolve when `<repo>` names a real sibling and the citing session's
   workspace can see it — or report a clean, distinct "cross-repo, file named" finding
   when it can't (e.g. the repo isn't in this workspace's umbrella). This is Option 1
   from the sibling bug, declined there specifically because nothing depended on it
   resolving — but the 7 References-section citations found here are a standing,
   pre-existing demand for exactly this.
2. **Accept there is no resolvable form, and say so.** Document in
   `get_guide("tracker-conventions")` that a cross-repo, file-qualified pointer in a
   `## References` section is PROSE ONLY — never expected to become a `link_scan` edge
   — and, if the `malformed_qualifier` bucket's noise is worth reducing, give it a
   third citation kind/report bucket (e.g. `cross_repo_file_qualified`) so a reader can
   tell "known-unsupported, intentional" apart from "redundant, should be stripped"
   without reading each entry's qualifier by hand. This does not make the citation
   resolve; it only makes the report legible.

No fix is prescribed here — this is a design decision (how much resolver machinery is
worth building for a documentation-only convenience), not a mechanical one.

## Tests added

None yet — no fix has landed.

## Workarounds

None needed for correctness: a citation of this shape was never an edge before or
after `9a517e54`, and still isn't — it is report-only in both the old (silently
degraded) and new (correctly flagged) behavior. The only cost today is report
legibility: a triager reading `malformed_qualifier` must check each entry's outer
segment against known sibling-repo names by hand to sort "fine as documentation" from
"should be stripped."

## Resume

Pick option 1 or 2 in § Fix — this is the concrete next action, since both are fully
scoped above. If 1: start from `resolve.rs`'s `CrossRepoToken` arm and the workspace's
umbrella/member repo list (see `get_guide("librarian")` § Worktree overlay and
`.codescout/workspace.toml` `[[project]]` / global umbrella registry for where sibling
repo names already live). If 2: add the doc note to `get_guide("tracker-conventions")`
§ *Citing an entry — bare, or qualified*, and decide whether a new report bucket is
worth the `CitationKind`/`Outcome`/`mod.rs` wiring cost demonstrated in `9a517e54` — it
is the same shape, so scoping it is quick even if implementing it isn't free.

## References

- `docs/issues/archive/2026-08-26-link-scan-double-qualified-citation-silently-drops-repo-prefix.md`
  — the fix (`9a517e54`) whose live verification surfaced this
- `docs/issues/archive/2026-08-26-session-log-template-cites-own-ledger-ids-bare.md`
  — the archived bug whose `## References` section holds 5 of the 9 live hits
- `docs/issues/archive/2026-08-26-cited-prefix-with-no-definer-is-invisible.md`
  — the archived bug whose `## References` section holds the other 3
- `get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified* — the
  guide that documents the two forms that DO work; silent on this third need

