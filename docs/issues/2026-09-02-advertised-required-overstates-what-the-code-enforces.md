---
id: '28f8efe6804ae266'
kind: bug
status: open
title: 'BUG: advertised `required` over-states what the code enforces — an alias set and a conditional default both name one member of an alternation'
tags:
- cluster/doc-contradicted-by-code
closed: null
opened: 2026-09-02
owner: marius
related:
- docs/issues/2026-09-02-workspace-schema-requires-an-action-the-code-does-not.md
severity: low
unverified: site count is a candidate list of ten, not a census — only read_markdown and read_file were probed; the other eight declare required:[path] but were not confirmed to route through the alias resolvers
---

# BUG: advertised `required` over-states what the code enforces, by two causes, on ten candidate tools

## Summary

A tool's `inputSchema.required` is a promise about what `call` refuses. On `workspace` and
on every path-bearing tool it names strictly more than the code enforces, so the advertised
contract is narrower than the real one. **The code is correct in both cases and the schema is
the wrong artifact** — `file_path` is a documented, intentional alias and `post_compact` is a
documented conditional default. The fix is to make `required` truthful, never to make `call`
stricter.

## Symptom (Effect)

Two calls on `target/debug/codescout` at `8fb5f638`, each omitting a parameter the wire says
is required, each succeeding:

```
read_markdown(file_path="docs/issues/_TEMPLATE.md")   # no `path`
  → full document, 171 lines

read_file(output_id="@cmd_60eff867")                  # no `path`
  → file content
```

Advertised schema for both: `"required": ["path"]`.

Independently reproduced by `codescout-0a` (PID 4165881) on the same binary — the
`read_markdown` call, 171 lines returned.

## Reproduction

`git rev-parse HEAD` → `8fb5f638`.

```
python3 - <<'PY'
import importlib.util
spec = importlib.util.spec_from_file_location("pts", "scripts/probe_tool_surface.py")
pts = importlib.util.module_from_spec(spec); spec.loader.exec_module(pts)
for t in pts.fetch_tools("target/debug/codescout"):
    req = (t.get("inputSchema", {}) or {}).get("required", [])
    print(f"{t['name']:18} required={req}")
PY
```

Then call any tool whose `required` names `path`, passing `file_path` instead.

## Environment

Linux 7.1.9-zen1-2-zen, codescout MCP over stdio, project `codescout`, branch `experiments`,
`~/.claude-sdd` profile.

## Root cause

One class, two causes. Both are a schema asserting a single required name where the code
implements an **alternation**.

**Cause A — alias sets.** `src/fs/mod.rs:230` declares
`PATH_PARAM_ALIASES = ["file_path", "relative_path", "file"]`. `get_path_param`
(`src/fs/mod.rs:234`) and `require_path_param` (`src/fs/mod.rs:249`) both resolve
`input["path"]` **or** any alias, erroring only when all four are absent. So `required:
["path"]` names one member of a four-way alternation as though it were the whole of it.

**Cause B — a conditional default.** `src/tools/config/mod.rs:46` declares `"required":
["action"]`; `:51-57` reads `post_compact` first and has an explicit
`None if post_compact => "status"` arm. The code's contract is *"action, or post_compact"*.

Measured 2026-09-02: both mechanisms read at the bytes; cause A confirmed by two live calls
above; cause B's live path independently evidenced by usage.db in the sibling file — 31 calls
passing `post_compact` with no `action`, all `error_msg NULL`.

**Why no test catches it.** `all_tools_have_valid_schemas` (`src/server.rs`) checks shape
only. `param_probe::assert_required_are_advertised` (`src/librarian/tools/mod.rs:578`) runs
the **other** direction — required-by-code ⇒ advertised — and only over the librarian family.
Nothing runs advertised ⇒ actually-enforced.

## Evidence

### Candidate sites — labelled candidate, not census

Tools whose advertised `required` names `path`: `read_file`, `edit_file`, `edit_markdown`,
`read_markdown`, `create_file`, `approve_write`, `symbol_at`, `references`, `call_graph`,
`edit_code`. Plus `workspace` under cause B.

**Two proven by probe (`read_markdown`, `read_file`); the other eight are NOT measured** —
they are tools that declare `required: ["path"]`, which makes them candidates on the
mechanism, not confirmed instances. Whether each routes through `get_path_param` /
`require_path_param` was not checked per tool.

