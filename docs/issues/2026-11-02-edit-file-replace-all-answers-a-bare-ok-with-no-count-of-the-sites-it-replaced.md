---
id: '200f89eca3eb91e5'
kind: bug
status: open
title: 'BUG: `edit_file(replace_all=true)` answers a bare "ok", so a call that replaced a different number of sites than intended looks the same as one that replaced the right number'
tags:
- cluster/instrument-omits-the-dimension-that-grows
opened: 2026-11-02
owner: marius
related:
- docs/issues/archive/2026-09-03-scoped-edit-silently-takes-the-first-of-several-old-string-matches.md
severity: low
unverified: 'Reproduced once on a scratch file and read in the code. Not checked: whether a test pins the bare "ok" response, which a fix would have to change, and the batch path was not re-read this session.'
---

# BUG: `edit_file(replace_all=true)` answers a bare "ok", so a call that replaced a different number of sites than intended looks the same as one that replaced the right number

## Summary

A successful `edit_file` returns `json!("ok")` whatever it did. With `replace_all=true` the function already counts the occurrences (`match_count`) but does not report the count, so the caller cannot see that it replaced three sites when it meant two, or one site when it meant two. Two incidents in August 2026 are recorded in the friction log as `codescout-usage-frictions:U-37` and `:U-38`.

## Class fit

`IC-21` is "an instrument reports presence or a count where the decision turns on magnitude". The response here reports presence (success) where the caller's next decision turns on a count: did the edit hit the number of sites I meant? It differs from the class's other members in one way: the figure does not grow over time. It is a per-call count that the tool has in hand and drops.

## Symptom (Effect)

The caller cannot tell a correct `replace_all` from a wrong one without a second call to grep for the old text.

- U-37 (2026-08-06): a `replace_all` after an insert rewrote the line inside the helper being introduced, so the helper called itself. A two-site edit became a three-site one.
- U-38 (2026-08-07): a `replace_all` matched 1 of 2 intended sites, because the other site had different indentation, and the response was plain "ok".

## Reproduction

Reproduced on 2026-11-02 on a scratch file with four lines, three containing `alpha` (one indented):

1. `edit_file(path=<file>, old_string="alpha", new_string="omega", replace_all=true)`.
2. The response is `{"status": "ok", "abs_path": ...}`, with no count.
3. `grep -c omega <file>` gives 3.

## Environment

codescout `experiments` at `294323bdb` plus uncommitted docs; the running server was rebuilt and reconnected on 2026-11-02.

## Root cause

`perform_edit` in `src/tools/edit_file/mod.rs` (the function spans lines 892 to 1152) computes `match_count = content.matches(old_string).count()` near its top, and on the normal success path ends with `Ok(json!("ok"))`. Only the refusal for several matches without `replace_all` reports the count and the line numbers. The escape-decoded and repaired paths return objects with a `note` but also no count. The batch path returns "ok" too (not re-read this session).

## Evidence

- Read this session: the tail of `perform_edit` (`Ok(json!("ok"))`) and the `match_count` line.
- Reproduced this session, as above.
- A test named `edit_file_replace_all_replaces_all_occurrences` (`src/tools/edit_file/tests.rs`) pins that all occurrences are replaced; whether any test pins the bare "ok" response was not checked.

## Hypotheses tried

None needed; the cause is in the code that was read.

## Fix

Not done. Proposal: when `replace_all` is true, return `{"status": "ok", "replaced": N}`. When `replace_all` is false the count is always 1 and the bare response can stay, which keeps the common case short. The same count belongs in the batch path.

## Tests added

N/A. Nothing is fixed. A fix needs a test that fails first: `replace_all` over three occurrences must report `replaced: 3`.

## Workarounds

Grep for the old text and the new text after a `replace_all`, as the friction log suggests.

## Resume

Open. Next step: write the failing test, add `replaced` for `replace_all`, and check the batch path.

## References

- `codescout-usage-frictions:U-37` and `codescout-usage-frictions:U-38` in `docs/trackers/codescout-usage-frictions.md`
- `docs/issues/archive/2026-09-03-scoped-edit-silently-takes-the-first-of-several-old-string-matches.md` (the opposite case: a count that is hidden when it is not 1)
