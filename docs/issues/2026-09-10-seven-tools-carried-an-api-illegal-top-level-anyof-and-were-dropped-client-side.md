---
kind: bug
status: fixed
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-09-10
opened: 2026-09-10
owner: marius
related: []
severity: high
unverified: fix is uncommitted in the working tree — no fix SHA or patch-id recorded yet; and the drop itself was never observed directly here, only inferred from the API contract plus the client-side rewrite this session's client performs
---

# BUG: seven tools carried an API-illegal top-level `anyOf` and were dropped client-side, invisibly

## Summary

Seven path-taking tools (`read_file`, `create_file`, `edit_file`, `edit_code`,
`references`, `symbol_at`, `call_graph`) advertised an `input_schema` with a top-level
`anyOf`. The Anthropic Messages API rejects that construct outright, so a client must drop
the tool rather than forward it — the whole request would 400 and the session would die.
The tools were therefore unreachable from any session whose client does not rewrite the
construct, for eight days (2026-09-02 → 2026-09-10), behind four green schema gates.

## Symptom (Effect)

On the affected machine, the seven tools are simply absent from the session's tool list.
There is no error, because the drop happens in the client before any request is sent. The
API error the construct would produce, if forwarded, is:

```
{"type":"error","error":{"type":"invalid_request_error",
"message":"tools.9.custom.input_schema: input_schema does not support oneOf, allOf, or anyOf at the top level"}}
```

## Reproduction

`git rev-parse HEAD` at filing: `96633d3315aaac40a871600418f37cd76d6fb223` (branch
`experiments`).

The defect is in the advertised schema, so it reproduces from `tools/list` without any
client:

```
printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"probe","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  | codescout start > /tmp/toolslist.json
```

Then read `.result.tools[] | select(.inputSchema.anyOf)`. Measured 2026-09-10: four
offenders in a bare (LSP-less) probe — `read_file`, `create_file`, `edit_file`, `edit_code`
— and three more (`references`, `symbol_at`, `call_graph`) that a bare probe cannot see
because `list_tools` hides LSP tools when no LSP is available (`src/server.rs:1408`). All
seven are unconditionally in the registry (`src/server.rs:322-341`); the probe's four is a
property of the probe, not of the population.

**The client-side drop is NOT reproducible on every client, and that is the defect's
signature rather than a gap in this record.** The client this bug was filed from
*sanitizes*: it rewrites the top-level `anyOf` into a synthesized description line
(`Input constraint: Provide parameters for at least one of: (path) or (file_path) or …`) and
keeps the tool. That string exists nowhere in this repo — `grep(pattern="Input constraint")`
returns 0 matches across the tree — which is how the rewrite was attributed to the client.
So on this machine all seven tools are present and working, while on a forwarding client
they are absent.

## Environment

Linux, `codescout 0.15.0`, stdio MCP transport, project `codescout`, branch `experiments`.

**A binary-age explanation was proposed and is falsified.** The initial analysis suggested
the unaffected Linux box was simply running a binary built before 2026-09-02, predating the
`anyOf`. Measured 2026-09-10: `~/.cargo/bin/codescout` is a symlink to
`target/release/codescout`, built the same day, and `codescout version` reports
`git_sha b057cc6d` — well after the offending commits. The schema carries the `anyOf` and
the tools reach the agent anyway. **The difference is the client, not the binary.**

## Root cause

`src/server.rs:2802`'s gate `required_names_no_key_that_has_a_declared_alias` reasons
correctly that naming `path` alone in a flat `required` is false once an alias
(`file_path`, `relative_path`, `file`) can discharge the same need, and prescribes the one
JSON Schema construct that states the alternation honestly: a top-level `anyOf`. Its
companion, `required_path_branch_covers_all_path_param_aliases` (`src/server.rs:3089`
before this fix), then *positively asserted* that each of the seven schemas carries an
`anyOf` branch per accepted alias.

That reasoning is sound as JSON Schema and unshippable as a wire format: the construct that
expresses the requirement honestly is the construct the API forbids. **A correctness gate
produced an unshippable artifact, and enforced it.**

