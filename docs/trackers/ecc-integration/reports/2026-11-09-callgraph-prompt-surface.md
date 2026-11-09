---
id: '34ef26ad7bbe051f'
kind: report
status: done
title: call_graph prompt surface — current guidance and native Codex routing
tags:
- ecc
- codex
- call-graph
- prompt-surface
topic: ecc-integration
---

# call_graph prompt surface audit — 2026-11-09

## Result and decision

Keep current call_graph routing. The preregistered [A-47 protocol](../../prompt-hamsa-audit-log.md) reached its satisfactory-base branch:10/10 chain tasks completed the correct transitive callers traversal and returned the exact affected public endpoints;3/3 direct-use controls used references first and returned the exact call/value-use names. No routing treatment ran and no quickref expansion is supported by this experiment. One separately retained calibration smoke is outside those denominators.

Corrected one factual rule in the stored project prompt: write responses should be compact acknowledgements or operation metadata without echoing file contents. The former json!(ok)-only statement did not describe the actual MCP-visible response layer. A private real-MCP probe observed create_file returning status/rel_path and doc(create) returning id/abs_path. CreateFile::call may still return json!(ok) internally: annotation/rendering adds metadata externally. No behavior-improvement claim is made for this wording correction.

## Active surface inventory

| Surface | Current role | Evidence |
|---|---|---|
| src/prompts/source.md server slice | Who-calls→references; points to symbol-navigation guide | Actual initialize.instructions prefix in every retained run |
| source.md onboarding slice | Requires call_graph chains/impact exploration | Current source inspection; unchanged |
| build_system_prompt_draft in builders.rs | Single/multi-project navigation includes references and call_graph callers/callees | Current symbol bodies inspected; unchanged |
| .codescout/system-prompt.md | Navigation step4 promotes blast radius/flow; corrected write-response rule | Exact Navigation Strategy stored as fixture AGENTS input; host consumption of every AGENTS byte is not independently attested |
| symbol-navigation guide | references for call sites, call_graph callers for impact/callees for flow; depth3/default,10/cap | Exact current guide text appears in completed tool results |
| CallGraph description/schema/guide trigger | Transitive graph; callers/callees/both, defaults and limits; symbol-navigation guide, overflow→progressive-disclosure | Actual tools/list metadata matches preflight in all13 runs; source bodies inspected |

The related active text inspected above states current behavior; no historical routing paragraph was found to remove. Focused literal history scans and exact source hashes are retained in [source-inventory.json](../evidence/2026-11-09-callgraph-prompt-surface/source-inventory.json). A literal scan is not a semantic proof about all tool descriptions, hints or unrelated guides. Current vocabulary about stale records is not incident history. The old A-26 record and its regression-test commentary remain in contributor/evaluation material; they are not active routing instructions. The existing test against re-adding refuted bare quickref text remains intact. Static slice1698 characters remains below the1900-character cap. No cap or onboarding template/version changed.

## Measurement and independent review

Configured native Codex gpt-6.1-sol/high, CLI0.167.0; each run used a fresh temporary CODEX_HOME with only its private auth copy and generated MCP/model config, no copied plugins/skills/personal AGENTS. This tests one small Python fixture plus the current generic Navigation Strategy and actual MCP instructions/tool metadata. The fixture separates three-hop impact from a direct call and a non-call value binding. The full fixed batch ran regardless of smoke result. Tasks, fixture, generated-config hashes, immutable binary SHA, raw events and preflight are retained. Requested model/effort are recorded configuration, not provider-resolved backend attestation.

[Independent review](../evidence/2026-11-09-callgraph-prompt-surface/review.txt) recounted all13 raw events/wire files:79 completed navigation calls, all qualifying graph calls on codescout, callers/max_depth10, all four expected edges at depths1/2/3, exact final answers. First relationship tool was call_graph7/10 and references3/10 in chain tasks. The registered endpoint is successful completed traversal plus answer, not call_graph as opening move. Controls were references-first3/3.

