---
id: a61dcd3c21709b76
kind: spec
status: active
title: Call graph multi-hop declaration identity — bounded repair contract
tags:
- ecc
- codex
- call-graph
topic: ecc-integration
---

# Multi-hop declaration identity — repair contract

## Scope and provenance

Frozen base7f0dbba5fd6ba25e4a50a8aad5c25403a11ae345, clean isolated .worktrees/ecc-bfs-identity-20261110. Installed ecc-spec-miner and ecc-silent-failure-review explicitly used; baseline captures their bodies/hashes,12 inspected inputs, replay ref and a private exact pre-action host prefix. No replay. Existing related module executed84passed/0failed. Main checkout has unrelated staged/dirty peer documents; they are excluded from source work.

## Observed implementation

Requirement: CallGraph.call pins a seed selected uniquely in the requested file; its position/direction scope completed adjacency cache lookups. Enforcement: CallGraph.call and CachedResolver.one_hop; code/test inspected,84-test baseline executed-pass. This is observed behavior, retained as a regression boundary.

Requirement: bfs expands each short name once, depth by depth, caps whole levels after the first and deduplicates displayed call sites with path counts. Enforcement: traversal::bfs and dedupe_with_paths; code/tests inspected and baseline executed-pass. Short-name identity is an observed defect, not an intended contract.

Requirement: recoverable analysis failures are disclosed through completeness_warning without discarding successful edges; trustworthy leaf empties remain quiet. Enforcement: CachedResolver.one_hop, attach_analysis_failures and CallGraph.format_compact; code and actual assertion bodies inspected, baseline executed-pass.

## Intended repair requirements

R1. Distinct declarations with the same display name expand independently. Queue/visited keys retain symbol and canonical definition file/line/column, separately from the provider query position; repeated reachability of the same declaration still expands it once. Preserve cycles, level parallelism, depth-coherent caps and displayed path counts.

R2. LSP hierarchy endpoints carry their declaration positions into next-hop expansion, including after a completed-cache hit. Use incoming.from/outgoing.to selection ranges, separate from existing displayed edge locations. Legacy completed LSP entries missing target identity must be recomputed; corruption remains an error.

R3. AST-only Rust calls through explicitly declared ordinary modules, including the retained a::helper/b::helper topology, resolve with call-site/module context to different declarations. Same-file ordinary module/type declarations and unique workspace fallbacks keep working. Local imports, callable values, nested items and generic type bindings require a semantic provider; without one the fallback discloses incomplete placement. Do not simulate type inference, alias resolution, macro expansion or overload selection from a short name.

R4. If next-hop identity is ambiguous or lookup coverage is incomplete, preserve the verified current edges and disclose the symbol/context/uncertainty. A complete fallback scan that finds no declaration remains a quiet miss, preserving the existing trustworthy-negative contract. Do not pick the first of multiple declarations or mark an unexamined scope as a complete miss. No next-hop warning is required when the requested depth stops before that expansion.

R5. Public tool arguments, edge rendering, direction handling, cap values and valid empty-result behavior remain stable. No new prompt routing or profile changes.

## Approaches and controller choice

A warning-only fix would be smaller, but retains the identity loss even for explicit module-qualified calls. A full cross-language name resolver is substantially larger than this bug and would require language-specific semantic engines. Chosen: declaration-keyed traversal, exact LSP targets and a bounded AST context resolver; unsupported/ambiguous cases disclose incomplete analysis. This is the controller's implementation choice under the user's continuing authorization, not a separately solicited product requirement.

## Limits and unknowns

The prior seven-case native fixture is all tree-sitter; it does not qualify general LSP semantics. Fresh source-bound native baseline/candidate execution and mocked wire endpoints will test distinct layers. Existing aggregate edge rows do not carry target metadata; only completed-resolution payloads drive this traversal. Prior ECC conditioning/solutions are controller-only for any future replay. No general ECC efficacy or latency claim.
