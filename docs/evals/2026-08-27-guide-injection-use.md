# Eval — is auto-injected guide text actually used? (2026-08-27)

Companion to [`../issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`](../issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md),
whose `## Not yet done` section asked for exactly this probe. Headline results live
there under *§ Measured — USE, 2026-08-27*; this file records the **method**, so
the numbers can be challenged or re-run rather than merely believed.

Raw data: [`data/2026-08-27-guide-injection/`](data/2026-08-27-guide-injection/)
— 10 per-session result JSONs, `corpus-frame.json` (1,007 sessions),
`truebytes.json` (4,194 injections with as-delivered byte counts), and
`rubric-BRIEF.md`, the brief every agent executed verbatim.

Delivery half is scripted: [`../../scripts/probe_guide_injection.py`](../../scripts/probe_guide_injection.py).
The **use** half is not scriptable — it needs judgement over a transcript — so it
was run by 10 subagents against one shared rubric.

---

## Design

**Question.** For each auto-injected guide: how many bytes, at what point relative
to the need, was any of it used, and how much.

**Population.** 2,202 transcripts across `~/.claude`, `~/.claude-sdd`,
`~/.claude-kat` → 1,754 with an assistant model line → 1,705 unique sessions after
deduping session-ids present under two profiles → 1,011 with ≥1 injection.

**Eligibility, stated before selection.** ≥1 real injection; dominant model exactly
`claude-sonnet-5` or `claude-opus-5`; ≥20 assistant turns (below that, "use" is not
*expressible*, so including them would manufacture `U0`s); main sessions only —
subagent transcripts share the parent's guide ledger, so "did this arrive at the
right time" is confounded there. → **128 eligible** (73 Opus, 55 Sonnet).

**Draw.** Deterministic: sort eligible by `md5(path)`, take 5 per model arm. No
human choice at any point. The draw's topic mix was compared against the
population's *before* analysis — it tracked (e.g. `librarian` 7/10 drawn vs 75% of
population).

**Analysts.** 10 subagents, all on Opus, one transcript each, all executing
`rubric-BRIEF.md`. Sonnet vs Opus in the sample names the **transcript's** model,
not the analyst's.

---

## The rubric

**`U3_CITED`** — later assistant text names the topic or `get_guide`, or states a
rule attributing it. **`U2_PRESCRIBED_CALL`** — a later tool call matches a shape
the guide prescribes *and* that shape appears nowhere before the injection (the
before-check is what separates *use* from *the agent already did it*).
**`U1_ECHO`** — ≥6 consecutive words from the guide body, absent earlier.
**`U0_UNUSED`** — none of the above. Plus an independent **`contradicted`** flag:
a later action violates a rule of the guide that just arrived.

Every non-`U0` classification required `{line, quote}` evidence. No evidence → `U0`.

**Utilisation** = bytes of the guide `##` sections any evidence touched ÷ topic
bytes. Section grain is an **upper bound** by construction.

### Two anti-fabrication devices, both of which fired

1. **A calibration gate.** Each agent was given the controller's independently
   measured injection count and per-topic breakdown for its transcript, and told to
   **STOP and report FAIL** on any mismatch rather than adjust. **10 of 10 PASSED** —
   which is what makes the aggregate addable.
2. **Explicit permission to return nothing.** *"If your transcript shows 0%
   utilisation across every injection, that IS the finding. Do not hunt for a nicer
   number."* Two agents returned exactly that.

The design also inverted the usual failure direction: **three of the four
controller errors in this study were caught by the subagents**, not by the
controller — see *Corrections* below.

---

## Corrections made during the run

Recorded because each was a real defect in the instrument, and two would have
shipped a wrong headline.

| # | defect | found by | effect |
|---|---|---|---|
| 1 | Corpus glob missed nested paths (181 files seen vs 585 with markers) | controller, via positive control | would have under-sampled 10× |
| 2 | Marker double-count — every injection has opening **and** closing | controller, via positive control | 2× on every topic count |
| 3 | `LATE` degenerate — the trigger's `tool_use` precedes its `tool_result`, so the inequality is true by construction | OPUS-4 | timing verdict carried no information; recomputed as `gap > 1` |
| 4 | **Today's guide sizes applied to historical injections** — but guides grow | OPUS-5 | overstated corpus total **1.17×**; `tracker-conventions` alone 0.72 ratio. Fixed by measuring bytes *between* the markers |
| 5 | Injections delivered on `queue-operation`/`attachment` lines (backgrounded MCP calls) are invisible to a `tool_result`-only scan | SONNET-2 | bounded at ~6 of ~2,128 injections (0.3%) — real, moves nothing |
| 6 | A "prescribed shape" that is the tool's **default value** is not evidence of use | OPUS-4, on its own result | tightened; that agent's `librarian` U2 collapses to U0 |

