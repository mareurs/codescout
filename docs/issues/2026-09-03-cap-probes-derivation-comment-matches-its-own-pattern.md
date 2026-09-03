---
id: aa8f15dc6b1d6a21
kind: bug
status: open
title: cap_probe.rs's published id-list derivation is false, and running it as written matches its own prose
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
opened: 2026-09-03
severity: medium
---

## Summary

`src/tools/core/cap_probe.rs`'s module doc comment claims the 66-row `RESULT_CAP` id list is "derived from the gate itself (`grep(pattern="cap-class: RESULT_CAP", path="src")`)". This is false on both counts: the real gate does not use that grep (it uses `every_cap_constant_is_classified`'s own const-declaration parser, and the sibling file `tests/result_caps.rs` says so explicitly), and running the named grep against tracked `src/` returns **69** matches, not 66 — three of them are `cap_probe.rs`'s own prose, including the derivation sentence itself.

## Symptom (Effect)

A contributor who does exactly what this branch's whole thesis asks — re-check a published derivation rather than trust the value — runs `grep(pattern="cap-class: RESULT_CAP", path="src")`, gets 69, and concludes 3 rows are missing from the table. There is no discrepancy to find; the grep is matching its own documentation.

## Reproduction

1. Check out `result-cap-marker-gate` at `2a32c043` (or later, once merged).
2. Run `grep(pattern="cap-class: RESULT_CAP", path="src")` (or `git grep -c 'cap-class: RESULT_CAP' -- src/`) against the checkout.
3. Compare the count to `cap_probe.rs`'s stated 66-row census.
4. Observe 69, not 66; `src/tools/core/cap_probe.rs` itself contributes 3 of the matches (its own module doc and a struct-field doc comment quoting the annotation form, plus the derivation-instruction line).

Reproduced independently by the whole-branch review (2026-09-03) and re-confirmed live by the controlling session the same day, after re-pinning the codescout MCP server's active project to the worktree (an intervening `/mcp` reconnect had silently reset it to the main checkout, which does not carry this branch's annotations — the first attempt returned 0, which was itself the tell).

## Environment

`result-cap-marker-gate` branch/worktree, `src/tools/core/cap_probe.rs` module doc comment (~`:172-176`).

## Root cause

`cap_probe.rs`'s module doc describes a derivation mechanism (a shell/tool grep over `"cap-class: RESULT_CAP"`) that is not what the gate actually runs, and — because `cap_probe.rs` itself contains that literal string three times in prose — the described mechanism, if actually run, matches its own documentation. This is a self-matching instrument: an instruction that reads its own text as data. The correct mechanism is already documented correctly one file over: `tests/result_caps.rs`'s header states the id list comes from `every_cap_constant_is_classified`'s own parser over `git ls-files src`, "not by a shell grep — a second selector answers a slightly different question, which is the `IC-18` mistake this gate exists to catch."

*Confirmed at the bytes: `src/tools/core/cap_probe.rs:172-174` (or nearby, may drift with edits) carries the false derivation text; `tests/result_caps.rs:25-29` carries the correct, contradicting text. Live grep re-run 2026-09-03 confirmed 69 matches with `cap_probe.rs` contributing 3.*

## Evidence

```
src/tools/core/cap_probe.rs:174 (approx):
/// the gate itself (`grep(pattern="cap-class: RESULT_CAP", path="src")`),

tests/result_caps.rs:25-29 (approx):
/// Derived by running `every_cap_constant_is_classified`'s own parser over
/// `git ls-files src` — not by a shell grep — a second selector answers a
/// slightly different question, which is the IC-18 mistake this gate exists
/// to catch.
```

Live grep, 2026-09-03: `grep(pattern="cap-class: RESULT_CAP", path="src", workspace=<worktree>)` → 69 matches in 34 files; `cap_probe.rs` contributes 3 (all prose, none a real annotation).

## Hypotheses tried

1. **Hypothesis:** the derivation comment is aspirational/historical rather than descriptive of the current mechanism. **Test:** compared against `tests/result_caps.rs`'s explicit, contradicting statement of the actual mechanism, dated to the same branch. **Verdict:** confirmed the `cap_probe.rs` text is simply wrong, not a stale-but-once-true description — the two files disagree about a design decision this branch itself made (grep vs parser) and cite it as a lesson (`IC-18`) in one place while contradicting it in the other.

## Fix

Not designed. Correct `cap_probe.rs`'s derivation comment to match `tests/result_caps.rs`'s accurate description (parser over `git ls-files src`, not a shell grep), and consider adding a one-line cross-reference between the two so a future edit to one is more likely to catch drift in the other.

## Tests added

None yet — this is a doc-comment defect; a code-level regression test does not directly apply, though a `doctest`-style assertion that the two files' derivation descriptions agree could be considered.

## Workarounds

Do not use the grep pattern quoted in `cap_probe.rs`'s module doc to audit the row count; use `every_cap_constant_is_classified`'s own logic, or trust `print_mutation_tally`'s reported total after independently spot-checking a handful of rows.

## Resume

1. Once merged, fix `cap_probe.rs:172-176`'s derivation text to match `tests/result_caps.rs:25-29`.
2. Consider whether the false claim should be caught by a lint (e.g. a test asserting the two files' stated derivation mechanisms are textually consistent) so this class of drift cannot recur silently.

## References

- `src/tools/core/cap_probe.rs:172-176` (approx.), `tests/result_caps.rs:25-29` (approx.)
- Surfaced during `result-cap-marker-gate` branch's whole-branch review (2026-09-03), session ledger `.superpowers/sdd/2026-09-02-result-cap-marker-gate/progress.md`, finding I2
- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md` (the no-escape half: a construct that means "cite this pattern in prose" is read by the very scanner it is written to describe)

