---
id: '80144119fec89964'
kind: spec
status: active
title: Prospective ordinary-linker classification
tags:
- effort-steering
- evaluation
topic: effort-steering
---

# Prospective ordinary-linker classification

## Intent

Before the eight-attempt pilot, distinguish an ordinary linker build failure from missing measurement or a fatal toolchain crash. This addresses F6 issue2f8500976a76db35 without changing historical tail-only replay, F3 missing-status policy or any frozen grade/result.

## Approaches and decision

A shared broader compiler regex would change historical replay semantics. Another model/rebuild retry would not repair lost attribution and adds cost. Choose a prospective-only ordinary Rust linker diagnostic after existing complete-stream/runner qualification. This is the implementer’s call under the operator’s standing autonomy instruction; no recurring design approval or publication.

## Behavior

A top-level canonical Rust error header linking-with-command failed with a positive ordinary exit status can establish compile_failure on a failed rebuild/test step without source spans. It never establishes pass, never overrides the earlier infrastructure check, and cannot use captured test payload or ambiguous post-capture text. Unknown/nonmatching text remains invalid where already required. Legacy adjudicate_private/compiler_diagnostic behavior remains unchanged. Prospective policy identity increments to semantic-stream-contract-v2 so an old registered verdict cannot silently acquire new semantics.

## Verification and limits

Tests must exercise the complete consuming adjudicate_streams entry point, capture binding and actual return codes: rebuild/test linker failure, stale selected pass overridden, successful step unable to invent failure, captured payload/ordinary text not compiler evidence, fatal signal/storage still invalid, and legacy old behavior retained. Generate a dependency-free undefined-symbol Rust control on BOTH frozen task images with exact argv/version/complete streams/closed state/OOM0/cleanup. This is a model-free compiler characterization, not a benchmark candidate or efficacy result.

No provider call, new onboarding, task-specific branch, oracle/gold change, merge/push/global config or destructive old-resource cleanup. Preserve current dirty complete-stream work before edits. One strongest final review applies actual mutations; one bounded correction pass, no re-review loop. F6 completion alone does not admit payment or achieve the research goal.
