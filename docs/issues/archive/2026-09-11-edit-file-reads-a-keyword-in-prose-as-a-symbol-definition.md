---
id: 0947b2a32cb86e00
kind: bug
status: fixed
title: 'BUG: edit_file reads a definition keyword in a COMMENT as a symbol definition, with no escape'
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-09-12
opened: 2026-09-11
severity: medium
---

## Summary

`edit_file`'s IL-2 guard refuses a multi-line edit whose **changed lines** contain a
symbol-definition keyword outside a line-leading comment. The residual cases are a keyword in a
**trailing** comment (`1 // mentions a fn but defines nothing`) and one inside a **string
literal** — and in a repo whose defect vocabulary is literally *defect class*, `cluster/<slug>` and
"a class gained a member", that prose is everywhere.

**Corrected 2026-09-11 after reading the predicate at the bytes and reproducing it.** This section
previously said the guard fires *"regardless of whether the occurrence is code"* and that *"an edit
that only touches a comment is refused"*. Both are false as stated: `find_def_keyword`
(`src/tools/edit_file/mod.rs:80-96`) explicitly skips lines starting with `//`, `/*`, `*` or `#`,
with a doc comment saying so. A two-line `//` comment containing `a fn but` passes — measured, not
argued. The defect is narrower and lives where the comment is **not line-leading**.

The "no escape" claim was also too strong: single-line edits are exempt (`:331`, `:335`), and the
refusal hint names that exemption. The honest complaint is that the escape does not **scale** — it
is unusable for a multi-line prose edit, which is the case that actually bites.
## Symptom (Effect)

```
edit_file(path="scripts/pre-commit-ledger-counts.py",
          old_string="    # CHECK 2 -- a class that GAINS a member must say something about it.",
          new_string="    # CHECK 2 -- ... \n    # ... a wrong class corrupts the counts ...")
-> edit contains a symbol definition ("class ") — use symbol tools for structural changes
```

Both strings are Python **comments**. Nothing in the edit defines anything.

The hint routes to `edit_code`, which cannot serve this edit either: `edit_code` addresses whole
symbols, so inserting eight comment lines in the middle of a 150-line `main()` means replacing
`main()` wholesale — reproducing its entire body to change a comment, which is the larger and
riskier edit the guard was meant to prevent. **The refusal's remedy is more dangerous than the
act it refuses**, which is the `CLAUDE.md` § *Testing Discipline* point about naming the next
action a guard's message produces.

## Reproduction

**The reproduction previously recorded here does not reproduce.** It is kept below as probe 1
because its failure is the finding: it specifies a *line-leading* comment, and those are skipped.
All three probes run 2026-09-11 against `4e2ff589`, on a scratch file under `.worktrees/`.

**Probe 1 — the filed reproduction. PASSES (does not refuse).**

```
edit_file(path=".worktrees/il2-probe/subject.py",
          old_string="def alpha():\n    return 1",
          new_string="def alpha():\n    # every defect class gains members over time\n    return 1")
→ {"status": "ok"}
```

The added line is `    # every defect class …`, which `find_def_keyword` filters out at
`src/tools/edit_file/mod.rs:80-96` before any keyword test. A `//`-leading Rust comment behaves the
same way — a two-line `// … mentions a fn but …` comment also passes.

**Probe 2 — the real defect: the comment is TRAILING, so the line is not filtered. REFUSES.**

```
edit_file(path=".worktrees/il2-probe/subject.rs",
          old_string="pub fn alpha() -> usize {\n    1\n}",
          new_string="pub fn alpha() -> usize {\n    1 // this trailing comment mentions a fn but defines nothing\n}")
→ error: edit contains a symbol definition ("fn ") — use symbol tools for structural changes
```

Isolated, not merely observed: `old_string` contains `pub fn alpha`, but the diff-scoping at
`:328-329` excludes it — that line is byte-identical in both strings, so `lines_only_in` yields only
the new comment line. The refusal is attributable to the trailing comment alone.

**Probe 3 — control. Same edit, `fn` → `function`. PASSES.**

```
… new_string="…    1 // this trailing comment mentions a function but defines nothing…"
→ {"status": "ok"}
```

One word is the only variable between probes 2 and 3, which is what makes probe 2 a measurement of
the keyword test rather than of the edit's shape.
## Environment

codescout MCP, 2026-09-11, `experiments`. Hit twice in one task: once on `class ` in a Python
comment, once earlier on `fn ` while legitimately adding a function (that one was correct).

## Root cause

