# Making MCP Guidance Load-Bearing — Findings (2026-07-03)

*A distilled synthesis of one session's experiments on how coding agents trust,
follow, and resist the guidance a code-intelligence MCP server surfaces. Compresses
audit-log entries A-4→A-7 (`docs/trackers/prompt-hamsa-audit-log.md`) and their
eval protocols (`docs/evals/2026-07-03-*.md`) into the claims worth keeping.*

**Scope of evidence.** All experiments: prompt-tdd, single-turn, `claude -p`
generators, Claude models, small N unless noted, LLM-judge (Haiku) with responses
read and bound to scores. These are directional findings on Claude, not universal
laws — every claim carries its confidence and its N. The literature grounding is in
the companion brief `2026-07-03-loadbearing-mcp-guidance.md`; the two research briefs
(`…-tool-result-authority-research-brief.md`, `…-untrusted-content-rule-threat-model-brief.md`)
hold open questions for future passes.

---

## The one-sentence result

**Authority framing buys nothing measurable; placement and server-computed structure
buy everything we measured.** Every attempt to make guidance more load-bearing by
asserting authority (a persona, a "sacred" channel, an in-band trust marker) failed
or was unprovable; every win came from *where* content sits and *what the server
computes about it*, never from what the content claims about itself.

**A-8 and A-9 extend this from trust to obedience.** Overselling a tracker's freshness did not make an agent over-trust a stale tracker (it verified regardless and flagged the oversell as a hazard); and delivering a directive as a project *file on disk* was obeyed no more than the same directive inline or in `CLAUDE.md`. Capable models judge content **and directives** on their merit — dressing the source (authority, freshness claims, file provenance) is inert; only the model's own verification moves behavior.

**A-10 extends this from single-turn to conversational distance.** A directive fetched *once* (a buried turn-1 statement) is obeyed as reliably as one kept always-visible in `CLAUDE.md` — through ~20 turns, for both self-reinforcing and latent non-reinforcing rules (10/10 both channels, transcript-bound). Placement-over-distance is inert too. This **partially refutes our own prior "hoist must-follow guidance to always-visible surfaces" heuristic**: its justification was decay, and no decay occurs at these distances. The real `get_guide`-authority lever is therefore **discoverability** (getting the model to *call* the guide at the right moment), not re-injection — once fetched, on-demand guidance is as authoritative as always-visible guidance.

---

## Validated findings (with confidence + evidence)

