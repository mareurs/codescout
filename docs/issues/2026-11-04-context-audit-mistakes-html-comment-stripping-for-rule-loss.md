---
id: '353229c45d9c546e'
kind: bug
status: mitigated
title: Context audit mistakes client HTML-comment stripping for missing global rules
tags:
- cluster/unclassified
- effort-steering
closed: 2026-11-04
opened: 2026-11-04
owner: marius
severity: low
unverified: STANDING — Calibration covers one actual client global block and a visible-rule deletion; not a universal Markdown or semantic-adherence verifier.
---

# BUG: Context audit reports missing rules from an exact byte comparison

## Summary
A diagnostic checks the complete raw global CLAUDE.md as a substring of serialized context. The client strips HTML comment wrappers, so the predicate returns false while visible policy lines are present.

## Class fit
The instrument measures raw byte identity and reports instruction delivery, a different property. This private measurement defect does not establish lost policy or license ignoring missing visible lines.

## Symptom (Effect)
client-auto/initial-context-containment.json records global=false and project=true. Full source/readback comparison shows only three nonempty missing source lines, all HTML comments; every nonempty visible policy line is present in the rendered source block. A user-facing claim of missing global text was corrected promptly.

## Reproduction
W=/home/marius/.cache/codescout-resident/codescout/worker-context-verification-20261104. Compare frozen global-CLAUDE.md in native-env-admission-20261104 against the isolated-global Contents of block in client-auto/request-0.json. Keep the raw exact-containment result as a failed instrument observation.

## Environment
Actual pinned Claude serialization; local scripted API, no real model. Comment normalization is observed here, not assumed for every client or markup language.

## Root cause
An exact whole-file substring predicate conflates comment bytes and effective visible rule content.

## Evidence
Actual source block, full source file and explicit missing-line population: BEGIN operator-rules comment, rules OP-1/OP-6 comment, END operator-rules comment. Host global inheritance is a separate real issue.

## Hypotheses tried
Ordered visible-text comparison after removing only HTML comments and blank lines is being calibrated against this actual overlap and a deliberately deleted visible-rule control.

## Fix

Calibrated ordered visible-text comparison removes only HTML comments and blank lines, matching the full actual rendered global block. A synthetic deletion of the visible ALWAYS VERIFY imperative is rejected. Raw exact-byte=false remains preserved. comment-normalization-calibration.json records the actual overlap and negative control; no missing visible global rule was observed.

## Tests added

Actual complete isolated-global block equals the ordered normalized source; deliberately deleting one visible imperative makes equality fail. Raw byte identity and visible policy delivery are reported separately. No generic Markdown transformation or model adherence claim.

## Workarounds
Read the complete rendered instruction block before interpreting global=false as a missing instruction.

## Resume
Calibrate against the actual client and prove that deleting a visible imperative is rejected.

## References

[Actual client context verification and preserved evidence](../research/2026-11-04-worker-context-verification.md). The full continuation report and bundle are in the main checkout.
