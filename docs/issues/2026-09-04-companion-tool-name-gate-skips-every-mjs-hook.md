---
id: dd487faf8140b79f
kind: bug
status: open
title: 'BUG: companion_surfaces_reference_only_real_tools reads no .mjs and checks a pre-collapse stale list, so every hook message is unguarded'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- companion-plugin
- gates
- tool-surface-collapse
- hooks
topic: companion plugin hook prescribes a retired tool name
opened: 2026-09-04
related:
- docs/issues/archive/2026-09-03-retired-tool-names-survive-in-the-surfaces-that-actually-reach-agents.md
- docs/issues/2026-09-02-four-manual-surfaces-still-describe-read-markdown-in-the-present-tense.md
repo: claude-plugins
severity: high
---

## Summary

`companion_surfaces_reference_only_real_tools` (`src/server.rs:3893-4031`) is the gate that
reaches out of this repo to check the companion plugin's hook surfaces for retired codescout tool
names. It is narrower than its name in **two independent ways**, and either alone is enough to
miss every hook message body:

1. **It reads no `.mjs`.** `if !matches!(ext, "sh" | "json") { continue; }` — and every hook in
   `claude-plugins/codescout-companion/hooks/` is `.mjs`. The gate walks essentially only
   `hooks.json`.
2. **Its stale-name list predates the collapse it should be policing.**
   `stale_names = ["replace_symbol", "insert_code", "remove_symbol", "edit_lines",
   "create_or_update_file"]` — none of the six names retired on 2026-09-02 (`read_markdown`,
   `edit_markdown`, `artifact`, `artifact_event`, `artifact_augment`, `artifact_refresh`) is in
   it. So even a `.sh` hook naming `read_markdown` would pass.

The gate's docstring states the intent it does not deliver: *"**Stale-name sentinel:**
known-removed names must not appear in live (non-comment) code in companion hook files. Catches
the wider text drift — message bodies that list nonexistent tools to the model on SessionStart,
BLOCKED notices, etc."* A BLOCKED notice is exactly what surfaced this.

## How it surfaced — a hard deny with no working escape

Session `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`, 2026-09-04, `~/.claude-sdd` profile.

`hooks/il4-deny-hook.mjs:19` denies **every** `read_file` call on a `.md` path and prescribes
`read_markdown` — a name `the_registry_is_exactly_the_post_collapse_surface`
(`src/server.rs:4092-4147`) asserts must never be registered, *"There is no alias shim by design
— the old name returns the MCP unknown-tool error."*

```
read_file(path="docs/conventions/cross-machine-catalog-resume.md")
  → IL4 violation — BLOCKED. Markdown files must use `read_markdown(path)` …

ToolSearch("select:mcp__codescout__read_markdown,mcp__codescout__edit_markdown")
  → No matching deferred tools found

read_file(path="CLAUDE.md", heading="## Git Workflow")
  → IL4 violation — BLOCKED.        ← heading param does not exempt it
```

So there is no working call left: the block is path-extension-only (`heading=`, `headings=`,
`start_line=`, `force=true` all still denied), and the prescribed remedy is unknown to the
server. Three times in one session the correct call was refused and the suggested one
unusable; work routed around it via `run_command("sed -n …")` and `doc(action="get", heading=…)`.

The hook's message also asserts *"`read_file` on `.md` is **also hard-rejected by the in-server
gate**"*. That is false — `read_file`'s own contract is *"Markdown: heading map by default;
`heading=`/`headings=` for a section, `force=true` for raw lines"* — and it is the load-bearing
falsehood, because it is what stops a reader suspecting the hook rather than the server.