**Measured 2026-09-11** by reading `src/tools/edit_file/mod.rs` and running the probes under
§ Evidence. Supersedes this section's previous *"Not read at the bytes"* note, whose inferred
mechanism — *"a substring test over the edit text with no comment/string-literal awareness"* —
would have sent a fixer to build something that already exists.

The predicate is narrowed in six places, not one:

| narrowing | site |
|---|---|
| non-source paths exempt | `:316` |
| non-LSP languages exempt | `:319` |
| language-specific keyword sets, not a universal list | `:15-34` |
| diff-scoped: only lines that DIFFER between old and new | `:328-329` |
| multi-line only; single-line edits exempt | `:331`, `:335` |
| left word-boundary aware (`via_trait` no longer matches `trait `) | `:59` |
| line-leading comments skipped (`//`, `/*`, `*`, `#`) | `:80-96` |

So the true residual is small and specific: a definition keyword at a word start, on a **changed**
line, in a **multi-line** edit, to an **LSP-supported source file**, where the occurrence is in a
**trailing comment or a string literal** rather than a line-leading comment.

**The module already names the string-literal half as a deliberate residual and identifies the
fix** (`:51-58`): *"Narrowing that needs literal-awareness (`crate::util::text::scan_line` has it)
and is a wider change than this guard warrants."* It does **not** name the trailing-comment half,
which is the gap this file is really about — and `scan_line` is plausibly the same lever for both,
since a trailing comment and a string literal are both "the rest of this line is not code".

The error asymmetry the module states is real and should survive any fix: a false positive costs
one rejected edit, a false negative risks the LSP range corruption the guard exists to prevent
(BUG-027). That argues for routing through `scan_line` rather than loosening the keyword match.

**Separately, the gate's advertised scope contradicts its predicate.** The condition text appended
to every refusal reads *"Imports, string literals, comments and config are allowed"* — but a
string literal is a documented residual and a trailing comment is the defect above. A reader who
believes that sentence will not suspect the guard, which is doc-contradicted-by-code on the one
surface every refused caller reads.
## Fix

**Shape 2, shipped 2026-09-12 in `c9c03a74`.** The three sketches below are the original
author's; each carries what the 2026-09-11 measurement said about it, because two were priced
against a mechanism that turned out to be wrong — and the first was **rejected outright**.

1. **Require the keyword to begin a line** (after indentation) before treating it as a definition.
   **REJECTED — it would ship a false negative in the dangerous direction.** Rust's keyword list is
   `["fn ", "async fn ", "struct ", "impl ", "trait ", "enum "]` (`:17`) and none of them accounts
   for a visibility modifier. Under this rule `pub fn smuggled_in()` begins with `pub `, not `fn `,
   and would no longer match — so would `pub struct`, `pub async fn`, and most public Rust
   definitions. Measured 2026-09-11: the guard catches exactly that case today (probe 4 below), and
   `BUG-050` is the reason it must — a new `fn` splicing into an unrelated function body is the
   failure this arm exists to prevent. The premise *"a definition always begins its line, prose
   almost never"* holds for Python's `class ` and fails for Rust the moment anything is `pub`.
2. **Skip comment and string spans when scanning, per language.** **The remaining candidate, and
   cheaper than this file assumed** — half of it already ships: line-leading comments are skipped
   for `//`, `/*`, `*` and `#` (`:80-96`). What remains is trailing comments and string literals,
   and the module already names `crate::util::text::scan_line` as the existing literal-aware lever
   it declined to reach for (`:51-58`). It also narrows in the SAFE direction — it removes matches
   that are provably not code, rather than removing matches that merely look unlike a definition.
3. **Give the caller an escape.** Still owed by `IC-6`, but one exists and is narrower than
   "none": single-line edits are exempt (`:331`, `:335`) and the refusal hint says so. The accurate
   complaint is that it does not scale to a multi-line prose edit.

**Probe 4 — why shape 1 is rejected. Run 2026-09-11 against `dffb89c2`.**

```
edit_file(path=".worktrees/il2-probe2/subject.rs",
          old_string="pub fn existing() -> usize {\n    1\n}",
          new_string="pub fn existing() -> usize {\n    1\n}\n\npub fn smuggled_in() -> usize {\n    2\n}")
→ error: edit contains a symbol definition ("fn ") — use symbol tools for structural changes
```

Caught today because `fn ` sits at a word start preceded by `pub `. Shape 1 tests a different
thing — line start — and this line starts with `pub`.

