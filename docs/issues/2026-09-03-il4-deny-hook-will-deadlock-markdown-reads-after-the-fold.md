---
kind: bug
status: taken
tags:
- cluster/gate-keyed-on-unobservable-event
claimed_at: 2026-09-04
claimed_by: d2bc134a-4e6b-470f-b742-1abd5b278279
opened: 2026-09-03
owner: marius
related: []
severity: high
---

# BUG: the companion plugin's IL-4 hook will deadlock every markdown read once the fold ships

## Summary

`il4-deny-hook.mjs` hard-denies `read_file` on any `.md` path and instructs the caller to use
`read_markdown`. Task 7 of the tool-surface collapse folded `read_markdown` into `read_file`,
so once the new binary ships the hook denies the **only** tool that can serve the call and
redirects to one the server no longer registers. Markdown reads have no working path at all.

## Symptom (Effect)

Observed live this session, from the current (pre-ship) binary:

```
IL4 violation — `read_file(path="src/prompts/guides/librarian.md")` on markdown. BLOCKED.

Markdown files must use `read_markdown(path)` — heading-addressed,
size-adaptive, slice-able. `read_file` on `.md` is also hard-rejected by
the in-server gate; calling it costs a wasted round-trip and a
`tool_calls` row.

Use `read_markdown` first try:
  • read_markdown(path)                              — heading map
  ...
```

Today this is merely wrong-but-survivable: the running binary predates the fold, so
`read_markdown` still exists and the redirect lands somewhere. **After the ship it is a
deadlock** — `read_file` denied by the hook, `read_markdown` unknown to the server.

## Reproduction

1. Build and install the collapsed binary (`cargo rb`), reconnect with `/mcp`.
2. With `codescout-companion` active, call `read_file(path="README.md")`.
3. Hook denies, naming `read_markdown`.
4. Call `read_markdown(path="README.md")` → MCP unknown-tool error.

## Environment

codescout `experiments` @ worktree `.worktrees/tool-collapse`; companion plugin at
`../claude-plugins/codescout-companion/`. Hook is `PreToolUse`, matcher `mcp__.*__read_file`,
`permissionDecision: deny`. Enforcement is per-profile; observed under `~/.claude`.

## Root cause

**The hook's condition lives outside its observation boundary, so it proxies.** The real
question is *"is `read_file` the wrong tool for this path?"* — a fact about the **server's
registered tool surface**. A `PreToolUse` hook is a JS file in a different repository with no
access to that surface, so it substitutes a hardcoded proxy: *the path ends in `.md`*.

The proxy was exactly right for eighteen months and silently stopped being right the moment
`read_markdown` was folded away. Nothing failed at the seam: the hook still evaluates
correctly against its own rule, still emits a confident, well-formatted message, and returns a
**plausible instruction** rather than an error. That is `IC-2`'s claim verbatim — *"a proxy
returns a plausible answer rather than an error"*.

Two properties make it worse than an ordinary stale rule:

- **The two halves are in different repositories**, so no gate in either can see both. The
  codescout-side gates (`prompt_surfaces_reference_only_real_tools`,
  `claude_md_contains_no_deprecated_tool_names`, `guide_bodies_contain_no_deprecated_tool_names`)
  scan codescout's surfaces and cannot reach a plugin; the plugin has no copy of the tool
  registry to check against.
- **It fails closed, not open.** A stale *advisory* would waste a round-trip. A stale
  `permissionDecision: deny` removes the capability.

Read from the hook's observed behaviour (message quoted above, this session) plus
`docs/architecture/companion-plugin.md`'s inventory line. The hook source was **not** read for
this filing — `docs/architecture/companion-plugin.md` warns that the hook source is not ground
truth for runtime behaviour and that the probe is; the probe is what is recorded here.

## Evidence

### The deny, observed 2026-09-03

Quoted verbatim under *Symptom*. It fired on a `read_file` call this session and named
`read_markdown` as the remedy.

### The other side of the deadlock

`read_markdown` is no longer registered: Task 7 (`aff08b09`) deleted `impl Tool for
ReadMarkdown`, and `src/prompts/mod.rs`'s `DEPRECATED_TOOL_NAMES` now lists `read_markdown`,
which is what makes three prompt-surface gates refuse to mention it.

## Hypotheses tried

1. **Hypothesis:** the hook is advisory and merely costs a round-trip.
   **Test:** read the inventory in `docs/architecture/companion-plugin.md`; the line sits
   under the heading *"PreToolUse (guards — hard `permissionDecision: deny`)"*, and the
   observed message says `BLOCKED`.
   **Verdict:** rejected — it denies.

2. **Hypothesis:** the in-server gate would still serve the call, so the hook is redundant
   rather than blocking.
   **Test:** the hook is `PreToolUse`; it decides before the server is reached.
   **Verdict:** rejected — the server never sees the call.

## Fix

Not applied here. Task 12 of the tool-surface collapse owns the companion plugin, and the fix
is to **delete the hook**, not to update its message: after the fold there is no wrong tool to
redirect away from, so the rule it enforces no longer exists.

**Ordering is the load-bearing part, and it is a cross-repo ship sequence.** The plugin change
must land *before or with* the binary. If codescout ships first, every session using the
plugin loses markdown reads until the plugin catches up — and the plugin is a different repo
with its own cadence, so "before or with" is a decision someone has to make rather than a
thing that happens.

## Tests added

None, and this is the uncomfortable part: **there is no test surface that spans both repos.**
codescout's gates cannot see a plugin hook; the plugin cannot see codescout's tool registry.
The nearest available check is a smoke probe in the plugin's own suite asserting that every
tool a hook names in a redirect is one the currently-installed server advertises — which
requires the plugin to read the server's tool list, the very thing the hook cannot do today.
Recorded as a design question for Task 12 rather than claimed as covered.

## Workarounds

Until the plugin is updated: `read_markdown` still works against a pre-fold binary. Against a
post-fold binary, `read_file(path=…, force=true)` may bypass the hook if its matcher inspects
only the path — **unverified, do not rely on it**. The dependable workaround is to not install
the collapsed binary alongside the un-updated plugin.

## Resume

Confirm the hook file's presence and matcher in `../claude-plugins/codescout-companion/`
(`hooks/il4-deny-hook.mjs` per the inventory), then delete it as part of Task 12 together with
the `mcp__.*__read_file` matcher entry in the plugin's hook config. Re-probe by calling
`read_file` on a `.md` path from a session with the plugin active and the collapsed binary
installed; the call must reach the server and return a heading map.

## References

- `docs/architecture/companion-plugin.md` — hook inventory; its line for this hook now carries
  the obsolescence note and points here.
- `docs/superpowers/plans/2026-09-02-tool-surface-collapse.md` § Task 12 (companion plugin).
- `docs/trackers/issue-clusters/IC-2-gate-keyed-on-unobservable-event.md`.
- Task 7 fold: `aff08b09`.
