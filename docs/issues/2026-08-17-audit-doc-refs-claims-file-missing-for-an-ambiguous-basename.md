---
status: fixed
opened: 2026-08-17
closed: 2026-08-17
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

**Still open and reproducible.** Re-verified 2026-08-17 against the binary built at 13:41
(HEAD `201628f9`; no commit has touched `src/librarian/tools/audit_doc_refs/` since
`f6140205`). Scoped run over this file plus `docs/trackers/tracker-hygiene-log.md` — every
`file_symbol` finding, and nothing filtered out:

```
file_missing   med  resolver.rs::note_degraded              <- this file:27
file_missing   med  resolver.rs::path_evidence              <- this file:114
file_missing   med  resolver.rs::note_degraded              <- tracker-hygiene-log.md:397
resolved       low  refresh.rs::call                        <- this file:71
resolved       low  severity.rs::cap_inferred_path          <- this file:114
```

`scan_meta.degraded: false`, so the LSP answered — these are not cold-start or
server-behind-index artifacts (the failure mode of
`docs/issues/archive/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`).

**That table is a controlled experiment, not just a reproduction.** Five refs of the
*identical* `basename::symbol` shape, three of them in one file, and the verdict splits on
exactly one variable:

| path part | files with that basename | verdict |
|---|---|---|
| `resolver.rs` | **2** | `file_missing` |
| `refresh.rs` | 1 | `resolved` |
| `severity.rs` | 1 | `resolved` |

So the basename shorthand demonstrably works — `refresh.rs::call` and
`severity.rs::cap_inferred_path` resolve through it. It fails only on **ambiguity**, which
is what rules out "the shorthand is broken" and pins the defect on the all-or-nothing
branch in § Root cause.

The symbol exists, and it is in the auditor's own implementation file:

```
symbols(name="note_degraded")
-> src/librarian/tools/audit_doc_refs/resolver.rs:590
     fn note_degraded(ctx: &ResolveCtx<'_>, lang: &str, cause: DegradedCause)
```

So `audit_doc_refs` reports a function inside `audit_doc_refs/resolver.rs` as living in a
missing file — twice over, since `path_evidence` is in that same file.
## Reproduction

Commit `201628f9`, branch `experiments`, binary built 13:41. **Re-verified 2026-08-17** with
a four-line fixture that discriminates every case at once — four refs, no truncation, no
surface drops to reason around:

```markdown
Full repo-relative path: `src/librarian/tools/audit_doc_refs/resolver.rs::note_degraded`
Partial path:            `audit_doc_refs/resolver.rs::note_degraded`
Bare ambiguous basename: `resolver.rs::note_degraded`
Bare unique basename:    `refresh.rs::call`
```

`librarian(action="audit_doc_refs", emit_tracker=false, paths=[…])` →

| Ref form | Verdict | Severity |
|---|---|---|
| full repo-relative path | `resolved` | low |
| partial path (`audit_doc_refs/resolver.rs`) | **`file_missing`** | med |
| bare **ambiguous** basename (`resolver.rs`, 2 matches) | **`file_missing`** | med |
| bare **unique** basename (`refresh.rs`, 1 match) | `resolved` | low |

Rows 3 and 4 are the bug: identical shape, opposite verdicts, and the only difference is
how many files carry the basename. Rows 1 and 2 are the workaround boundary (§ Workarounds).
`scan_meta.degraded: false` on every run, so no row is an LSP artifact.

This fixture is what § Tests added should become — it is already minimal, already
discriminating, and it pins the two correct behaviours alongside the two defective ones, so
a fix cannot pass it by making everything resolve.
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


### Severity is capped by `cap_inferred_path`, verified on a surface with no drop

The two naturally-occurring instances both landed `med`, but for **surface** reasons —
`severity_reason: issues_drop` in a `docs/issues/` file and `historical_drop` in the
tracker. Neither proves the path-evidence cap does anything, so the claim that this bug
cannot gate CI was untested. Probed directly on a full-severity surface
(`docs/architecture/`, no drop applies):

```
resolver.rs::note_degraded   file_missing   med   severity_reason: inferred_path
refresh.rs::call             resolved       low   severity_reason: policy_default
```

`inferred_path` is `cap_inferred_path` firing, so the cap holds on the surface that matters
and `high` stays reserved for "definitely a local path, definitely gone". **That is why this
filing is `low` rather than `medium`** — measured, not reasoned. The ratified archive gate
("0 high findings") is not exposed to this bug.
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

**Fixed 2026-08-17 in `3faddb15` (`experiments`).** Promotion to `master` is a
fast-forward, so this SHA *is* the master SHA once promoted — no second one to record.

The `else` arm of the path-resolution branch in `resolve_file_symbol` now consults
`try_basename_fallback`:

```rust
} else {
    // Ambiguous is not missing. `unique_basename_path` answers "which file?" and so
    // returns None for BOTH zero matches and two-or-more — which meant this branch
    // claimed `FileMissing` for a file that exists twice. `try_basename_fallback`
    // answers "what verdict?", and is the same helper `resolve_file_path` and
    // `resolve_file_line` use to tell the two apart. Here it can only return
    // `AmbiguousBasename`: a single match would already have been taken above.
    return try_basename_fallback(path_str, ctx).unwrap_or_else(|| {
        verdict_with_drops_for_ref(Verdict::FileMissing, path_str, …)
    });
};
```

`FileMissing` is **preserved** for the genuinely-absent case rather than widened away —
that is what the companion test pins.

The parity comment above the branch was corrected in the same commit. It had asserted
*"without this the three ref kinds disagree about identical path parts"* while only the
unique half was implemented: true of one outcome, false of the other, and doc-vs-code drift
inside the very fix that introduced the parity.

**Deliberately not done: resolving the ambiguity by searching for the symbol.** Checking
every candidate file and resolving when exactly one contains the symbol would make
`resolver.rs::note_degraded` resolve outright, and it is tempting because it makes correct
docs go clean rather than merely honest. Rejected because it costs one `document_symbols`
call per candidate per ambiguous ref, on a subsystem whose LSP budget is already the
subject of three archived bugs (cold-start budget, stubbed-off client, server-behind-index).
`AmbiguousBasename` is the answer the sibling resolvers give for this input, and parity was
the defect — so parity is the fix. If the noise ever justifies it, the capability belongs
behind the same measurement the other three got.
## Tests added

Two, in `src/librarian/tools/audit_doc_refs/resolver.rs`, placed directly after
`resolver_file_line_reports_ambiguous_basename` so the three ref kinds' ambiguity tests sit
together:

- **`resolver_file_symbol_reports_ambiguous_basename`** — the RED test. Written first,
  watched fail with `left: FileMissing, right: AmbiguousBasename`, then passed.
- **`resolver_file_symbol_still_reports_file_missing_when_no_file_matches`** — the
  discriminating companion. It passed from the start **by design**: it pins the behaviour
  the fix must not break, since a fix that turned every unresolvable path part into
  "ambiguous" would hide genuinely dead references, and nothing else would catch that.

**Mutation-verified.** Reverting the `else` arm fails exactly
`resolver_file_symbol_reports_ambiguous_basename` and leaves 146 audit_doc_refs tests green,
including the companion — so the test is specific to this fix, and nothing else in the suite
depended on the old behaviour.

Still wanted, and now the more valuable test: the **per-kind parity table** over
`file_path` / `file_line` / `file_symbol`, asserting all four verdicts from § Reproduction
for each kind. Every bug in this family has been one resolver lagging its siblings —
2026-05-17 for two of them, 2026-08-06 for `file_line`, this one for `file_symbol` — and a
parity table is the single guard that would have caught all three, plus the next.
## Workarounds

Cite the symbol with the **full repo-relative path** —
`src/librarian/tools/audit_doc_refs/resolver.rs::note_degraded` resolves, because the
positional lookup succeeds and the basename branch is never reached.

That ref is written inline here on purpose, so this file's own audit is the proof: a scoped
`audit_doc_refs` reports it `resolved / low` while the two bare-basename forms above it
report `file_missing / med`. Verified 2026-08-17 — the first draft of this section put the
working form in a fenced block, where it was never extracted and so never tested, and
asserted it worked from reasoning alone.

**A partial path does NOT work, and this file said it did until 2026-08-17.** The original
text suggested `audit_doc_refs/resolver.rs::note_degraded` — "enough path to be unique".
That prescription is wrong, and `basename_candidate`
(`src/librarian/tools/audit_doc_refs/resolver.rs:151-157`) is why:

```rust
let stripped = raw_ref.strip_prefix("./").unwrap_or(raw_ref);
if stripped.is_empty() || stripped.contains('/') {
    return None;
}
```

Any `/` disqualifies the ref from the basename fallback, while a path that is not
repo-root-relative fails the positional attempt — so a partial path is unresolvable **both
ways**. That is the identical dead zone the same function's doc comment describes for
`./`-prefixed refs, which was
`docs/issues/archive/2026-08-17-audit-doc-refs-misreads-include-str-arg-as-doc-relative.md`.
I walked into it while writing the workaround for its sibling bug.

Either give the whole path or give the bare basename. There is no middle.
## Resume

One step left before archiving: **replay on the wire.** The fix is compiled into the test
binary but the running MCP server serves the previous release build, so `cargo rb` + `/mcp`
is required, then re-run the § Reproduction fixture and confirm the bare ambiguous basename
reports `AmbiguousBasename` / med instead of `file_missing`. That is the standard this
family is held to — `f6140205` archived its sibling as *"replayed on the wire"* — and it is
stronger than a green unit test, because the unit test supplies its own
`basename_index` while the wire path builds one from the real repo.

After the replay: archive through the catalog to `docs/issues/archive/`, and re-point the
citations of this path in the same commit — `src/librarian/tools/audit_doc_refs/resolver.rs`
carries one in the new tests' doc comment.
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
