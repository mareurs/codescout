---
id: '4c00caf1fcf49365'
kind: bug
status: open
title: 'BUG: the patch-id scan is per-line, so a declaration whose value wraps is invisible'
tags:
- cluster/selector-narrower-than-its-population
- doctor
- provenance
- line-scan
closed: null
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-09-02-tracked-only-staging-commits-half-an-archive-move.md
- docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md
severity: medium
---

# BUG: the patch-id scan is per-line, so a declaration whose value wraps is invisible

## Summary

`declared_patch_ids` (`src/librarian/tools/doctor.rs:5245-5278`) iterates
`content.lines()` and, for each line, finds the literal `patch-id` and then searches
**only the remainder of that same line** for the opening backtick:

```rust
while let Some(rel) = lower[from..].find("patch-id") {
    let after = from + rel + "patch-id".len();
    if let Some(open) = t[after..].find('`') {          // `t` is ONE line
```

A declaration whose 40-hex value wraps to the next line therefore yields no match at
all. The check that consumes it, `non_terminal_status_with_fix_anchor`, then reports a
correct-looking **zero** for a record that did declare its provenance — which is the
exact failure mode that check exists to catch, arriving at the instrument instead of
the corpus.

## Symptom (Effect)

An open bug file that HAS recorded a fix SHA and patch-id in its `## Fix` section is
not reported as `non_terminal_status_with_fix_anchor`. The finding is silently absent;
nothing distinguishes "no such record exists" from "the scan could not see it". The
check returns a number, never an error.

## Reproduction

Four live instances, all verified by hand on 2026-09-02 against the working tree.
`patch-id` ends the line; the backticked value opens the next:

```
docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md:57-58
  Observed 2026-08-31 with the desktop at what is now `4d2e5e58` (patch-id
  `3687655cd2dc5849e87278015774349302fd977d`; the original SHA `97d3a4ec` was orphaned

docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md
  (`d5af3d3ceff1d08c`), fixed at `74b9cc67`, patch-id
  <value on the following line>

docs/issues/archive/2026-09-02-a-byte-ceiling-test-cannot-see-a-member-stop-delivering.md
  (archived 2026-09-02, fix round 1 — path updated here to the archived location; the line
  quoted below is exactly as it read at the time this reproduction was captured)
  Implemented and verified, **not landed**: branch `p50-absorption-demo`, `13ee893b`, patch-id
  <value on the following line>

docs/issues/archive/2026-09-02-a-peer-build-unlinks-the-test-binary-and-reds-fourteen-tests.md
  **Fixed on `experiments` at `50b1605f`**
  (`50b1605fb0d63adfe9f084a2c4b8d91d2df68b34`), patch-id
  <value on the following line>
```

The fourth is the sharpest: that file was flagged `terminal_status_without_fix_anchor`
by the *sibling* check and archived at `c71e97c7` only after a `## Fix provenance`
block was added by hand. Its prose declaration was correct the whole time and no
instrument could read it.

## Environment

`experiments`, shared checkout, nine sessions across three profiles. Found while
verifying the six archive candidates for `c71e97c7`, not by a corpus pass.

## Root cause

The scan window is a **line**; the population is a **declaration**, and declarations
wrap because prose wraps. The token that would terminate the value — a newline — is
invisible to a per-line scan by construction, so the scan cannot even represent the
case, which is why no fixture encodes it: the unit suite's fixtures are all
single-line, so they agree with the implementation.

Note the asymmetry with the *other* direction. `36cb17ed` narrowed this same helper's
**section** scope on purpose, after its first live run returned 5 findings of which 4
were false — patch-ids in `## Symptom (Effect)`, `## Evidence` and `## References`,
each citing a commit the bug was merely *observed* at. That fix was correct and must
not be undone here.

**So the repair widens the LINE window, never the SECTION scope.** Those are
independent axes, and conflating them would reintroduce the four false positives while
fixing the false negatives.

## Evidence

### E1 — the structured form survives by luck, not by design

`docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md:145-149`
carries nine anchors in a two-line shape:

```
- **SHA:** `374c75dc`
  **patch-id:** `de1f07d691d833ce028bfb389050a689f4ae737f`
```

These ARE detected — but only because `**patch-id:**` and its backtick happen to land
on the same line. One reflow that pushes the label to a line end takes all nine
invisible at once, silently, in the file that is the closure record for that feature.
So the blast radius is not "four prose mentions": it is every structured block one
whitespace edit away from the defect. Contributed by peer session
`9716a130-c93d-4a65-9ab2-ddc53d6d9cfb`, verified here at the bytes.

### E2 — the fence handling is unaffected

`declared_patch_ids` skips fenced lines, and correctly. That half is orthogonal: a
worked example teaching the syntax is a quotation, not a declaration. Widening the line
window must preserve it.

## Hypotheses tried

- *"The count gate shares this blind spot."* **Falsified**, and worth recording as a
  denominator rather than absorbed. `cluster_tags` (`tests/issue_clusters.rs:101-132`)
  strips `-` both bare and after `trim_start()`, and reads block sequences *and* inline
  flow lists; its own doc comment already says *"Reading only one silently
  under-reports."* The enforcement layer knew. A reader still walked into the
  indentation variant of that trap the same evening, because the knowledge lives in a
  test-module header — `OB-1` § *the third position*.

## Fix

Not yet implemented. The constraint above is the design: widen the window, keep the
section scope.

- **Candidate.** Scan a two-line window — the current line joined to its successor —
  or scan the `## Fix` section body as a single string with newlines normalised to
  spaces before the `patch-id` search, keeping `FenceState` line-driven so E2 still
  holds. The second is simpler and strictly more general; the first is a smaller diff.
- **Test it must satisfy, and the trap.** A fixture with the value on the following
  line. Note the sibling defect this file's own class predicts: a fixture written as a
  single line with `\n` embedded is not the same input as two real lines once the
  helper joins them, so the fixture must be written as genuine multi-line text.

## Tests added

None yet — capture-on-notice record.

## Workarounds

Keep `patch-id` and its backticked value on the same line. The `## Fix provenance`
shape `get_guide("tracker-conventions")` prescribes does this by default, so authors
following it are covered today, by luck rather than by the parser.

## Resume

Open, unstarted. `## Root cause` carries the one constraint that makes this a
five-line fix rather than a regression: widen the line window, never the section
scope. Read `## Fix` before touching `declared_patch_ids`.

## References

- `src/librarian/tools/doctor.rs:5245-5278` — `declared_patch_ids`, the per-line scan.
- `36cb17ed` — *fix(doctor): a patch-id outside a Fix section is a citation, not an
  anchor*. The section-scoping fix that must not be undone.
- `c71e97c7` — the archive commit whose verification surfaced this.
- `IC-18`, `cluster/selector-narrower-than-its-population`. Filed here rather than
  `IC-6`, which a peer proposed: `IC-6` is about a scheme with no way to write a token
  **literally** or to disambiguate two that **collide**, and `36cb17ed` already fixed
  the over-detection direction. This is under-detection by a selector narrower than its
  population, which is `IC-18`'s claim almost verbatim — *"runs to completion over a
  subset and returns a plausible answer."*
- `docs/issues/2026-09-02-a-git-verb-regex-swallows-longer-subcommands-sharing-its-prefix.md`
  — `IC-6`'s shell-gate member, offered as a cross-reference by the same peer. Related
  in shape (a boundary token the pattern cannot express) and a different class in
  direction (it matches too much; this matches too little).