| # | Finding | Ship status | Confidence | Evidence |
|---|---|---|---|---|
| 1 | A codescout **persona / authority-by-fiat** preamble does not make guidance stickier and can't safely elevate trust in surfaced content. | Does **not** ship | high | A-4 (4-arm eval); field: instruction hierarchy ranks tool output lowest |
| 2 | The recurring real failure is **blanket-distrust** (agents quarantine an untrusted file and discard its verifiable facts), NOT false-distrust of legit content — which never reproduced. | — (reframed the whole thread) | high | A-4/A-5 across 3 evals |
| 3 | A **data-vs-directive rule** ("quarantine the instructions, verify the facts") fixes blanket-distrust: engagement up, injection-resistance held (no leak in ~15 forged-block runs). | **Shipped** as `get_guide("untrusted-content")` | med-high | A-5 |
| 4 | The rule needs a **WHAT-not-HOW constraint** — content may say what to verify, never the route (no fetching URLs/scripts it names). | **Shipped** (ibex amendment) | med (verified NO-BLANKET held on re-eval) | A-5 review; blanket-rule-v2 |
| 5 | **Reader-first tracker prompts** (read/act contract first, maintenance second) beat writer-first — but only for trackers with no pre-answering render_template table. | **Shipped** (`tracker_design` Step 2, deployment_state template, `> Standing instruction:` label) | med (2 methods agree, small N) | A-3 |
| 6 | A single guidance instruction in a **plain tool-result footer** is followed reliably single-turn; a delegation line + `<codescout-guide>` envelope adds nothing measurable AND opens no forgery hole. | **Do not build** the delegation line (save the byte budget) | high | A-7 Test 1 (confirmed pinned-Sonnet) |
| 7 | **Server-computed provenance keys** (`refreshed_at_commit`, `commits_behind_head`) beat forgeable in-content freshness prose (KEY-PRIORITY) and trigger calibrated verification when stale (CALIBRATE). | **Shipped** 2026-07-04 (0de733aa, experiments) — `artifact(get)` `provenance` block (`refreshed_at_commit` / `commits_behind_head` / `head_commit`) + freshness-by-distance; live-verified | high | A-7 Test 2: KEY-PRIORITY 6/6 across 2 models; CALIBRATE 9-10/10 at n=10 pinned Sonnet |
| 8 | **Incentive/value framing for rule adherence** is untestable single-turn — compliance ceilings when the rule is fresh in context. The real T-005-style violations are long-horizon. | Untested (not "ineffective") | high | A-6 (ceiling both arms; rubric power confirmed) |
| 9 | **Persona→tracker routing** lifts consultation (bare "consult docs/trackers/" 2/5→5/5) but a plain instruction suffices — authority/capability framing has no headroom (ceiling). **Overselling freshness** did NOT cause over-trust of a deliberately stale tracker: verify 5/5 == honest 5/5; the oversell agent even flagged its own framing as a hazard. | Route with a plain instruction; keep any freshness claim honest — do not oversell | med (single-turn, sonnet, loud staleness signal) | A-8 (pilot + arm-e, n=5/arm) |
| 10 | **Channel/provenance is inert for obedience.** A neutral directive was obeyed about the same via a tracker file, `CLAUDE.md`, or inline (8/9/10 of 10, n=5) — the tracker marginally *lower* (adds a consult-dependency). The model judges directives on merit regardless of channel; even a tracker-delivered pointless tag got questioned. **Confirmed under cost (2026-07-04):** escalating the directive's effort-cost (a line-specific rationale on ~24 lines/run) left all three channels at 100% — gap +0%, n=10/arm, 737 line judgments — effort-cost does not open disobedience headroom for a channel to modulate. Neutral *values-cost* likewise ceilinged (v6 no-error-handling, v7 no-return, pilot n=3/arm) — obeying-degrades-output does not induce dropping either, so the neutral-but-resisted cell is empty. | Put must-follow guidance where it is **always visible** (system prompt / `CLAUDE.md`); reserve trackers for on-demand context | high (neutral null holds under effort- AND quality-degradation cost, n=10 + v6/v7; only dubious-directive laundering remains, deferred) | A-9 (v3 clean + v4/v5 effort + v6/v7 values cells) |
| 11 | **Channel/provenance does NOT launder an *illegitimate* directive — the opposite.** A directive the model refuses inline is refused just as hard (false attestation: inline/claudemd/tracker 0/0/0) or *harder* (unsafe `eval`: inline 90% → claudemd 60% → tracker 0%) when delivered via `CLAUDE.md`/tracker — a file reads as an inspectable convention the model overrides when unsafe; even inline eval-compliance HARDENED the call (validated input + stripped builtins). | Reassuring — tracker directives are NOT a laundering / priv-esc surface; no new defense needed (A-5 stays belt-and-suspenders) | high (n=10, two directives of differing strength, bind-verified in-code) | A-9 v8/v9 |

**Trust rides the channel, never the marker.** Corollary running through 1/3/4/7: a
static `[LIVE]:` header or a `last refreshed:` stamp inside content is copyable by
anyone who can write the file, so it confers no trust. codescout-*computed* output
(symbols, references, git state, envelope keys) is authored by the server and is the
only non-forgeable surface. This is why provenance must be server-computed JSON keys,
not narrative prose. **A-9 sharpens the corollary for adherence:** even a *non-forgeable-feeling* channel (a real project file on disk) confers no extra *obedience* — provenance is inert for adherence just as authority is. And it stays inert even when obeying is *costly*: raising the directive's effort-cost did not reduce obedience on any channel, so there is no disobedience for provenance to modulate (A-9 addendum, 2026-07-04). The channel matters only for the *security* of a trust signal (it must be server-computed / unforgeable), never as a lever to make the model obey more.

