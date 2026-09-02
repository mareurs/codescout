---
status: open
opened: 2026-09-02
closed:
severity: low
owner: marius
related:
  - docs/issues/2026-09-02-index-description-omits-the-verify-action.md
tags:
  - cluster/doc-contradicted-by-code
kind: bug
---

# BUG: `memory`'s description enumerates seven actions; the enum, the dispatcher and 35 calls have eight

## Summary

The `memory` tool's description enumerates its actions in two labelled groups —
*"Topic-based: read/write/list/delete … Semantic: remember/recall/forget"* — totalling
seven. Its `action` enum has eight. `refresh_anchors` is fully wired (enum, dispatcher,
`is_write`, error text, regression test) and has been called **35 times**, and it appears
in no description surface an agent reads before its first call.

This is the same mechanism as the sibling bug filed the same day for `index`/`verify`, and
it was found by running that bug's reproduction over the whole population rather than the
one tool. **The sibling's own comparison population omitted `memory`** — see § *Why this
went unfiled*, which is the part of this report worth more than the fix.

## Symptom (Effect)

Wire (`tools/list`, 2026-09-02, HEAD `09c68634`):

```
description: Persistent project memory. Topic-based: read/write/list/delete with path-like
keys. Semantic: remember/recall/forget with bucket classification and meaning-based search.
properties.action.enum: ["read", "write", "list", "delete", "remember", "recall", "forget",
                         "refresh_anchors"]
```

`.codescout/usage.db`, all time:

```
SELECT count(*) FROM tool_calls WHERE tool_name='memory' AND input_json LIKE '%refresh_anchors%';  → 35
SELECT count(*) FROM tool_calls WHERE tool_name='index'  AND input_json LIKE '%verify%';           → 17
```

The undocumented action here is called **twice as often** as the one in the filed sibling.

## Reproduction

`git rev-parse --short HEAD` → `09c68634`. Dump the live wire and compare each tool's
description against `inputSchema.properties.action.enum`:

```
python3 - <<'PY'
import importlib.util, os
spec = importlib.util.spec_from_file_location("pts", "scripts/probe_tool_surface.py")
pts = importlib.util.module_from_spec(spec); spec.loader.exec_module(pts)
for t in pts.fetch_tools("target/debug/codescout"):
    enum = (t.get("inputSchema", {}).get("properties", {}).get("action", {}) or {}).get("enum", [])
    if not enum: continue
    missing = [v for v in enum if v not in t["description"]]
    if missing: print(t["name"], "missing", missing)
PY
```

Reports `index missing ['verify']` and `memory missing ['refresh_anchors']`, plus two
thematic-description false positives (`artifact`, `edit_markdown`) that the gate must
exclude by scoping to descriptions that *enumerate*.

`scripts/probe_tool_surface.py --json` alone does **not** reproduce this — it aggregates
the wire to character counts and discards the text. The sibling bug's Reproduction section
says to use it and compare `description` with the enum; that is not a thing its output can
do. Use the snippet above, which reuses its `fetch_tools` transport.

## Environment

Not environment-dependent. Requires a current `target/debug/codescout`.

## Root cause

`src/tools/memory/mod.rs:643-646` (`fn description`) names seven actions in two groups;
`src/tools/memory/mod.rs:689` (enum) and `:1104` (`"refresh_anchors" => …`) carry eight.
`refresh_anchors` is topic-based — it takes `topic`, as the *parameter* description at
`:693` states (*"For read/write/delete/refresh_anchors. Path-like key"*) — so it belongs in
the first group and is absent from it.

The action is not internal: `:637` lists it in `is_write`, `:1138` and `:1141` name it in
the unknown-action error and hint, `src/tools/memory/tests.rs:1813`
(`refresh_anchors_clears_staleness`) covers it, and `src/server.rs:5559` asserts its
write-classification. Every surface names it **except the one an agent reads first**.

No test relates a description's enumerated actions to its enum — the same gap the sibling
bug names. `all_tools_have_valid_schemas` checks shape only;
`tool_descriptions_stay_under_budget` checks length only.

Measured 2026-09-02 at HEAD `09c68634`: wire dump (above); `src/tools/memory/mod.rs`
`:643/:689/:1104`; usage.db counts.

