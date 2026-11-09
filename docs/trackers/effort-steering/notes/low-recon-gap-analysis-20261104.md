---
id: '841bd65f93e02b1d'
kind: research-note
status: archived
title: Low/Recon gap analysis — 2026-11-04
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/low-recon-gap-analysis-20261104
original_sha256: 610cc2336b3c34b8df0561b3e972493851890cf25b77a1fd49c9e358d17e0a90
---

# Low/Recon gap analysis — 2026-11-04

Follow-up to CLOSED C causal-diagnosis and D recon-factorial. User asks generic mechanism for Codescout/Recon/another skill, explicit task/high-vs-low comparison and why low+Recon appears worse. Investigation is read-only over sealed roots, no paid call/new skill/config/code change. No claim generic fix validated. Mactive, V retained, no push/merge. Dresult and LMC-3 unchanged; current closed recovery remains authoritative for budget/seals.

## Direct artifact findings

C task issues/actual high candidate patches/full assistant tool traces read again. RA22849 input macro expands s.00E+10 and panic occurs in float_split on no-dot float literal. High reads parser shortcuts body, token conversion/event flow and sink methods, reports parse error AND emits ERROR/FLOAT_NUMBER, preserves token text/node closures and valid tuple chains. Writes two tests, one for invalid literal error node+diagnostic and one for valid tuple chain. Does NOT execute them: no shell/callabletest in worker. Dplainlow also reads canonical body and succeeds. DplainRecon receives successful Grep result toolu_01BjaJCsJQFiWBmL6xaHUbcG naming parser/src/shortcuts.rs/do_float_split explicitly, but never Reads that body; reads grammar/parser interface and emits NAME_REF/INT_NUMBER without diagnostic. Original snapshot fails. Ddiagnosislow reads shortcuts yet omits diagnostic, claims sink absent; diagnosisRecon additionally inspects real sink error() and succeeds. This separates discovery, body inspection and semantic use; merely reading a file is insufficient.

Bindgen3390 issue macro #define FooBar0x00000000 with default_alias_style(NewType) gives invalid u32(0). Need primitive literal unwrapped while real alias-newtype values remain wrapped. All4Dlow variants produce identical patch4727e0798d4d5f25c96fba327f55f616d89a610d043728aaa9b2f4c921e7473a: guard TypeKind::Alias on direct ctx.resolve_item(var_ty), no reference traversal. High inspects alias_style, neighboring code and resolver functions via actual successful Grep, uses into_resolver().through_type_refs().resolve(ctx) BEFORE Alias/style check, and writes primitive-constant regression fixture. High correctly follows references without flattening genuine aliases; following through_type_aliases would be a different semantic operation. Controller fixed checks pass high, all low fail original header_new_type_alias_h and hidden real-alias-newtype. High writes tests but does not run them.

C high and Dplain-control prompts byte-identical per task (7453RA/2664bindgen bytes), production base/oracles same; D uniformly adds7skillassets to everyactor. Actual trace toolnames onlyRead/Grep/Glob/Edit/Write (no MCP, Bash or test tool). Thus no native Codescout treatment was evaluated; do not infer Codescout failure or nativeRecon benefit from D. Existingfullraw source/effort/authority audits retained, no re-execution of sealed helpers.

## Recon apparent deterioration

Dplain control1/2 vs plainRecon0/2 differs only RA; bindgen both fail. In diagnosis stratum direction reverses0/2to1/2. Earlier CplainRAfails while DplainRApasses; CdiagnosisRApasses while DdiagnosisRAfails. One sample per task/config cannot establish stable adverse causal effect. Observed endpoint difference is concrete, cause of differing trajectory not identified. Do NOT claim added instructions reduce reasoning or fatigue without ablation/repeats.

Raw skill14020bytes + adapter2325bytes (file bytes, NOTtokens), all4treatments successfully Read before editing; no successfulreferenceReads/localF/Wlogevents,0/4fullrequesteddiagnosespreedit. Skill focuses on current shape, caller reads conditional on shapechange and stop after an answered shape question. Text includes significant unavailable harness bookkeeping even with adapter skipping it. Hypotheses: semantic assumptions remain unexamined when shapes match; stopping rule may license early closure; long procedural context may compete with task context. Only first is directly visible as unchecked behavior, latter two are untested causal explanations. The skill's own counterfactual clause already exists; repeating it or adding more prose is not evidence of behavioral enforcement.

Public RA no-panic/text contract misses diagnostic/AST mismatch; public bindgen cases are primitive and explicitly plain alias, missing preservation of real newtype alias. Public8/8pass versus independent2/8accept -> repair never triggers. This is a controller/detector coverage gap as well as agent behavior. Do not reveal frozen hidden cases retrospectively; any stronger prospective public checks must preserve separate final evaluation.

## Generic candidate, not implementation

Recommend testing a short semantic-contract step, separate from logging-oriented Recon, using existing tools first. Before a new guard/recovery branch: identify one correctness-critical assumption; read actual producer/representation plus a working sibling/consumer; derive a case that must change and one that must stay correct from real code; state a discriminating check and unresolved facts. For a predicate, verify both sides through actual representation/lookup path, not hand-waved input types. For recovery, include observable outputs, diagnostics/side effects and state invariants. No task-specific names/answers in candidateprompt.

A controller can enforce evidence artifact before Edit (source SHA/range+observed read+two cases+check status) and check execution availability; it cannot verify semantic truth merely because fields are filled. Unknown evidence must lead to further bounded investigation or explicit uncertified status, not 'matched plan' as a correctness claim. Runtime validation/repair consumes budget and must be counted; success cannot be declared from generic process compliance.

Codescout direction: orchestrate its existing navigation into a small on-demand packet of changed function, relevant value producer, comparable consumer/sibling and nearest tests; verify actual tool capabilities before implementing a new tool. Do not automatically dump entire repo or pretend LSP proves value semantics. Low selection from already-found matches, not search access alone, was a measured bottleneck here.

Validation recommendation: current two cases are exposed development/negative controls only. Freeze concise generic candidate and acceptance rules; compare fresh repeated lowbaseline vs lowcandidate with same tools and matched high as benchmark on new cases. Separate skill-content ablation from controller/tool changes; hold tools constant when testing skill and charge whole workflow. Measure complete acceptance, regression preservation, costs and actual assumption checks; opening skill/word DIAGNOSIS/call count are secondary observations. No new paid run authorized by this analysis itself.

## Evidence anchors

C=/home/marius/.cache/codescout-resident/codescout/causal-diagnosis-20261103: both public-issue.md files, arms/A-P-X-I/final/candidate.patch, arms/B-P-X-I/final/candidate.patch, exact-session transcripts and original-test patches. D=/home/marius/.cache/codescout-resident/codescout/recon-factorial-20261103: all8trajectory-observations, candidate patches, skill-assets/SKILL.md and HARNESS.md, contract-cases-v2.json and paid-independent-audit.json. Reportbcc6e5c99f2bd2db/LMC-3 retain full outcomes. All old seals immutable; only canonical close.py verify allowed to execute there.
