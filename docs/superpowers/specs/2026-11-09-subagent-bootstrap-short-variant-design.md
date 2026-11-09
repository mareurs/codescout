# Subagent bootstrap — short variant (design)

**Status:** approved by the operator 2026-11-09 (design A).
**Bug:** `7579b32b1cd2362f` (`docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`), § "Layer 2 result, second reader".

## Intent

A subagent receives the whole `project-activation-bootstrap` guide (3,107 B body,
~3.3 KB wrapped) as its session opener. 1,296 of 1,299 subagents in a 30-day census
received it. Two readers agree that 5 of 60 subagents (8%, 95% CI 4–18%) used it, and
every use was a Phase 0 or Phase 1 step:

- a memory read (`gotchas`, `onboarding`);
- the exact open-bug query `doc(find, kind=bug, filter status in [open, taken, investigating, zombie])`;
- `get_guide("error-handling")`, a topic the guide names;
- `references` for callers (Phase 1 routing; plausible only).

No reader found a use of Phase 2, the reconnaissance section or the subagent-briefing
section. A subagent should therefore get a short body that keeps those steps and points
at the full guide.

## Goals

- A subagent's session opener carries a short body (~850 B) instead of the whole guide.
- The parent's session opener is unchanged.
- An explicit `get_guide("project-activation-bootstrap")` from anyone returns the full body.
- The census can tell the short variant from the whole one.

## Non-goals and anti-goals

- No new entry in `GUIDE_TOPICS`; the short body is another body for the same topic, not
  a topic. (A new topic would fall under `every_guide_topic_is_triggered_xor_declared_pull_only`
  and add a pull-only guide nobody asks for.)
- No change to any other guide or to section-grain delivery.
- Anti-goal: a subagent without the companion hook (no principal stamp) must not lose
  anything it gets today. Without the stamp it shares its parent's ledger and gets no
  opener at all, which is today's behaviour; keep it.
- Anti-goal: deciding "subagent" from the shape of the ledger key (`contains('/')`). A
  `_meta` conversation id or a native Codex root could contain `/`.

## Approaches

- **A — server-side flag (chosen).** `dispatch_tool_call` already derives `asserted_agent`
  (`src/server.rs`, from the `<session_id>/<agent_id>` principal stamp). It sets a
  `caller_is_subagent` flag on the call's resolved ledger, exactly where it already sets
  `session_start_source`. `emitters::emit_session_opener` reads the flag and builds the
  short block. The ledger stamps the same topic key either way.
- **B — companion hook.** The `SubagentStart` hook pre-stamps the topic and injects the
  short text itself. Rejected: the logic splits across two repos, and Codex has no such hook.
- **C — nothing for subagents.** Pre-stamp the topic for subagents. Rejected: it drops the
  part that 8% of subagents use.

## Decided

- The flag is per call and set from `asserted_agent.is_some()`, not persisted (it describes
  the caller of this request, like `session_start_source`).
- Body file: `src/prompts/guides/project-activation-bootstrap-subagent.md`, contents:
  1. memory: read the topic the task needs (`gotchas`, `conventions`, `architecture` usually pay off), `memory(action="list")` when unsure;
  2. bug work: the exact open-bug query, with the one-line reason `status="open"` alone is not enough;
  3. contract guides: the `get_guide` topic list that the full guide names;
  4. lookup routing: the four Phase 1 lines;
  5. one "verify at the bytes" line (kept deliberately: operator rule OP-1, although no reader saw it used);
  6. a pointer: full orientation is `get_guide("project-activation-bootstrap")`.
- Marker: mirror the existing `preamble` form — open
  `<!-- auto-injected get_guide('project-activation-bootstrap') subagent — … -->`, close
  `<!-- end auto-injected get_guide('project-activation-bootstrap') subagent -->`. Openings
  still match every parser's `get_guide('<topic>')` prefix; `scripts/census_guide_deliveries.py`
  learns a `subagent` form.

## Left to the builder

- The exact wording of the short body, within ~850 B and the six points above.
- Where the flag lives (a `GuideLedger` field beside `session_start_source` is the
  precedent) and the name of its setter.
- Which other parsers (`scripts/measure/join.py`, `scripts/probe_guide_injection.py`,
  `scripts/probe_guide_section_use.py`) need a test for the new form.
- Whether `src/prompts/README.md` caps or `ONBOARDING_VERSION` rules apply to the new file.

## Tests that prove it

- A stamped subagent call that triggers the opener receives the short body and not the full one.
- An unstamped (parent) call receives the full body, byte-identical to today.
- After the short opener, an explicit `get_guide("project-activation-bootstrap")` returns the full body.
- The census classifies a short-variant block as form `subagent`.
