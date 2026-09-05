---
id: 581a1a6378878fef
kind: bug
status: fixed
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
fix_patch_id: 037ce550126c46ca6569843e7a8ef1133dcc35d1
fix_patch_id_plugin: ecf9efa7a9202f8f613fef23f42ad070949a41ab
fix_sha: 1dacd204ddd593fe2187fe6a18f18a0bc7eb6848
fix_sha_plugin: claude-plugins:677fb6c9433477b921c8620e94323b2d41d2c490
fixed: 2026-09-05
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
## Fix

**Fixed 2026-09-04/05.** All four suggested steps taken, and step 1's prediction (*"expect it to
red; the reds are the worklist"*) was right: widening to `.mjs` produced **14 drift items across 10
files**.

### The gate (codescout)

1. **`mjs` added to the extension filter.** Measured before: the gate walked **3 of 41** files in
   the hooks directory and **0 of the 20 `.mjs`** that carry every message body it exists to
   police.
2. **`scrub_js_comments`**, the `.mjs` twin of the existing `scrub_shell_comments`. Not optional:
   the hooks cross-reference codescout's *Rust* symbols in comments (`guide_ledger.rs`'s
   `read_entries`, `src/util/fs.rs`'s `state_dir_from()`). Measured: scrubbing removes **7 of the
   8** non-tool snake_case identifiers, and all seven are such cross-references.
3. **`RETIRED_TOOL_NAMES`, shared.** Exactly as this file proposed — the six names in
   `the_registry_is_exactly_the_post_collapse_surface` and the five here were two hand-maintained
   copies of one list, and their **union of 11** is the real population. Both tests now read it, so
   retiring a tool is one edit and both directions follow: *must not be registered* and *must not
   be named in companion text*.

### One thing this file could not have predicted, and it is a real flaw in the obvious fix

Merging the lists put **`artifact`** into a `\b`-anchored scan — and `artifact` is this domain's
most common noun. The first green-to-red run flagged *"For a librarian-managed **artifact**
(docs/trackers)"* as drift, in a message this very fix had just written. The old five names
(`replace_symbol`, `insert_code`, …) are not English words, so the problem could not arise before.

So entries carry a third field, `ambiguous_as_english`. Exactly one is true. An ambiguous name
matches only in a **tool-reference context** — adjacent to `` ` ``, `'`, `"`, `|`, `_`, `(`, or
introduced by `codescout ` — while unambiguous names keep matching as bare words, which is what
catches un-delimited drift like `read_markdown/edit_markdown` in a bullet list.

**The limitation is documented at the refusal site rather than left to be discovered:** bare prose
of the form *"run artifact find"* with no delimiter and no `codescout ` prefix is not caught. That
is the price of not redding the build on the word "artifact", and it is the right trade only
because every *mechanical* reference — an argv entry, a matcher alternation, a call form, a
backticked mention — carries a delimiter by construction.

### The hooks (claude-plugins)

10 files. The two that matter most were not stale prose:

- **`goal-stop-hook.mjs` was broken at runtime.** It shells out to `codescout artifact find`,
  `artifact get`, and `artifact-event list --artifact-id`. Verified positively with a control:
  `codescout artifact find` → *"error: unrecognized subcommand 'artifact'"*, exit 2, while
  `codescout doc find` → exit 0. Its own error branch logs *"failed or returned empty"*, so three
  dead subprocess calls presented as "no active goal" and the hook silently did nothing. A second
  latent break on the same line: `--artifact-id` is now `--id`.
- **`il4-deny-hook.mjs` was a total capability outage** — see its own section below.

The rest: `subagent-guidance.mjs` (`artifact(action=…)` → `doc`, plus the `status="open"` query
that hides `taken`/`investigating`/`zombie`), `hooks.json` matcher, and five files carrying
`edit_markdown`/`read_markdown` in tool lists, matcher regexes and guidance text — including two
copies of a line that had become **exactly inverted**: *"Markdown: read_markdown/edit_markdown, NOT
read_file/edit_file"*, which now names the dead tools as correct and the live ones as wrong.

### `il4-deny-hook.mjs` — deleted, not reworded

This file's step 3 said to fix the remedy text to `read_file(path, heading=…)`. Text alone could not
work: the hook **denies** `mcp__*__read_file` on every `.md`, so corrected text would have denied a
call and then instructed the reader to make that same call.

