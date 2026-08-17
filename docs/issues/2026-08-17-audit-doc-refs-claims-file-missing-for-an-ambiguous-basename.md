---
status: open
opened: 2026-08-17
closed:
severity: low
owner: marius
related: []
tags: [audit-doc-refs, verdict-accuracy, resolver-parity]
kind: bug
---

# BUG: `resolve_file_symbol` reports `FileMissing` where its sibling resolvers report `AmbiguousBasename`, so a present file is called absent

## Summary

A `file_symbol` ref whose path part is a **non-unique** basename resolves to
`Verdict::FileMissing`. The same path part in a `file_path` ref resolves to
`Verdict::AmbiguousBasename`. The second is true and the first is not: the file exists —
twice. The verdict vocabulary already carries the right answer and this one resolver of
four does not reach for it, while its own doc comment claims parity with the ones that do.

## Symptom (Effect)

Measured 2026-08-17 on `docs/trackers/tracker-hygiene-log.md`:

```
med  file_symbol  resolver.rs::note_degraded   verdict: file_missing
```

The symbol exists, and it is in the auditor's own implementation file:

```
symbols(name="note_degraded")
-> src/librarian/tools/audit_doc_refs/resolver.rs:590
     fn note_degraded(ctx: &ResolveCtx<'_>, lang: &str, cause: DegradedCause)
```

So `audit_doc_refs` reports a function inside `audit_doc_refs/resolver.rs` as living in a
missing file.

## Reproduction

Commit `021c130d`, branch `experiments`.

1. In any tracked markdown file, write a `file_symbol` ref whose path part is a basename
   with two or more matches in the repo — `` `resolver.rs::note_degraded` `` works, since
   `src/` holds two `resolver.rs`:

   ```
   src/librarian/tools/audit_doc_refs/resolver.rs
   src/tools/symbol/call_edges/resolver.rs
   ```

2. `librarian(action="audit_doc_refs", emit_tracker=false, paths=["<that file>"])`
3. → `verdict: file_missing`, `ref_kind: file_symbol`.
4. Change the ref to a bare `` `resolver.rs` `` (a `file_path`) and re-run →
   `AmbiguousBasename`, which is the accurate verdict.

## Environment

Linux, `experiments` @ `021c130d`. Affects any repo with a duplicated filename cited
`basename::symbol` — common in Rust, where `mod.rs`, `resolver.rs`, `tests.rs` and
`error.rs` repeat by convention.

## Root cause

`resolve_file_symbol` (`src/librarian/tools/audit_doc_refs/resolver.rs:429-558`) resolves
its path part with `unique_basename_path`, which is all-or-nothing:

```rust
// Same basename shorthand `resolve_file_path` and `resolve_file_line` accept:
// a doc citing `refresh.rs::call` means the repo's one `refresh.rs`. Without
// this the three ref kinds disagree about identical path parts.
let direct = ctx.repo_root.join(path_str);
let path = if direct.exists() {
    direct
} else if let Some(resolved) = unique_basename_path(path_str, ctx) {
    resolved
} else {
    return verdict_with_drops_for_ref(Verdict::FileMissing, …);
};
```

`unique_basename_path` returns `Some` only for exactly one match, so **two matches take the
same branch as zero matches** — and that branch claims `FileMissing`.

The sibling resolvers do not do this. `resolve_file_path` (:100-119) and `resolve_file_line`
(:254-271) route a failed positional lookup through `try_basename_fallback` (:163), which
distinguishes the two cases explicitly:

```rust
/// `Some(Resolution)` with `ResolvedBasename` (single hit) or
/// `AmbiguousBasename` (multiple hits); `None` means "no match — caller should
/// fall through to the default missing verdict".
```

So `AmbiguousBasename` exists precisely for this input and `resolve_file_symbol` never
reaches it. *Measured 2026-08-17: the report above, the two `resolver.rs` paths from
`find src -name resolver.rs`, and the three function bodies read at the lines cited.*

**The comment is the second half of the defect.** It asserts parity — *"Same basename
shorthand `resolve_file_path` and `resolve_file_line` accept … Without this the three ref
kinds disagree about identical path parts"* — which holds for the unique case and fails for
the ambiguous one. The three kinds *do* disagree about identical path parts, in exactly the
way the comment says it prevents. Doc-vs-code drift inside the fix that introduced the
parity.

## Evidence

### This is not the severity-calibration gap it first looked like

The filing this file replaces claimed `audit_doc_refs` lacks severity calibration for
inferred paths, and recommended adding it. That was wrong: `2026-08-06` already shipped
`severity.rs::cap_inferred_path` + `resolver.rs::path_evidence`, whose stated contract is
*"`high` now means 'definitely a local path, definitely gone'"*. That mechanism is why the
15 broken refs in the measured run are **all `med` and none `high`** — the system working,
not failing. Checking the family before filing changed the scope from "the extractor cannot
tell refs from data" to this one verdict.

