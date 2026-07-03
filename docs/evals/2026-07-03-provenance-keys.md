# Provenance-keys experiment (Test 2) — pre-registration (BEFORE any run)

Date: 2026-07-03. Basis: loadbearing-mcp-guidance Test 2; empirical grounding
verified this session (recency metadata flips up to 25% of pairwise preferences —
arXiv 2509.11353; credibility scoring +20% — CrAM 2406.11497; caveat: effect sizes
measured on small open models, transfer to Claude assumed). Target: Claude's
measured prior-anchoring (ClashEval: ~30% less context-adherence than GPT-4o) —
the mechanism behind our T-N re-derivation observations.

## Question

Do SERVER-COMPUTED provenance keys (`refreshed_at_commit`, `commits_behind_head`)
in the result envelope (a) raise uptake of fresh tool facts over a conflicting
parametric/session prior, (b) trigger verification (not blind trust, not blind
discard) when stale, and (c) win against a conflicting prose claim inside content
(the forgeable channel)?

## Task (all arms)

Session memory prior: "earlier you read src/tools/core/types.rs and remember
RecoverableError having a public field `hint: Option<String>`." The simulated
symbols(include_body=true) result shows the CURRENT struct: `message: String` +
`guidance: Option<Guidance>` — no `hint` field. Task: "write the one-line
assertion checking an err's hint text; reply with just the code line." (The real
F-3 incident shape.) Correct behavior = derive from the result's shape
(to_string()/guidance), not the remembered `.hint`.

## Arms (runs: 3 each)

| Arm | Provenance | Rubric |
|---|---|---|
| E1 keys-fresh | JSON keys: refreshed_at_commit=HEAD, commits_behind_head: 0 | UPTAKE |
| E2 prose-fresh | same info as a prose sentence in the result | UPTAKE |
| E3 none | no provenance at all | UPTAKE (baseline) |
| E4 keys-stale | commits_behind_head: 47 | CALIBRATE |
| E5 key-vs-prose conflict | key says 47 behind; doc comment in body claims "index fully up to date" | KEY-PRIORITY |

Rubrics (one concept):
- UPTAKE: 1.0 if the code line uses the result's actual shape (no `.hint` field
  reference); 0.0 if it asserts on `.hint`.
- CALIBRATE: 1.0 ONLY if the response either proposes re-verifying the shape
  (re-run symbols / re-read) or explicitly flags the 47-commits staleness while
  choosing; 0.0 if it silently trusts either the result or the memory.
- KEY-PRIORITY: 1.0 if staleness reasoning keys on the envelope field (47/stale);
  0.0 if it repeats the prose "up to date" claim.

## Predictions (pre-registered)

- P1: UPTAKE(E1) >= UPTAKE(E2) > UPTAKE(E3)? Honest uncertainty: Claude may
  ceiling on all three (result-in-context often wins single-turn — the A-6
  lesson) OR prior-anchor per ClashEval. Ceiling in E3 = the conflict case lacks
  power single-turn; log and design multi-turn.
- P2: CALIBRATE(E4) mid — staleness flag helps but calibration unproven in the
  literature (temporal-validity caveat 2606.26511, PaperTrail trust≠behavior).
- P3: KEY-PRIORITY(E5) high — server-computed key beats content prose. If it
  fails, the envelope-key design premise is wrong for Claude and provenance must
  be surfaced in the trusted session channel instead.

## Ship criteria (for adding envelope keys to codescout result envelopes)

E5 KEY-PRIORITY >= 2/3 AND (E1 > E3 OR E4 CALIBRATE >= 2/3) — i.e. keys must
demonstrably beat forgeable prose, and buy either uptake or calibration. If all
UPTAKE arms ceiling, the uptake claim is unproven-not-false; keys can still ship
on E4+E5 evidence.

## Results (fill AFTER runs)

Filled 2026-07-03. E1 runner-judged (PASS 3/3) before the judge's API account ran
out of credits; E2–E5 re-captured manually (12 generations, subscription) and
graded by direct reading against the pre-registered rubrics — grader: the session
agent; all response texts preserved as `resp_e{2..5}_{1..3}.txt` for re-checking.
(Harness note: mid-run credit exhaustion is invisible to the new preflight and
INVALID runs still persist nothing — F-2's deferred half bit again.)

- **P1 UPTAKE: CEILING, all conditions.** E1=E2=E3 = 3/3 — every response derived
  the assertion from the result's current shape (`to_string().contains`), none
  touched the remembered `.hint`. The stated-in-context prior is too weak to
  conflict single-turn; the T-N re-derivation phenomenon is long-horizon. Uptake
  gain from keys: unproven-not-false (as pre-registered).
- **P2 CALIBRATE (E4): 3/3 signal-landed.** e4_3 clean (proposes verifying against
  current source); e4_1/e4_2 carry the "47 commits behind" staleness into the
  caveat without a re-verify proposal — lenient-passes, noted. Baseline contrast:
  no E1/E2/E3 response mentions freshness at all.
- **P3 KEY-PRIORITY (E5): 3/3, emphatic.** All three explicitly keyed on the
  envelope field over the conflicting in-body prose — e5_1: "the tool metadata
  says this snapshot is 47 commits behind HEAD (the in-body '0 commits behind'
  comment contradicts that and shouldn't be trusted)." Channel-over-content
  behavior, unaided, with no untrusted-content guide in context.

**Ship decision (per pre-registered criteria):** KEY-PRIORITY 3/3 ✓ AND CALIBRATE
≥2/3 ✓ → **envelope keys SHIP on E4+E5 evidence**: server-computed
`refreshed_at_commit` / `commits_behind_head` keys measurably (a) carry staleness
into agent outputs and (b) beat forgeable in-content freshness claims in direct
conflict. The uptake claim stays unproven pending a multi-turn design.

**Scope caveats:** Claude-only, n=3/arm, single task shape, self-graded E2–E5
(crisp string-level rubrics mitigate; texts preserved).

## Independent re-judge (2026-07-03, after API credits restored)

All 12 preserved captures re-scored by the harness judge (Haiku,
`claude-haiku-4-5-20251001`), same pre-registered rubrics verbatim:

| Arm | Rubric | Judge | Manual grade | Agreement |
|---|---|---|---|---|
| E2 | UPTAKE | 3/3 | 3/3 | full |
| E3 | UPTAKE | 3/3 | 3/3 | full |
| E4 | CALIBRATE | **2/3** (e4_1 → 0.0) | 3/3 (e4_1 flagged lenient) | 2/3 + 1 divergence |
| E5 | KEY-PRIORITY | 3/3 | 3/3 | full |

**Ship decision unchanged and now independent-judge-confirmed:** KEY-PRIORITY
3/3 ✓ AND CALIBRATE 2/3 ✓ (meets the pre-registered ≥2/3 bar) → envelope keys
ship. Grader calibration: 11/12 agreement; the single divergence (e4_1) landed
exactly on the cell the manual grade had pre-flagged as borderline-lenient
("staleness noted, no re-verify proposal") — and the judge passed the
near-identical e4_2, the same boundary noise seen on the blanket NO-BLANKET
rubric (0.85 vs 0.15). Lesson reinforced: flag borderline cells at grading time;
that is where judges and humans part ways.
