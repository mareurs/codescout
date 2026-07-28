---
status: fixed
opened: 2026-06-11
closed: 2026-06-11
severity: low
owner: marius
related: []
tags: [get_guide, guide_hints_emitted, token-efficiency, usage-db]
kind: bug
---

# BUG: get_guide re-fetches the same static guide topic within one session

## Summary
`get_guide(topic)` could be invoked twice for the same topic in a single MCP
session, re-injecting a static guide body (~1.3k tokens for `tracker-conventions`)
that the model already had. The explicit `get_guide` tool was stateless w.r.t.
the per-session `guide_hints_emitted` ledger, and six of eight topics receive no
auto-inject "don't re-call" hint at all. Pure context waste; no correctness risk.

## Symptom (Effect)
Same-`(session, topic)` `get_guide` rows in `usage.db` (debug builds, which
persist `input_json`). Observed in two independent projects:

```
codescout/.codescout/usage.db        session 9f68db0a  tracker-conventions x2  (12:19:41 → 12:19:46)
code-explorer.old/.codescout/usage.db session 38e3c0be  tracker-conventions x2
```

Out of ~23 topic-bearing `get_guide` calls across all live debug DBs, 2 were
redundant same-topic re-fetches (~9% — a floor, since pre-2026-05-24 rows lack
`input_json` and are undetectable).

## Reproduction
- `git rev-parse HEAD` at fix: `039e8829` (experiments).
- In one MCP session, call `get_guide("tracker-conventions")`, do unrelated
  work, then call `get_guide("tracker-conventions")` again. Pre-fix: both
  return the full body with no warning. Post-fix: the second returns the body
  plus a `note` flagging the prior fetch.
- Evidence query (debug builds only):
  ```sql
  SELECT session_id, json_extract(input_json,'$.topic') topic, COUNT(*) n
  FROM tool_calls WHERE tool_name='get_guide'
  GROUP BY session_id, topic HAVING n > 1;
  ```

## Environment
codescout v0.15.0, Linux, MCP stdio transport, `--debug` (so `usage.db`
records `input_json` — gated since commit `86a8573a`, 2026-04-02). All three CC
profiles launch codescout with `["start","--debug"]`; topic-less historical rows
predate `--debug` being added to the config (~2026-05-22 to 05-24), not a
misconfiguration.

## Root cause
A two-part asymmetry in the `guide_hints_emitted` ledger
(`CodeScoutServer.guide_hints_emitted`, `Arc<parking_lot::Mutex<HashSet<String>>>`):

1. **`GetGuide::call` ignored the context.** Its signature was
   `async fn call(&self, input, _ctx: &ToolContext)` — `_ctx` underscore-ignored
   (`src/tools/guide.rs`). It returned `self.topics.get(t)` (a static compile-time
   map) unconditionally, never reading or writing the ledger. So a repeat fetch
   was never detected, and an explicit fetch never marked the topic emitted.
2. **Only auto-injected topics get a "don't re-call" hint.** That hint
   (`_guide_hint` + V2 body block) fires from `Tool::call_content`
   (`src/tools/core/types.rs:485-615`), gated by `relevant_guide_topic()`. Only
   `librarian` and `progressive-disclosure` are wired for it. The other six topics
   — `tracker-conventions`, `error-handling`, `workspace-state`,
   `iron-laws-detail`, `symbol-navigation`, `librarian-runtime` — are never
   auto-injected, so the model never sees any re-fetch discouragement for them.
   `tracker-conventions` is the canary: high-traffic (tracker work) and in the
   unhinted six.

## Evidence

### usage.db duplicate query
`9f68db0a` timeline (codescout DB) — no `workspace(activate)` between the two
`get_guide` calls, ruling out the ledger-reset-on-activation path:
```
12:19:41  get_guide      tracker-conventions
12:19:42  librarian      (tracker_design)
12:19:46  get_guide      tracker-conventions   <- redundant
```

### The stateless signature
`src/tools/guide.rs`, pre-fix:
```rust
async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<Value> {
    ...
    Some(body) => Ok(json!({ "topic": t, "body": *body })),
```

### Auto-inject keyspace (the symmetry target)
`src/tools/core/types.rs:514` — auto-inject inserts the bare topic string:
```rust
if should { emitted.insert(topic.to_string()); Some(topic.to_string()) }
```

## Hypotheses tried
1. **Ledger reset on `workspace(activate)` between the two calls.** Test:
   inspect the `9f68db0a` tool_calls timeline. Verdict: **rejected** — no
   activation between 12:19:41 and 12:19:46.
2. **Auto-inject fired the second copy.** Test: auto-injected guides ride on
   another tool's response and never create a `tool_name='get_guide'` row; both
   dups are real `get_guide` rows. Also `tracker-conventions` has no
   `relevant_guide_topic()` producer. Verdict: **rejected** — these are explicit
   model calls.
3. **Misconfiguration (some projects lack `--debug`).** Test: read `.claude.json`
   on all three profiles + correlate null-topic rows with dates. Verdict:
   **rejected** — uniform `--debug`; null rows are pre-config-change history.

## Fix
Implemented in `src/tools/guide.rs` (`GetGuide::call`). Experiments-side commit
`039e8829` — **not yet on master**; update to the master-side SHA after
cherry-pick (see CLAUDE.md § "After cherry-pick: cite the master SHA").

`GetGuide::call` now takes `ctx` (not `_ctx`) and, on a successful topic fetch,
`ctx.guide_hints_emitted.lock().insert(t.to_string())`. `insert` returns false
when already present, driving a `note` that flips from first-fetch ("don't
re-call this session unless your context was compacted") to repeat ("already
fetched … re-fetch only needed after compaction"). The body is **never
withheld** — the ledger is not cleared on `/compact`, so a legitimate
post-compaction re-fetch must still return the guide. Side benefit: explicit and
auto-inject paths now share one keyspace, so `get_guide("librarian")` suppresses
a later librarian-tool auto-inject and vice-versa.

Doc accuracy: `src/prompts/guides/workspace-state.md` ledger row updated to
record both writers. No `ONBOARDING_VERSION` bump (guides load fresh per call;
the `onboarding_prompt` surface and `builders.rs` are untouched).

## Tests added
`tools::guide::tests::repeat_fetch_keeps_body_and_flags_static`
(`src/tools/guide.rs`) — two fetches of the same topic on a *shared* `ctx`;
asserts both return the full body (repeat is not a stub) and the note flips
first→repeat. The pre-existing `every_topic_has_non_empty_body` uses a fresh
ctx per call, so it only exercised the first-fetch path — the new test is the
first to cover the repeat path.

## Workarounds
None needed — the change is non-breaking and the cost was token waste only.
Before the fix, the mitigation was model discipline (don't re-call get_guide).

## Resume
N/A — fixed. If revisited: consider whether `error-handling` /
`iron-laws-detail` / `symbol-navigation` warrant an auto-inject producer
(`relevant_guide_topic()`) like `librarian` has, vs. the explicit-only note
this fix adds.

## References
- Fix: `src/tools/guide.rs` `GetGuide::call`; commit `039e8829` (experiments).
- Mechanism: `src/tools/core/types.rs:485-615` (`Tool::call_content` auto-inject).
- Ledger owner: `src/server.rs:61` (`guide_hints_emitted`).
- Debug-gating origin: commit `86a8573a` (2026-04-02).
