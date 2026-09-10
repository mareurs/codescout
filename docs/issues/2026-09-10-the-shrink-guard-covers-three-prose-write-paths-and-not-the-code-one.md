---
id: '12a8a56bf8a25138'
kind: bug
status: open
title: 'BUG: the shrink guard covers three prose write paths and not the code one'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
---

# BUG: the shrink guard covers three prose write paths and not the code one

## Summary
`crate::util::shrink_guard::check` is wired to exactly **three** call sites — `doc(action="update")`
body writes (`src/librarian/tools/update.rs:615`), markdown `edit_file`
(`src/tools/markdown/edit_markdown.rs:1503`), and `memory(write)`
(`src/tools/memory/mod.rs:770,782`). `edit_code` is not among them:
`src/tools/symbol/edit_code.rs` contains no reference to it and the tool exposes no `force`
parameter.

So the three surfaces that write **prose** refuse a body losing >50% of its bytes or lines, and
the one surface Iron Law 2 **mandates** for structural code edits accepts an arbitrary net
deletion and returns `status: "ok"`.

## Symptom (Effect)
`edit_code(action="replace")` replaces the symbol's whole range with the supplied body. A caller
who supplies a partial body — having read only part of the function — deletes the remainder, and
the response reads as a success. There is no warning, no delta, and no refusal.

The response *does* carry `replaced_lines`, so the information needed to notice is present. What
is absent is any comparison of it against the body's own length, which is the one arithmetic the
caller cannot do without already knowing what they destroyed.

## Reproduction
Live instance, 2026-09-10, on `scripts/probe-double-frontmatter.py` at `9fa6012a`'s parent.

`scan()` spanned lines 63-101 (39 lines). Working from a `grep(context_lines=10)` window that
showed only lines 63-74, I called:

```
edit_code(path="scripts/probe-double-frontmatter.py", symbol="scan",
          action="replace", body=<19 lines>)
```

Response:

```json
{"status": "ok", "replaced_lines": "63-101", "wrote_to": "...", "rel_path": "..."}
```

`git diff` immediately afterwards showed **27 lines deleted** — the entire YAML-mapping
discriminator and the closing `return found`:

```
-        close1 = next((i for i in range(1, len(lines)) if lines[i].strip() == "---"), None)
...
-        found.append((rel, text, lines, close1, open2, close2, fm1, fm2))
-        return found        (as `-    return found`)
```

The same call against a markdown file of the same shape is refused: `body-shrink guard: write to
<path> ...`.

## Environment
codescout MCP `edit_code`, Python extractor. Project codescout, branch `experiments`,
around `9fa6012a`. Not language-specific — the guard's absence is at the tool layer.

