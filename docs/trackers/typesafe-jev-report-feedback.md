---
id: '0517d18ca272e132'
kind: tracker
status: draft
title: TypeSafe/Jev report feedback
tags:
- reflective
- report-feedback
- typesafe
topic: local-semantic-evaluator
---

# Feedback on the TypeSafe/Jev × codescout brief

**Purpose.** A high-level review for the author of the 17 September 2026 brief. This records corrections and research questions; it is not approval of a model, integration, or pilot. The full evidence review is [Local semantic decisions for codescout](../research/2026-09-18-local-semantic-evaluator-review.md); the internal follow-on is the [living design brief](local-semantic-evaluator-design.md). The original PDF remains a machine-local source, so this tracker cites durable project and public sources instead.

**Scope correction.** The first review interpreted the opportunity as an advisory local evaluator. Marius's intended design includes selected autonomous investigations, tests, fixes, and multi-step tool use, with context injected at the right phase. The current [deep-agent design](local-semantic-evaluator-design.md) supersedes that product ceiling. The factual corrections to vendor comparisons, confidence, and historical retrieval evidence below still stand. Local inference is preferred where suitable; an external API is also acceptable under an explicit data/provider policy.

## What the brief gets right

The useful idea is the interface discipline: assemble bounded state, ask a typed semantic question, and let code own composition and consequential actions. That matches [TypeSafe's introduction](https://docs.typesafe.ai/introduction). Codescout already has structural search, artifact retrieval, memory anchors, and librarian evidence gathering; a narrow advisory decision could help an investigator choose which precedent or check to inspect next. The brief properly marks vendor cost, speed, and stability figures as unvalidated on codescout.

The corpus strengthens one part of that intuition. [R-101](reconnaissance-patterns.md) records a wrong-sign interpretation of an experiment despite the result being available, while [claim-decay DC-4](claim-decay.md) records a conclusion that stayed in prose after maintained premises changed. These are plausible places to ask a bounded question over supplied evidence. They do not demonstrate that a small classifier can solve either task, nor that the pattern is frequent. The simpler candidate is ranking prior incidents worth reading; [archived R-32](archive/reconnaissance-patterns-archived-entries.md) is a useful warning against treating similar titles as the same root cause.

## Corrections to make in the report

1. The [parallel-questions cookbook](https://docs.typesafe.ai/cookbooks/parallel_questions) reports batching Jev questions versus making separate Jev calls. The brief's wording makes the 12.2× cost and 10× speed result sound like a Jev-versus-LLM comparison; amend the comparator. The [consistency cookbook](https://docs.typesafe.ai/cookbooks/consistency_noul_cookbook) uses one insurance claim, 14 questions, and repeated runs. It measures that example's output stability, not code-domain accuracy or calibration. The [legal reranking example](https://docs.typesafe.ai/cookbooks/rerank_typesafe) likewise does not transfer its ranking gain to codescout.
2. [Confidence](https://docs.typesafe.ai/confidence) is derived from the returned distribution. A stable or high-confidence score can still be wrong. Separate correctness, calibration, repeated-run consistency, and action value. Typed outputs can be labels or rankings; free-form rationale generation is optional and needs its own design if desired.
3. Replace “zero execution risk” and “swap-out trivial” with a narrower claim. The model need not have write access, yet a wrong ranking, suppressed warning, or bad threshold can redirect a review. Keeping access checks and writes in code limits direct authority; it does not erase downstream risk. A vendor swap also needs an output contract, task labels, calibration/threshold reevaluation, and a failure fallback.
4. “Closed-source, cloud-only” is too absolute based on the checked public material. The [public organization](https://github.com/typesafe-ai) shows SDK/adapter material but does not establish downloadable Jev weights; specific private hosting terms were not exhaustively checked. Marius prefers local, controllable inference. An open pretrained model is a plausible path, with license and provenance still to review.

## What codescout's own history adds

The strongest counterevidence to an easy reranking pilot is the [later clean A/B in the reranker issue](../issues/archive/2026-07-28-reranker-costs-42x-latency-and-lowers-score.md): in one historical 25-case setup, rerank ON scored 23/75 versus OFF 26/75 and added warm median latency. Other [retrieval benchmark](retrieval-benchmark.md) changes improved results through code filtering, title-bearing chunks, and candidate diversity; title-prefix effects were confounded by re-embedding. These are reasons to freeze retrieval candidates and compare against metadata, lexical search, chunking, and existing reranking before crediting a new judge.

The archive also contains many cases where no learned judgment was needed. [U-17/U-40](codescout-usage-frictions.md) favored path-scope and escaped-newline diagnostics; [IC-13](issue-clusters/IC-13-capped-result-presented-as-complete.md) needs a truncation marker; [OB-24](observer-blindness.md) needs an independently enumerated denominator. The [PR-review log](pr-review-session-log.md), `pr-review-session-log:F-4` and `pr-review-session-log:W-3`, shows why passing author tests can coexist with later adversarial failures and false positives. These records are evaluation material, not clean labels: later diagnoses, fixes, and cross-links leak answers.

## Suggested revision and follow-up

Reframe the original report as an input to a lightweight codescout deep agent, not as a proposal for a universal semantic score. Typed judgments may select a workflow, rank evidence, or decide which context to present. A tool-using model—local or an explicitly configured API—can then inspect, test, patch, and retest through codescout's existing tools under a preauthorized capability profile and an independent task verifier. Compare a native single-worker loop with a thin framework adapter before adding a planner/critic hierarchy. Keep the existing corrections: vendor performance needs a codescout-specific measurement; historical trackers require time-frozen inputs and independent outcomes; deterministic parser/instrumentation repairs remain the right answer for exact defects. The next concrete experiment should exercise a bounded substantive workflow in an isolated workspace, with a real pre-fix observation and post-fix oracle, while measuring context timing, tool use, recovery, and handback cost. The [deep-agent design](local-semantic-evaluator-design.md) owns the execution architecture and open decisions.
