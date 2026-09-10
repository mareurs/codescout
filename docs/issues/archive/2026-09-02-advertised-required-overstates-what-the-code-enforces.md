---
id: 1cbf3728d07c507f
kind: bug
status: fixed
title: 'BUG: advertised `required` over-states what the code enforces — an alias set and a conditional default both name one member of an alternation'
tags:
- cluster/doc-contradicted-by-code
closed: 2026-09-10
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-09-02-workspace-schema-requires-an-action-the-code-does-not.md
severity: low
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

## The census the `unverified:` field asked for (2026-09-10)

That field read: *"site count is a candidate list of ten, not a census — only read_markdown and
read_file were probed; the other eight declare required:[path] but were not confirmed to route
through the alias resolvers."* Here is the census, derived rather than assumed, and it is what the
field existed to force.

**Method** — intersect the callers of the alias-accepting resolvers with the tools that actually
advertise a schema:

```
grep -rln 'get_path_param\|require_path_param\|PATH_PARAM_ALIASES' --include=*.rs src/
  -> 15 files; then for each, does it declare `fn param_aliases`, and does it `impl Tool for`?
```

### CAUSE A — CLOSED, and only eight of the ten were ever sites

| site | disposition |
|---|---|
| `create_file`, `edit_file`, `grep`, `read_file` | **collapsed** — `d5f2b736` |
| `references`, `symbol_at`, `call_graph`, `edit_code` | **collapsed** — `34cad9d9` |
| `read_markdown`, `edit_markdown` | **ceased to be sites.** Both carry `impl Tool for` **zero** times — folded into `read_file`/`edit_file`, which now normalize at the `call_content` boundary before either helper is reached. A module that advertises no schema cannot over-state a `required`. |
| `symbols` | **never a site.** Grepping `"required"` in `src/tools/symbol/symbols.rs` returns nothing — it declares no top-level `required` at all, so there is no false claim to make. This is the ADR amendment's own stated exclusion reason, now confirmed at the bytes rather than taken from the ADR. |
| `list_overview` | **never a site** — `impl Tool for` zero times; a helper module behind `symbols`. |

So the ten-candidate list resolves to **eight real sites, all collapsed**, plus three non-sites
(two by folding, one by never having had a `required`). The remedy was not to edit any `required`
array: with exactly one advertised name per concept, `required: ["path"]` became *true*.

That is also why `path_requiring_tools_never_name_path_or_an_alias_in_required` was **deleted**
rather than kept — post-collapse it would red on an honest schema. Its residue is now covered from
the other direction: a registry-wide `required ⊆ properties` assertion, so `required: ["file_path"]`
— a key no caller can discover — reds.

### CAUSE B — ALSO CLOSED, independently and before this plan

The conditional default was `workspace`'s. `src/tools/config/mod.rs` now declares **no `required`
array at all**, and `action`'s own description reads *"Required unless post_compact=true, which
implies status."* So the false claim is gone and the alternation is documented where a caller reads
it. Fixed in `dac1068a`, which both removed the array and added that sentence — found with
`git log -S` on each half, not inferred.

Other tools still declaring `required: ["action"]` (`librarian`, `doc`, `peer`, `library`, `memory`,
`index`, `edit_code`) are **not** instances: they were never claimed to be, and none was reported to
carry a conditional default. Not re-probed here, and that is stated rather than implied — this
census covers the ten candidates this file named, not every `required` array in the registry.

### Guards, so this cannot silently return

Cause A: four gates in `src/server.rs` — an honesty check with a per-tool `EXPECTED_ALIAS_PAIR_COUNTS_BY_TOOL`
table (member-level, all eight tools), a real-boundary gate driving `call_tool_inner`, an extracted
alias-declaration predicate with a positive/negative fixture, and a per-member gate on `read_file`'s
two extra pairs. `bf0a5241` and `0711600d`. Cause B: the registry-wide schema gates `dac1068a` added.

Gate green in both lanes at the time of this update, verified per binary with `--no-fail-fast`:
lean 33 binaries / 3714 passed / 40 ignored / 0 failed; default 36 / 5738 / 52 / 0.

### Fix provenance — SHA and patch-id per CLAUDE.md, recorded once at fix time

| commit | patch-id | what |
|---|---|---|
| `d5f2b736` | `026119dc39ce896447166170bd44346170df31a1` | cause A, four file tools |
| `34cad9d9` | `c09bc0ab94cae1b3c522610a070ccd171f759f5c` | cause A, four symbol tools |
| `bf0a5241` | `0dd42eee5905c91a9a0ed9bd1f40ccab1f85af65` | gates replaced |
| `0711600d` | `0b93eafc32028c61c492f811ca7072b482b7f53d` | gates: member-level table + real boundary |
| `dac1068a` | `371bee7c5081481311866cd797d7591abb2c5bc3` | cause B |

All on `experiments`. The census was requested as a condition of handing this file over, by the
session that had been holding it — their point being that the sweep makes this session the party who
can *answer* the `unverified:` field rather than close it by assumption, which is what that field
exists to prevent.
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

**Note added 2026-09-09:** cause B (`workspace`) is now fixed and archived — `dac1068a`,
patch-id `371bee7c5081481311866cd797d7591abb2c5bc3`, which also shipped the schema-shape gate
as `required_names_no_key_that_has_a_declared_alias` (`src/server.rs:2842`). **Cause A, the
alias half this file owns, is not closed by that** — verify it against the gate before
archiving this file, since the gate's existence is not the same claim as the alias instances
being gone.
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

- `docs/issues/archive/2026-09-02-workspace-schema-requires-an-action-the-code-does-not.md` — cause B,
  owned by `tool-collapse` Task 3.
- `src/fs/mod.rs:227-262` — the alias constant and both resolvers.
- `src/tools/config/mod.rs:46,51-57` — the conditional default.
- `src/librarian/tools/mod.rs:578` — `assert_required_are_advertised`, the converse check.
- `655c0b6f` (patch-id `2ae27c8a135edae59191b0b840b90956bb97ca6d`) — the description-vs-enum
  gate whose derive-the-population shape this should copy.
