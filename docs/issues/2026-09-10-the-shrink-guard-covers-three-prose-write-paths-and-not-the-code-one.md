---
id: '12a8a56bf8a25138'
kind: bug
status: mitigated
title: 'BUG: the shrink guard covers three prose write paths and not the code one'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
unverified: 'Advisory only - edit_code(action=replace) WARNS and does not refuse, so a caller who ignores the warning still loses the code. src/tools/create_file.rs (overwrite: true) remains unguarded, now recorded in the class gate''s EXEMPT list with its reason rather than silently. The class gate proves each surface''s module CONTAINS a guard call, not that the call runs on the right operands.'
---

# BUG: the shrink guard covers three prose write paths and not the code one

## Summary
`crate::util::shrink_guard::check` was wired to exactly **three** call sites — `doc(action="update")`
body writes (`src/librarian/tools/update.rs:615`), markdown `edit_file`
(`src/tools/markdown/edit_markdown.rs:1503`), and `memory(write)` (`src/memory/mod.rs:142`, whose
refusal text is built in `src/tools/memory/mod.rs:151`). `edit_code` was not among them:
`src/tools/symbol/edit_code.rs` contained no reference to it and the tool exposes no `force`
parameter.

*(Citation corrected 2026-09-11: this file originally cited the memory surface at
`src/tools/memory/mod.rs:770,782`, which is the tool layer's message, not the `check` call. The
call is one layer down in `src/memory/mod.rs`. Found by running `references` on the guard rather
than re-reading the file — the same drift class as the open bug
`156ecb3578d22ece`.)*

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

**Mitigated, not fixed** — `edit_code(action="replace")` now runs the guard and **warns**; it
still does not refuse, so the loss remains possible for a caller who ignores the warning. Status is
`mitigated` deliberately, to keep the refusal question in the open-bug queries.

**Shipped on `experiments` at `35d1618c`**, patch-id
`369f19d6ba9d6312db9ff8eb02e2ac3b8f086ed9`. The pair is recorded together because they fail
differently: the SHA is positional and dies when `experiments` is rebased, the patch-id is a content
hash of the diff and survives both rebase and cherry-pick.

What shipped:

- `src/tools/symbol/edit_code.rs` computes
  `shrink_guard::check(&lines[start..end].join("\n"), &effective_body)` immediately before the
  splice, where `lines` (the pre-edit file) and `effective_body` are both live. **Scoped to the
  replaced range, never the whole file** — a whole-file check is monotone under this defect for any
  symbol that is a small fraction of its file, so it would pass on exactly the writes worth catching.
- The advisory rides the existing `response["warning"]` key, which `range_repair` already owns at all
  three destructive actions. The two are **joined**, not assigned: a second
  `response["warning"] = …` silently drops the first, and the case where both fire is the one a
  caller most needs both halves of. A lone warning renders byte-identically, so no existing
  assertion moved — confirmed by 30 pre-existing `replace_*` tests staying green.
- `ShrinkReport::describe_applied` is the past-tense twin of `describe`, sharing one format string
  through a private `describe_with(verb)`. The tense is the reason it exists rather than a second
  caller of `describe`: *"would reduce"* inside a response whose `status` is already `"ok"` reads as
  a refusal that did not happen, sending the caller to look for a write that never landed.

**Why warn and not refuse** — decided by the operator, 2026-09-11. A refactor that legitimately
collapses a long function into a short one is byte-identical to an accidental partial body, so a
refusal blocks the first in order to catch the second. The `force` escape the original plan
prescribed is therefore not shipped either; there is nothing to escape.

### What is still owed

**Shipped at `638b1e02`** as three tests in `src/server.rs`'s test module: `every_content_bearing_tool_is_shrink_guarded_or_has_a_reason`, `every_guarded_call_site_still_holds_its_guard`, and `the_content_bearing_population_is_not_vacuous`. They live in `src/server.rs` rather than `tests/` because `CodeScoutServer::tools` is private, so no integration test can enumerate the registry.

