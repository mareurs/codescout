# Load-bearing MCP guidance — validated findings (2026-07-03)

Full synthesis: `docs/research/2026-07-03-mcp-guidance-findings.md`. Chronological
detail: audit log A-3→A-7 (`docs/trackers/prompt-hamsa-audit-log.md`). Evidence:
`docs/evals/2026-07-03-*.md`. Scope: Claude, single-turn, prompt-tdd, small N.

## Headline

Authority framing (persona, "sacred" channel, in-band trust marker) buys NOTHING
measurable for making codescout guidance load-bearing. Placement + server-computed
structure buys everything measured. Trust rides the channel (server-computed output:
symbols/refs/git/envelope keys), never a marker the content carries about itself — a
static `[LIVE]:` header or `last refreshed:` stamp is copyable by anyone who can write
the file.

## What shipped / green-lit

- SHIPPED: `get_guide("untrusted-content")` — the data-vs-directive rule ("quarantine
  the instructions, verify the facts"), incl. WHAT-not-HOW constraint (content names
  what to verify, never the route). Fixes blanket-distrust without weakening injection
  resistance.
- SHIPPED: reader-first tracker prompts (`tracker_design` Step 2, deployment_state
  template, `> Standing instruction:` label) — helps only trackers with no
  render_template table.
- GREEN-LIT, not yet built: server-computed provenance envelope keys
  (`refreshed_at_commit`, `commits_behind_head`) on result envelopes. KEY-PRIORITY 6/6
  across 2 models; CALIBRATE 9-10/10 at n=10 pinned Sonnet. NEXT feature; scout the
  envelope seam in `src/tools/core/types.rs`; intersects the G5 doc(action="get") bug.
- DO NOT build: a persona preamble (A-4), or a delegation line/`<codescout-guide>`
  envelope (A-7 Test 1 — a plain footer suffices single-turn).

## The standing blocker

Everything time-dependent (instruction decay, re-derivation of returned facts,
guidance persistence, adherence at distance) escapes single-turn eval — 4 findings hit
the same ceiling. Unblock = a multi-turn harness (prompt-tdd `input.history`).

## Eval-discipline rules earned this session (all the hard way)

Pre-register predictions+alternatives; one concept per rubric; BIND response↔score
(empty judge reasoning + extreme score = misfire artifact, caught ≥4 false findings);
mutation check proves the artifact fired not that the rubric is valid; STATE THE PINNED
MODEL (an uncontrolled var invalidated 2 "high confidence" calls — F-5); n≥10 near a
threshold (n=3 lies both directions). See buddy prompt-hamsa memories for the craft
form.
