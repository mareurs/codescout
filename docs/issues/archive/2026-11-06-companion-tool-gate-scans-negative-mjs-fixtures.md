---
id: ecf86374143251f0
kind: bug
status: fixed
title: Companion tool-name gate treats negative .test.mjs fixtures as live tool references
tags:
- cluster/unclassified
- testing
- companion
- gate
closed: 2026-11-07
opened: 2026-11-06
owner: marius
severity: medium
---

# BUG: Companion tool-name gate scans negative .test.mjs fixtures

## Summary

MAIN gate @bg_00000109 exits1: LEAN=100 and DEFAULT=100, each solely failing server::tests::companion_surfaces_reference_only_real_tools. Python light1642 passed plus529 subtests; formatter/clippy exit0. The research preparation changed no product source.

## Class fit

Unclassified test-versus-production surface classification gap. Scanner explicitly excludes .test.sh but not .test.mjs.

## Symptom (Effect)

Failure names cs-verified-relocate.test.mjs tokens mcp__codescout__y, mcp__codescout__evil__, mcp__codescout__evil__edit_code and mcp__codescout___edit_code as nonexistent tools. Lean6161/6162 pass; default9029/9030 pass. Shared gate is red.

## Reproduction

./scripts/gate.sh --check in MAIN with sibling claude-plugins present. Job record /tmp/codescout-bg/455879-a720241d-bg_00000109.json names full log. Failure spans2355-2375 and11495-11520.

## Environment

2026-11-06 MAIN experiments, leased RAM target. The failure is a Rust assertion about sibling hook text; not a measured disk/compiler/model failure.

## Root cause

Read src/server.rs tests/companion_surfaces_reference_only_real_tools:7168-7304. It reads .sh/.json/.mjs but skips only fname.ends_with(".test.sh"); positive regex scans full text. Fixture intent verified 2026-11-07: the four tokens are deliberate negative inputs (see `## Hypotheses tried`).

## Evidence

Closed job/raw logs and source read this session. Literal cs-verified-relocate/invalid-token search found no existing issue in MAIN docs/issues; semantic lookup found older unrelated companion scanner records.

## Hypotheses tried

1. **Hypothesis:** the failure comes from this session's `edit_code` change, not from the scanner. **Test:** ran `companion_surfaces_reference_only_real_tools` on `565999f11` (the parent of `4d66ee760`, no `edit_code` change) in a worktree placed beside `claude-plugins`, because the test reads `CARGO_MANIFEST_DIR/../claude-plugins/codescout-companion/hooks` and skips silently when that is absent. **Verdict:** rejected. It failed with the same four tokens and printed no `skipping` line.
2. **Hypothesis:** the four tokens are deliberate negative inputs. **Test:** read `cs-verified-relocate.test.mjs` (`:458`, `:505`, `:517`). **Verdict:** confirmed. They are tool names of other servers (`mcp__x__mcp__codescout__y`, a server named `codescout__evil`, a server named `codescout_`), fed to the hook to prove it stays silent.

## Fix

Applied, commit `914c2a60f` on `experiments` (on `origin/experiments` since 2026-11-07; patch-id `57d0c2acc13a0ecf0bffe0d57305eba451a8df3a`). `src/server.rs` gains `NEGATIVE_FIXTURE_TOKENS`, the four exact `(file, token)` pairs, and `is_negative_fixture_token`; `companion_surfaces_reference_only_real_tools` skips a captured name when the pair is listed.

Chosen over two alternatives, both measured against the source:
- Skipping `*.test.mjs` like `*.test.sh`: would also stop checking `host-normalization.test.mjs`, which carries one real `mcp__codescout__` reference, and the retired-name sentinel on every test file.
- Refusing captured names that contain `__` or start with `_` in the regex: would also hide a real matcher branch such as `mcp__codescout__(edit_code|replace__symbol)`, because the capture splits on `|` and the second name contains `__`.

No negative fixture was deleted, and the plugin repo was not touched.

## Fix provenance

- **SHA:** `914c2a60f043a45ceb9abfea3ae39b3a7d0c0d10` (`experiments`)
- **patch-id:** `57d0c2acc13a0ecf0bffe0d57305eba451a8df3a`

## Tests added

- `negative_fixture_exemption_is_exactly_the_four_relocate_tokens`, `src/server.rs` (`mod tests`): the four tokens are exempt in `cs-verified-relocate.test.mjs`; a new nonexistent token in that file, the same token in `cs-verified-relocate.mjs`, and `y` in `host-normalization.test.mjs` are not. Red: 4 errors `E0425` on the missing predicate.
- The existing `companion_surfaces_reference_only_real_tools` is the regression test for the scan itself. Mutant: removing the `evil__` entry fails it with exactly `mcp__codescout__evil__` and no `skipping`; reverted.
- Full gate on the committed tree: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, LEAN 6165 passed, DEFAULT 9033 passed, Python 1642 passed, doctests ok.
- **Not covered:** an entry is not checked for still being needed. If the fixture leaves the plugin, the entry stays inert until the same token reappears in that file.

## Workarounds

Report gate red; research controls retain their independently archived evidence.

## Resume

Done 2026-11-07. Archived 2026-11-07, after `914c2a60f` was pushed. Re-open if a new negative fixture is added to a companion `.test.mjs` and the gate goes red on its token: add the `(file, token)` pair, do not widen the scan.

## References

Semantic-contract report docs/research/2026-11-06-semantic-contract-preparation.md.
