---
id: c225771a02a41002
kind: bug
status: open
title: grep's invalid-regex hint always says "escape metacharacters", which is wrong advice when the cause is look-around
tags:
- cluster/hint-composed-without-the-request
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

Open. When the error text names look-around, hint "use two greps, or `(?=…)`-free alternatives; the engine has no look-around".

## Tests added

None. A test would compile `a(?!b)` through the grep tool and assert the hint mentions look-around, not escaping.

## Workarounds

Rewrite the pattern without look-around.

## Resume

Found by the 2026-10 friction deep dive (`docs/trackers/friction-deep-dive-2026-10.md`, group G5). Severity low.

## References

- `src/tools/grep.rs`
