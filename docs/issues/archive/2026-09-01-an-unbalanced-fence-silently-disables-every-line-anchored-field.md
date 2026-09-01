---
id: 073c72c36e845402
kind: bug
status: fixed
title: 'BUG: one stray ``` disables every line-anchored field for the REST of the file, and doctor reports the result as "none declared"'
tags:
- cluster/addressing-without-an-escape-hatch
- librarian
- doctor
- markdown
- silent-failure
closed: 2026-09-01
opened: 2026-09-01
owner: marius
related:
- docs/issues/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md
severity: medium
---

## Summary

`structured_fix_pointers` skipped fenced lines using a **hand-rolled boolean toggle** that
flipped on any line starting with three or more backticks. Two consequences: a run shorter
than an enclosing fence (a ``` quoted inside a ````) flipped it wrongly, and an
unterminated fence left it inverted for the rest of the file. Either way every `- **SHA:**`
below the fault is skipped, and `doctor` then states *"no `## Fix provenance` pointer is
declared"* for a file that visibly contains one — true of the parse, false of the file.

**A correct fence tracker already existed in-tree.** `crate::util::markdown_fence::FenceState`
matches the opening character and run length, requires a closer to be at least as long and
followed only by whitespace, and rejects a backtick opener whose info string contains a
backtick. `src/librarian/statements.rs` documents delegating to it *"rather than a
hand-rolled toggle"* as **"required, not incidental"**. `structured_fix_pointers` was the
one consumer of the convention that did not.
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

**Do not count delimiters — that instrument is wrong in both directions, measured.** The
first version of this file prescribed an odd/even parity count. Against the three real
files the shipped check finds, parity gets **two of three wrong**:
`docs/superpowers/plans/2026-02-28-prompt-injection-design.md` and
`…/2026-05-01-call-graph.md` both have EVEN parity and a genuinely open fence. It also
false-positives on any file that quotes triple-fence syntax inside a quadruple fence — an
odd count and perfectly well formed — which is a shape this corpus contains *because* the
sibling checks teach people to write worked examples.

The reproduction is the shipped check itself:

```
cargo build --bin codescout && ./target/debug/codescout doctor \
  | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['summary']['by_check']['unterminated_fence'])"
```

Measured 2026-09-01 at the fix commit: **3**.
## Environment

codescout `experiments`, `src/librarian/tools/doctor.rs`. Applies to any consumer of the
fenced-line convention, not just this check.

## Root cause

`structured_fix_pointers` (`src/librarian/tools/doctor.rs`) carried `let mut fenced = false`
and flipped it on `t.starts_with("```") || t.starts_with("~~~")`. That predicate cannot
distinguish a delimiter from a line that merely begins with fence characters, and it has no
notion of run length, so a `` ``` `` nested in a `` ```` `` block toggles it. Read at the
bytes 2026-09-01.

**Correction — the blast radius claimed by the first version of this file was wrong.** It
said the same convention governs `**Valid:**` and `**Rests on:**`, so every line-anchored
field was exposed. Verified false: `parse_validity` / `parse_rests_on`
(`src/librarian/statements.rs`) already use `FenceState`, **and** they receive one section's
text at a time with a fresh tracker per call, so an unterminated fence cannot leak across a
section boundary into a later entry. Both axes are closed there. The exposure was
`structured_fix_pointers` alone, which is whole-file and was hand-rolled — narrower than
filed, and worth stating because the wrong version made the defect sound systemic when it
was one un-migrated caller.

What makes the class hard is unchanged: the failure is **silence**, and silence is exactly
what a file with nothing to declare produces. "Declared nothing" and "declared, but
unreadable from line N" are the same empty result to every consumer.
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

Two changes, both in `src/librarian/tools/doctor.rs`.

**1. `structured_fix_pointers` now delegates to `FenceState`** instead of its own toggle —
the abstraction the codebase already required of every other consumer.

**2. New read-only `doctor` check, `unterminated_fence`.** Reports a catalogued markdown
file that reaches EOF with a fence still open, naming the **file line** the opener sits on,
because "somewhere below here" is the whole remedy in a ledger thousands of lines long.
Registered in `declare_checks!` **and** wired into the dispatcher — the codebase's own
`declare_checks!` guard caught the first omission by panicking on an undeclared name, and
`references()` caught the second, which was three test callers and no production one.

No `fix=` mode. Closing versus deleting a stray delimiter is a content judgement about the
prose around it, and a repair tool guessing wrong would rewrite meaning.
## Fix provenance

- **SHA:** `800f1dec` (`experiments`)
- **patch-id:** `94f488b4355fc72d6874408582acbb175b84317d`

One commit carries both halves — the `FenceState` migration and the new check — because the
second is only trustworthy given the first: a detector built on the same broken toggle would
have inherited its blind spot.
## Tests added

All four written before the code, each watched failing first.

- `structured_fix_pointers_reads_a_declaration_after_a_quadruple_fenced_example` — the
  toggle/`FenceState` discriminator. Failed `left: []` against the old toggle.
- `unterminated_fence_fires_only_where_a_fence_is_left_open` — fires on the open file,
  **silent on a balanced control**; without the control a check that fired on every
  markdown file would pass.
- `unterminated_fence_is_silent_on_a_quadruple_fenced_example` — the false-positive guard.
  This one is monotone under "does nothing" and passed against the stub, so it is a control
  and not a discriminator; it is load-bearing only in company with the two above.
- `unterminated_fence_names_the_line_the_silenced_region_starts_at` — pins the file line,
  not a body offset.

The two sibling tests that already existed (`…ignores_a_fenced_worked_example`,
`…returns_every_declared_pair_in_order`) stayed green across the `FenceState` swap, which
is what makes it a migration rather than a behaviour change.
## Workarounds

Run the parity probe under § *Reproduction* before trusting any "none declared" finding on
a file whose body you can see declares something.

## Resume

N/A — fixed, gate green, and the check verified end-to-end against the real repo rather
than only in tests (3 findings, none of them a file a parity counter would have named
correctly).

The **3 files it reports are not repaired** — that is content work in three plans owned by
other work streams, and the check now makes them visible, which was the point.
## References

- `src/librarian/tools/doctor.rs:4535-4579` — `structured_fix_pointers`, the fence toggle
- `docs/issues/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md` — the
  file this was found on
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — a zero must name its scope