### The remedy direction was wrong on first reading, and inverting it matters

This file's first framing was *"two independent mechanisms"* with a gate asserting
**advertised ⇒ actually-refused**. `codescout-0a` corrected both, and the correction is load-
bearing rather than editorial:

- A gate in that shape **fails on all ten path-bearing tools on the merits** — the code
  accepts `file_path` and should. A gate whose only true reading demands a wrong change gets
  weakened or ignored, which is worse than no gate.
- It also needs a per-tool *minimal valid input* to probe with, which is a test harness of
  its own; recorded independently as Deviation 4 of that session's plan.

So the two causes are **one class with one remedy direction**: make the schema express the
alternation.

## Hypotheses tried

1. **Hypothesis** — this is a single-site defect in `workspace`.
   **Test** — dumped `required` for all 26 tools, then probed a tool from a different family.
   **Verdict** — rejected. `read_markdown` and `read_file` fail by a different cause.
   **Evidence** — § *Symptom*.

2. **Hypothesis** — the fix is to make `call` refuse the alias.
   **Test** — read the alias declaration and its doc comment; read the schema property
   descriptions, which say *"Alias for path"*.
   **Verdict** — rejected. The affordance is deliberate and documented, and breaking it would
   fail live agent calls. The schema is the wrong artifact.
   **Evidence** — `src/fs/mod.rs:227-230`; § *Evidence* above.

3. **Hypothesis** — a behavioural gate (advertised ⇒ refused) is the right guard.
   **Test** — evaluated against the ten path-bearing tools.
   **Verdict** — rejected, per § *Evidence*. Superseded by the schema-shape gate below.

## Fix

Not implemented here. **Make `required` truthful; do not make the code stricter.** A reader
who implements the strict version breaks a documented affordance — that is the one wrong turn
this section exists to prevent.

Express the alternation in the schema, e.g. `anyOf` over the accepted spellings, or drop the
name from `required` and let the code's `RecoverableError` remain the contract.

**The guard that generalises is a schema-shape gate, and it needs no minimal inputs:** for
every tool, for every name `N` in `required`, if any other declared property is an alias of
`N`, then naming `N` alone in `required` is false — the schema must express the alternation.
Derivable statically from `server.tools` plus `PATH_PARAM_ALIASES`, in the same
derive-the-population shape as the description-vs-enum gate at `655c0b6f`.

A single-site test (`workspace_does_not_require_action`) is **monotone under the defect
existing elsewhere** — it passes forever while nine siblings stay wrong. That is
CLAUDE.md § *Testing Discipline*'s "mutate once per guarded SITE, not once per feature".

**Ownership, so this does not get fixed twice or merged by a later reader.** The `workspace`
instance (cause B) is owned by `codescout-0a`'s `tool-collapse` Task 3, together with the
schema-shape gate, which replaced the single-site test that branch originally specified.
This file owns **cause A**, the alias half, which no task on that branch touches.

## Tests added

None. The gate described above is the test, and it lands with cause A's fix.

## Workarounds

None needed — the code accepts more than the schema advertises, so no call fails. The cost is
informational: an agent reading the schema believes `path` is mandatory when it is not, and a
client that enforces `required` client-side would reject calls the server would have served.
Claude Code does not enforce it today, which is why 31 `post_compact`-only calls succeeded.

## Resume

Write the schema-shape gate first — it derives the real population and turns the candidate
list in § *Evidence* into a measured one, which is the cheaper order. Read
`src/server.rs` around the description-vs-enum gate from `655c0b6f` for the
derive-over-`server.tools` shape to copy, and `src/fs/mod.rs:230` for the alias constant it
must read. Then fix the schemas the gate names.

## References

- `docs/issues/2026-09-02-workspace-schema-requires-an-action-the-code-does-not.md` — cause B,
  owned by `tool-collapse` Task 3.
- `src/fs/mod.rs:227-262` — the alias constant and both resolvers.
- `src/tools/config/mod.rs:46,51-57` — the conditional default.
- `src/librarian/tools/mod.rs:578` — `assert_required_are_advertised`, the converse check.
- `655c0b6f` (patch-id `2ae27c8a135edae59191b0b840b90956bb97ca6d`) — the description-vs-enum
  gate whose derive-the-population shape this should copy.