---

## Limits

1. **Thinking text was unavailable for these transcripts — a fixable defect, not
   model behaviour, and now fixed.** All 10 sampled transcripts store `thinking`
   signature-only (1,185 blocks, all zero-length); Langfuse showed the same at the
   time. **Cause, found 2026-08-27 (`llm-proxy:6f3cb62`): two request-side
   settings, either sufficient alone** — Claude Code sends `anthropic-beta:
   redact-thinking-2026-02-12` (a client-side terminal-UI choice, not an Anthropic
   restriction), and `thinking: {"type": "adaptive"}` with no `display` key, which
   several current models default to `"omitted"`. Post-fix, live traces carry
   readable thinking on both models, and CC's JSONL began carrying it within
   minutes — the transcript redaction was downstream of the same cause.
   → For **these** results, `U1`/`U3` are floors, so `66.7% U0_UNUSED` is an
   **upper bound on non-use**; `U2` and `contradicted` read tool calls and are
   unaffected. → For a **re-run**, thinking is now visible in both instruments.
   This is the single highest-value change to the method.
2. Section-grain utilisation overstates. Two independent subsection-grain estimates
   put the real figure at low single digits.
3. Half the sample predates the 2026-08-19 ledger fix. Post-fix, contradiction
   falls 76% → 12%.
4. n = 10 sessions / 81 injections. Small.
5. Sonnet vs Opus is **not** cleanly separable — the arms differ in era and task
   type as much as in model. Do not report it as a model effect.

### A seventh correction, made after publication

The table above lists six defects caught during the run. A seventh was caught
*after* the results were written up, and it is the most instructive:

| # | defect | found by | effect |
|---|---|---|---|
| 7 | Concluded thinking was irreducibly unavailable at the API, from a negative measurement | the fix's author, next session | wrote a false limit into three durable surfaces before correction |

The reasoning error: a proxy has a **request** side and a **response** side. The
response side was verified (`BlockAcc::apply_delta` handles `thinking_delta`,
unit-tested), and the whole component was then treated as excluded — after which
the absence was attributed to Anthropic. **Eliminating a component by checking one
half of it is not elimination.** The supporting token-accounting figure (~95% of
billed output tokens never returned as text) was also treated as decisive when it
does not discriminate: it is equally consistent with *the model omitted the text*
and *our own request asked for it to be omitted*.

Filed as `../trackers/prompt-surface-measurement-session-log.md` `F-37`.
---

## Re-running it

```bash
python3 scripts/probe_guide_injection.py --json > /tmp/delivery.json   # delivery half
```

For the use half, re-read `data/2026-08-27-guide-injection/rubric-BRIEF.md` and
dispatch one agent per transcript with its calibration values. Keep the calibration
gate — without it, ten agents produce ten unfalsifiable numbers.

### Re-run trigger — this study has a known, now-removable limitation

**Limit 1 was fixed hours after this study ran** (`llm-proxy:6f3cb62`): thinking
text is captured again, in Langfuse *and* in Claude Code's JSONL. So `U1_ECHO` and
`U3_CITED` — which this study could only report as floors — are measurable now, and
`66.7% U0_UNUSED` is an **upper bound that a re-run can sharpen**.

This is written as a trigger rather than a note because the failure mode is
predictable: the limitation quietly disappears, nobody re-runs, and the upper bound
gets quoted as a settled figure.

**Fire when ALL of:**

- **≥30 post-fix main sessions** carry non-empty `thinking` blocks
  (`>= 2026-08-27`, excluding `/subagents/`), and
- they span **≥5 distinct projects**, and
- **excluding `codescout` and `claude-plugins`** — sessions about guide injection
  measure themselves (the R-51 self-measurement seam; the original study excluded
  its own transcript for the same reason).

Checkable with the scan in this session's scratchpad, or equivalently: walk the
transcript corpus counting assistant `thinking` blocks whose text is non-empty,
group by project.

As of 2026-08-27 09:15 the count was **27 transcripts**, hours old and concentrated
in `codescout` / `claude-plugins` — i.e. the trigger does **not** yet fire, and
firing it early would reproduce exactly the sampling defect the original study was
built to avoid.

**What a re-run changes:** `U1`/`U3` stop being floors, so "was this guide read and
reasoned over but not acted on?" becomes separable from "was it never read at all".
That distinction is the one the design directions (a)/(b)/(c) actually turn on —
a guide that is read and ignored needs different treatment from one that is never
opened. Nothing else about the method needs to change; the rubric, the sampling
rule and the calibration gate all carry over unaltered.