An intermediate fix narrowed the predicate so an **addressed** read (`heading`, `headings`, a line
range, `force`) passed and only a bare whole-file read was nudged. **The operator then directed the
simpler answer: remove the hook entirely**, on the grounds that codescout already computes the
output after it arrives and buffers only when needed — so a pre-call gate was redundant from the
start, not merely mis-worded.

Verified after removal, all three regimes:

| input | result |
|---|---|
| 159-byte markdown | full content inline |
| 57 KB markdown, bare read | heading map + line numbers + an `@file_*` handle + a slice recipe |
| librarian-managed artifact | refused, with a hint naming live `doc(…)` tools |

So the adaptivity the hook was written to enforce is a property of the server, and had been all
along. `il4-deny-hook.mjs`, `il4-deny-hook.test.sh` and the `hooks.json` matcher are gone.

**Reproduced live before the fix and re-verified after**, which is the only reason the incoherence
of the text-only fix was noticed at all: `read_file(path="docs/PROBES.md", heading="## Related")`
was denied outright, and now returns the section list.

### The archived bug that said this was already fixed

`docs/issues/archive/2026-09-03-il4-deny-hook-will-deadlock-markdown-reads-after-the-fold.md`
(`13382b706c9c77b0`) **predicted this exact deadlock, was archived as fixed on 2026-09-04, and every
checkable claim in its `## Fix` is false.** The commit SHA it cites does not exist in the plugin
repo; `il4-deny-hook.mjs`'s whole history is one commit and it was never deleted; both files were
tracked at `HEAD`; it claims a 1.20.4 ship while all three profiles record 1.19.9.

The mechanism is the part worth keeping. `sdd-misc-plugins` is a `"source": "directory"` marketplace
whose `installLocation` is the **working tree itself**, so the plugin is served from source and the
version-numbered `plugins/cache/…/1.19.x/` directories are inert leftovers — which is what the
archiving session inspected. That is `reconnaissance-patterns:R-89` **inverted**: the law warns the
installed copy can be staler than source, and here the consumer loads source while the *cache*
misleads. Every upstream proxy — install record, version number, cache diff — agreed with the wrong
answer, and the single artifact that settles it (`known_marketplaces.json`) is not one anybody
thinks to read, because nothing suggests a version-numbered cache is inert.

That file now carries the full correction and is marked `zombie`. `docs/architecture/companion-plugin.md`
carried the same false retirement — and had **struck through a line that was correct** (*"It still
fires (observed this session)"*) in order to assert it — and is corrected too.
### Fix SHAs — two repos, cite both

The gate and the hooks it polices live apart, and either half alone leaves a defect: the gate
without the hook fixes reds the build, the hook fixes without the gate go unguarded again.

| repo | SHA | patch-id |
|---|---|---|
| codescout | `1dacd204ddd593fe2187fe6a18f18a0bc7eb6848` | `037ce550126c46ca6569843e7a8ef1133dcc35d1` |
| claude-plugins | `677fb6c9433477b921c8620e94323b2d41d2c490` | `ecf9efa7a9202f8f613fef23f42ad070949a41ab` |

**Gate:** `cargo fmt` scoped to `src/server.rs` (a peer held uncommitted Rust, and workspace-wide
`fmt` would have rewritten it — `2fc50a3d46aa77a9`); clippy `-D warnings` exit 0; lean lane exit 0;
default lane exit 0.

Read by **test name, not lane total**: `companion_surfaces_reference_only_real_tools`,
`the_registry_is_exactly_the_post_collapse_surface`, and
`retired_name_matching_discriminates_tool_references_from_prose` all pass. The last is the one that
makes the other two mean anything — it shares `retired_name_regex` with the gate rather than
re-implementing it, so it cannot be green while the gate is broken.

**One honest note on the clippy reading.** The first run was **red**, with four errors in
`src/librarian/tools/reindex.rs` — a file this change never touches. Attributed to a peer by
`git status` rather than assumed, reported as *"red, attributed to a peer"* rather than as clean,
and re-run to exit 0 after they reverted. It turned out to be a deliberate mutation-test mid-round,
which from outside is indistinguishable from work left broken. A clippy reading, like a peer count,
is valid only at its instant.
## Resume

Not started. **The fix is two-sided:** the gate widening is `src/server.rs` and lands on
`experiments` normally; the hook text is `claude-plugins/` and must be cited with the
`claude-plugins:<sha>` cross-repo prefix per CLAUDE.md § *Git Workflow*. Do not archive on the
gate half alone — a widened gate that reds is not a fixed hook.
