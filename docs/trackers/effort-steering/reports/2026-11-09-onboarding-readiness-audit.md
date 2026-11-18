---
id: 0254ee27013a7638
kind: research-note
status: active
title: 'Onboarding readiness audit: native capability filtering and reusable preparation'
tags:
- effort-steering
- onboarding
- native-readiness
topic: effort-steering
---

# Onboarding readiness audit — 2026-11-09

## Finding

Confirmed the reported mechanism in one fixed native MCP process: the public Rust fixture initially had languages [] and advertised 18 tools. After one programmatic onboarding() call, it had markdown/rust and advertised 21 tools, including references, symbol_at and call_graph. Actual references found the public caller; symbol_at resolved the function definition; call_graph returned the caller edge tagged lsp. A different executable is unnecessary to explain this controlled result.

Current source agrees: CodeScoutServer/current_capabilities derives has_lsp from the active project's configured languages and registered server configurations; list_tools filters the advertised surface using those capabilities. That predicate establishes eligibility, not that an LSP server starts successfully.

The pinned native executable reports build e6f07e9a6 and SHA-256 cdc53728d28a659b5a697048e6e5b719149693df2cf57cb8554ee0751ebf51bb, inside image sha256:3767f31b9b1a3288ad7132740dcff712737d5e855029079a4171a8065a2040ef. No image build/pull or provider call was made.

## Onboarding and index reuse

Programmatic onboarding alone left index.status=not_indexed. It also selected local:AllMiniLML6V2Q, rather than the study's pinned local:JinaEmbeddingsV2BaseCode.

A separate owned container copied the public fixture/config, selected the pinned existing local model and indexed it once. It made zero programmatic onboarding calls and zero onboarding-author/provider calls. Native index verification returned complete, expected_files=3, stored_files=3, chunks_without_vectors=0. Semantic search returned both actual Rust function bodies; all three LSP canaries still succeeded. Both owned containers were removed and independently confirmed absent.

This is a runtime control, not an authored source-knowledge corpus, a Claude instruction-delivery proof, a repair outcome, or a savings estimate. No old sealed root was activated, indexed or modified.

## Historical inputs actually audited

Read every root configuration in the six-arm semantic-contract registration: three arms on Nushell 18863 and three on rust-analyzer 22992. All six contain rust, onboarding_version=31, and exactly match their registered launch configuration hashes. Each retains eight root Markdown memory files: architecture, conventions, development-commands, domain-glossary, gotchas, language-patterns, onboarding and project-overview. Root system prompt, project CLAUDE.md and global CLAUDE.md hashes also match the launch registration in each arm.

The later baseline-first oracle low/xhigh configurations also contain rust. Their complete retained API-request-file populations were inspected for advertised names: low 16 files, xhigh 54 files. References appears in xhigh's advertised API surface. Deferred tool advertisement means absence in low's API tool list does not establish MCP unavailability.

These are input audits, not retrospective proofs of every active scope, LSP operation, memory fact or prompt delivery. The empty-language omission found in the other study is not established for these six registered configurations. Keep prior outcomes and validity limits unchanged; do not rerun or discard closed experiments on this evidence alone.

## Generic prevention under implementation

The new isolated adapter requires an explicit backend mode. Native mode consumes host-captured readiness before any actor call and binds its hash into started.json. Readiness checks executable/image/source/common-knowledge identity, actual root and source languages, complete MCP inventory, successful public-source LSP and semantic canaries, and native index verification of files, vectors and memories. Scripted controls identify themselves explicitly.

All 17 readiness tests pass. Three missing-index-verification rejections were observed failing before the guard was tightened, in addition to the earlier consuming adapter regressions (two failures/one missing-key error). The actual captured native wire, including index verification, passes the final validator.

The final combined suite discovered 104 tests: one native CLI skip and one Docker cleanup timeout in test_real_receipts_gate_the_transition; the other 102 executed tests passed. The exact failed test passed without any source change in a separate 2.369-second recheck. The original full-suite error remains preserved; its precise daemon/operation cause is unresolved, issue 5e14fca2462c0c10.

The required product gate also remains red: Python light/FMT/CLIPPY/LEAN exited0; DEFAULT reported7881passed/1failed/14ignored. The failure is the existing attribution directory-swap fallback on the pinned fe8 source, already documented as pre-fix research-source behavior in issue87cfa556ce471ef7. This does not show a regression on current experiments, and a targeted pass does not turn the gate green.

Code remains uncommitted in /home/marius/.codex/worktrees/cf66/codescout on research/evidence-protocol-20261109. The three-phase native CLI backend is not implemented yet; effective global/project/system prompt delivery, independent applied-mutation review and final admission remain required. The successful MCP runtime control does not admit a paid run. Reuse source-only onboarding once per project/version; do not repeat the LLM author merely because runtime configuration needs correction.

## Evidence

[Compact evidence archive](../../../research/data/2026-11-09-onboarding-readiness-evidence.tar.gz), 44 entries, 115023 bytes; SHA-256 3fd8732734ad8092408bed196f98a2b07135728c1986573bf638464bc85cbbd7. [Per-file manifest](../../../research/data/2026-11-09-onboarding-readiness-manifest.json). Includes actual native replies, public fixture/configuration, control scripts, archived-input audit, guard source and observed red/green logs. Excludes credentials, old raw API histories, model/cache copies and SQLite stores.

Related local issues: 4522e8f1e6d6495e (missing phase admission), 6a0d42f442180385 (test fixture aliasing). The historical source snapshots and their costs are unchanged.

## Final evidence checkpoint

[Final evidence archive](../../../research/data/2026-11-09-onboarding-readiness-final-evidence.tar.gz), 50 entries,361432bytes; SHA-25668e0350e907a66cbe01e2c9dbefba1c40ec9efb7f2870e09aae77431b7f4df04. [Final per-file manifest](../../../research/data/2026-11-09-onboarding-readiness-final-manifest.json). Every listed member hash was verified from the archive. Includes the final index guard/source, actual native readback, failed full suite, separate isolated recheck and the failed product gate. The earlier compact archive remains the pre-tightening checkpoint; it is not overwritten.
