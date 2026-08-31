---
id: e9667199520251e4
kind: bug
status: fixed
title: 'BUG: commit_like_hashes paired backticks across the whole file, so one stray backtick silently voided every hash after it'
owners:
- marius
tags:
- librarian
- doctor
- heuristic
- silent-wrong-answer
- markdown-parsing
- cluster/addressing-without-an-escape-hatch
topic: record-legibility
closed: 2026-08-19
opened: 2026-08-19
owner: marius
related:
- '53e35aaefb9f7c71'
severity: low
---

> **Status: fixed, severity low.** Shipped and repaired the same day, inside one session.
> Filed anyway because the mechanism is reusable and the number it corrupted had already
> been published.

## Summary

`commit_like_hashes` (`src/librarian/tools/doctor.rs`), the decoy heuristic behind
`terminal_status_without_fix_anchor`, found inline-code spans with:

```rust
for span in content.split('`').skip(1).step_by(2) {
```

That takes alternate segments of the **whole document**, which is only correct if every
backtick pairs. A single unmatched one shifts the parity, so from that character onward every
inline span is read as prose and every prose gap as inline code. No error, no warning — just
a smaller number.

## Symptom (Effect)

`terminal_status_without_fix_anchor` reported **zero** commit-like hashes for
`docs/issues/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md`,
which cites `c7bdfd22` on three separate lines.

The corpus figure was wrong as a result: **7 of 9** unanchored records were reported as
carrying decoy hashes, where the true count is **8 of 9**. That figure was already written
into the check's doc comment, a bug file, and a session-log entry before the defect surfaced.

## Reproduction

```
$ grep -o '`' docs/issues/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md | wc -l
393                      # odd
$ head -59 <same file> | grep -o '`' | wc -l
71                       # odd, so line 60's span is inverted
$ grep -n 'c7bdfd22' <same file>
60:codescout `experiments` @ `c7bdfd22`. Measured 2026-08-18 against the live catalog
```

## Root cause

Line 51 of that file, **inside a fenced block**, is a shell command whose regex contains one
backtick:

```
grep -n '^#\{1,6\}[[:space:]]\+`\?T-[0-9]\+' docs/trackers/tool-usage-patterns.md
```

Fences themselves are balanced (open + close = six backticks), so they were not the problem
directly — but they are where unbalanced backticks *live*, because fenced content is quoted
material from elsewhere and obeys no markdown convention at all.

The deeper cause is a parity assumption over a delimiter that is not guaranteed to pair. It
is the same shape as an unclosed quote in a CSV parser: the failure is not local to the bad
character, it propagates to the end of the input, and it degrades a *value* rather than
raising.

## Evidence

Direction of the error matters and was checked: the inversion causes an **under**-report.
After a flip, the segments treated as inline code are the prose gaps, which are almost never
exactly 7–12 hex characters — so the heuristic loses hashes rather than inventing them.
That means the finding degrades to the *weaker* of its two messages ("plainly unanchored"
instead of "reads as anchored") rather than making a false accusation. Wrong, but not
slanderous.

## Fix

Two changes, one decision:

1. **Skip fenced blocks.** Content inside a fence is quoted material, not the document's own
   references — a `git show <sha>` example should never count as a decoy — and it is where
   stray backticks concentrate.
2. **Pair backticks per line.** Inline code does not span lines, so nothing legitimate is
   lost, and a stray backtick now corrupts at most its own line instead of the remainder of
   the document.

## Tests added

Two, and the second exists only because a mutation was actually applied:

- `commit_like_hashes_survives_a_lone_backtick_inside_a_fence` — the real failing shape.
- `commit_like_hashes_confines_a_stray_prose_backtick_to_its_own_line` — a stray backtick
  **outside** any fence, where fence-skipping cannot help.

| # | Mutation | Observed |
|---|---|---|
| M6a | fence detection disabled | fence test FAILS — the fenced hash is swept in |
| M6b | pair backticks globally over the non-fenced text | prose test FAILS, returning `[]` |

**M6b passed the fence fixture**, which is why the second test exists. That fixture was
written to pin parity robustness and does not discriminate it: once fences are skipped, its
remaining backticks are incidentally *even*, so global and per-line pairing agree on it. A
test can pass a mutation of the very property it was written for. Recorded as
`prompt-surface-compaction-session-log:W-13`.

## Fix provenance

- **SHA:** `fea7101e` (`experiments`) — *fix(doctor): commit_like_hashes paired backticks
  across the whole file*. Positional; does not survive a rebase of `experiments`.
- **patch-id:** `a4d737626a5c058ff7b3c2d496dfb96d61575918` — content hash of the diff;
  survives rebase and cherry-pick.

The defect was introduced by `375225cc` (patch-id `ca1fca68b7c3be51d51d816716d4243079773e90`),
archived at
`docs/issues/archive/2026-08-19-terminal-bug-file-with-no-recoverable-fix-anchor.md`.

## References

- `src/librarian/tools/doctor.rs` — `commit_like_hashes`, `scan_terminal_status_without_fix_anchor`
- `docs/issues/archive/2026-08-19-terminal-bug-file-with-no-recoverable-fix-anchor.md` — the
  check this heuristic serves, and the corrected 8-of-9 figure