The failure could not surface server-side by construction: the client drops the tool before
sending, so the server is never consulted and every server-side probe returns clean. The
four schema gates added in `dac1068a` each asked whether a schema was *honest*; none asked
whether it was *sendable*.

Measured 2026-09-10: `git show -s --format='%H %ad' --date=short ff60dbc4 af974c0a dac1068a`
→ 2026-09-02, 2026-09-03, 2026-09-02. The construct is eight days old at filing.

## Evidence

### The served schema carries the combinator

From the `tools/list` probe above, `read_file`'s served top-level keys are
`['anyOf', 'properties', 'type']`, with:

```json
"anyOf": [
 {"required": ["path"]}, {"required": ["file_path"]},
 {"required": ["relative_path"]}, {"required": ["file"]},
 {"required": ["output_id"]}
]
```

### The API contract

`input_schema` must be a plain `"type": "object"` at the root; `oneOf`/`allOf`/`anyOf` are
rejected there, though they are permitted *nested* inside a property. The recommended
remedy is exactly the one taken here: make the parameters optional and validate at runtime
that at least one was supplied. Reported repeatedly against Claude Code
(anthropics/claude-code#4886, #5973, #27337, #40075) and reproduced outside Anthropic's
stack (home-assistant/core#160565, whose `anyOf` likewise encoded "at least one of these
fields"). `skipSchemaValidation` does not help — it bypasses local validation only; the API
still rejects.

### The second gate would have blocked the obvious fix

`required_path_branch_covers_all_path_param_aliases` asserted the `anyOf` branches exist for
exactly the seven affected tools. Removing the construct without rewriting that gate reds
the build, and the *first* gate's failure text instructed the reader to "express it with
`anyOf`" — so the guidance in the tree actively steered the next session into
re-introducing the defect. Both were rewritten as part of this fix.

## Hypotheses tried

1. **Hypothesis** — a platform difference: the Linux binary predates the `anyOf`, so it has
   no offending schema. **Test** — `codescout version` on the running binary, plus `ls -la`
   on the `~/.cargo/bin` symlink and `git show -s --date=short` on the three commits.
   **Verdict** — rejected. Binary is `b057cc6d`, built 2026-09-10, and serves the `anyOf`
   while the tools still reach the agent. **Evidence** — § Environment.
2. **Hypothesis** — the tools were collapsed away rather than dropped (`read_markdown` and
   `edit_markdown` are genuinely gone). **Test** — read `ff60dbc4` and `af974c0a` subjects.
   **Verdict** — rejected as an explanation for the seven; those two commits fold
   `read_markdown` into `read_file` and `edit_markdown` into `edit_file`, which is a
   deliberate collapse and unrelated to the drop.
3. **Hypothesis** — the drop is client-side, and this session's client sanitizes rather than
   drops. **Test** — `grep(pattern="Input constraint")` over the whole tree; compare against
   the synthesized line present in this session's own tool descriptions. **Verdict** —
   confirmed. 0 matches in-tree, so the string is client-generated. **Evidence** —
   § Reproduction.

## Fix

Applied in the working tree; **uncommitted at filing, so no SHA or patch-id is recorded
yet.** Four parts:

1. **Removed the top-level `anyOf` from all seven schemas**, each replaced by a four-line
   comment naming the API constraint and pointing at the new gate, so the next reader does
   not restore it: `src/tools/read_file.rs:37`, `src/tools/create_file.rs:36`,
   `src/tools/edit_file/mod.rs:390`, `src/tools/symbol/edit_code.rs:116`,
   `src/tools/symbol/references.rs:233`, `src/tools/symbol/symbol_at.rs:359`,
   `src/tools/symbol/call_graph/mod.rs:387`.
2. **Added no compensating top-level `required` naming `path`** — that is the false claim
   the first gate exists to prevent. The schemas now state *nothing* about which alias is
   needed, which is the only honest thing they can state without a combinator. This is the
   shape `grep` already ships and has shipped throughout, so the target state was already
   proven in-tree.

   **A real cost, stated so nobody pays it back with the defect.** On a sanitizing client
   the top-level `anyOf` was being rendered *for the agent* as a synthesized
   `Input constraint: Provide parameters for at least one of: (path) or (file_path) or …`
   line, and that line is now gone: the schema no longer tells a caller a path is needed.
   What replaces it is the runtime `RecoverableError`, whose hint names the accepted
   aliases and a concrete correct call — which is this repo's documented preference anyway
   (`docs/PROGRESSIVE_DISCOVERABILITY.md`: fail with a recovering hint rather than bloat
   every schema). **Do not restore the constraint by re-adding the combinator**; it is
   API-illegal, and the seven per-site comments plus
   `no_tool_schema_declares_a_top_level_combinator` exist to stop exactly that. If the
   discoverability gap ever measures as real, pay for it in the tool's own
   `description` prose, which is legal on the wire.
3. **Presence is enforced at runtime**, where it always was:
   `require_path_param` (`src/fs/mod.rs:251`) accepts `path` plus every
   `PATH_PARAM_ALIASES` entry and fails with a `RecoverableError` naming them;
   `read_file` has its own equivalent chain including `output_id`
   (`src/tools/read_file.rs:79`), and `create_file`/`edit_file` use
   `require_str_param_or_hint` with the same alias set (`src/tools/core/params.rs:139`).
4. **Ratcheted `TOOL_SURFACE_CHAR_BUDGET`** 57_296 → 56_485 (−811). The rule on that
   constant is to set it to the exact measured total rather than bank headroom; the 811
   chars were what the seven `anyOf` blocks occupied, and they bought seven unreachable
   tools.

## Tests added

- `no_tool_schema_declares_a_top_level_combinator` (`src/server.rs:3160`) — registry-wide:
  no tool's `input_schema` carries top-level `oneOf`/`allOf`/`anyOf`. Scoped to the whole
  registry rather than a hand-list, because the constraint belongs to the transport, not to
  any tool's semantics. Carries two non-vacuity assertions (registry size ≥ 15, and each of
  the seven affected tools present), because an empty or truncated registry would otherwise
  satisfy it by finding nothing.
- `path_requiring_tools_never_name_path_or_an_alias_in_required` (`src/server.rs:3075`) —
  the rewritten former `required_path_branch_covers_all_path_param_aliases`. Keeps the
  honesty half, **widened**: the old form checked only `path`, so `required: ["file_path"]`
  was the same lie and passed. Asserts its hand-list fully intersects the live registry, so
  a renamed tool cannot silently drop out.

**Both mutations were run and both REDs were observed** (2026-09-10), per § *Testing
Discipline*'s demand for an observed red on the production path rather than an assertion's
existence:

- Re-added `"anyOf": [...]` to `read_file`'s schema → `no_tool_schema_declares_a_top_level_combinator`
  FAILED naming `read_file`. Reverted.
- Set `create_file`'s `required` to `["content", "file_path"]` →
  `path_requiring_tools_never_name_path_or_an_alias_in_required` FAILED naming
  `create_file` and `"file_path"`. Reverted. This is the mutation the **old** gate would
  have passed, so the widening is demonstrated rather than asserted.

## Workarounds

None needed on a sanitizing client — the tools are present. On a client that drops them,
there is no client-side workaround: rebuild from a tree carrying this fix
(`cargo rb`, then `/mcp` to reconnect).

## Resume

N/A — fix applied and both regression gates observed red under mutation. Outstanding only:
commit it, then record the fix SHA **and** its patch-id
(`git show <sha> | git patch-id --stable`) in § Fix, and archive via
`doc(action="move", …)` once the four-command gate is green on `experiments`.

## References

- `src/server.rs:2802` — `required_names_no_key_that_has_a_declared_alias`, the gate whose
  correct reasoning produced the unshippable schema.
- `docs/conventions/what-green-is-evidence-for.md` — the loudness law this instantiates at
  layer granularity: a gate that cannot observe the client cannot speak about it.
- anthropics/claude-code#4886, #5973, #27337, #40075 — the API rejection, reported
  repeatedly.
- home-assistant/core#160565 — the same `anyOf`-as-"at-least-one-of" encoding, same
  rejection, outside this stack.
