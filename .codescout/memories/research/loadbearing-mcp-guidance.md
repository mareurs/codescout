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
- SHIPPED (re-verified 2026-09-08, was "green-lit, not yet built" in the original
  finding): server-computed provenance envelope keys `refreshed_at_commit`,
  `commits_behind_head`, `head_commit`. Live in `doc(action="get")`'s `provenance`
  object — `src/librarian/tools/get.rs:502-507` ("Finding 7: server-computed
  provenance keys (unforgeable staleness signal)"), with `refreshed_at_commit`
  falling back from the augmentation's own field to the tracker's latest `reviewed`
  event's commit (`get.rs:390-406`), and `commits_behind_head` derived from a git
  topo-distance walk between that commit and HEAD (`get.rs:407-421`). Covered by
  `src/librarian/tools/get.rs:2003-2014` and `:2060-2063`. Ship rationale unchanged:
  KEY-PRIORITY 6/6 across 2 models; CALIBRATE 9-10/10 at n=10 pinned Sonnet.
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
