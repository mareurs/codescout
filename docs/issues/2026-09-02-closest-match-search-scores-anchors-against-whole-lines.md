---
id: '81f3a79b97b4b14e'
kind: bug
status: open
title: edit_markdown's closest-match diagnostic scores anchors against whole lines, so it goes silent exactly when the anchor is short
tags:
- cluster/truncated-window-ordered-by-the-wrong-key
- edit_markdown
- diagnostics
closed: ''
opened: 2026-09-02
owner: marius
related: []
severity: medium
---


# BUG: edit_markdown's closest-match diagnostic scores anchors against whole lines, so it goes silent exactly when the anchor is short

## Summary

`edit_markdown(action="edit")`'s miss diagnostic locates the "closest text" by normalized Levenshtein over **whole-line windows**. When `old_string` is a verbatim substring of a longer line — the ordinary shape of a copy-a-short-unique-anchor edit — the score is exactly `len(old_string) / len(line)`, so the diagnostic falls below its 0.5 threshold and reports *"nothing scored above 0.5 similarity"* about text that is present verbatim. The help disappears precisely as the surrounding line grows, which is when it is most needed.

## Symptom (Effect)

Two failures of the **same** caller mistake — an `old_string` ending in `\n` where the line continues — produce opposite diagnostics. Both sought strings are verbatim prefixes of a line in the section.

Short line (91 chars), useful:

```
old_string not found in section '## Section A'. Closest text (did it change since you read it?):
  want: Task 1: fix round 1 dispatched, interrupted by a rate limit.

  have: Task 1: fix round 1 dispatched, interrupted by a rate limit. Verified: nothing to clean up.
```
`scoped_miss_tier: "visible_drift"`

Long line (451 chars), useless — and actively misleading:

```
old_string not found in section '## Section B'. The text must match exactly (whitespace-sensitive). I looked, and nothing scored above 0.5 similarity.
```
`scoped_miss_tier: "no_similar_match"`, hint: *"old_string isn't in this section — verify the heading, or re-read the current section text and retry."*

The second message asserts a search was performed and found nothing similar. The sought string was sitting at the start of a line in that very section.

## Reproduction

`git rev-parse HEAD` at time of filing: `cb6aed69`. Both files under a scratch dir; no repo file is touched.

**Fires correctly.** Write a file whose `## Section A` contains one 91-char line beginning `Task 1: fix round 1 dispatched, interrupted by a rate limit.` and continuing ` Verified: nothing to clean up.` Then:

```
edit_markdown(path=…, heading="## Section A", action="edit",
              old_string="Task 1: fix round 1 dispatched, interrupted by a rate limit.\n",
              new_string="REPLACED\n")
```

→ `visible_drift`, with a `want`/`have` diff that shows the caller exactly what they got wrong.

**Goes silent.** Same shape, but the line is 451 chars — a long markdown bullet — and the anchor is its 82-char leading sentence:

```
edit_markdown(path=…, heading="## Section B", action="edit",
              old_string="- **On `cargo fmt` and this shared checkout — the obvious scoping does not work.**\n",
              new_string="REPLACED\n")
```

→ `no_similar_match`.

Ratios, measured in **characters** (`normalized_levenshtein` operates on chars, and the second anchor contains an em-dash, so a byte count would misreport it by 2):

| case | anchor | line | ratio | predicted | observed |
|---|---|---|---|---|---|
| Section A | 60 | 91 | 0.659 | fires | `visible_drift` |
| Section B | 82 | 451 | 0.182 | silent | `no_similar_match` |

Prefix-ness asserted rather than eyeballed: `line.startswith(anchor)` is true in both.

## Environment

codescout `0.15.0`, branch `experiments`, Linux. Reached through the MCP `edit_markdown` tool; `artifact(action="update", patch={body_edits: […]})` routes to the same diagnostic (`src/librarian/tools/update.rs:531` reads `scoped_miss_tier`), so the librarian-managed path inherits it.

## Root cause

`diagnose_scoped_miss` (`src/tools/markdown/edit_markdown.rs:1100-1207`) builds candidate windows of exactly `n` whole lines, where `n` is the line count of `old_string`, and scores each window against `old_string` in full:

```rust
for start in 0..=(lines.len() - n) {
    let window = lines[start..start + n].join("\n");
    let s = similarity(&window, old_string);
    …
}
if best_score < SIM_THRESHOLD { return no_close("I looked, and nothing scored above 0.5 similarity.", "no_similar_match"); }
```

`similarity` is `strsim::normalized_levenshtein` (`edit_markdown.rs:972-974`), i.e. `1 - lev(a,b) / max(|a|,|b|)`. For an `old_string` `p` that is a substring of line `L`, the edit distance is pure insertion — `lev = |L| - |p|` — and `max = |L|`, so:

> **similarity = |p| / |L|**

With `SIM_THRESHOLD = 0.5` (`edit_markdown.rs:1074`), the diagnostic therefore **suppresses itself for every anchor shorter than half its containing line**, however exactly that anchor matches. The scoring key (whole-line similarity) is unrelated to the caller's actual predicate (is `old_string` a substring, and where does it end?).

Measured 2026-09-02 by the two reproductions above; formula confirmed against both observations. The mechanism was read at `edit_markdown.rs:1100-1207` **and** exercised at runtime — this is not inferred from source alone.

