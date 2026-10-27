---
id: 637ac39f6a9e7f73
kind: bug
status: fixed
title: grep's invalid-regex hint always says "escape metacharacters", which is wrong advice when the cause is look-around
tags:
- cluster/hint-composed-without-the-request
closed: 2026-10-27
opened: 2026-10-26
owner: marius
severity: low
---

# BUG: grep's invalid-regex hint always says "escape metacharacters", which is wrong advice when the cause is look-around

## Summary

`grep` (`src/tools/grep.rs:864-868`, read 2026-10-26) answers every regex that fails to compile and looks like a regex with the constant hint `patterns are full regex syntax — escape metacharacters like \( \. \[ for literals`. When the pattern uses look-ahead or look-behind, the Rust `regex` crate rejects it and the error text already says "look-around, including look-ahead and look-behind, is not supported". The hint then tells the agent to escape characters, which does not help.

## Class fit

The hint is built without the compile error it accompanies, so it cannot adapt to the actual cause. That is the hint-composed-without-the-request trait. It differs from the class's other members because the right information is in the same error and the hint ignores it.

## Symptom (Effect)

The agent receives a correct error and an irrelevant hint. All six `invalid_regex` errors in the window were look-around patterns such as `(?!None)`, `(?! ESCAPE)` and `(?!probed)`. Recovery was one call in 6 of 6 cases (subagent figure).

## Reproduction

Source read; the six rows exist in `usage.db`. `grep(pattern="foo(?!bar)")` fails to compile and reaches the constant hint.

## Environment

codescout `experiments` at `8df90f1b9`.

## Root cause

The hint is a constant in the `Err(e)` arm of `compile`.

## Evidence

`usage.db` `err_family='invalid_regex'`: 6 rows in 5 sessions; the four shown messages read `error: look-around, including look-ahead and look-behind, is not supported`.

## Hypotheses tried

N/A.

## Fix

Fixed in `src/tools/grep.rs`: a new `invalid_regex_hint(&regex::Error)` helper chooses the hint, and `build_grep_regex` calls it in the `Err(e)` arm. When the compile error text contains "look-around", the hint says the engine has no look-around and suggests two greps or a rewrite. Every other invalid pattern keeps the escape advice.

- SHA: `f2aad964a` on `experiments`.
- patch-id: `9826ccfcaeccd91c0f62dd3c82d1875076976b82` (`git show f2aad964a | git patch-id --stable`).

## Fix provenance

- **SHA:** `f2aad964a` (`experiments`)
- **patch-id:** `9826ccfcaeccd91c0f62dd3c82d1875076976b82`

## Tests added

- `invalid_regex_hint_names_look_around_not_escaping` in `src/tools/grep.rs` (compiles `a(?!b)`, asserts the hint names look-around and not escaping).
- `invalid_regex_hint_keeps_escape_advice_for_other_errors` in `src/tools/grep.rs` (compiles `a(b`, asserts the escape advice stays).

Both test the helper, not the full `grep` tool call. The tests were written after the fix, so no red run is recorded for them. The helper takes the pattern as a runtime parameter because clippy's `invalid_regex` lint denies invalid regex literals.

## Workarounds

Rewrite the pattern without look-around.

## Resume

N/A — fixed.

## References

- `src/tools/grep.rs`
