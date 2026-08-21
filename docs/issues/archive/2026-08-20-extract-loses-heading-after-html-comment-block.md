---
id: 2104fc471db2f769
kind: bug
status: fixed
title: link_scan extract() loses a heading swallowed by a preceding HTML comment block
tags:
- link_scan
- extract
- pulldown-cmark
- citation-graph
- html-block
closed: 2026-08-21
---

# BUG: `link_scan::extract()` loses a heading swallowed by a preceding HTML comment block

## Summary

`crate::librarian::tools::link_scan::extract` does not report `CAP-4` as a definition in
`docs/trackers/capability-proposals.md`. A `<!-- … -->` HTML comment block immediately
above it causes pulldown_cmark to treat the following `## CAP-4 — …` heading as part of
the HTML block, so no `Event::Start(Heading)` is emitted and `def_re` never sees the line.
An entry that `extract()` cannot define is an entry nothing can cite: every citation of it
counts as **dangling**, and `doctor`'s `entry_without_definition` reports it as missing
from a body that plainly contains it.

## Symptom (Effect)

Measured 2026-08-20 across `docs/**/*.md` (1066 files), comparing `extract()`'s definition
set against `entry_sections()`'s:

```
CORPUS files=1066 sections=1101 disagreeing_files=1 end_line_overshoot_sections=0
  docs/trackers/capability-proposals.md: extract_only=[] (0) sections_only=["CAP-4"] (1)
```

`entry_sections` finds `CAP-4`; `extract()` does not. It is the **only** remaining corpus
disagreement between the two.

## Reproduction

Bisect the file by start line and re-run `extract()`:

- input beginning at **line 1160** → `extract()` finds `CAP-4`
- input beginning at **line 1159** → it does not

Line 1159 of `docs/trackers/capability-proposals.md` opens an HTML comment block:

```
<!-- Insert new CAP-N entries above the "## Anti-goals" heading. …
```

The `## CAP-4 — …` heading is at line 1161.

## Environment

codescout `experiments` at `eb822f86`, Linux. `extract()` is
`src/librarian/tools/link_scan/extract.rs`; the markdown parser is `pulldown_cmark`.

## Root cause

pulldown_cmark follows CommonMark's HTML-block rules: an HTML block opened by `<!--`
continues until its end condition is met, and intervening lines — including ATX headings —
are emitted as `Event::Html`, not as heading events. `extract()` only treats a `def_re`
match as a definition when it arrives inside a heading event, so the swallowed heading is
invisible to it.

**Confirmed 2026-08-21** by dumping the actual pulldown_cmark event stream for the
reproduction shape (`<!-- comment opens here\n\n## CAP-4 — a heading\n\nbody\n-->`):

```
Start(HtmlBlock) @ 0..58 = "<!-- comment opens here\n\n## CAP-4 — a heading\n\nbody\n-->\n"
Html(Borrowed("## CAP-4 — a heading\n")) @ 25..48 = "## CAP-4 — a heading\n"
End(HtmlBlock) @ 0..58 = ...
```

The whole block — heading included — is one `Html` event; no `Event::Start(Tag::Heading)`
ever fires for line 3. The hypothesis in this section was correct; it is no longer a
hypothesis wearing a conclusion's clothes.

Measured 2026-08-20 by the corpus comparison above (Task 1 review + re-review of
`docs/superpowers/plans/2026-08-20-statement-validity-layers-1-2.md`).

## Evidence

The original review's corpus scan, before the Task 1 fence fix, found two disagreeing
files; the re-review after the fix found one:

| Metric | Before Task 1's fix | After |
|---|---|---|
| sections found | 1073 | 1101 (+28) |
| disagreeing files | 2 | **1** (`CAP-4` only) |
| `end_line` overshoot | 20 | 0 |

The other disagreement — 28 definitions dropped in `docs/trackers/bug-fix-session-log.md`
by a hand-rolled fence toggle — was a defect in the new code and is fixed
(`eb822f86`). This one predates it and lives in `extract()`.

## Hypotheses tried

1. **Hypothesis:** the heading is malformed and legitimately defines nothing.
   **Test:** `entry_sections()`, which takes headings from
   `librarian::preview::headings::parse` (a line-oriented, `FenceState`-backed ATX
   parser), over the same file.
   **Verdict:** rejected — `entry_sections` reports `CAP-4` normally, so the heading is
   well-formed and the difference is in how each side finds headings.

