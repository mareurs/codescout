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
| A-2 | 2026-06-21 | codescout `CLAUDE.md` (42 KB, injected every session) | Stale dead tool names (`search_pattern`/`replace_symbol`/`insert_code`) + harness↔CLAUDE.md memory contradiction + 4× rule redundancy + ~18 KB reference/forensics resident in a per-session prompt | denylist gate for dead names; de-dup each rule to one canonical home; relocate tracker-protocol + incident forensics to docs/ leaving pointers | dead-name tool calls vanish; CLAUDE.md ~42 KB→~15 KB; no rule-following regression | gap high / cut-benefit N=0 | pending measurement |

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

## A-2 — codescout `CLAUDE.md`: dead tool names, a cross-surface memory contradiction, 4× rule redundancy, and ~18 KB of non-instruction resident in a per-session prompt

**Symptom:** Marius asked the Hamsa to review the codescout session-start prompt as "quite a big prompt." `CLAUDE.md` is ~42 KB and rides into every session as a ~45 KB `<system-reminder>` (it is *not* `include_str!`'d — read from disk by the CC harness; W-8). Four distinct defects found by reading; three are verified facts, one is an unverified-benefit cut.

**Prompt under audit:** `/home/marius/work/claude/codescout/CLAUDE.md` (whole file), cross-read against `.codescout/system-prompt.md`, the `server_instructions` slice, and the generic CC harness `system` block.

**Defect 1 — WRONG (verified): dead tool names.** "Companion Plugin" § lists `search_pattern`; "Design Principles → Agent-Agnostic" names `replace_symbol`, `insert_code`. All three are on the codebase's own deprecated list (`src/prompts/mod.rs` test `rendered_server_instructions_contains_no_deprecated_tool_names`: `find_symbol, list_symbols, replace_symbol, insert_code, rename_symbol, search_pattern`) and absent from the live tool registry. Current names: `grep`, `edit_code`. Irony: CLAUDE.md carries an ~80-line "Prompt Surface Consistency" section preaching tool-name currency, but CLAUDE.md is not one of the 3 gated surfaces, so it drifted to the banned names (sibling of refactor-log F-9).

**Defect 2 — CONTRADICTION (verified, first-person): memory.** The CC harness `system` block says *"persistent file-based memory at …/memory/ — write to it directly with the Write tool."* The global `CLAUDE.md` says *"Use Codescout, Not Claude Code Memory … do not write durable facts there."* Both arrive every session; the superpowers priority rule (user > system) resolves it, but the model pays to reconcile it each turn, and a less-careful model writes to the dead store. Harness half is Anthropic's (not editable) — lever is to make the override explicit about the conflict. (Out of the 4-task scope; flagged for a possible task 5 in the global CLAUDE.md across 3 profiles.)

**Defect 3 — REDUNDANT (verified): same rule, multiple homes.** `json!("ok")`/no-echo ×3 (CLAUDE.md Design-Principles ¶ + Key-Patterns line + system-prompt.md); `cargo fmt/clippy/test` ×2; `RecoverableError` vs `anyhow::bail!` ×2; progressive-disclosure/two-modes ×3 (CLAUDE.md + server-instructions + `get_guide`). A rule stated three ways is three things to keep in sync — defect 1 is what desync produces. **Correction (on close reading, 2026-06-21):** 3 of the 4 are *intentional* cross-client redundancy — the `server_instructions` slice + the generated `system-prompt.md` must restate core rules because non-CC clients (Copilot/Gemini) receive no `CLAUDE.md` (per CLAUDE.md's own *Agent-Agnostic Design* principle). Only the within-`CLAUDE.md` `json!("ok")` double (No-Echo ¶ + Key-Patterns line) is true waste — fixed this session by dropping the Key-Patterns line. So defect 3 downgrades from "4× redundancy" to "1 within-file duplicate."

**Defect 4 — BLOAT (inspection, unverified benefit): reference + forensics resident.** ~170-line "Session Intelligence Trackers" § (append protocols, frontmatter shapes, status-vocab, how-to code) re-documents what it opens by pointing at (`docs/TAXONOMY.md`); most sessions never append a tracker. "Git Workflow" § embeds incident forensics ("added after F-13", "Lesson source: 2026-05-23 …", "Datapoints: fired twice …") that justify rules to a human reader, not the model. Three lifetimes interleaved — durable rules (keep), reference protocols (→ docs, pointer), changelog/forensics (→ the tracker each cites). Only durable rules earn residency in a per-session prompt.

**Eval status:** N=0. Defects 1–3 are verified facts, not predictions — read against source + both texts in hand. Defect 4's *benefit* (does trimming change behavior?) is the only measurable claim and is unverified. The measurement plan (Marius): open fresh sessions in codescout + backend-kotlin after the cut, observe (a) no dead-name tool calls, (b) rule-following unchanged, (c) start-prompt byte count.

**Recommended move:** (task 1) denylist gate scanning CLAUDE.md for the 6 dead names — denylist, not the allowlist guard of F-9, because CLAUDE.md prose would false-positive an allowlist; (task 2) fix the 3 dead names → gate green; (task 3) de-dup each rule to one canonical home; (task 4) relocate the tracker-protocol reference + incident forensics to `docs/`, leaving pointers. Target shape: codescout CLAUDE.md closer to backend-kotlin's ~12 KB layered form.

**Prediction:** Defects 1–3 — post-fix the model never reaches for a dead tool name cued by CLAUDE.md, and the new gate blocks re-drift permanently. Defect 4 — post-relocation, fresh sessions follow the same rules with CLAUDE.md ~27 KB lighter; falsified if any relocated rule stops being followed (caught by the measurement sessions).

**Confidence:** high on defects 1–3 (verified); medium on defect 4 (the cut-helps-behavior claim is N=0 until the sessions run).

**Outcome (shipped 2026-06-21, uncommitted; behavior measurement still pending):** Defects 1–3 fixed — 3 dead tool names → `grep`/`edit_code`; new gate `claude_md_contains_no_deprecated_tool_names` added red→green, sharing `DEPRECATED_TOOL_NAMES` with the server-instructions gate (closes the CLAUDE.md half of F-9 via denylist); within-file `json!("ok")` duplicate dropped. Defect 4 was cut **conservatively**: collapsed "Bug Tracking" + "Querying active trackers" to pointers at `get_guide("tracker-conventions")` + `docs/TAXONOMY.md` (content verified already present in the guide), and stripped 3 Git-Workflow forensics paragraphs. **CLAUDE.md 42,175 B → 38,794 B (−8%, −70 lines)** — NOT the hand-waved ~15 KB. The deeper cut (Session-Intelligence append-guidance ~100 ln, verbose Git release/ship procedures, Prompt-Surface-Consistency ~80 ln, Companion-Plugin ~80 ln) is operational or not-yet-relocated and was held back **pending the measurement** — if fresh sessions show no rule-following regression at −8%, that licenses the deeper cut. 88/88 prompt tests green; `cargo fmt` + `clippy --all-targets -D warnings` clean. Measure on fresh codescout + backend-kotlin sessions: (a) zero dead-name tool calls, (b) rule-following unchanged, (c) start-prompt byte count.

**Deeper cut (2026-06-21, same session, uncommitted):** Relocated the verbose middle to discoverable homes and made CLAUDE.md pointers-only. Git release/ship procedures → `docs/RELEASE.md` (new); companion-plugin hook inventory + cross-repo flow → `docs/architecture/companion-plugin.md` (new); prompt-surface operational rules (bump matrix, 2200-byte cap, verify-slice hazard) → `src/prompts/README.md` (extended, and its intro repointed away from CLAUDE.md); Development Commands → memory `development-commands`/`gotchas`; Design + Testing + Key Patterns merged → memory `conventions` + `architecture` (added the missing **Agent-Agnostic Design** principle to `conventions` first, and folded the Testing-Patterns detail in, so nothing was lost); Language-LSP already pointed to `gotchas`. **CLAUDE.md 42,175 B (session start) → 12,535 B (−70%, 677 → 184 lines)** — meets the original ~15 KB target. 88/88 prompt tests green; `claude_md_contains_no_deprecated_tool_names` still green. The behavior measurement is now the load-bearing check: at −70% the model relies on `get_guide` + `memory(read)` + the new docs for detail it previously had resident — falsified if fresh sessions stop following a relocated rule.

**Cross-refs:** refactor-log F-9 (ungated tool-name surfaces — CLAUDE.md is a third), F-10 + W-8 (this session's recon: clippy const trap + include_str! scout); `docs/architecture/mcp-channel-caps.md` (notes CLAUDE.md "defends a phantom contract for ~95% of the file").
