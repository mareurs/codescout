---
id: '52542a0ec81771a3'
kind: bug
status: open
title: 'BUG: one stray ``` disables every line-anchored field for the REST of the file, and doctor reports the result as "none declared"'
tags:
- cluster/addressing-without-an-escape-hatch
- librarian
- doctor
- markdown
- silent-failure
closed: null
opened: 2026-09-01
owner: marius
related:
- docs/issues/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md
severity: medium
---

## Summary

`structured_fix_pointers` skips fenced lines, deliberately and correctly — a worked example
is a quotation, not a declaration. But fence state is a running toggle over the whole file,
so **one unmatched ``` flips every subsequent line to "fenced" forever**. Every
line-anchored field after it becomes invisible: the `- **SHA:**` pair, and by the same
convention `**Valid:**` / `**Rests on:**`. Nothing reports the imbalance. `doctor` then
states, confidently, that no pointer is declared — for a file that visibly contains a
`## Fix provenance` section with the SHA in it.

## Symptom (Effect)

`docs/issues/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md` carried a
`## Fix provenance` section naming `73066479` and its patch-id. `doctor` reported:

```
status is `fixed` but no `## Fix provenance` pointer is declared, so nothing records
which commit closed this.
```

Both statements are true of the parse and false of the file. A reader who opens the file to
act on the finding sees the section the finding says is absent.

## Reproduction

Measured 2026-09-01 at `8982f775`. Count fences the way the parser does — `trim_start()`
then `starts_with("```")`, so indented fences count:

```
awk '{t=$0; sub(/^[ \t]+/,"",t); if (t ~ /^```/ || t ~ /^~~~/) n++} END{print n}' <file>
```

Odd result ⇒ every line-anchored field below the last marker is silently unreadable. The
file above returned **11**. The cause was a doubled fence at lines 53–54 — a block closed,
then immediately reopened by a stray marker and never closed again.

## Environment

codescout `experiments`, `src/librarian/tools/doctor.rs`. Applies to any consumer of the
fenced-line convention, not just this check.

## Root cause

`structured_fix_pointers` (`src/librarian/tools/doctor.rs:4535-4579`) carries `let mut
fenced = false` and toggles on every ``` or ~~~ line, skipping while true. Read at the
bytes 2026-09-01. The toggle is correct per-block and unbounded per-file: there is no
parity check, no "fence still open at EOF" diagnostic, and no way for a file to escape a
literal ``` at line start. The same convention governs `**Valid:**` and `**Rests on:**`
detection, so the blast radius is every line-anchored field, not just fix anchors.

**The failure direction is what makes it hard to see.** An unbalanced fence produces
*silence*, and silence is exactly what a file with nothing to declare produces. The two are
indistinguishable downstream — the check cannot tell "declared nothing" from "declared, but
unreadable from line 54 onward".

## Evidence

### Two independent defects in one file, and the fence hid the other

The file ALSO declared its pair as `- SHA:` / `- patch-id:` rather than the bolded
`- **SHA:**` / `- **patch-id:**` the parser matches. Either defect alone silences the
anchor, so fixing one would have left the check still firing and the second cause
unsuspected. Both were repaired in the same commit as this filing.

### The finding text is accurate and still misleads

`terminal_status_without_fix_anchor` says "no `## Fix provenance` pointer is **declared**".
That is a precise claim about the parse. Every reader will read it as a claim about the
file.

## Hypotheses tried

1. **Hypothesis:** the block landed inside a fence. **Test:** counted fence markers at
   column 0 only — got an even number, concluded balanced. **Verdict:** rejected, and the
   rejection was WRONG. The probe did not match parser semantics: the parser trims leading
   whitespace first, so indented fences count and mine did not. Re-counting with
   `sub(/^[ \t]+/,"",t)` gave 11, odd. **A probe that does not replicate the parser's own
   normalisation returns a confident wrong answer** — recorded because it cost a full
   detour.
2. **Hypothesis:** the write never landed. **Test:** `grep -n '^- \*\*SHA:\*\*'` on disk.
   **Verdict:** rejected — the line was present and correctly formed.

## Fix

Not fixed. The stray fence in the one affected file is repaired, which is a data fix and
not a fix for this class. Options, cheapest first:

- **a. A `doctor` check for odd fence parity in any catalogued markdown file.** Cheap,
  read-only, and it converts a silent misparse into a named finding. Should report the last
  marker's line, since that is where the invisible region begins.
- **b. Have `structured_fix_pointers` report an unterminated fence** rather than returning
  a clean empty vector — turning a plausible negative into an error, per
  `docs/adrs/2026-08-27-negative-results-name-their-scope.md`.
- **c. Widen the finding text** on `terminal_status_without_fix_anchor` to say "none
  parsed" rather than "none declared", and name fence state as a cause. Weakest: it
  documents the trap instead of removing it.

(a) and (b) are complements — (a) covers every consumer of the convention, (b) covers this
one caller precisely.

## Tests added

None — nothing is fixed yet. A fix under (a) wants a fixture with an odd fence count and a
valid `- **SHA:**` line below it, asserting the check fires; plus a balanced-fence control
asserting it stays silent, since a check that fires on everything is not a check.

## Workarounds

Run the parity probe under § *Reproduction* before trusting any "none declared" finding on
a file whose body you can see declares something.

## Resume

Decide between (a) and (b) — (a) is the wider net. If (a): add `scan_unbalanced_fence` to
`src/librarian/tools/doctor.rs` alongside the other read-only scans, walking catalogued
markdown and reporting `(path, last_marker_line)` on odd parity.

## References

- `src/librarian/tools/doctor.rs:4535-4579` — `structured_fix_pointers`, the fence toggle
- `docs/issues/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md` — the
  file this was found on
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — a zero must name its scope

