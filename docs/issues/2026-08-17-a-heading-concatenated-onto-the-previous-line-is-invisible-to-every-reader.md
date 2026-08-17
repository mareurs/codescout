---
status: fixed
opened: 2026-08-17
closed: 2026-08-17
severity: medium
owner: marius
related: []
tags: [docs, markdown, silent-corruption, read-markdown, detector-candidate]
kind: bug
---

# BUG: a heading concatenated onto the end of the previous line is invisible to every reader, and nothing detects it

## Summary

`docs/PROGRESSIVE_DISCOVERABILITY.md` carried an entire documented pattern — *Pattern 6:
Never Exceed MCP Output Limits* — whose `###` marker was welded onto the end of the
preceding paragraph with **no newline at all**. It was therefore not a heading in any
markdown parser: absent from `read_markdown`'s heading map, unaddressable by
`edit_markdown`, and rendered as a run-on sentence anywhere it is read.

Found only because I tried to insert a sibling after it and the heading did not exist. Then
a scan found **five more**, including one in `docs/TAXONOMY.md` and one in the published
manual — so this is a recurring silent-corruption class, not a typo.
## Symptom (Effect)

Measured 2026-08-17 at commit `8638d9fb`:

```
docs/PROGRESSIVE_DISCOVERABILITY.md:196
spliced into the response body at the top level.### Pattern 6: Never Exceed MCP Output Limits
```

`read_markdown("docs/PROGRESSIVE_DISCOVERABILITY.md")` lists Pattern 5a and then jumps
straight to `## Anti-Patterns`. Pattern 6 appears in **neither** the heading map nor the
available-headings list that `edit_markdown` returns on a miss:

```
edit_markdown(heading="### Pattern 6: Never Exceed MCP Output Limits", …)
-> heading not found — Available headings: …, ### Pattern 5a: …, ## Anti-Patterns, …
```

## Reproduction

1. Append a `### Heading` to a line with no intervening newline.
2. `read_markdown(path)` — the heading is absent from the map.
3. `edit_markdown(path, heading=<that heading>, …)` — refused as not found.
4. `grep '^### Pattern 6'` — **0 matches**, while `grep 'Pattern 6'` finds it. That pair is
   the whole diagnosis, and it is the cheapest detector.

## Environment

Linux, `experiments` @ `8638d9fb`. Applies to any markdown in the repo; nothing about the
file is special.

## Root cause

A missing `\n` in whatever write produced it. Not attributable to a specific tool from the
artifact alone — `git log -S` on the line would name the commit, and that is worth doing
only if it recurs.

The interesting part is not the typo but that **nothing noticed**. Three surfaces that scan
this file all passed it:

- `read_markdown` silently omits the heading — a map with a hole looks exactly like a
  document with fewer sections;
- `edit_markdown` reports it as not found, which reads as "you got the name wrong" rather
  than "the file is malformed";
- `audit_doc_refs` scans this file for stale refs and has no opinion about structure.

*Measured 2026-08-17: the grep pair above, and the heading map before and after the fix.*

## Evidence

### The failure mode is a plausible wrong answer, not an error

I read the section with `read_markdown(heading="### Pattern 5a…")`, which returned Pattern
6's text as part of 5a's body — raw bytes, faithfully. So I concluded Pattern 6 existed as a
heading and that the next free number was 7. That conclusion was right by luck; the evidence
did not support it. A missing heading is indistinguishable from a long section unless you
look at the heading map, which is precisely the artifact the defect corrupts.

Same shape as `R-104` in `docs/trackers/reconnaissance-patterns.md`: absence from an index
is a claim about the index, not about the world.


### Six instances, found by scanning after the first was fixed

*Measured 2026-08-17* with the narrowed detector in § Fix:

| File | Swallowed heading | Disposition |
|---|---|---|
| `docs/PROGRESSIVE_DISCOVERABILITY.md` | `### Pattern 6: Never Exceed MCP Output Limits` | fixed |
| `docs/TAXONOMY.md` | `## Status vocabularies (per prefix)` | fixed |
| `docs/manual/src/tools/library-navigation.md` | `## Indexing a Library for Semantic Search` | fixed |
| `docs/trackers/archive-cadence-policy.md` | `## Design surfaces (open)` | fixed |
| `docs/superpowers/specs/2026-03-26-onboarding-versioning-design.md` | `### Response for Successful Fast Path (Version OK)` | fixed |
| `docs/archive/bug-reports/2026-03-to-2026-04-tool-misbehaviors.md` | `### BUG-021 — …` | **left alone** — archived |
| `docs/trackers/archive/findings-goal-audit.md` | `### 2026-05-17 — First fix shipped` | **left alone** — archived |

The two archived ones are deliberate: archived surfaces are historical snapshots, the same
rule that governs citation repointing in `archive-cadence-policy` § 3. Seven total, five
repaired.

That `docs/TAXONOMY.md` is on the list matters more than the count. It is the one-page index
CLAUDE.md tells agents to start from when routing an observation, and one of its section
headings has been invisible to every `read_markdown` call against it.
## Hypotheses tried

1. **Hypothesis:** Pattern 6 exists and the heading map was truncated by a cap.
   **Test:** `grep '^### Pattern 6'` versus `grep 'Pattern 6'`.
   **Verdict:** rejected — 0 matches anchored at line start, 1 unanchored. Not truncation;
   the line-start form does not exist.

## Fix

**Fixed 2026-08-17 in this commit** — five instances split, the two archived ones left alone.
Verified structurally rather than visually: `edit_markdown` now targets each heading
successfully, which is the operation that failed before.

**The detector took two attempts, and the first was worthless.** Recorded because shipping
it unverified was one keystroke away:

```
# ATTEMPT 1 — do not use
grep -rnE '[^ \t]#{1,6} ' --include='*.md' .     -> 16,033 matches in 1,027 files
```

Useless. `#` is ubiquitous in prose — `C#`, `#123`, shell comments inside fences — so "a hash
run not at line start" describes normal markdown.

```
# ATTEMPT 2 — the one that works
grep -rnE '[.!?:;]#{2,6} [A-Z0-9`]' --include='*.md' .   -> 15 matches in 9 files
```

The discriminator is the **sentence terminator immediately before the hash run**: a swallowed
heading is glued to the end of a finished sentence, which prose hashes are not. 15 matches, 7
real, 8 false positives — all of them legible at a glance (one plan uses `N:### Heading`
notation deliberately; one bug file quotes a `grep` command). A detector with eight
eyeball-able false positives is usable; one with sixteen thousand is not.

Deliberately **not** done: hunting the originating commits. Five separate files across five
months says the cause is a class of write, not one bad edit, and `git log -S` on each would
cost more than a recurrence check.
## Tests added

None — data fixes in docs, not a code change.

The **detector** is the durable artifact, and it now has a measured false-positive rate
rather than a guessed one (§ Fix). Worth folding into the tracker-hygiene sweep as a
structural check beside the entry-heading rules in
`get_guide("tracker-conventions")` § *Entry headings — the definition rule*: that section
already governs headings that fail to define what they appear to, and a heading no parser
can see is the degenerate case of exactly that.

If it graduates to a real gate, `audit_doc_refs` is the natural host — it already walks every
markdown file in the repo, and this is a structural claim about the same files it reads for
reference claims.
## Workarounds

N/A — fixed.

## Resume

N/A for the five repairs.

One follow-up worth doing when the tracker-hygiene sweep next runs: add the § Fix detector as
a structural check, with the two archived instances as its known-exempt cases. Re-run it then
— a sixth live instance appearing between now and then is the signal that the write path
producing these is still active and worth identifying with `git log -S`.
## References
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — the affected file; Pattern 6 and the new Pattern 7
- `docs/trackers/reconnaissance-patterns.md` — `R-104`, the general form of the reasoning error
- `get_guide("tracker-conventions")` § *Entry headings — the definition rule* — the sibling
  rule about headings that fail to define what they appear to