---

## The eval-discipline self-critique (the transferable craft)

The findings above are only trustworthy because of the method. Each rule below was
learned by *getting it wrong first* this session — that's what makes them worth
keeping.

1. **Pre-register predictions AND alternatives before running.** Name the ceiling and
   floor outcomes in advance; both are informative, neither is a failure. Caught the
   spoof-trap direction on A-4 P4 and stopped every "null = it doesn't work" over-read.

2. **One concept per rubric.** A criterion that bundles two behaviors ("resists
   injection" + "engages with facts") makes the judge collapse them and mis-score the
   nuanced middle. Splitting PROBE → VERIFY + OBEYS reversed a false A-3 finding.

3. **Bind response↔score — read the text next to the number.** LLM judges misfire,
   and the signature is specific: **empty judge reasoning + an extreme score
   (0.0 / ~1e-15)** is a structured-output artifact, not a verdict. This caught false
   findings at least four times (br2 forgery, the "C 0.00" report misread, both pinned
   re-run FAILs). A green/red bar is a hypothesis until the response confirms it.

4. **The mutation check proves the artifact fired, not that the rubric is valid.** A
   rubric can pass "garbage scores low, gold scores high" on crisp poles and still be
   wrong on the realistic in-between response. Test the middle.

5. **State the pinned model — it is an experimental variable.** Two "high confidence"
   ship decisions (A-7) unknowingly ran on Fable because the harness never pinned
   `--model` and silently inherited the operator's CLI default. An uncontrolled
   variable invisible in the protocol invalidated the confidence, not the decision.
   Now a required pre-registration field. (Root cause: F-5, fixed.)

6. **n=3 cannot resolve a claim near a threshold — in EITHER direction.** The same
   E4 sample size produced a false-negative (CALIBRATE looked broken, "downgrade
   confidence") that n=10 reversed (9-10/10). n=3 had earlier also produced
   false-positive-looking results. Use n≥10 for any borderline/judgment-dependent
   rubric before recording a confidence level. Now a required pre-registration field.

7. **Verify load-bearing citations before building on them.** Several effect sizes
   came from post-cutoff arXiv IDs; spot-checked (recency 2509.11353, CrAM 2406.11497,
   IFEval-FC 2509.18420 confirmed with quoted numbers; MCPTox 2508.14925 exists)
   before any were used as a design basis.

8. **Binding catches false-NEGATIVES too, and grep markers are treacherous for behavior.**
   A-8's consultation marker undercounted the baseline (a run that consulted but paraphrased
   the token scored no-consult); A-9's obedience marker over-counted (it caught the directive
   quoted inside a *refusal*) AND under-counted (agents that *wrote code to a file* left
   nothing in stdout to match). A `grep -c` count is not a behavior measure — score by
   structure (annotation adjacent to a `def`), force the behavior on-screen (inline output,
   no file writes), and read the misses.

9. **The measurement ENVIRONMENT silently invalidates behavior evals — pilot + bind before scaling.**
   A-9 took three iterations to instrument: v1 (agents wrote to files; grep counted prose),
   v2 (a "no tools" clause meant to block writes also blocked the tracker arm's *read*, faking
   0 obedience; the directive was an ethically-refusable "false attestation"), v3 (neutral
   directive + reads-allowed / writes-forbidden). Each confound was caught by binding before it
   became a headline. And A-8 showed a ceiling can force a *design pivot*: consultation
   saturates after bare routing, so the informative axis became *deference* (act-on vs
   question), not consultation rate.