**Whichever shape is chosen, the guard's advertised scope needs correcting with it.** The condition
text on every refusal claims *"Imports, string literals, comments and config are allowed"* — false
for trailing comments (probe 2) and false for string literals (a documented residual). A reader who
believes it will not suspect the guard.

Preserve the error asymmetry the module states at `:45-49`: a false positive costs one rejected
edit, a false negative risks LSP range corruption. That asymmetry is exactly what disqualifies
shape 1 and recommends shape 2.

**What shipped.** `src/util/text.rs`'s literal scanner was private and reported only its end
state, so `scan_line` now delegates to `scan_line_into`, which additionally writes a mask; the
public entry point is `blank_non_code(block, line_comment)`. One implementation with two outputs
rather than a second copy of the literal rules, which is the shape that drifts.
`find_def_keyword` blanks before scanning, and `line_comment_tokens` supplies `#` for
Python/Ruby and `//` elsewhere.

Two decisions worth keeping, both annotated at their site:

- **Lines are scanned independently.** `find_def_keyword` receives the lines an edit CHANGED,
  which are not contiguous source. Carrying literal state between them would blank real code
  lying between two unrelated quotes — a false negative, the direction § Root cause's asymmetry
  forbids. `blank_non_code_does_not_carry_literal_state_between_lines` is the regression guard,
  and it reds on the "optimisation" that looks like a correctness improvement.
- **The line-leading filter is kept ON TOP of the mask, not replaced by it.** Blanking subsumes
  `//` and `#`; `/*` and `*` are block comments the scanner does not model, so dropping the
  filter would newly admit them.

**Residual, named at the refusal site rather than narrowed silently:** a keyword inside a
**mid-line block comment** (`1 /* mentions fn */ + 2`) still refuses. The IL-2 condition text in
`src/prompts/mod.rs` now says so, along with dropping the false claim that string literals and
comments were already allowed.

Fix SHA: `c9c03a74`
Patch-id: `25a355c556921889c22bc2c4b3c10198746f0831`
## Workarounds

Native `Edit`, which has no such guard — used for the refused edit in this task. Rephrasing the
prose also works and is worse: it lets a tool defect edit the documentation's wording.

## Tests added

Five, at two sites — `find_def_keyword` is a unit and `guard_structural_rewrite` is where the
defect was observed, and a kill at one says nothing about the other.

`src/tools/edit_file/tests.rs`

- `find_def_keyword_ignores_a_keyword_in_a_trailing_comment` — probe 2, plus the Python `#`
  form. Observed RED before the fix: `left: Some("fn ")`.
- `find_def_keyword_ignores_a_keyword_inside_a_string_literal` — the second residual. Observed
  RED before the fix: `left: Some("fn ")`.
- `guard_reads_a_trailing_comment_as_prose_but_still_catches_a_smuggled_definition` — probes 2
  and 4 **paired in one test**, deliberately. The halves must move in opposite directions, so a
  fix that simply stopped looking for keywords satisfies the first assertion and fails the
  second. This is the acceptance floor § Resume asked for.

`src/util/text.rs`

- `blank_non_code_keeps_code_and_blanks_comments_and_literals` — asserts byte length directly
  rather than letting the shapes imply it, and asserts the token set is honoured in **both**
  directions (`#` blanks under `["#"]`, does not under `["//"]`).
- `blank_non_code_does_not_carry_literal_state_between_lines` — the guard described above.

The pre-existing floor stayed green throughout and was checked at each step, not only at the
end: `find_def_keyword_ignores_class_in_comment`,
`find_def_keyword_still_catches_real_definitions`,
`find_def_keyword_ignores_a_keyword_inside_an_identifier`, and the `guard_blocks_*` family.
## Resume

**Nothing to resume — fixed and verified on `experiments` at `c9c03a74`.** The floor § Fix set
was met as specified: probe 2 flipped to pass, probe 3 stayed passing, probe 4 stayed refusing.

One honest caveat about the verification, since it is the kind that goes unrecorded: the gate
ran over a working tree that also held a peer's uncommitted `src/memory/anchors.rs`. Both lanes
were green (`LEAN exit=0`, `DEFAULT exit=0`), and the commit is a strict subset of that tree —
but the green is evidence about the tree as it stood, not about this commit in isolation.
## References

- Hit 2026-09-11 while fixing
  `docs/issues/archive/2026-09-09-the-pre-commit-cluster-hook-enforces-a-subset-of-the-gate-it-mirrors.md`.
- `CLAUDE.md` § *Parsers Over a Namespace* — "how does a caller write this token literally", and
  the instruction to state an unaffordable escape at the refusal site.