## Evidence

### Wire, description-vs-enum over all 26 tools

```
DRIFT artifact             enum=12 named= 3 missing=[find, get, create, move, delete, …]   ← thematic, excluded
ok    artifact_event       enum= 2 named= 2
ok    artifact_refresh     enum= 2 named= 2
ok    edit_code            enum= 4 named= 4
DRIFT edit_markdown        enum= 5 named= 1 missing=[replace, insert_before, …]            ← thematic, excluded
DRIFT index                enum= 4 named= 3 missing=[verify]                               ← sibling bug
ok    librarian            enum=10 named=10
ok    library              enum= 2 named= 2
DRIFT memory               enum= 8 named= 7 missing=[refresh_anchors]                      ← THIS BUG
ok    workspace            enum= 3 named= 3
```

### The enumerating population is five tools, not four

`workspace` 3/3, `library` 2/2, `edit_code` 4/4, `index` **3/4**, `memory` **7/8**.

## Why this went unfiled

The sibling bug's § *Evidence → Same-surface comparison* reads:

> Of the tools whose description enumerates actions, `workspace` names 3/3, `library` 2/2,
> `edit_code` 4/4, `index` **3/4** (computed over the wire dump, 2026-09-02).

Four tools. `memory` was never in the comparison, so the second instance could not be found by
reading the record — only by re-running the measurement over the whole surface.

> **This section said "The population is five" until 2026-09-02. It is superseded by
> § *This file's own correction was also short* above** — the derived population is **10 tools
> carrying an `action` enum, 8 inventories and 2 thematic**, and "five" was itself a count of the
> tools someone had examined rather than a derivation. The sentences below that read *"two of five
> fail"* are left as written, because they record what this file claimed at the time and the
> reasoning they support does not depend on the denominator. Read the figure from the correction,
> not from here.

Two consequences, both load-bearing:

1. **The sibling's fix plan states a false premise.** It says the proposed gate means
   *"`index` fails today, the other three pass"*. Two of five fail today. A gate written and
   tuned against that sentence would be authored expecting one red and would meet two, and
   the natural repair under time pressure is to narrow the gate until it matches the
   expectation.
2. **This is `CLAUDE.md` § *Testing Discipline*'s recording law, not its member law.** The
   defect was not a sample too small to contain the second instance — the wire dump contains
   all 26 tools and always did. The *comparison* filtered `memory` out before anything could
   be observed about it, so the refuting case left no artifact. Widening the corpus would
   have changed nothing.

## Hypotheses tried

1. **Hypothesis:** `refresh_anchors` is an internal/maintenance action deliberately kept out
   of the description, unlike `verify`.
   **Test:** 35 calls in `usage.db`; present in `is_write`, both unknown-action error
   strings, a named regression test, and a `server.rs` assertion; documented as an agent-
   facing action in the `topic` parameter's own description.
   **Verdict:** rejected — it is public by every measure except the description.