`src/tools/markdown/read_markdown.rs` still exists as the **implementation** behind `read_file`'s
markdown path; only the `impl Tool` went (`src/server.rs:2313`: *"Was 21 until `read_markdown`
was folded into …"*). That is why grepping the source finds the name and appears to vindicate
the hook.

## Why this is the interesting part, not the hook line

`docs/issues/archive/2026-09-03-retired-tool-names-survive-in-the-surfaces-that-actually-reach-agents.md`
(`efebb412864f9252`, `status: fixed`, same class) already found that the collapse's sweep was
measured over *"what directories did I think of"* rather than *"what text reaches an agent"*. Its
§ *Why the gates missed it* reads:

> The gate author did think past the repo: `companion_surfaces_reference_only_real_tools`
> (`src/server.rs:3759`) reaches into the plugin. So the population was extended once, on the
> axis that was salient …

**That sentence is the reason nobody checked how far it reaches.** It is offered as the
reassuring half of a finding about incomplete populations — the one gate that *did* extend — and
it is the gate this file is about. The archived bug's § *The gate gap that remains* names only
one residual (paren-less runtime strings); the `.mjs` exclusion and the pre-collapse stale list
are in neither that section nor § *Two things deliberately NOT changed*. They are new.

That also means the class instance is **`guard-narrower-than-its-name`, not
`doc-contradicted-by-code`**. The hook text is drift; the defect is a guard whose stated subject
(*"companion hook files"*, *"message bodies"*) is broader than its parsed extent (`.sh`/`.json`,
five pre-collapse names). This file was filed under `cluster/doc-contradicted-by-code` for about
fifteen minutes before reading the gate; recorded because the mis-framing is the ordinary
mistake here — the symptom is a text surface, so the text surface looks like the class.

Note the shape is also a near-twin of `IC-14`'s documented exemplar — a guard whose *stated*
subject is a field and whose *parsed* extent is that field's first line
(`scripts/pre-commit-ledger-counts.py:376`). Same defect, different substrate: stated population
vs. walked population.

## Blast radius, not yet enumerated

Enumerated 2026-09-04 over all 40 files in `claude-plugins/codescout-companion/hooks/`
(20 `.mjs`, 18 `.sh`, 1 `.json`, 1 `.jsonl`).

**The gate is GREEN right now.** Observed, not assumed:
`cargo test --workspace companion_surfaces_reference_only_real_tools` →
`test server::tests::companion_surfaces_reference_only_real_tools ... ok`.

| retired name | live files naming it |
|---|---|
| `read_markdown` | **6** — `explore-inject.mjs`, `il4-deny-hook.mjs`, `pre-tool-guard.mjs`, `session-start.mjs`, `subagent-guidance.mjs`, **`hooks.json`** |
| `edit_markdown` | **5** — `cs-activate-project.mjs`, `session-start.mjs`, `subagent-guidance.mjs`, `worktree-activate.mjs`, `worktree-write-guard.mjs` |
| `artifact` | **1** — `hooks.json:160`, inside a live PreToolUse matcher alternation |
| `artifact_event` / `artifact_augment` / `artifact_refresh` | 0 |

**What the gate actually walks is 3 files of 40**, not the "companion hook files" its docstring
names: `ext` must be `sh` or `json`, and `*.test.sh` is skipped by design — which excludes 16 of
the 18 `.sh`. That leaves `detect-tools.sh`, `il3-deny-hook.sh` and `hooks.json`. The 20 `.mjs`
holding every hook's logic and message bodies are 100% excluded.

### The third narrowness — why even the one walked file passes

`hooks.json:160` is a live `PreToolUse` matcher:

```json
"matcher": "mcp__.*__(symbols|run_command|read_file|grep|semantic_search|edit_code|edit_file|read_markdown|tree|references|call_graph|symbol_at|workspace|artifact|memory|index|onboarding|librarian)"
```

The gate's positive check is `positive_re = mcp__codescout__\(?([a-z_|]+)\)?` — it requires the
**literal** `mcp__codescout__`. This matcher uses the **wildcard** `mcp__.*__(…)` form, so the
alternation list is invisible to it. Two retired names sit in that one line, in the one file the
gate reads, and the check designed for exactly this shape cannot match it.

So there are three independent narrownesses, and closing any two still leaves the gate green:

1. **extension** — `.mjs` never read (20 files, all hook logic and message bodies);
2. **stale list** — `stale_names` predates the 2026-09-02 collapse, so none of the six retired
   names is checked even where the gate does read;
3. **matcher form** — positive check keys on literal `mcp__codescout__`, missing the wildcard
   `mcp__.*__` form that `hooks.json` actually uses.

### One thing deliberately NOT a defect

`il4-deny-hook.test.sh:65` is the only place using the `mcp__codescout__read_markdown` form:

```sh
assert "wrong-tool-read_markdown" "$(mkinput 'CLAUDE.md' 'mcp__codescout__read_markdown')" "allow"
```

The gate skips `*.test.sh` on purpose — *"those exercise stale names on purpose as regression
sentinels"* — so this is correctly excluded and must stay. Recorded so it is not "fixed" later.
It is worth noting what it asserts, though: that a `read_markdown` call is **allowed** through,
which encodes the retired name as a live tool in the hook's own test expectations. Whoever fixes
the hook text will need to decide what that case should assert instead.
## Suggested direction (not a plan — enumerate first)

1. **Widen the extension filter to `mjs`** and re-run. Expect it to red; the reds are the
   worklist.
2. **Derive `stale_names` from the registry's own dead list** rather than restating it.
   `the_registry_is_exactly_the_post_collapse_surface` (`server.rs:4137`) already holds the six
   retired names as a literal; two hand-maintained copies of one list is the mechanism that
   produced this bug. Extract it to a shared `const` both tests read, so retiring a tool updates
   one place.
3. **Fix `il4-deny-hook.mjs`'s remedy text** to `read_file(path)` / `read_file(path, heading=…)`
   and delete the false in-server-gate claim.
4. The general mechanizable shape is `IC-11`'s open note — a registry-keyed check over
   *everything delivered to a model*, not a scanner over a path set. This instance is a
   known-answer fixture for it.

## Resume

Not started. **The fix is two-sided:** the gate widening is `src/server.rs` and lands on
`experiments` normally; the hook text is `claude-plugins/` and must be cited with the
`claude-plugins:<sha>` cross-repo prefix per CLAUDE.md § *Git Workflow*. Do not archive on the
gate half alone — a widened gate that reds is not a fixed hook.
