---
id: '447d98db54393338'
kind: bug
status: taken
title: 'BUG: edit_file reads a definition keyword in a COMMENT as a symbol definition, with no escape'
tags:
- cluster/addressing-without-an-escape-hatch
claimed_at: 2026-09-11
claimed_by: b80a27d4-9729-40ef-8c28-ad8982df6d13
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

Not chosen. The three sketches below are the original author's; each now carries what the
2026-09-11 measurement says about it, because two of the three were priced against a mechanism that
turned out to be wrong.

1. **Require the keyword to begin a line** (after indentation) before treating it as a definition.
   **Still viable, and now precisely targeted** — it is exactly the trailing-comment case in
   § Reproduction probe 2, since a trailing comment's keyword never begins its line. Note the
   original claim that it *"would have accepted both refused edits here"* rests on the superseded
   reproduction; re-derive it against probe 2 before quoting it.
2. **Skip comment and string spans when scanning, per language.** **Cheaper than this file
   assumed — half of it already ships.** Line-leading comments are already skipped for `//`, `/*`,
   `*` and `#` (`:80-96`). What remains is trailing comments and string literals, and the module
   already names `crate::util::text::scan_line` as the existing literal-aware lever it declined to
   reach for (`:51-58`). So this is a smaller change than "needs the same comment-span logic on
   every language".
3. **Give the caller an escape.** Still owed by `IC-6`, but note one exists and is narrower than
   "none": single-line edits are exempt (`:331`, `:335`) and the refusal hint says so. The accurate
   complaint is that it does not scale to a multi-line prose edit.

**Whichever shape is chosen, the guard's advertised scope needs correcting with it.** The condition
text on every refusal claims *"Imports, string literals, comments and config are allowed"* — false
for trailing comments (probe 2) and false for string literals (a documented residual). A reader who
believes it will not suspect the guard.

Preserve the error asymmetry the module states at `:45-49`: a false positive costs one rejected
edit, a false negative risks the LSP range corruption this guard exists to prevent (BUG-027). That
argues for routing through `scan_line` rather than loosening the keyword match.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*
## Workarounds

Native `Edit`, which has no such guard — used for the refused edit in this task. Rephrasing the
prose also works and is worse: it lets a tool defect edit the documentation's wording.

## Tests added

None yet.

## Resume

**The question this section used to ask is answered, so do not re-derive it.** It asked whether
the guard reads `new_string` only or both strings, noting shape 1's cost depends on it.

**Both**, and each is diff-scoped. `guard_structural_rewrite` computes `old_changed =
lines_only_in(old_string, new_string)` and `new_changed = lines_only_in(new_string, old_string)`
(`src/tools/edit_file/mod.rs:328-329`), tests each for a keyword only when that string is
multi-line (`:331`, `:335`), and refuses on `old_kw.or(new_kw)` (`:339`). So an unchanged anchor
line carrying a definition never trips it — verified in § Reproduction probe 2, where `pub fn
alpha` sits in `old_string` and is correctly ignored.

Next action: pick between shapes 1 and 2 in § Fix, both re-priced against the measured mechanism,
and correct the refusal's condition text in the same change. Run § Reproduction probes 2 and 3 as
the before/after pair — probe 3 is the control that keeps a fix from passing by disabling the
keyword test altogether.
## References

- Hit 2026-09-11 while fixing
  `docs/issues/archive/2026-09-09-the-pre-commit-cluster-hook-enforces-a-subset-of-the-gate-it-mirrors.md`.
- `CLAUDE.md` § *Parsers Over a Namespace* — "how does a caller write this token literally", and
  the instruction to state an unaffordable escape at the refusal site.
