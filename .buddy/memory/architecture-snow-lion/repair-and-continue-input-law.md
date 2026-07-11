---
specialist: architecture-snow-lion
scope: project
slug: repair-and-continue-input-law
created: 2026-07-10
updated: 2026-07-10
tags: [error-handling, cross-cutting-law, input-repair, llm-round-trips, tool-design]
---

**Lesson:** codescout has a cross-cutting input-handling law (sibling to
[[outputguard-cross-cutting-law]]): when a tool can **deterministically** infer
intent from a malformed input, it repairs the input, runs, returns the result,
and attaches an advisory `corrections` note — it does NOT return
`RecoverableError`. RecoverableError is reserved for input that is genuinely
missing or ambiguous. Decided 2026-07-10; ADR
`docs/adrs/2026-07-10-repair-and-continue-input-handling.md`; commits
`e92529e8` + `19fb6b88`.

**Why:** Every RecoverableError forces the agent to retry, which is a SECOND
full LLM inference (latency + tokens + cost). The retry is the expensive part,
not the error object. A usage.db sweep (72 DBs, ~152k calls) showed a large
share of errors are deterministically-repairable shape/synonym mistakes
(`file_path`→`path`, inverted filter leaf `{op:{field,value}}`, buffer handle
under `output_id`). Repairing in-process eliminates the round-trip; the note
still teaches. Marius framed the scope: "this should be the entire pattern
everywhere to save a second llm call." The moat argument
([[agentic-surface-as-moat]]) reinforces it — this is the LLM-facing surface,
and repair+note teaches every MCP client at the moment of the mistake, so it
needs NO `server_instructions` change (self-describing when it happens).

**How to apply:** Reviewing any tool that returns a RecoverableError, ask: is
the malformed input deterministically recoverable — exactly ONE correct reading
(synonym, mechanical shape inversion, coercible scalar)? If yes → repair + note,
don't error. If missing/ambiguous → keep the teaching error; never guess. WRITES
get a higher bar than reads: accept an *explicit* write target, never
auto-*guess* one (a wrong guess on a write is unrecoverable — the same asymmetry
that makes [[cross-cutting-side-effects-at-the-chokepoint]] default to the safe
value). Repair at the tool's INPUT BOUNDARY; keep core validators strict as
defense-in-depth (`filter::compile` stays strict; `repair_inverted_leaves` runs
at the `find` handler *before* it). Notes ride only on object-shaped responses;
`json!("ok")` write tools repair silently — reshaping ~40 responses to carry a
note isn't worth it, since the round-trip saving is in the repair, not the note.
Shared machinery: `crate::fs::PATH_PARAM_ALIASES`, `require_str_param_or_hint`,
`filter::repair_inverted_leaves`. And before "fixing" any telemetry-surfaced
friction, verify it against CURRENT code first — usage.db is commit-mixed and
time-spanning (reconnaissance-patterns R-40).