The population is derived from the **tool registry**, not from a grep over write primitives, and the difference is why the first attempt was abandoned. Deriving it as *"source files that read existing content and then write over it"* selected **70 files under `src/`, 66 needing an exemption** (a list nobody reads, registering as coverage while checking nothing) and it **missed `create_file`**, which overwrites without reading and so matched neither half of the conjunction. Schema-derived (`body` / `content` / `new_string` at the top level of a tool's input schema) it selects **5 of 21 tools**, with the known answer intact: `doc`, `edit_file`, `memory` and `edit_code` guarded, `create_file` exempt with its reason.

Each test was driven to an observed RED, one of them by mutating the **production** path rather than the test's own lists:

- delete the guard call in `edit_code.rs` -> `every_guarded_call_site_still_holds_its_guard` RED
- drop `edit_code` from `GUARDED` -> `every_content_bearing_tool_is_shrink_guarded_or_has_a_reason` RED
- typo `CONTENT_PROPS` -> `the_content_bearing_population_is_not_vacuous` RED

The third is the one worth keeping. With the predicate typo'd the population is empty and **both other gates pass**: *"every content-bearing tool is guarded"* is trivially true of zero tools, and the call-site check never consults the population. One typo in one const silently disarms the whole gate while showing green.

**What the class gate does NOT prove**, stated because a reader would otherwise credit it with more: it establishes that each surface's module *contains* a guard call, not that the call runs on the right operands or is reachable. A guard handed the whole file instead of the replaced range satisfies it. That property is per-surface and belongs to the surface's own tests.

### Still genuinely owed

`src/tools/create_file.rs` under `overwrite: true` remains unguarded, but now explicitly rather than invisibly: it sits in the gate's `EXEMPT` list with its reason, so it is a recorded decision instead of a gap. It cannot compute a shrink report without a read it does not currently perform, which is a behaviour change rather than a wiring fix.

And `edit_code` still **warns** rather than refusing, so the loss remains possible for a caller who ignores the warning. That is the operator's decision of 2026-09-11, not an oversight.
## Tests added

`tests/symbol_lsp.rs`:

- `replace_warns_when_the_new_body_is_less_than_half_the_symbol`
- `replace_stays_silent_when_the_new_body_is_proportionate`

They are a **pair**, and neither is worth much alone: the first is monotone under "always warn", the
second under "never warn". Both were driven to an observed RED by mutating the **production** call,
not the test inputs:

| mutation | result |
|---|---|
| `let shrink = None` (guard disabled) | `replace_warns…` RED, silent test correctly green |
| `check(&content, …)` (whole file, not range) | **both** RED — the positive one on `20 → 3 lines` instead of `10 → 3`, the silent one because a proportionate replace now warns spuriously at `20 → 9 lines (55%)` |

The second mutation is the one that justifies the fixture's shape. `SHRINK_FIXTURE` pads the file
with five lines of real code before and after the symbol **so that a range-scoped check and a
whole-file check produce different numbers**; without the padding the assertion still passes and
stops discriminating between them. The padding is code rather than `//` comments because
`editing_start_line` walks back over a lead region of doc comments and attributes, which would pull
comment padding into the replaced range. Both facts are annotated on the fixture.

Verified by name in **both** gate lanes, rather than from either lane's total:
`grep 'replace_warns_when' ` returns `ok` in the lean and default runs alike, as do
`shrink_guard`'s 7 unit tests (7 in both).
## Workarounds

**Read the `warning` field on every `edit_code(action="replace")` response.** It is now populated
whenever the replacement is less than half the symbol it replaced, and it names both dimensions and
the symbol.

The pre-fix workaround still applies and is stricter: after any `replace`, compare `replaced_lines`
against the line count of the body you supplied. That is the one arithmetic the caller cannot do
without already knowing what they destroyed, which is why the tool now does it for you.

For the surfaces still unguarded — notably `create_file(overwrite: true)` — there is no advisory.
Read the file first.
## Resume

Design the population predicate for the set-difference test described under **What is still owed**.
It cannot be a grep over read/write primitives: that yields 70 files needing 66 exemptions AND
misses `src/tools/create_file.rs`, measured 2026-09-11. Start from the tool schemas instead — a
write-capable tool whose input schema carries a caller-supplied content property (`body`, `content`,
`new_string`) — and check that against the four known-guarded surfaces plus `create_file` as a
known-answer fixture.

Separately, decide whether `create_file(overwrite: true)` should read-before-write in order to be
guardable at all. That is a behaviour change, not a patch, and belongs with whoever owns that tool.
## References
- `src/util/shrink_guard.rs` (the guard, and its own "three copies" note)
- `src/librarian/tools/update.rs`, `src/tools/markdown/edit_markdown.rs`,
  `src/tools/memory/mod.rs` (the three wired surfaces)
- `src/tools/symbol/edit_code.rs` (the unwired one)
- `docs/trackers/issue-clusters/IC-14-guard-narrower-than-its-name.md` (cluster membership; the
  axis-omission sub-shape, and the set-difference mechanism this instance wants)