## Fix

Applied — the Structural direction, made tractable by something the Narrow-vs-Structural
framing below hadn't yet connected: `entry_sections()`'s own doc comment already states
the intended end state ("so the three can never disagree") and already delegates heading
detection to `crate::librarian::preview::headings::parse` — the same shared, line-oriented,
fence-aware, **HTML-oblivious** ATX parser `entry_sections` uses. `extract()`'s definition
loop now backfills from that same parser: after the main pulldown_cmark pass, any heading
`headings::parse` finds whose line wasn't already captured as a definition gets added.
Additive, not a rewrite — the existing event-driven pass still owns the common case
(definition detection AND same-heading citation-scanning); the backfill only fires for the
rare swallowed case.

**Deliberately partial.** A citation embedded in a swallowed heading's *remainder* text
(e.g. "## CAP-4 — title (see CAP-2)") is not scanned — recovering it would mean re-parsing
`Event::Html` payloads for inline citations too, and the common real-world shape (per this
bug's own reproduction) is a whole entry disappearing into the comment, not just its
heading with a citation-bearing remainder. Left as a known, smaller gap.

**A second, pre-existing risk from the Narrow option turned out not to apply.**
Using `headings::parse` (like `entry_sections` already does) means a heading-shaped line
genuinely INSIDE an intentional HTML comment (someone deliberately hiding a draft entry)
would also now get defined — the same behavior `entry_sections` already has today, so this
fix makes `extract()` consistent with existing, already-shipped behavior rather than
introducing a new risk class.

**Original two directions, for the record:**

- Narrow: scan `Event::Html` payloads for `def_re` matches directly. Not taken — would
  have re-implemented heading detection inside HTML text from scratch, where
  `headings::parse` already exists and is already the trusted comparand.
- Structural: what shipped.

- **SHA (experiments):** `e24b6ad8a2c521e20d5d383e1606ff4b199fa127`
- **patch-id:** `f13a105e4e72382b455cd657c9713009fa345b98`
## Tests added

- `heading_swallowed_by_a_preceding_html_comment_block_still_defines` — the bug as
  reproduced (unclosed `<!--`, heading, `-->`); asserts the definition is now found.
- `entry_sections_and_extract_agree_on_the_live_corpus` (pre-existing, from the
  statement-validity-layers-1-2 work) — its `CAP-4` known-exception carve-out was
  removed per the test's own instructions once it stopped reproducing; it now asserts
  full agreement across the whole `docs/**/*.md` corpus with zero exceptions.

Both RED-first (new test failed with `left: []`; the corpus test failed exactly as its own
failure message predicted — "the known CAP-4 exception ... did not reproduce this run").
`cargo test --lib link_scan` — 71 passed. Full `cargo test` + `cargo clippy --all-targets
-- -D warnings` clean on `experiments`.
## Tests added

None. The corpus comparison that found this is not committed — see *Resume*.

## Workarounds

Move the `<!-- … -->` block so it does not immediately precede an entry heading, or close
it with a blank line before the heading. That repairs one file and not the class.

## Resume

Two independent next actions:

1. **Confirm the mechanism rather than infer it.** Dump the pulldown_cmark event stream
   for `docs/trackers/capability-proposals.md` around lines 1155-1165 and observe whether
   `Event::Html` covers line 1161. The root cause above is currently a hypothesis wearing
   a conclusion's clothes.
2. **Commit the agreement check.** The comparison of `extract()` against
   `entry_sections()` over `docs/**/*.md` exists only in a review transcript in a
   gitignored workspace. As a test it is a strong regression guard — and with this bug
   open, `CAP-4` is the *entire* expected signal, so the assertion is
   `disagreeing_files == 1 && sections_only == ["CAP-4"]` until this is fixed.

## References

- `src/librarian/tools/link_scan/extract.rs` — `extract`, `def_re`
- `src/librarian/preview/headings.rs` — `parse`, the line-oriented alternative
- `src/util/markdown_fence.rs` — `FenceState`, from
  `docs/issues/archive/2026-08-11-artifact-nested-fence-closes-outer-fence.md`
- `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md` — Layer 3
  depends on `extract()`'s definition set