Four prior bugs in the same family, all archived:

| Bug | Shipped |
|---|---|
| `2026-05-17-audit-doc-refs-basename-false-positives.md` | the basename index + `ResolvedBasename` / `AmbiguousBasename` verdicts |
| `2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` | extractor polarity, `resolve_file_line`'s missing fallback, `cap_inferred_path` |
| `2026-08-15-audit-doc-refs-classifies-comment-markers-as-paths.md` | comment-marker suppression |
| `2026-08-17-audit-doc-refs-misreads-include-str-arg-as-doc-relative.md` | leading `./` reaching the fallback |

Note the shape: 2026-08-06 fixed `resolve_file_line`'s *missing* fallback, and 2026-08-17
fixed a `./` case in `resolve_file_path`'s. Each pass fixed the resolver in front of it.
`resolve_file_symbol` is the fourth of four and the only one still holding an all-or-nothing
basename lookup.

### The remaining 14 findings in that run are designed behaviour, not bugs

Recorded so a later reader does not re-file them: `archive/` (×6) and `archive/archive` earn
`file_path` classification from a **trailing slash**, which `is_path_segment` treats as
positive evidence of pathness by design; `plugin.json`, `mcp.json` and
`tracker-hygiene/SKILL.md` exist in the `claude-plugins` repo, which is not this search
root; `1.16.7`/`1.16.8` land as `module_path` at `low`; `outcome.frontmatter_max` is a
struct field access. All `med` or `low`, none gating. The convention for a deliberately
unreal path is to name it **bare**, un-code-spanned — documented in CLAUDE.md for
`docs/ARCHITECTURE.md` and applied in `021c130d`.

## Hypotheses tried

1. **Hypothesis:** `resolve_file_symbol` never received the basename fallback its siblings
   have — the same *missing capability* shape as 2026-08-06's item 2.
   **Test:** read the function body rather than grep for the call.
   **Verdict:** rejected. It *does* have a basename shorthand; it uses
   `unique_basename_path` instead of `try_basename_fallback`. The grep that suggested
   otherwise showed no `try_basename_fallback` call between lines 429-558, which is true and
   misleading — the capability is present under a different name. Reading the body is what
   corrected it.
2. **Hypothesis:** the false positives would gate a full-severity surface at `high`.
   **Test:** read `severity` on each finding, then read what 2026-08-06 shipped.
   **Verdict:** rejected — `cap_inferred_path` caps an inferred-path `missing` at `med`
   precisely to prevent that. This is why the bug's severity is `low`, not `medium`.

## Fix

Not implemented. One-line shape: in `resolve_file_symbol`, replace the
`unique_basename_path`-else-`FileMissing` branch with `try_basename_fallback`, keeping
`unique_basename_path` for the case where a concrete path is needed to hand to the LSP.
The two are not interchangeable — `try_basename_fallback` answers *"what verdict?"* and
`unique_basename_path` answers *"which file?"*, as their doc comments say — so the branch
needs both: fall back for the verdict when the basename is ambiguous, and only attempt
symbol resolution when a single file was identified.

Then correct the parity comment to state what is actually true after the change.

## Tests added

None. Wanted: a fixture repo with two files sharing a basename, asserting that a
`basename::symbol` ref reports `AmbiguousBasename` rather than `FileMissing`. Worth
asserting for all three ref kinds in one test, since the recurring defect in this family is
the four resolvers drifting apart — a per-kind parity test is the guard that would have
caught 2026-08-06's item 2 as well.

## Workarounds

Cite the symbol with enough path to be unique —
`` `audit_doc_refs/resolver.rs::note_degraded` `` — or with the full repo-relative path.
The positional lookup then succeeds and the basename branch is never reached.

## Resume

Read `unique_basename_path` and `try_basename_fallback` together
(`src/librarian/tools/audit_doc_refs/resolver.rs:160-210`) and settle how to use both in
one branch: the verdict must come from the fallback, the LSP call needs a single concrete
path. Then write the three-kind parity test from § Tests added *first* — it should fail on
`file_symbol` only, which both confirms this filing and proves the other two resolvers are
already correct.

## References
- `src/librarian/tools/audit_doc_refs/resolver.rs:429-558` — `resolve_file_symbol`
- `src/librarian/tools/audit_doc_refs/resolver.rs:163` — `try_basename_fallback`, and the
  doc comment naming the ambiguous case
- `src/librarian/tools/audit_doc_refs/resolver.rs:100,254` — the two resolvers that use it
- `docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` — the
  same *missing capability* shape, one resolver over, plus `cap_inferred_path`
- `docs/issues/archive/2026-05-17-audit-doc-refs-basename-false-positives.md` — where
  `AmbiguousBasename` came from
- `docs/trackers/tracker-hygiene-log.md` — HY-6 (`degraded` flag, same file), HY-13
