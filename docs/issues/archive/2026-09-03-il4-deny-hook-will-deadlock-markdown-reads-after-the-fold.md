---
kind: bug
status: fixed
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-09-04
opened: 2026-09-03
owner: marius
related: []
severity: high
unverified: no regression guard exists in either repo, so nothing prevents a plugin hook from naming a retired tool again; the fix was established by a live probe on one profile at one instant, never by a test
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

Applied in the **plugin** repo, not here. The prescribed fix was *"delete the hook, not update its message"* — after the fold there is no wrong tool to redirect away from, so the rule it enforced no longer exists — and that is what landed.

- **SHA:** `claude-plugins:bb24b7f` (`bb24b7f7d351b27bb6ffad06cc6a0a5f5e4d905b`), 2026-09-03 08:40:42 +0300 — *"feat: follow codescout's tool collapse — doc replaces artifact, IL-4 retired, read_file/edit_file handle markdown"*.
- **patch-id:** `1175e6f8b54fff099d6342967d4ea09b8f92a6a4`.

Shipped as **1.20.4** (`34f5da6`, 09:13:27). `git ls-files | grep -c il4` in that repo returns **0**, and the installed `1.20.3 → 1.20.4` diff removes exactly two files: `il4-deny-hook.mjs` and `il4-deny-hook.test.sh`.

**The cross-repo ordering this file flagged resolved correctly, and by 7h35m rather than by design.** The bug was filed at `99cc3313` (2026-09-03 01:05) and the plugin retired the hook the same morning, so the window in which a collapsed binary could meet an un-updated plugin never opened on this machine. That was a decision someone made, exactly as the original text predicted it would have to be — it is not a property the system now has.

**Verified live, not inferred.** Probe 2026-09-04 01:33 EEST, one path, one profile (`~/.claude-sdd`): `read_file(path="README.md")` reached the server and returned a heading map plus an `@file_*` handle — no deny. Corroborated on the distribution axis by **content, not mtime** (the installed `1.20.4` directories stat two days *before* the commit that created 1.20.4): `installed_plugins.json` reads 1.20.4 in all three profiles, the only surviving `read_file` matcher in `hooks.json` routes to `cs-liveness.mjs` (an advisory), and no `settings*.json` in any profile registers the hook directly.
## Tests added

**None, and the gap named in the original filing is untouched by this fix.** There is still no test surface spanning the two repos: codescout's gates (`prompt_surfaces_reference_only_real_tools`, `claude_md_contains_no_deprecated_tool_names`, `guide_bodies_contain_no_deprecated_tool_names`) scan codescout's own surfaces and cannot reach a plugin; the plugin holds no copy of the tool registry to check a redirect against.

So the deadlock is gone and the **mechanism** that let a stale `permissionDecision: deny` remove a capability is not. Recorded as a class in `observer-blindness:OB-16` rather than claimed as covered here — a deny fails closed, so the next occurrence costs a capability rather than a round-trip.
## Workarounds

Until the plugin is updated: `read_markdown` still works against a pre-fold binary. Against a
post-fold binary, `read_file(path=…, force=true)` may bypass the hook if its matcher inspects
only the path — **unverified, do not rely on it**. The dependable workaround is to not install
the collapsed binary alongside the un-updated plugin.

## Resume

N/A — fixed and verified. The residual is `observer-blindness:OB-16` (no gate spans the two repos), which is a design item, not a resumption of this bug.
## References

- `docs/architecture/companion-plugin.md` — hook inventory; its line for this hook now carries
  the obsolescence note and points here.
- `docs/superpowers/plans/2026-09-02-tool-surface-collapse.md` § Task 12 (companion plugin).
- `docs/trackers/issue-clusters/IC-2-gate-keyed-on-unobservable-event.md`.
- Task 7 fold: `aff08b09`.
