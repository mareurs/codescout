---
id: b3c0fe6ee49d9b57
kind: bug
status: open
title: 'BUG: the worktree read notice is injected into the response and then discarded by every tool with a compact renderer'
tags:
- cluster/declared-not-wired
- worktree
- notice
- output-form
- format-compact
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-09-02-worktree-guard-refuses-writes-and-lets-unpinned-reads-through.md
- docs/issues/archive/2026-08-17-worktree-reads-resolve-against-the-old-project.md
severity: medium
unverified: 'Reproduced live and diagnosed at the bytes, but NOT confirmed per-tool: the four-row table is two text-form tools (tree, symbols) against one JSON-form tool (artifact), plus a write refusal as the control. The claim that all 18 `format_compact` implementations drop the field is derived from a grep showing `_workspace_notice` appears nowhere outside `call_content` and the tests — sound, but it is an absence over a population, which is the assertion shape this repo treats as weakest. Anyone fixing this should enumerate the 18 rather than trust the grep, and must drive the regression test through a tool with a real `format_compact`; `EchoTool` takes the pretty-JSON branch and is why the existing two tests are green against this defect.'
---

## Summary

**Corrected 2026-09-02, and the correction is larger than the number.** This file was filed
claiming *"18 tools implement `format_compact`"* as the affected population. **That is the wrong
criterion.** `format_compact` is only consulted when a tool also declares
`fn output_form() -> OutputForm::Text`, and the trait default is `OutputForm::Json`.

The real population is **ten**, all declaring `OutputForm::Text`: `grep`, `tree`, `symbols`,
`symbol_at`, `references`, `call_graph`, `read_file`, `read_markdown`, `memory`, `library`.

The original figure came from a grep for `format_compact` implementations — the file's own
`unverified:` field flagged that step as "an absence over a population, the assertion shape this
repo treats as weakest" and said to enumerate rather than trust it. Enumerating is what found
the error. The caveat was right; writing it down did not prevent the mistake, it only made the
mistake findable.

`worktree_read_notice` computes correctly and `inject_notice` writes `_workspace_notice` into the
response `Value` — then, for those ten, `format_compact` builds its text from that `Value` by
selecting the fields the *tool* knows about, and a field the framework added after the tool
returned is one no renderer reads. The notice survived only on the pretty-JSON branch.
## Symptom (Effect)

Measured live 2026-09-02, one session, one server process, within seconds, all conditions held
constant and independently confirmed:

| call | shape | `_workspace_notice` |
|---|---|---|
| `tree(path="scripts")` | compact text | **absent** |
| `symbols(name="worktree_read_notice")` | compact text | **absent** |
| `create_file(...)` | — | **REFUSED**: *"worktrees detected but `workspace(action='activate')` has not been called"* |
| `artifact(action="find", kind="tracker")` | JSON object | **present** |

Row 3 is the control, and it is what makes the other rows diagnostic rather than
circumstantial: `guard_worktree_write` gates on the same `is_project_chosen_this_session()`
flag as the notice, so its refusal proves the slot was empty and a linked worktree
(`.worktrees/tool-collapse`) existed at that moment. Every precondition for the notice held.
Two reads got nothing; the JSON-shaped read got it.

## Reproduction

1. `/mcp` reconnect (clears `project_chosen_this_session`) in a checkout with a linked worktree.
2. `tree` or `symbols` — no notice.
3. Any write — refused, naming the worktrees. Conditions confirmed.
4. `artifact(action="find")` — notice present.

## Root cause

`src/tools/core/types.rs`, small-output path:

```rust
if let Some(notice) = &workspace_notice {
    inject_notice(&mut val, notice);          // writes _workspace_notice into val
}
if form == OutputForm::Text {
    if let Some(text) = self.format_compact(&val) {
        Content::text(text)                    // renders from val; never reads that key
    } else {
        Content::text(serde_json::to_string_pretty(&val)...)   // preserves it
    }
}
```

`format_compact` is each tool's own renderer, selecting the fields it wants. A field injected
by the framework after the tool returned is invisible to a renderer written before it existed.
The `else` branch — pretty-JSON — preserves the notice, which is why the defect looks
intermittent rather than total.

