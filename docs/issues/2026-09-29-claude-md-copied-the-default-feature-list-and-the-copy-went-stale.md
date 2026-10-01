---
id: '96745f0ce9636580'
kind: bug
status: fixed
title: CLAUDE.md copied Cargo.toml's default feature list, and the copy went stale
tags:
- cluster/doc-contradicted-by-code
closed: 2026-09-29
opened: 2026-09-29
owner: marius
related: []
severity: low
---

# BUG: CLAUDE.md copied Cargo.toml's default feature list, and the copy went stale

## Summary

`CLAUDE.md` § *Development Commands*, in the bullet that begins "AND THE DEFAULT LANE IS VACUOUS FOR `server-stack`", quoted
`default = ["remote-embed", "http", "librarian"]`. `Cargo.toml`'s `default` has also carried
`local-embed` since 2026-09-17, which the same file says in its ONNX bullet. The two statements
contradicted each other inside one file.

## Symptom (Effect)

A reader of that sentence learns a wrong default feature set. The sentence's **conclusion** still
held: no default feature enables `server-stack` (`server-stack = ["dep:qdrant-client",
"remote-embed"]`, and `local-embed = ["codescout-embed/local-embed"]`), and `cargo rb` is
`build --release --features server-stack`. So the defect was a stale premise, not a wrong rule.

## Reproduction

Compare the quoted list with `grep -n '^default = ' Cargo.toml` at any commit after 2026-09-17 and
before `c67f4552`.

## Root cause

The sentence restated a value that `Cargo.toml` owns. When `local-embed` joined `default`, the
change updated the ONNX bullet and not this copy. A copied value decays silently; no test read it.

## Evidence

Found 2026-09-29 while reviewing Codex's derived intervention labels
(`docs/research/2026-09-29-codex-system1-derived-adjudications.md`, row CTL3-7). CTL3-7 is a
control passage in `docs/evals/rule-tell-controls.md` chosen because nobody ever corrected it.
Performing the label's suggested check (inspect the feature expansion) showed that the passage was
already stale at the controls' own tree. So "never corrected" did not mean "correct" here.

## Fix

`c67f4552`, patch-id `05a947cd94029f7bfa9fc2df9b45b22ef34bc4ca`. The sentence now names
`Cargo.toml` as the source instead of copying the list, so there is no copy left to decay. The
phrase `so the four commands never` is unchanged because the controls corpus uses it as a pickaxe
needle.

## Tests added

None. Nothing is copied any more, so there is no value for a test to compare. The conclusion
itself stays guarded by `every_declared_feature_has_a_lane_or_a_reason` (`tests/feature_lanes.rs`)
and CI's `test-server-stack` job.

## Fix provenance

- **SHA:** `c67f4552ffbe8a59e1c11373832155a5b4d97c84` (`experiments`)
- **patch-id:** `05a947cd94029f7bfa9fc2df9b45b22ef34bc4ca`

## References

- `docs/evals/rule-tell-controls.md`, passage CTL3-7 (frozen at the controls' tree; not edited).