**The inversion is the point.** A caller is advised to anchor on a short unique string. The longer and more prose-heavy the line, the more valuable a `want`/`have` diff would be — and the more certainly it is withheld.

## Evidence

### Threshold and constants

```
src/tools/markdown/edit_markdown.rs:1074   const SIM_THRESHOLD: f64 = 0.5;
src/tools/markdown/edit_markdown.rs:1075   const SECTION_LINE_CAP: usize = 400;
src/tools/markdown/edit_markdown.rs:1076   const SECTION_BYTE_CAP: usize = 65_536;
src/tools/markdown/edit_markdown.rs:1077   const OLD_STRING_CAP: usize = 8192;
```

Note that the other bail-outs each get their **own** tier and a message naming the specific cause (`old_string_empty`, `old_string_too_large`, `section_too_many_lines`, `section_too_many_bytes`, `old_string_longer_than_section`). The function's own doc comment argues for exactly that: *"a caller error …, a declined search …, and a genuine no-match are different problems with different fixes, and reporting them identically forces a blind retry regardless of which one actually happened."* The sub-threshold case is a **declined search** reported as a **genuine no-match** — the distinction the function is built around, missing at the one branch that most needs it.

### Live occurrences, this session

Both were real edits, not probes, and both cost a re-read of a section that had not changed:

- `.superpowers/sdd/2026-09-02-layer-2a-1-coordinator/progress.md`, `## Progress` — anchor was a sentence prefix of a ~400-char line.
- `docs/superpowers/plans/2026-09-02-layer-2a-3-wiring-and-one-budget.md`, `## Global Constraints` — anchor was the bolded lead of a ~700-char bullet.

In both cases the hint's advice (*"verify the heading, or re-read the current section text"*) sent me to re-read a section whose text was exactly what I already had.

## Hypotheses tried

1. **Hypothesis:** the near-miss reporter is simply broken / not wired.
   **Test:** reproduced the same caller mistake against a short line.
   **Verdict:** rejected — it produced a correct, useful `visible_drift` diff.
   **Evidence:** § Symptom, first block.

2. **Hypothesis:** the discriminator is the length of the *containing line*, not anything about the input.
   **Test:** same mistake, same shape, only the line length changed (91 → 451 chars).
   **Verdict:** confirmed — `visible_drift` → `no_similar_match`.
   **Evidence:** § Reproduction table.

3. **Hypothesis:** the ratio is exactly `|anchor| / |line|`, derivable from `normalized_levenshtein` over a pure-insertion distance.
   **Test:** derived, then checked both observations against the 0.5 threshold.
   **Verdict:** confirmed — 0.659 fires, 0.182 does not, matching prediction.
   **Evidence:** § Root cause.

## Fix

Not yet fixed. Plan, in preference order:

1. **Check containment before falling back to similarity.** Before the window scan in `diagnose_scoped_miss`, test whether `old_string.trim_end_matches('\n')` is a substring of any line. If so, the answer is not "nothing similar" but "found it, and here is the boundary you got wrong" — emit a `want`/`have` on that line with a tier of its own (e.g. `substring_present_boundary_differs`). This is the caller's actual predicate and it is O(section).
2. **If (1) is declined, make the threshold length-relative** rather than absolute, or score against the best-matching *window of the anchor's own length* instead of whole lines.
3. **At minimum, stop asserting absence.** The current message reads as a claim about the section. The honest form names the method: *"no whole-line window scored above 0.5 similarity — note this search cannot see a short anchor inside a long line."*

Remedy (1) is the one this bug's cluster argues for: the fix is the **selection**, not the marker.

## Tests added

None — not fixed. A regression test wants both reproductions as a pair: the short-line case asserting `visible_drift` and the long-line case asserting the new tier. Asserting only the long-line case would pass against a build that had simply lowered the threshold, which is remedy (2) and not (1).

## Workarounds

- Never end `old_string` at a point where the source line continues. Anchor on a whole line, or on a substring that does not stop mid-line.
- When `no_similar_match` comes back, re-read the section with `read_markdown(start_line=…, end_line=…)` and copy the whole line, rather than trusting the hint's implication that the text is absent.
- The message is reliable in one direction only: `visible_drift` means it found something close; `no_similar_match` means **either** the text is absent **or** your anchor is under half its line.

## Resume

Read `diagnose_scoped_miss` (`src/tools/markdown/edit_markdown.rs:1100-1207`) and insert a containment check ahead of the window scan at `:1152`. Add the tier to the enum of `scoped_miss_tier` values the tests in `src/tools/markdown/tests.rs:3239-3365` assert on, and check whether `src/librarian/tools/update.rs:531` — which routes on `visible_drift` specifically — should also route on the new tier.

## References

- `src/tools/markdown/edit_markdown.rs:972-974` (`similarity`), `:1074-1077` (constants), `:1100-1207` (`diagnose_scoped_miss`)
- `src/tools/markdown/tests.rs:3239-3365` — the existing tier tests, including `diagnose_giant_old_string_bails_to_no_close_cheaply`, which pins one of the *other* bail-outs
- `src/librarian/tools/update.rs:531` — the one consumer that routes on a tier value
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the standing rule this violates: name the scope you examined when the zero is suspicious
- `docs/trackers/issue-clusters.md` `IC-19` — the class. `IC-13` (`capped-result-presented-as-complete`) was considered and rejected: its widened clause deliberately excludes a marker the caller *can* see, and this message does name its own 0.5 cap.