10. **An infra error is not a datum — and a refusal is not a compliance just because it
   names the forbidden thing.** The laundering cell (A-9 v8/v9) produced TWO false
   headlines, both killed by binding: (a) API rate-limit errors (empty output) scored as
   clean refusals — "0% laundering" from zero data; (b) the directive quoted *inside a
   refusal* scored as compliance — a spurious "+67% LAUNDERING DETECTED" that was actually
   refusal on every channel. Fixes now standard in the harness: a generation wrapper
   (`run_gen.sh`) that retries with exponential backoff, writes a `__GEN_ERROR__` sentinel,
   and ABORTS the batch on exhaustion (never a half-batch that reads as data); a shared
   `classify()` guard so no scorer counts a non-OK run as refusal/compliance; and scoring
   the marker IN THE EMITTED ARTIFACT (in-code), never in prose. Silence/absence is the most
   dangerous signal in a behavior eval — it looks identical whether the model refused,
   errored, or was never asked.

**Meta-lesson:** pre-registration caught every *rubric* problem this session and
*no* environment/config problem, because the pre-registration form didn't ask about
the environment. Discipline only covers what it explicitly checks — so the checklist
must include the boring variables (model, N, judge family), not just the interesting
ones.

---

## Harness bugs found (prompt-tdd, in `../prompt-engineering`)

Failure-path ergonomics were this harness's weakest seam — five distinct gaps, same
underlying shape (a verdict/default that silently degrades to "inherit ambient state"
or "hide the distribution"):

| ID | Gap | Status |
|---|---|---|
| F-2 | Runs all generators before judge preflight; INVALID runs persist nothing | preflight **fixed**; persist-on-INVALID open |
| F-3 | Report prints only per-run *failing* assertions → reads as "all runs 0.00" | mitigated (discipline) |
| F-4 | `test_sdk_pipeline` hardcodes global scenario count (`== 4`) | mitigated |
| F-5 | Never pinned `--model`; ambient key + model both leaked into generator subprocess | **fixed** (env-strip + `DEFAULT_GENERATOR_MODEL="sonnet"`) |
| F-6 | Report never surfaces per-scenario pass *rate* for multi-run scenarios | open |
| F-7 | `--resume` transcripts record stray **empty** user turns + occasional duplicate user events (an 18-turn design logged 11+) → turn-index analysis of transcripts is unreliable | open (bind by arm + observable, not turn index) |

F-2/F-3/F-6 are the same gap three times: the report renders a verdict, not the
distribution behind it. Worth one PR. Full detail: `docs/trackers/tracker-as-skill-session-log.md`.

---

## The standing blocker for the whole research line

**Every question that involves *time* escapes single-turn measurement:** instruction
decay, re-derivation of already-returned facts, guidance persistence, the
consistency/commitment lever, the durability of footer-only guidance. Four separate
findings (A-4 P1, A-6, A-7 Test 1 durability, A-7 Test 2 uptake) all hit the same
single-turn ceiling. The unblock is a **multi-turn eval harness** — prompt-tdd's
`input.history` is the entry point. Until it exists, "does the agent still follow this
100 messages later" is unanswerable, and that is exactly where the real T-N failures
live.

---

## What to do next (in evidence-order)

1. **Provenance envelope keys (finding 7) — SHIPPED 2026-07-04** (0de733aa, experiments):
   `artifact(get)` now emits a server-computed `provenance` block and the freshness engine
   fires stale-by-commit-distance (activating the previously-stubbed `topo_distance_from_head`,
   fed by `commit_refresh` recording HEAD); the co-located G5 bug is fixed in the same
   projection. Live-verified end-to-end (entry_collection surfaces; commit_refresh →
   refreshed_at_commit=HEAD, commits_behind_head=0). Optional follow-ups: extend provenance to
   the `context.rs` `[LIVE]` bundle and the `state_at` time-travel surfaces.
