---
status: draft
opened: 2026-05-19
owners: [marius]
tags: [mcp, prompt-surfaces, channel-caps, redesign]
related:
  - docs/architecture/mcp-channel-caps.md
---

# MCP Prompt Channel Redesign

## Summary

Claude Code truncates the MCP `initialize.instructions` field and each
tool's `description` field at ~2 KB per block, appending `… [truncated]`.
MCP resources are not exposed to the model in this profile. The only
model-autonomous channel with `>2 KB` capacity is tool call results.

This redesign accepts those constraints and reshapes codescout's prompt
delivery accordingly. The `source.md` system prompt is cut from ~42 KB
to ~1,500 chars and tightened with paired do-instead clauses on every
Iron Law. The librarian guide and other deep content move into a new
`get_guide(topic)` tool. A first-call hint mechanism auto-suggests
`get_guide(...)` exactly once per topic per session, so the model
discovers the guide channel without needing to be told twice.

Evidence base: [docs/architecture/mcp-channel-caps.md](../../architecture/mcp-channel-caps.md).

## Goals

1. Every Iron Law reaches the model intact.
2. Every Iron Law has a paired do-instead clause (Hamsa heuristic #1).
3. The librarian guide reaches the model on demand, not as dead text.
4. Subagents inherit the same prompt surface; no Claude-Code-specific
   hooks required.
5. Future content additions land in `get_guide`, not in `source.md`.

## Non-goals

- Negotiating a higher cap with Claude Code (out of our control).
- Bypassing truncation via hooks or per-host shims.
- Rewriting `onboarding_prompt.md` or `build_system_prompt_draft()`.
  Those are separate surfaces governed by `ONBOARDING_VERSION`; this
  redesign touches only `server_instructions.md` and the runtime
  composition site.

## Constraints

- `source.md` rendered output: ≤ 1,800 chars (~450 tokens) to stay
  under the observed ~2 KB cap with safety margin.
- Per-tool `description()` rendered output: ≤ 1,800 chars per tool.
- All other content delivered via tool call results, subject to
  codescout's `MAX_INLINE_TOKENS = 2,500` buffering and the
  `@tool_*` reference workflow.
- No code path may reintroduce a concat that ships through
  `initialize.instructions` and exceeds the 1,800-char budget.

## Architecture

Four surfaces, one tool, one tracker.

```
┌──────────────────────────────────────────────────────────────────┐
│ Surface A — source.md (≤1,800 chars, lives in instructions)      │
│   • Iron Laws (paired do-instead)                                │
│   • Search/Edit decision quickref                                │
│   • Buffered tool results pattern (@ref)                         │
│   • Workspace activate/restore gate                              │
│   • get_guide topic discovery line                               │
└──────────────────────────────────────────────────────────────────┘
                                │
                  references ↓
                                │
┌──────────────────────────────────────────────────────────────────┐
│ Surface B — get_guide(topic) tool (registered in tools[])        │
│   • Returns full guide text as tool result                       │
│   • Topics: librarian, tracker-conventions,                      │
│             progressive-disclosure, error-handling               │
│   • No-arg call lists topics + 1-line summaries                  │
│   • Output exceeds MAX_INLINE_TOKENS → buffered to @tool_*       │
└──────────────────────────────────────────────────────────────────┘
                                ↑
                  auto-suggests │
                                │
┌──────────────────────────────────────────────────────────────────┐
│ Surface C — First-call hint (in Tool::call_content wrapper)      │
│   • Per-session, per-topic flag in CodeScoutServer               │
│   • On first call of a tool with relevant_guide_topic = X:       │
│     prepend `_guide_hint` pointing to get_guide("X")             │
│   • Reset on workspace(activate_project)                         │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│ Surface D — docs/trackers/get-guide-topics.md                    │
│   • Librarian-discoverable tracker, status: active               │
│   • Sections: Live topics | Candidate topics | Declined topics   │
│   • Promotion rule: a friction (F-N or T-N) cites missing rule   │
│     that would live in candidate topic X → promote X to live     │
└──────────────────────────────────────────────────────────────────┘
```

## Surface A — `source.md` (≤ 1,800 chars)

Reshape the entire file. Discard everything past the existing Iron Laws.
Final shape:

```
codescout MCP — semantic code intelligence.
Subagents inherit these rules. Pass them along.

## Iron Laws (never X, do Y)

1. NEVER read_file source code → symbols(path) for overview,
   symbols(name=..., include_body=true) for bodies.
2. NEVER edit_file structural code → edit_code (LSP-aware).
3. NEVER pipe unbounded run_command output → run bare, query
   the @cmd_* buffer (grep "ERROR" @cmd_abc). Bounded LHS
   (ls, cat, awk, sed, find -maxdepth N) is OK.
4. NEVER read_file markdown → read_markdown (heading-addressed).
5. NEVER edit_file markdown → edit_markdown (heading-addressed).

## Search/Edit decision quickref

- Know name → symbols(name=X) | symbol_at(path, line, col)
- Know concept → semantic_search(query)
- Exact string/regex → grep(pattern, path=optional)
- Who calls X → references(symbol, path) — NOT grep
- Structural code edit → edit_code | Text/import edit → edit_file

## Buffered tool results (@ref)

When a tool returns {output_id: "@tool_xyz", summary, hint}:
- Result was too big to inline. Stored in the buffer.
- Query it: grep PATTERN @tool_xyz | read_file(@ref, json_path="$.foo")
  | read_file(@ref, start_line=N, end_line=M).
- Don't re-call the tool. Don't ask the user to paste content.

## Workspace gate

After workspace(activate, path=foreign), call workspace(activate, path=home)
before finishing the turn. Foreign-project state otherwise leaks.

## Deeper guidance

Call get_guide(topic) where topic in:
- "librarian"               — artifact model, filters, trackers
- "tracker-conventions"     — frontmatter, archive flow, status
- "progressive-disclosure"  — output budgets, @ref buffer details
- "error-handling"          — RecoverableError vs anyhow::bail
```

**Build-time invariant**: `build.rs` measures rendered output and fails
the build if `>1,800` chars. Test
`prompt_surfaces::source_md_under_cap` asserts the same at the unit
level.

## Surface B — `get_guide(topic)` tool

New file: `src/tools/guide.rs`.

```rust
pub struct GetGuide {
    topics: BTreeMap<&'static str, &'static str>,  // topic → body
}

impl GetGuide {
    pub fn new() -> Self {
        let mut topics = BTreeMap::new();
        topics.insert("librarian", include_str!("../prompts/guides/librarian.md"));
        topics.insert("tracker-conventions", include_str!("../prompts/guides/tracker-conventions.md"));
        topics.insert("progressive-disclosure", include_str!("../prompts/guides/progressive-disclosure.md"));
        topics.insert("error-handling", include_str!("../prompts/guides/error-handling.md"));
        Self { topics }
    }
}

#[async_trait::async_trait]
impl Tool for GetGuide {
    fn name(&self) -> &str { "get_guide" }

    fn description(&self) -> &str {
        "Fetch deep guidance for a topic. Returns the full text as the tool \
         result. Use when a codescout principle in the system prompt points \
         to a topic (e.g. 'see get_guide(\"librarian\")'). Topics: librarian, \
         tracker-conventions, progressive-disclosure, error-handling. Call \
         with no args to list topics + one-line summaries."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "topic": {
                    "type": "string",
                    "enum": ["librarian", "tracker-conventions",
                             "progressive-disclosure", "error-handling"]
                }
            },
            "additionalProperties": false
        })
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<Value> {
        let topic = input.get("topic").and_then(|v| v.as_str());
        match topic {
            None => Ok(json!({
                "topics": self.topics.keys().collect::<Vec<_>>(),
                "summaries": {
                    "librarian": "artifact model, filter syntax, trackers, augmentations",
                    "tracker-conventions": "frontmatter shape, archive flow, status vocabulary",
                    "progressive-disclosure": "MAX_INLINE_TOKENS, @ref buffer, overflow patterns",
                    "error-handling": "RecoverableError vs anyhow::bail, is_error routing"
                }
            })),
            Some(t) => match self.topics.get(t) {
                Some(body) => Ok(json!({ "topic": t, "body": *body })),
                None => Err(RecoverableError::new(
                    format!("unknown topic '{t}'"),
                    format!("available: {}", self.topics.keys().cloned().collect::<Vec<_>>().join(", "))
                ).into())
            }
        }
    }
}
```

**Content sources** (one commit per topic file):

| Topic | Source | Action |
|---|---|---|
| `librarian` | `src/prompts/librarian-guide.md` (9 KB, exists) | move to `src/prompts/guides/librarian.md`, delete `librarian::INSTRUCTIONS` |
| `tracker-conventions` | `CLAUDE.md` "Session Intelligence Trackers" + `docs/issues/_TEMPLATE.md` header | extract to `src/prompts/guides/tracker-conventions.md` (~3 KB) |
| `progressive-disclosure` | `docs/PROGRESSIVE_DISCOVERABILITY.md` (exists) | extract model-facing subset to `src/prompts/guides/progressive-disclosure.md` (~4 KB) |
| `error-handling` | `src/tools/core/types.rs::RecoverableError` docs + relevant `CLAUDE.md` sections | extract to `src/prompts/guides/error-handling.md` (~2 KB) |

**Registration**: in `src/server.rs::from_parts`, append
`Arc::new(GetGuide::new())` to the `tools` vector. Position: after
config tools (`Workspace`), before librarian adapters. Always
registered — not feature-gated.

**Delete `librarian::INSTRUCTIONS`**: after the redesign lands and
`get_guide("librarian")` is verified to return the content. Track in
the implementation plan.

## Surface C — First-call hint

Trait addition: `Tool::relevant_guide_topic(&self) -> Option<&str>`.
Default: `None`.

```rust
// src/tools/core/types.rs
fn relevant_guide_topic(&self) -> Option<&str> { None }
```

Override in:

| Tool file | `relevant_guide_topic` returns |
|---|---|
| librarian adapters (`crates/librarian-mcp/src/tools/*.rs`) | `Some("librarian")` |
| `src/tools/run_command/*.rs` (only when result is buffered) | conditional — see below |
| `src/tools/symbol/symbols.rs` (with `include_body=true` and overflow) | conditional — see below |
| All others | None (default) |

**State storage**: extend `CodeScoutServer` with

```rust
guide_hints_emitted: Arc<parking_lot::Mutex<HashSet<String>>>,
```

Initialized empty. Reset to empty on `workspace(action="activate", ...)`
in `WorkspaceTool::call`.

**Injection site**: `Tool::call_content` default impl at
`src/tools/core/types.rs:422-456`. Modify after `tool.call(input, ctx)`
returns `val`:

```rust
if let Some(topic) = self.relevant_guide_topic() {
    let mut emitted = ctx.guide_hints_emitted.lock();
    if !emitted.contains(topic) {
        // For progressive-disclosure, only hint when the result actually
        // overflowed (i.e. we're about to buffer). For other topics, hint
        // on first call unconditionally.
        let should_hint = match topic {
            "progressive-disclosure" => exceeds_inline_limit(&json),
            _ => true,
        };
        if should_hint {
            // Inject _guide_hint into the eventual response envelope.
            // Concrete shape depends on inline vs buffered path; see below.
            emitted.insert(topic.to_string());
        }
    }
}
```

`ToolContext` gains a `guide_hints_emitted: Arc<Mutex<HashSet<String>>>`
field, populated from the server's state at dispatch.

**Hint shape**:

- **Inline result**: top-level JSON object gains a `_guide_hint` field:
  ```json
  { "_guide_hint": "First call this session for 'librarian'. Run get_guide(\"librarian\") for full rules.",
    "<actual fields>": "..." }
  ```
- **Buffered result**: hint joins `output_id` / `summary` / `hint`
  in the envelope:
  ```json
  { "output_id": "@tool_abc", "summary": "...", "hint": "...",
    "_guide_hint": "First call this session for 'progressive-disclosure'. Run get_guide(\"progressive-disclosure\") for buffer/overflow details." }
  ```

**Why per-topic, not per-tool**: a session that already saw the
librarian hint via `artifact(...)` doesn't need it again via
`artifact_event(...)`. Topic dedup, not tool dedup.

**Why progressive-disclosure fires only on overflow**: the rule is
about the `@ref` buffer; emitting before a buffer exists would be
noise. Strictly more useful when the rule becomes actionable.

## Surface D — Tracker for future topics

New file: `docs/trackers/get-guide-topics.md`.

Frontmatter:

```yaml
---
kind: tracker
status: active
title: Get-guide candidate topics
owners: []
tags: [prompt-surfaces, guide, channel-caps]
---
```

Body sections:

- `## Live topics` — table: topic | source file | last_updated
- `## Candidate topics` — numbered entries (see seed below)
- `## Declined topics` — entries considered and rejected (avoid rehash)

**Seed entries** under `## Candidate topics`:

| Candidate | Source | Promote-when |
|---|---|---|
| `anti-patterns` | extract from `docs/trackers/tool-usage-patterns.md` T-N entries | ≥3 distinct T-N cite a missing rule outside the 4 live topics |
| `run-command-budget` | `src/tools/run_command/*.rs` + Iron Law #3 extended | A friction shows the bounded-LHS clause was ignored |
| `symbol-navigation` | `src/prompts/language_nav.rs` (14 KB, exists) | A friction shows model failed at symbol-vs-references choice |
| `subagent-coordination` | new — handoff patterns, isolation modes | Any subagent-related F-N or T-N entry |
| `prompt-surface-consistency` | `CLAUDE.md` section of same name | (low priority — meta-rule for codescout devs, not model usage) |

**Promotion process**: when a candidate's promote-criterion fires,
move row from Candidates to Live, write
`src/prompts/guides/<topic>.md`, register topic in `GetGuide::new()`,
update Surface A's topic list if budget allows. One commit per
promotion.

## Data flow — three illustrative paths

### Path 1: Model calls `artifact(find, kind="bug")` for the first time

1. Tool dispatched via `Tool::call_content`.
2. `Artifact::call(...)` returns a normal find result.
3. Wrapper checks: `relevant_guide_topic == Some("librarian")` →
   topic not in `guide_hints_emitted` → set should_hint = true.
4. Wrapper inserts `_guide_hint: "First call this session for
   'librarian'..."` into the JSON response.
5. Inserts `"librarian"` into `guide_hints_emitted`.
6. Subsequent `artifact_event(...)` call: same topic, set contains
   `"librarian"` → no hint.

### Path 2: Model calls `run_command("cargo test")` with overflow

1. Tool returns a long output → `exceeds_inline_limit(json) == true`.
2. Wrapper buffers as `@cmd_xyz`, builds the standard envelope
   `{output_id, summary, hint}`.
3. Wrapper checks: `relevant_guide_topic == Some("progressive-disclosure")`,
   topic not in set, overflow occurred → emit hint.
4. Envelope becomes `{output_id, summary, hint, _guide_hint}`.

### Path 3: Model calls `get_guide("librarian")`

1. `GetGuide::call` looks up `"librarian"` in its topic map.
2. Returns `{topic: "librarian", body: "<9 KB content>"}`.
3. Wrapper: body exceeds `MAX_INLINE_TOKENS` → buffered to
   `@tool_xyz`.
4. Model receives `{output_id: "@tool_xyz", summary, hint}`.
5. Model queries the buffer using the pattern Surface A taught.

## Components

| Component | Path | New? |
|---|---|---|
| Reshaped system prompt | `src/prompts/source.md` | rewrite |
| Build-time invariant | `build.rs` (size assert), `tests/...` | new |
| Guide tool | `src/tools/guide.rs` | new |
| Guide content files | `src/prompts/guides/*.md` | new (4 files) |
| Trait addition | `src/tools/core/types.rs` (`relevant_guide_topic`) | extend |
| Per-tool overrides | librarian adapters, run_command, symbols | extend |
| Session state | `src/server.rs::CodeScoutServer` (`guide_hints_emitted`) | extend |
| Hint injection | `src/tools/core/types.rs::call_content` | extend |
| Activation reset | `src/tools/config/workspace.rs::activate` | extend |
| Librarian concat removal | `src/server.rs::from_parts` lines 89-92 | delete |
| `librarian::INSTRUCTIONS` deletion | `src/librarian/...` | delete |
| Candidate topics tracker | `docs/trackers/get-guide-topics.md` | new |
| ADR promotion | `docs/architecture/mcp-channel-caps.md` | status → adopted |

## Tests

| Test | File | Asserts |
|---|---|---|
| `source_md_under_cap` | `src/prompts/mod.rs` tests | rendered `server_instructions.md` ≤ 1,800 chars |
| `every_iron_law_has_do_instead` | `src/prompts/mod.rs` tests | each `NEVER ...` line is followed within 2 lines by `→`/`do`/`use` clause |
| `get_guide_lists_topics` | `src/tools/guide.rs` tests | no-arg call returns the 4 topic names |
| `get_guide_returns_librarian_body` | `src/tools/guide.rs` tests | content len > 2 KB, includes a known marker string from the source |
| `get_guide_unknown_topic_is_recoverable` | `src/tools/guide.rs` tests | error is `RecoverableError`, lists available topics |
| `first_artifact_call_emits_librarian_hint` | `src/server.rs` tests | response JSON contains `_guide_hint`, set updated |
| `second_artifact_call_no_hint` | `src/server.rs` tests | response lacks `_guide_hint` |
| `artifact_event_after_artifact_no_hint` | `src/server.rs` tests | same topic suppresses |
| `activate_project_resets_hints` | `src/server.rs` tests | hint re-emits after activation |
| `run_command_without_overflow_no_progressive_hint` | `src/server.rs` tests | inline result, no hint |
| `run_command_with_overflow_emits_progressive_hint_once` | `src/server.rs` tests | buffered result, hint present; second overflow, no hint |
| `librarian_instructions_const_removed` | grep test | `librarian::INSTRUCTIONS` no longer referenced |
| `every_tool_description_under_cap` | `src/server.rs` tests | every `Tool::description()` returns ≤ 1,800 chars |

## Error handling

- `get_guide("unknown")` → `RecoverableError` (`is_error: false`), so
  sibling tool calls in the same turn survive. The error message
  includes the available topics.
- A topic body file missing at compile time is a build error
  (`include_str!` fails); no runtime drift.
- The session-state lock contention path: `parking_lot::Mutex` lock
  is held for the duration of a set lookup + insert; trivial. No
  observable latency impact.
- If `guide_hints_emitted` is poisoned (logic bug), the wrapper
  falls back to emitting the hint (safer default — model gets the
  hint redundantly rather than never).

## Migration

Three commits, in order:

1. **Surface B + guide content**: ship `GetGuide` tool with all four
   topic files. `librarian::INSTRUCTIONS` still concatenated for
   backward compatibility. No `source.md` changes yet. Tests:
   guide_* tests above.
2. **Surface A rewrite + invariant**: shrink `source.md` to the
   ≤1,800-char shape. Remove the runtime `librarian::INSTRUCTIONS`
   concat from `from_parts`. Add the build-time size assert. Tests:
   `source_md_under_cap`, `every_iron_law_has_do_instead`,
   `librarian_instructions_const_removed`.
3. **Surface C hint mechanism**: add `relevant_guide_topic` trait
   method, session state, injection in `call_content`, activation
   reset. Override on librarian adapters, run_command, symbols.
   Tests: full hint suite.

Surface D (tracker) ships alongside commit 2 — it documents the
boundary of what landed in Surface B.

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Hamsa's audit was wrong: some non-coding workflow needed librarian content in `instructions` and we never measured it | medium | get_guide is autonomously callable; first-call hint catches the model on its first librarian-touching action. Recovers within one tool call. |
| Per-tool `description` cap turns out to be lower than 2 KB on some MCP client we don't test | low | Build-time invariant on `description()` length per tool. Cap at 1,800 chars per tool — same budget as `source.md`. |
| `get_guide("librarian")` returns 9 KB → buffered → model needs to know `@ref` pattern → which lives in `source.md` → which the model may not have read carefully | medium | Surface A's Buffered tool results section is in the first 1,500 chars. First-call hint also reminds the model. Worst case: model queries the buffer with grep, finds the section it needs. |
| Adding a trait method (`relevant_guide_topic`) breaks downstream `impl Tool` blocks | low | Default implementation returns `None`. No downstream change required to compile. |
| The hint pollutes JSON shape for tool consumers that programmatically parse responses | low | `_guide_hint` keys are advisory and never required. Field name prefixed with `_` to mark non-load-bearing. |

## Open questions resolved (in brainstorming session)

- **Topic set**: B — librarian, tracker-conventions, progressive-disclosure, error-handling. Confirmed.
- **Headroom in Surface A**: used for decision quickref, buffer pattern, workspace gate. Confirmed.
- **First-call hint**: auto-suggest yes. Confirmed.
- **`librarian::INSTRUCTIONS` deletion**: after Surface C lands, not before. Confirmed.
- **Progressive-disclosure hint trigger**: only on actual buffer overflow, not on first call. Confirmed.

## References

- Evidence base: `docs/architecture/mcp-channel-caps.md`
- Friction trackers: `docs/trackers/skill-frictions.md`,
  `docs/trackers/tool-usage-patterns.md` (artifact `b3fa993849ac83ab`)
- Current prompt source: `src/prompts/source.md`
- Composition site: `src/server.rs::from_parts`
- Resource composition site: `src/server.rs::build_resource_registry`
- Tool trait + buffer pattern: `src/tools/core/types.rs`
- Existing librarian content: `src/prompts/librarian-guide.md`
- Probe artifacts (debug-only): `src/tools/probe.rs`, `src/mcp_resources/probe.rs`