Wire capture records initialize and tools/list only; actual completed call arguments/results come from native Codex events.jsonl. Generated profile/config strings are hashed; temporary profiles are deleted and no full before/after profile-files inventory proves immutability. No default profile configuration changed. This is neither the full default-plugin profile nor a causal ECC A/B, a production-code benchmark, or the deferred no-ECC repair replay.

The reviewer actually perturbed copies of raw06-chain events and found two scorer false positives: wrong server and endpoint names without edges. The original scorer and observations are preserved. Hardened scoring binds server=codescout and requires the four real depth-qualified edges in the primary tool result. [Observed re-score](../evidence/2026-11-09-callgraph-prompt-surface/rescored.json):16 controls pass, both formerly surviving perturbations rejected (0/2 survive), all13 original traces retain their result. These are input-trace perturbations, not Cargo source-mutation verdicts. The fixture-specific scorer is not a general graph evaluator. Frictions:bug-fix-session-log:F-220 (stale positions, mitigated) and F-221 (scorer gap, fixed-verified).

## Verification and integration

Source base018bbfc49d679c7b5247f5dbf6f29a5e17fbc117. One-line source commit877003e9907a8a104d890e9e3f0b1b0d61748899, integrated locally on experiments as6586622a30f0d790db17b1bf234aa7962837f510. Prompt file SHA256cc80985e14fbfa7ee6bd3716748b3832e6877cd5d5e7d15afea91dc153e95e29 matches before/after verification. Stable patch-id05cb71e8fcfa8b7513ecf3a5904847d211da967a matches both commits, recorded in integration.json.

[cargo test --lib prompt](../evidence/2026-11-09-callgraph-prompt-surface/prompt-tests-result.json):201 passed. [Frozen source gate](../evidence/2026-11-09-callgraph-prompt-surface/gate-result.json):exit0, FMT/CLIPPY/LEAN/DEFAULT all0; lean6314 passed/48 skipped, default9203 passed/62 skipped; Python1764 passed/1 skipped/554 subtests. External scratch with optional /tmp override disabled. This gate verifies the isolated base plus one-line prompt change; peer Python/tool changes entered main afterward. The [exact integration gate](../evidence/2026-11-09-callgraph-prompt-surface/integration-gate-result.json) subsequently passed at commit6586622a30f0d790db17b1bf234aa7962837f510/tree4b4b065297d3911fe88808341551e711f3e25ddd:exit0, FMT/CLIPPY/LEAN/DEFAULT all0; lean6316 passed/48 skipped, default9205 passed/62 skipped, Python1764 passed/1 skipped/554 subtests. This includes those intervening committed peer changes. Raw-data re-score and source-inventory checks also exit0; documentation added afterward receives separate link/frontmatter and ownership checks.

No push, release build, shared-MCP rollout or repair replay. This existing conversation's initialize instructions were not refreshed by the file edit. New conversations receive newly built static instructions; stored-project guidance is loaded through its normal workspace lifecycle.

## Evidence and reproducibility

[Evidence directory](../evidence/2026-11-09-callgraph-prompt-surface/), [base adjudication](../evidence/2026-11-09-callgraph-prompt-surface/base/adjudication.json), [instrument and raw-data re-score](../evidence/2026-11-09-callgraph-prompt-surface/rescore.py), [source inventory](../evidence/2026-11-09-callgraph-prompt-surface/source_inventory.py). Run rescore.py to validate the retained traces without new model calls; source_inventory.py binds reviewed committed surfaces to preflight/native guide delivery. run_base.py intentionally launches new model runs only when explicitly executed. Do not supply solution/evaluation artifacts to a future blind repair replay; earlier repair baselines remain unchanged.


Native batch runtime records the controller checkout HEAD acf72c2d6d8705762250d5150bf96a011a03ce04, independently of the isolated source-edit base018bbfc49. Relevant source hashes and delivered metadata/guide bytes are retained, rather than claiming the whole concurrent checkout was frozen. The ignored owned executable snapshot is identified by binary-snapshot.json (SHA256a73d3df5eeedd66e91376ac43ff9e931827fbf87c25841da67e2715142638699); the binary itself is retained locally outside versioned evidence. No exact compiled-from-HEAD claim is inferred from that hash alone.
