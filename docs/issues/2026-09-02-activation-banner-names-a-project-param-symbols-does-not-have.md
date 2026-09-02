---
status: open
opened: 2026-09-02
closed:
severity: medium
owner: marius
related: []
tags:
  - cluster/accepted-parameter-silently-dropped
kind: bug
---

# BUG: the activation banner tells agents to pass `project:` to `symbols`, which has no such param and drops it silently

## Summary

Every `workspace(action="activate")` response ends with *"Use `project: "<id>"` in `symbols` /
`semantic_search` / `memory` to scope to a specific project."* Of the three tools named,
`symbols` accepts no project parameter at all and silently ignores one, `semantic_search`'s
is spelled `project_id`, and only `memory` honours `project` — via an alias added for a
2026-06-09 bug filed against this very sentence. The banner was fixed once, on one tool of
three, by making the tool match the prose; the prose was left to keep misdirecting the other
two.

## Symptom (Effect)

Live, 2026-09-02, after `workspace(action="activate", path=".")` printed the banner:

```
symbols(name="ClientIdentity", project="codescout-embed")
→ src/tools/core/types.rs (1)
    Struct  254  ClientIdentity
```

`src/tools/core/types.rs` belongs to the `codescout` project, not `codescout-embed`. The
param was dropped: no error, no `corrections`, a result from the wrong project scope
indistinguishable from a correct one.

Field instances, `.codescout/usage.db`, 30 days to 2026-09-02: **0** `symbols` calls carry a
`project` or `project_id` key. (A first query returned 8; all 8 were searches *for a symbol
named* `project_id` — `{"name":"project_id",…}` — not a param. Recorded so the next reader
does not publish the 8.)

## Reproduction

`git rev-parse HEAD` → `4dc0daa2`. In a workspace with two `[[project]]` entries:

```
workspace(action="activate", path="<root>")        # read the last line of the response
symbols(name="<symbol only in project A>", project="<project B id>")
```

Observe the project-A result.

## Environment

Linux, codescout `experiments` @ `4dc0daa2`, Claude Code over stdio. Requires a
multi-project workspace for the banner to print the sentence.

## Root cause

The sentence is emitted at `src/prompts/mod.rs:203`:

```rust
"\nUse `project: \"<id>\"` in `symbols` / `semantic_search` / `memory` to scope to a specific project.\n"
```

Against the three tools it names:

| tool | advertised param | reads `project`? | source |
|---|---|---|---|
| `symbols` | none — schema has `scope` only | no | `src/tools/symbol/symbols.rs:153`; no `project` lookup anywhere in the file |
| `semantic_search` | `project_id` | no (`project_id` only) | `src/tools/semantic/semantic_search.rs:575`, `:622` |
| `memory` | `project_id`, alias `project` | yes | `src/tools/memory/mod.rs:447-448` — comment: *"project" accepted as an alias for project_id (2026-06-09 onboarding-prompt bug)* |

Nothing checks that a `tool(param=…)` a prompt surface names is a param that tool
advertises. `prompt_surfaces_reference_only_real_tools` (`src/server.rs:2727`) checks
backticked **tool names** against the registry and deliberately nothing finer.

Measured 2026-09-02: the live call above; the grep of `src/tools/symbol/symbols.rs` for `"project"` (one hit,
the `scope` description's word *project*); the alias comment at `src/tools/memory/mod.rs:448`.

## Evidence

### Live call
Under *Symptom*.

### The 2026-06-09 repair that fixed one tool of three
`src/tools/memory/mod.rs:448` — the alias exists *because* this banner said `project`. The
fix made `memory` match the banner and did not touch `symbols` or `semantic_search`, which
the same sentence names.

### usage.db false positive
`SELECT input_json FROM tool_calls WHERE tool_name='symbols' AND input_json LIKE '%"project%'`
→ 8 rows, every one `"name":"project_id"` or `"query":"project_id"`. Zero real instances.

## Hypotheses tried

1. **Hypothesis:** `symbols` reads `project` under another name (`project_id`).
   **Test:** grep `src/tools/symbol/symbols.rs` for both. **Verdict:** rejected — neither is read.
2. **Hypothesis:** agents have been burned by this in the window. **Test:** the usage.db
   query. **Verdict:** rejected at the 30-day floor — 0 instances; the 8 hits were symbol
   searches. Severity is therefore about the silent-drop shape, not observed damage.

## Fix

Plan, not implemented. Two halves:

1. **Correct the sentence** at `src/prompts/mod.rs:203` to what the tools accept —
   `project_id` for `semantic_search` and `memory`; and either drop `symbols` from the list
   or give `symbols` a `project_id` param (it has `scope`, which is a different axis:
   project/libraries/all). The `workspace` pin is the documented cross-project mechanism for
   every pinnable tool and may be the better thing to advertise here.
2. **Gate it.** Extend `prompt_surfaces_reference_only_real_tools` — or add a sibling — to
   parse `tool(param=` / ``param: "…"` in `tool`` pairs and assert each param exists in that
   tool's advertised `input_schema()`. Tool names alone passed this sentence for three months.

## Tests added

None yet. Owed: the gate in *Fix* step 2, plus a `symbols` test asserting an unknown
`project` key produces a `corrections` note or an error rather than silence.

## Workarounds

Use `workspace="<abs path>"` (the pin every pinnable tool advertises) or
`workspace(action="activate", …)` to switch project. For `semantic_search`/`memory`, pass
`project_id`.

## Resume

Edit `src/prompts/mod.rs:203`; write the param-existence gate; run
`cargo test --lib prompt_surfaces`.

## References

- `src/tools/memory/mod.rs:448` — the 2026-06-09 partial repair.
- `docs/trackers/prompt-surface-compaction-session-log.md`, 2026-09-02 review section.
- `docs/trackers/issue-clusters.md` `IC-15`.