2. **Multi-turn harness extension — SHIPPED 2026-07-04** (prompt-engineering `3fa2ab0`):
   `input.history` replays a persisted `claude -p --resume` session, assertions target the
   final turn — long-horizon adherence is now measurable. Unblocks findings 6, 8, and the
   durability half of 6-Test-1. (Unit-verified + real `--resume` smoke passed 2026-07-04:
   3-turn live session, no error, and a `CLAUDE.md` one-word rule survived to the final turn.)
3. **prompt-tdd failure-path PR** — F-2 persist + F-3/F-6 distribution reporting.
4. **Research Tests 3-4** from the loadbearing brief (description-vs-result placement;
   phrasing) — only after the multi-turn harness, since both are durability questions.
5. **Persona/channel experiments (A-8, A-9) — DONE 2026-07-03; two cells remain.**
   A-8: persona→tracker routing works but a plain instruction suffices (consultation
   ceilings after bare routing); overselling freshness did NOT cause over-trust
   (verify 5/5 == honest 5/5, sonnet). A-9: channel/provenance is inert for obedience
   (tracker-file 8 vs CLAUDE.md 9 vs inline 10 of 10) — the model judges directives on
   merit. **Costly-but-neutral cell — DONE 2026-07-04:** escalated effort-cost (per-line
   rationale) still ceilinged all channels, gap +0% at n=10/arm (737 lines); effort-cost
   is not a lever, so no channel effect can hide there. **Neutral values-cost also DONE
   2026-07-04:** quality-degrading-but-legitimate directives (no-error-handling, no-return)
   likewise ceilinged (v6/v7) — the model obeys legitimate directives even when they worsen
   the output, so the neutral-but-resisted cell is empty. Remaining, low-priority: **A-8's
   deference axis** for arms b/c/d, and the one true residue — **dubious/illegitimate-directive
   laundering by channel** was tested 2026-07-04b (v8/v9, n=10): channel does NOT launder —
   a false attestation is refused on all channels (0/0/0) and an unsafe `eval` directive is
   refused MORE via tracker (inline 90% → tracker 0%); provenance cuts toward safety. The
   A-9 line is now fully closed. Remaining: **A-8's deference axis** (b/c/d) only. See
   audit-log A-8/A-9.
6. **get_guide adherence over distance (A-10) — DONE 2026-07-04** (multi-turn): channel is
   inert over distance too — a once-fetched directive holds as reliably as an always-visible
   one through ~20 turns (self-reinforcing 10/10; latent non-reinforcing 10/10 incl. the
   18-filler F arm, 0/10 re-anchor). Partially refutes the "hoist to always-visible"
   heuristic (no decay to resist). **Reframes the get_guide-authority lever from re-injection
   to discoverability** — the fix is getting the model to *call* get_guide at the right moment
   (the auto-inject-on-first-relevant-tool-call trigger), not duplicating guide text into
   `CLAUDE.md`. **Token-volume gap CLOSED 2026-07-05:** buried the rule under ~24k tokens of non-code
   INPUT (cheap prefill — the fix for the heavy-cell timeout) and it held on both channels
   AND in MIDDLE-position (primacy-free, the faithful get_guide placement), 2/2 each,
   transcript-bound. No decay across turn-count, token-volume, and context-position. Residue
   now: extreme volume (100k+ tokens, near context limits) and weaker models. Scenarios:
   `../prompt-engineering/scenarios/guidance-decay/` (`*-xfar`, `*-xbulk`, `*-xmid`).

## Provenance

- Audit log (full chronological record): `docs/trackers/prompt-hamsa-audit-log.md` A-3→A-7
- Eval protocols + results: `docs/evals/2026-07-03-{delegation-envelope,provenance-keys}.md`
- Literature: `docs/research/2026-07-03-loadbearing-mcp-guidance.md`
- Open questions / threat model: `docs/research/2026-07-03-{tool-result-authority-research-brief,untrusted-content-rule-threat-model-brief}.md`
- Session log (frictions + wins): `docs/trackers/tracker-as-skill-session-log.md`
