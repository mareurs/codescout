---
kind: bug
status: fixed
tags:
- cluster/accepted-parameter-silently-dropped
closed: 2026-09-02
opened: 2026-09-02
owner: marius
related: []
severity: medium
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

**Fixed on `experiments` at `2fc064f7`**
(`2fc064f71c0af239570323b8673c8a54596cd740`), patch-id
`67a3401033bd4e57f470586d30b65acc644a5af2`. The SHA is positional and dies when
`experiments` is rebased; the patch-id survives rebase and cherry-pick.

Both halves this section prescribed, plus a third the fix forced.

**1. The sentence.** Now names only what the tools accept, and gives `symbols`
its real mechanism — `path` — rather than a phantom one, noting that its `scope`
is a different axis (project/libraries/all). The `workspace` pin was considered
as this section suggested and not used: it takes an absolute path and is the
cross-*root* mechanism, where this sentence is about sub-projects of one
workspace.

**2. The gate.** `prompt_surfaces_name_only_params_their_tools_advertise`, a
sibling rather than an extension — see *Tests added*. It also scans a **fourth
surface** this file did not identify: the sentence is emitted from
`build_project_status_segments`, which is not `SERVER_INSTRUCTIONS`, the
onboarding prompt, or the draft. So `prompt_surfaces_reference_only_real_tools`
could not have caught it *at any grain* — the diagnosis in *Root cause*, that
tool names alone are too coarse, is true but is the second reason, not the first.

**3. The alias had to go, and had to become a refusal.** Correcting the prose
left `memory` honouring a `project` key its schema never advertised — the
mirror of this bug, and unreachable to any agent reading the tool list. It is
removed. It is a **RecoverableError naming `project_id`**, not a deletion:
deleting the `or_else` outright would have restored the original 2026-06-09
defect in its harder direction — `project` silently dropped, the write misrouted
to the focused project, a scoped-looking result that is not scoped. No live
caller used it; every `memory(project=` hit in the tree is inside an **archived**
bug file recording the original incident.

**A side effect worth its own line: the tool-surface ratchet was LOWERED**
56_547 → 56_497, the first payback in its log. The alias's description spent 50
characters explaining itself (*"key is project_id; project accepted as an
alias"*), and removing the alias removed the sentence. Old budget minus 50 is
the new measured total exactly, so the whole headroom is attributable. The
general form: **the cheapest bytes in a surface budget are in a description that
is long because the tool is wrong.**
## Tests added

`prompt_surfaces_name_only_params_their_tools_advertise` (`src/server.rs`),
plus a rewritten `memory_refuses_the_project_key_and_still_routes_project_id`
and a repaired assertion in `build_with_workspace_appends_project_table`.

**A sibling, not an extension of `prompt_surfaces_reference_only_real_tools`.**
That test's two-way allowlist tripwire is tuned to token-vs-registry drift;
folding a param check into it would have made one failure message answer two
different questions. The new one reads `` `param: …` `` declarations and
attributes them to backticked tool names in the same sentence, then checks each
against that tool's advertised `input_schema()` properties.

**Scope, stated so nobody credits it with more.** `tool(param=…)` call-form is
*not* covered — a larger population, left to a later pass rather than smuggled
in untested. And it asserts `checked > 0`, so a rewording that removes the
pattern entirely fails loudly instead of going vacuously green.

**The repaired assertion is the more instructive one.**
`build_with_workspace_appends_project_table` asserted
`block.contains("project: \"<id>\"")` — **satisfied by the defect**, for three
months, in the same test file that renders the sentence. A substring check that
both the right and wrong spellings satisfy is not a guard; it is a guard-shaped
line. It now asserts the corrected spelling *and* that `symbols` is not named as
taking one.

**Mutation-verified, per site:**

| mutation | result |
|---|---|
| original sentence restored | gate names all three: `symbols`, `semantic_search`, `memory` |
| alias restored (`or_else`) | refusal test fails — the call succeeds where it must not |
| alias deleted with no refusal (the *dangerous* alternative) | refusal test fails on the silent-drop path |

The third is the one worth keeping: it is the mutation that a naive fix would
have shipped, and the test catches it.
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