## Root cause
The guard is not missing; it is **unwired at one of four destructive-write surfaces**. Its own
module header already says so obliquely: `src/util/shrink_guard.rs:7` reads *"`SHRINK_GUARD_MIN_BYTES
= 200`. Three copies is how the gap below survived"* — the module knows it has three call sites
and treats that as a hazard, without naming the fourth surface that has none.

Why the code surface is the one that ended up unguarded is worth stating, because it is not
arbitrary: a prose body write is *always* a whole-file overwrite, so a shrink check is obviously
needed there. `edit_code` looks safer because it is scoped to one symbol — the scoping reads as
the safety property. But the scope bounds the *blast radius*, not the *proportion*: within its
range, replace is exactly as total an overwrite as `body=`.

## Evidence
Call sites of `crate::util::shrink_guard::check`, derived by grep over `src/**/*.rs` on
2026-09-10 at `f48a2b41`:

| surface | writes | guarded |
|---|---|---|
| `doc(action="update")` `patch={body:…}` | prose | yes — `update.rs:615` |
| `edit_file` markdown grammar | prose | yes — `edit_markdown.rs:1503` |
| `memory(action="write")` | prose | yes — `src/tools/memory/mod.rs:770`, `:782` |
| `edit_code(action="replace")` | **code** | **no** |

Three of the four also expose a `force` escape; `edit_code` exposes none, because it has nothing
to escape.

## Hypotheses tried
1. **Hypothesis:** this is one of the eight archived `edit_code` bugs.
   **Test:** read all eight (`edit-code-replace-drops-visibility-modifier`,
   `edit-code-replace-strips-doc`, `edit-code-replace-misses-outer-attrs`,
   `edit-code-replace-stray-brace`, `edit-code-insert-mid-function`,
   `edit-code-impl-method-selection-range-refusal`,
   `edit-code-replace-drops-doc-comment-after-range-repair`, and the range-repair sibling).
   **Verdict:** rejected, and the distinction is the finding. All eight are *"the tool silently
   removes something the caller kept"* — an edge the replace range absorbed. This one is *"the
   tool silently accepts something the caller removed"*. They are mirrors, and they need
   different remedies: those needed range fixes, this needs a delta check. Filing it under the
   same family would put it behind eight `fixed` files.

2. **Hypothesis:** the display truncation is the cause, i.e. this is
   `2026-05-02-edit-code-insert-mid-function` (symbol body truncated in display) again.
   **Test:** check what the caller actually read. It was a `grep(context_lines=10)` window, not a
   truncated `symbols` body — no truncation notice was involved, and the tool never claimed to
   have shown the whole symbol.
   **Verdict:** rejected. A partial read is a normal thing for a caller to have; the defect is
   that nothing downstream of it checks. Treating this as a display bug puts the remedy on the
   reader.

## Fix
*Not fixed by this bug file, and deliberately so — the design decision is not a drive-by.*

The shape that would hold: `edit_code(action="replace")` runs
`crate::util::shrink_guard::check(old_symbol_text, body)` over the **replaced range**, not the
whole file, and refuses a >50% loss unless a new `force` is passed. Scoping the check to the
range is the whole point; a whole-file check is monotone under this defect for any symbol that is
a small fraction of its file, which is most of them.

**The reason this needs a decision rather than a patch:** a legitimate refactor that collapses a
long function into a short one is exactly this shape and must stay possible. So the guard is only
worth adding together with its escape, and the escape has to be discoverable from the refusal
text — otherwise the guard converts a silent data loss into a blocked legitimate edit, which is
the trade CLAUDE.md's remedy-text law says to price before shipping.

**And it should be wired as a set-difference, not a fourth copy.** IC-14's own mechanism note
prescribes this: *"every entry point to a guarded operation routes through the guard —
expressible as `references()` on the guard function differenced against the public write entry
points."* A test asserting that every destructive-write entry point appears in
`references(shrink_guard::check)` reds when a fifth surface is added unguarded. That closes the
class; a fourth call site only closes this instance, and the module header's *"three copies is how
the gap survived"* is the argument against adding a fourth by hand.

## Tests added
None. See § Fix — the guard does not exist at this surface yet, so there is nothing to pin. The
test worth having is the set-difference one, which belongs with the fix.

## Workarounds
**Read the whole symbol before replacing it.** `symbols(name=…, include_body=true)` returns the
full body; a `grep` window does not and does not claim to. This is the workaround, not the fix,
because it is a discipline against a silent failure — CLAUDE.md § *Observer Blindness* position 3
is explicit that "be careful" is the wrong instrument for a class that returns a plausible answer.

**And `git diff` immediately after any `edit_code(action="replace")`.** That is what caught this
one, within a single tool call, before the truncation ever reached a commit. It is cheap and it is
the only step that closes the loop today.

## Resume
Decide whether to wire the range-scoped shrink guard plus `force` into `edit_code`, or to accept
the asymmetry and document it at the tool's schema instead. Either is defensible; the current
state — guarded prose, unguarded code, and nothing saying so — is the one that is not.

## References
- `src/util/shrink_guard.rs` (the guard, and its own "three copies" note)
- `src/librarian/tools/update.rs`, `src/tools/markdown/edit_markdown.rs`,
  `src/tools/memory/mod.rs` (the three wired surfaces)
- `src/tools/symbol/edit_code.rs` (the unwired one)
- `docs/trackers/issue-clusters/IC-14-guard-narrower-than-its-name.md` (cluster membership; the
  axis-omission sub-shape, and the set-difference mechanism this instance wants)