2. **Hypothesis:** `artifact` and `edit_markdown` are further instances of the same defect.
   **Test:** read both descriptions. Neither claims to enumerate — `artifact` describes by
   theme ("Artifact CRUD and query"), `edit_markdown` by capability ("Edit a Markdown
   document by heading"). A reader takes neither as an action inventory.
   **Verdict:** rejected — false positives of the crude check, and exactly the exclusion the
   sibling bug predicted the gate would need.

## Fix

**Fixed on `experiments` at `655c0b6f`** (`655c0b6f6794be223f85fbad8360cd3002cc13d3`),
patch-id `2ae27c8a135edae59191b0b840b90956bb97ca6d`. The SHA is positional and dies when
`experiments` is rebased; the patch-id is a content hash of the diff and survives rebase
and cherry-pick. Both recorded here once — there is no promotion path to check.

> **This FILE is not in that commit — only the code fix it describes is.** It was
> untracked and carries `cluster/doc-contradicted-by-code`, so staging it moved that class
> from 11 to 12 while `docs/trackers/issue-clusters.md`, which publishes the count, holds
> three other sessions' concurrent edits and was held back by `7d2b3ee7`. The
> `ledger-counts` pre-commit hook refused the pair, correctly: staging the ledger would
> commit their work, and staging this file alone ships a corpus its own count contradicts.
> The coupling belongs to whoever untangles that ledger. Nothing is lost — the file is
> intact on disk, and `src/tools/memory/mod.rs` carries the fix.

**Implemented 2026-09-02**, in one change with the sibling `index` bug and the shared gate.

1. `src/tools/memory/mod.rs:644` — `Topic-based: read/write/list/delete` →
   `read/write/list/delete/refresh_anchors` (+16 chars; description 169 → 185, cap 300).
2. **A second surface this file did not name.** `long_docs()` lists the topic-based actions
   as four bullets and omitted `refresh_anchors` too, so the drift was on *both* prose
   surfaces, not one. Added `` - `action="refresh_anchors"`: re-hash a topic's code anchors,
   clearing staleness. ``, worded from the handler at `:1105`
   (`memory::anchors::refresh_hashes`) rather than guessed. `long_docs` is fetched on demand,
   so it costs nothing against the surface budget — and it is **not** covered by the gate,
   which reads `description()` only. Recorded here so no reader credits it with coverage it
   does not have.
3. The gate. Design, matcher rationale and the four-mutation verification are in the sibling
   bug's § *Tests added*; this file's prediction that it must red on **two** members before
   the fixes land was correct.

### This file's own correction was also short

Correcting the enumerating population from four to five was right about `memory` and still
not the population. The derived figure is **10 tools carrying an `action` enum — 8
inventories, 2 thematic (`artifact`, `edit_markdown`)**. The sibling additionally exempted
`librarian` by name as "describes by theme"; it names all 10 of its actions and is an
inventory, so that exemption would have dropped the surface's largest description (1,621
chars) out of the guarded set. That is a *second* error in a *different* direction — a
mis-assigned member, not a short count — which is why re-deriving the count could not reach
it.

The general form, and the reason both files now point at the gate rather than at a number:
**a population stated in prose decays independently of the surface it describes, and every
restatement is a fresh chance to be wrong.** The gate derives it from `server.tools` on
every run, and fails any tool with an `action` enum and no declared contract.

Surface cost: +37 chars across both fixes, funded by trimming `index`'s `build` clause, which
restated its own `scope` parameter. `TOOL_SURFACE_CHAR_BUDGET` was **not** raised — 56,479 →
56,516 against a 56,519 ratchet.
## Tests added

`server::tests::tool_descriptions_name_every_action_they_claim_to_enumerate`, sited beside
`tool_descriptions_stay_under_budget` in `src/server.rs` as this file proposed.

It red on exactly two members before the fixes — `memory omits ["refresh_anchors"] of 8;
index omits ["verify"] of 4` — and on nothing else, which is the check this file asked for
against the sibling's false "one red" premise.

Verified per MEMBER as this file requires: reverting each description independently kills the
gate naming that tool alone, so neither kill is credited to the other. Two further mutations
cover the arms neither bug anticipated (`undeclared`, `thematic_but_complete`) — table in the
sibling bug's § *Tests added*.

**Not covered: `long_docs()`.** The gate reads `description()` only; the `long_docs` omission
fixed in § *Fix* item 2 was found by hand and would not have been caught. Extending the gate
there is not obviously right — `long_docs` is optional, free-form, and absent on most tools —
but the gap is real and is named here rather than left to look guarded.
## Workarounds

Read the enum, or the `topic` parameter's description at `src/tools/memory/mod.rs:693`,
which names `refresh_anchors` correctly. `memory(action="refresh_anchors", topic=…)` works.

## Resume

Edit `src/tools/memory/mod.rs:644` and `src/tools/semantic/index.rs:836` together; add the
enumeration gate in `src/server.rs`; `cargo test --lib memory index`. Note
`src/tools/mod.rs` and `src/librarian/tools/` had uncommitted `param_probe` relocation work
in the tree at filing time — rebase onto it rather than around it.

## References

- `docs/issues/2026-09-02-index-description-omits-the-verify-action.md` — sibling instance,
  same mechanism, and the source of the incomplete comparison population.
- `docs/trackers/issue-clusters.md` `IC-11`.
- `CLAUDE.md` § *Testing Discipline* — the recording law this instance illustrates.