**Precedent exists and covers exactly one shape.** `inject_notice` already special-cases
`run_command`, prepending the notice into `stdout` "so the warning sits in the channel that is
actually read" (`docs/issues/archive/2026-08-17-worktree-reads-resolve-against-the-old-project.md`).
That is this same problem solved once, for one tool, by naming its output channel. The general
case was not carried across.

## Evidence

### The tests cannot see it, and that is the part worth reading

`a_read_says_which_tree_it_answered_from_when_worktrees_are_unchosen` and
`a_pinned_read_gets_no_worktree_notice_even_though_the_tree_is_unchosen` both drive `EchoTool`,
a test fixture. Its output takes the **pretty-JSON** branch, which preserves the field. So both
tests exercise `:1038` and never `:1030` — the branch 18 production tools take.

The suite therefore proves the notice is *computed and injected* and says nothing about whether
any real tool *delivers* it. This is CLAUDE.md § *Testing Discipline* twice over: an assertion
computed over a fixture cannot verify a claim about a member it never instantiates, and the
mutation discipline that caught two other defects here was applied to the production path of
the **decision** (`notice_once`, `workspace_override`) and not to the **delivery** path, which
no test reaches at all.

It is also `OB-15` in its own right — the notice's silence on `symbols` is indistinguishable
from the notice not existing, which is precisely how it survived a fix, two mutation runs, and
a live production check that used the one response shape where it works.


### The identical mechanism was already measured and written down — for the sibling field

`src/server.rs:7684`, in a test comment predating this file:

> Probed via the APPENDED GUIDE BODY, not via `_guide_hint`. Measured, not assumed: `tree` is
> `OutputForm::Text` with a `format_compact` (`src/tools/tree.rs:58-69`), so its primary block is
> rendered text and the `_guide_hint` field injected into the `Value` **never reaches the wire**
> — `extract_hint` returns `None` here even when the opener fired.

So `inject_hint` has the same defect, on the same line of `call_content`, and someone measured it
while working on the guide ledger. This file was written as a discovery of the mechanism; it was
a rediscovery of a documented one, on the neighbouring field. `F-104`'s *Promote-when* asked for
"a second instance of a framework-injected response field that a per-tool renderer drops" — the
second instance already existed, in the tree, and neither party knew of the other's half. That is
`OB-15` exactly: the discriminator was in the code, not in the absence, and nobody greped.

**But the two are not equally harmful, and the next sentence of that comment is why:** *"The
second block is pushed regardless of output form."* A guide's **body** is appended as its own
content block, so the model still receives the guidance — only the `_guide_hint` metadata field
is lost. The worktree notice has no second block, so its loss is total. `inject_hint` is
degraded; `inject_notice` was silent.

**Consequence for the fix:** re-attaching the notice to the rendered text is sufficient here and
is *not* automatically the right move for `_guide_hint`, whose body already arrives by another
route and would be duplicated. Not changed in this fix; recorded so the symmetry is not applied
blind.
## Fix

Not implemented. The decision is where the notice goes for a text-rendered tool, and it is a
real one:

1. **Append to the rendered text in `call_content`**, after `format_compact` returns. One
   change, covers all 18, no per-tool work. Costs: the notice lands outside whatever structure
   the compact form has, and compact forms are deliberately terse.
2. **Have `format_compact` receive the notice and place it**, per tool. Most faithful, 18
   edits, and each is a chance to forget.
3. **Suppress the compact form when a notice is pending.** Smallest diff, and it makes a
   worktree-bearing checkout verbose on every read, which is the shape that gets a mechanism
   disabled.

(1) is the recommendation. Whichever is taken, **the regression test must drive a tool with a
real `format_compact`, not `EchoTool`** — otherwise it re-proves what the existing tests
already prove and the delivery path stays untested.

## Resume

Unclaimed. Filed by the session that shipped `7a3aee93`, which made the notice fire on every
unpinned read and thereby made this reachable often enough to notice; the drop itself predates
that and has been there since the notice shipped on 2026-08-15.
