---
id: '808b56f05218d250'
kind: bug
status: open
title: 'BUG: grep''s zero says nothing about gitignored paths, and include_hidden=true — which its own warning recommends — does not reach them, so widening the search removes the warning'
tags:
- grep
- false-negative
- warning-composition
closed: null
opened: 2026-08-27
owner: marius
severity: med
---

## Summary

`grep`'s zero-match warning names `.superpowers/` among the hidden paths it skipped and
says *"Pass `include_hidden=true` to search them"*. Passing it does not search them — the
directory is also **gitignored**, which `include_hidden` does not lift — and the resulting
zero carries **no warning at all**. Widening the search made the caller *less* informed
than not widening it.

Same class as `444d756c` (*"a zero from a glob that opened no file says so"*), one
exclusion mechanism over.

## Symptom (Effect)

```
grep(pattern="l3-measures-vocabulary-overlap")
→ 0 matches
  warning: this zero describes what was searched, not the pattern. Hidden paths were
  not searched, including .buddy/, .cargo/, .claude/, .fastembed_cache/, .github/,
  .superpowers/, .worktrees/, .env and 8 more at the search root. Pass
  include_hidden=true to search them — …

grep(pattern="l3-measures-vocabulary-overlap", include_hidden=true)
→ 0 matches
```

Bare. No warning, no caveat, nothing to distinguish it from a genuine absence.

Ground truth: the string is present, once, in
`.superpowers/sdd/2026-08-25-unanchored-blast-radius-eval/progress.md`.

## Reproduction

No spend, three calls, on this repo:

```bash
# ground truth — the string exists
grep -c "l3-measures-vocabulary-overlap" \
  .superpowers/sdd/2026-08-25-unanchored-blast-radius-eval/progress.md
# 1

git check-ignore -v .superpowers/sdd/2026-08-25-unanchored-blast-radius-eval/progress.md
# .superpowers/sdd/.gitignore:1:*    .superpowers/sdd/…/progress.md
```

Then the two `grep` calls above. The first warns and names `.superpowers/`; the second is
silent.

Any gitignored dotdir reproduces it. `.superpowers/sdd/` is a good one because the
SDD ledger lives there and is a routine search target between sessions.

## Environment

- codescout `444d756c` on `experiments`, freshly rebuilt (`cargo rb`) + `/mcp` reconnect
- Found while re-pointing citations after an archive move, i.e. during exactly the sweep
  `tracker-conventions` prescribes — where a false zero means a dangling reference ships

## Root cause

Not established beyond the observable, and deliberately not guessed at. What is
established:

- The walk excludes gitignored paths (standard, and almost certainly intended).
- `include_hidden=true` lifts the **dotfile** exclusion, not the **gitignore** exclusion.
  These are two filters; the flag addresses one.
- The zero-match warning is composed per condition — verified separately: a glob that
  opened no file gets the glob clause, and a `path`-narrowed search with no hidden entries
  correctly omits the hidden clause. There is simply **no clause for gitignore**, so with
  the hidden clause suppressed by the flag, nothing is left to emit.

The interesting part is the interaction, not either filter: the hidden clause is
suppressed *because the caller acted on it*, and the exclusion that actually caused the
zero was never mentioned in the first place.

## Evidence

- The two calls above, run consecutively, same pattern, same repo.
- `grep -c` = 1 on the file, and `git check-ignore -v` naming the rule. Ground truth
  established through a permitted tool before the claim was written — see `R-118`, which
  is the entry about *not* doing that.
- The warning machinery is otherwise sound: three probes on the same rebuild showed it
  discriminating correctly between "glob opened nothing", "path narrowed the root", and
  "files opened, pattern missed".

## Hypotheses tried

1. **The string is not actually there** — refuted, `grep -c` = 1.
2. **`.superpowers/` is only hidden, so `include_hidden` should suffice** — refuted,
   `git check-ignore` shows a second, independent exclusion.
3. **The warning is boilerplate and just didn't render** — refuted; the same rebuild emits
   different clause combinations for different conditions, so its silence here is a
   missing clause, not a missing warning.

## Fix

**Candidate (1) as first written is wrong. Corrected 2026-08-27 after reconnaissance — see `R-121`.**

The original text proposed a gitignore clause *"symmetric with what already exists"*, i.e.
modelled on `WalkAudit::hidden_at_root` (`src/tools/grep.rs:1146-1177`). Scouting that
function before implementing showed the symmetry does not hold:

- `hidden_at_root` inspects only the search root — one `read_dir`, no recursion, and its
  doc comment says so explicitly. That is honest for dotfiles **because the clause's remedy
  is root-agnostic**: `include_hidden=true` lifts the dotfile filter at every depth, so a
  root-level list is a fair sample of what the remedy will reach.
- Gitignore has no honest root approximation. `git check-ignore` over all 40 entries at
  this repo's root returns six — `.env`, `.fastembed_cache`, `models`, `target`,
  `temp-docs`, `.worktrees` — and **`.superpowers/` is not among them.** It is hidden-only.
  The rule that produces the reported zero is `.superpowers/sdd/.gitignore:1:*`, at depth 2.

So the root-scan version would name six innocent paths and still miss the guilty one —
precisely the failure mode `completeness_warning`'s own doc comment names: *"naming an
unchecked cause ends the search for the real one."* Worse, its natural regression test
plants a match in a **root-level** gitignored directory, so it passes, and the repro in
this file goes on reproducing behind a green gate.

### The corrected fix

**Emit a mechanism clause, not a path list, and gate it on `include_hidden == true`.**

> Gitignored paths were not searched. `include_hidden=true` lifts the dotfile filter only —
> `.gitignore` rules at any depth, including nested ones, are a second and independent
> exclusion that no `grep` argument lifts.

Three properties earn that shape:

- **It claims only what is certain.** No enumeration, so no unchecked cause is named, and
  nested rules are covered because the sentence is about the mechanism rather than a
  location.
- **`None` stays load-bearing.** Gating on the flag leaves the default path byte-identical:
  a clean walk over a tree with no hidden entries still returns a bare zero. Only the
  widened search gains a clause — and a widened search that returned nothing is both rare
  and, by construction, one whose caller has already acted on the first warning.
- **It fires exactly where the defect is.** The reported regression *is* the widened call.

Candidate (2) — a `no_ignore` / `include_ignored` flag — remains the capability the warning
implies exists, and remains out of scope here.

Fix SHA + `git patch-id --stable`: *not yet fixed.*
## Tests added

None — not fixed. A regression test should plant a match inside a gitignored directory and
assert the zero carries a gitignore clause, both with and without `include_hidden=true`.
Pinning **both** matters: the flag-set case is the one that regressed to silence, and a
test covering only the default would stay green through it.

## Workarounds

- Shell `grep`/`git grep --no-index` for anything under a gitignored path.
- When a search is part of a citation sweep, remember gitignored scratch (`.superpowers/`,
  `.buddy/`) is invisible to `grep` regardless of flags, and check it separately.

## References

- `src/tools/grep.rs` — the zero-match warning, rebuilt in `444d756c`
- `docs/issues/archive/2026-07-18-grep-glob-literal-path-false-negative-unconfirmed.md` —
  the bug `444d756c` fixed; this is the same class through a different filter
- `docs/trackers/reconnaissance-patterns.md` — `R-118`, from the same pass; law C, and why
  ground truth came first this time
