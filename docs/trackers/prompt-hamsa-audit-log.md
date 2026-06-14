---
id: '59ebeebb6ed05c89'
kind: tracker
status: draft
title: Prompt Hamsa — Audit Log
owners: []
tags:
- prompt-hamsa
- prompt
- audit
topic: null
time_scope: null
---

# Prompt Hamsa — Audit Log

One row per audit the Hamsa produces (spoken or written). Each row records the
named **gap**, the recommended **move**, and the **prediction** (what the move
should change). `Outcome` starts empty and is filled when evidence later
arrives — the rewrite shipped, the eval ran, the behavior changed or did not.
The log is how an *unverified, N=0* inspection becomes a measured hold-rate.

Audit IDs are `A-N`, monotonic, never reused.

## Index

| ID | Date | Artifact | Gap (1-line) | Recommended move | Prediction | Confidence | Outcome |
|----|------|----------|--------------|------------------|------------|------------|---------|
| A-1 | 2026-06-14 | `source.md` Iron Law 1 (`server_instructions` slice) | "NEVER read_file source" forbids the only tool that reads imports/glue; `symbols` can't return them | Scope `NEVER` to "a whole source file" + line-range carve-out; push contract to `get_guide("iron-laws-detail")` | Model picks line-range `read_file` for import/glue intents; full no-range large-source reads drop; no regression on body-reads | medium | **held + shipped** — pre-ship B 90% vs A 30%; shipped tight wording re-eval **100%/100%** (disc/controls, 2 runs); gate green; uncommitted |

## A-1 — Iron Law 1 over-absolute: forbids `read_file` for imports/glue that `symbols` cannot return

**Symptom:** Iron Law 1 ("NEVER read_file source code") produced a false-positive whistle this session against two legitimate `read_file`-on-`.rs` calls, one reading for imports. Evidence of mis-routing: across 4 projects (codescout, backend-kotlin, eduplanner-ui, MRV-poc) 82–94% of source reads are line-slices; `symbols` returns 0 import lines in Rust/Kotlin/Python.

**Prompt under audit:** `src/prompts/source.md`, `server_instructions` slice, Iron Law 1 (L7–8). Current: `NEVER read_file source code → symbols(path) for overview, symbols(name=..., include_body=true) for bodies.`

**Read-as-stranger gap:** Stranger reads "NEVER read_file source" as absolute; for an import lookup the offered replacement (`symbols`) returns nothing and no other route is named. The law forbids the only working tool for imports/glue/macros and supplies no alternative — Heuristic 1 (pure "don't X" with an incomplete "do Y").

**Decoration to cut:** none in the current one-liner. In the first draft, "the AST omits" → "symbols omits" (tie to the tool the stranger calls).

**Contract missing:** the 2200-byte slice cap cannot hold the full contract (symbol-overlapping ranges auto-redirect; `force=true` bypasses; full large-source read → outline). Pin it in `get_guide("iron-laws-detail")`. `read_file`'s `description()` already states the redirect+`force` contract — dialect-audited, leave unchanged.

**Placement defects:** surface header is `## Iron Laws (never X, do Y)`; laws 2–4 are genuine `NEVER X → Y` prohibitions. Law 1 is a *routing* decision forced into the prohibition mold. Keep the frame, scope `NEVER` to "a whole source file."

**Eval status:** absent (N=0). Gap is evidenced; rewrite *efficacy* is unverified. Proposed eval: ~8–10 graded source-read intents (import lookup, function-body read, macro-impl read, whole-file browse) scored old-law vs new-law on tool selection.

**Recommended next move:** scope `NEVER` to "a whole source file" + append line-range carve-out; move the contract to `get_guide("iron-laws-detail")`. Measure the slice byte count on current HEAD before choosing whether the carve-out fits the slice or moves entirely to the guide.

**Prediction:** post-change, the model chooses line-range `read_file` for import/glue/macro intents instead of dead-ending at `symbols`; full no-range large-source reads drop; no regression on body-read intents. Falsified if tool-selection accuracy does not move on the graded set.

**Confidence:** medium (high on the gap; medium on the wording — the "whole source file" scoping is a hypothesis about the stranger's reading of "whole").

**Outcome:** **held (measured 2026-06-14).** A/B, slice-only, 10 intents (5 discriminators / 5 controls), 2 fresh subagents per arm, pre-committed ground truth. **Discriminators (imports/re-exports/macro/exact-bytes/kotlin-package): Arm B 9/10 (90%) vs Arm A 3/10 (30%).** Controls: Arm B 10/10 — NO over-route to `read_file` for bodies/overview/references (the flagged regression did not occur); Arm A 9/10 (one whole-file over-read). Prediction confirmed. Caveats: N small; one model; current law-A injected ambiently into all arms (conservative for B — it won despite fighting its own ambient). Finding: Arm A is *unreliable* — a literal reading scored 0/5 discriminators, a rule-defying reading 3/5. Residual: `imports` is stickiest — one Arm B run still chose `symbols` for intent 1, so the slice MUST keep the literal word 'imports'. CAVEAT: the tested Arm B wording is the explicit/longer form; if the 2200B cap forces trimming, the trimmed wording is re-N=0 (re-test or move detail to the guide). **RE-EVAL of shipped tight wording** (`NEVER full-read source → symbols… Line-range read_file is fine for imports/glue.`, slice-only, 2 runs): **discriminators 10/10, controls 10/10** — exceeds the pre-ship explicit wording (9/10); re-N=0 gap CLOSED. Macro (#5) + exact-bytes (#6) routed correctly though unnamed in the slice (generalized); caveat: #5 likely aided by the eval's tool-blurb mentioning 'AST-extractor drops', but #6 generalized from wording alone. Gate green: 87/87 prompt tests; `source_md_under_cap` 2167<2200 (33B headroom); snapshot regenerated. SHIPPED to working tree (uncommitted); no `ONBOARDING_VERSION` bump (server_instructions is live-on-connect). Guide `iron-laws-detail` Law 1 reframed (overlap-gate, read_file-correct-not-rare, force=true, evidence cites).

**Cross-refs:** Pika `U-27` / `H-7` (codescout-usage trackers, same investigation); recon `R-32`; sibling `F-22` (read_file offset/limit → line-slice normalization, reinforces sliced-read legitimacy).
