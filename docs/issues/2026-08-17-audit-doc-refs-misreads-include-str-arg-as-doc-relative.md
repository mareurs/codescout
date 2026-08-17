---
id: e55b9263451c6647
kind: bug
status: fixed
title: 'BUG: audit_doc_refs resolves an `include_str!` argument against the markdown file, failing the CI gate on a correct doc'
tags:
- audit-doc-refs
- false-positive
- ci-gate
- docs
closed: ''
opened: 2026-08-17
owner: marius
related:
- docs/issues/archive/2026-07-28-audit-doc-refs-json-pointer-false-positive.md
- docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md
- docs/issues/archive/2026-08-15-audit-doc-refs-classifies-comment-markers-as-paths.md
severity: high
---

## Summary

A markdown code span that quotes Rust — `` `include_str!("./render_template.j2")` `` — is
parsed as a file-path reference and resolved **relative to the markdown file's own
directory**. The path is source-relative: rustc resolves an `include_str!` argument against
the *containing `.rs` file*, and the target does exist at
`src/librarian/tools/legibility_scan/render_template.j2`. So a correct doc produced a
`severity: high`, `verdict: missing` finding — and `--fail-on high`, which is exactly what
CI runs, exited 1.

## Symptom (Effect)

`librarian(action="audit_doc_refs", fail_on="high", emit_tracker=false)` at `a1540c8c`:

```json
{
  "n": null,
  "md_file": "docs/architecture/augmented-artifacts.md",
  "md_line": 232,
  "raw_ref": "./render_template.j2",
  "ref_kind": "file_path",
  "verdict": "missing",
  "severity": "high",
  "severity_reason": "policy_default",
  "status": "open",
  "notes": null
}
```

with `exit_code=1`, and it was the **only** `high` in the run (1 high, 49 med in the shown
window).

The source line it flagged:

```markdown
| `legibility_scan::render_managed_body` | the `.md` file body | **no** — `include_str!("./render_template.j2")`, compiled in, for the `legibility-backlog` tracker only |
```

## Reproduction

```
git checkout a1540c8c
./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json --project .
```

This is byte-for-byte the command the `audit-doc-refs` CI job runs
(`.github/workflows/ci.yml:370`).

## Environment

- Branch `experiments`, repo `codescout`, Linux, MCP stdio transport.
- Present from `a1540c8c` (the commit that wrote the table row) onward.
- Scan reported `scan_meta.degraded=true` (`rust: lsp_behind_index`) — **irrelevant to this
  finding**: `ref_kind` is `file_path`, a filesystem check, not an LSP symbol resolution.

## Root cause

**Correction (2026-08-17, after reading the code): the base directory is
`repo_root`, not the markdown file's directory — and Hypothesis 4's own evidence
says so.** The paragraphs below are kept because the *behaviour* they record is
correct; the mechanism they infer is not.

`resolve_file_path` (`src/librarian/tools/audit_doc_refs/resolver.rs:100-126`) joins
against the repo root and never consults `c.md_file` for resolution — `md_file` is
read only by `verdict_with_drops_for_ref`, for the archive-severity drop:

```rust
let path = ctx.repo_root.join(&c.raw_ref);
if path.exists() { … Resolved … }
if let Some(r) = try_basename_fallback(&c.raw_ref, ctx) { return r; }
```

Hypothesis 4 rewrote the ref as the repo-relative
`src/librarian/tools/legibility_scan/render_template.j2` and observed it resolve.
That outcome **distinguishes** the two candidate bases rather than confirming the
markdown one: under a markdown-relative join the base would have been
`docs/architecture/`, so `docs/architecture/src/librarian/…` would not exist and the
ref would have stayed broken. It resolved, so the base is `repo_root`. The
experiment was right and the verdict written under it was backwards.

**Where the markdown-relative base really lives** — and why the confusion is
reasonable: `resolve_link` *does* anchor `./` and `../` to `md_dir`
(`resolver.rs:305-407`, "Refs starting with `./` or `../` are *explicitly* relative to
the file containing the link"). So the doc-relative rule exists, in the resolver for
`RefKind::Link`. The finding carried `"ref_kind": "file_path"`, which is the other
resolver. Two bases, two kinds; the bug attributed one kind's behaviour to the other.

**The actual mechanism is a dead zone, and it takes both halves.**

1. `resolve_file_path` joins `repo_root`, where a leading `./` buys nothing:
   `<root>/./render_template.j2` does not exist, because the file lives under `src/`.
2. `try_basename_fallback` opened with `if raw_ref.contains('/') { return None; }` —
   and `./render_template.j2` contains a slash **inside its own `./`**. So the ref was
   disqualified from the fallback by the very prefix that made the positional attempt
   fail.

Unresolvable positionally *and* ineligible for the fallback built for exactly the
"named without its full path" case. Neither half is wrong alone; together they leave
every `./`-prefixed `file_path` ref with nowhere to land but `Missing` / `high`.

**Why the parser cannot be where this is fixed.** `tokenize_code_span`
(`parser.rs:398-410`) splits on whitespace and on `( ) " ' , ; \``, so
`include_str!("./render_template.j2")` is already two tokens — `include_str!` and
`./render_template.j2` — before `looks_like_path` ever runs. The macro context the
original sketch wanted to condition on is *gone by classification time*; recovering it
would mean re-parsing the span. The resolver is where the information still exists.

Measured 2026-08-17: the audit command in § Symptom → `exit_code=1`. Mechanism then
read at `resolver.rs:100-126`, `:132-169`, `:305-407` and `parser.rs:398-410` — not
inferred.
## Evidence

### The target does exist, at the Rust base

```
$ find src -name '*.j2' -maxdepth 6
src/librarian/tools/audit_doc_refs/render_template.j2
src/librarian/tools/legibility_scan/render_template.j2
```

Note the audit tool has one of its own — so the basename is **ambiguous**, not missing. The
`Verdict` enum already carries `AmbiguousBasename`, which buckets as `unknown` rather than
`broken` and would not have gated.

### The gate is real

`.github/workflows/ci.yml:345-370` — job `audit-doc-refs`, final step:

```
./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json --project .
```

with the comment *"Gate is now `--fail-on high` so any future hi-sev reintroduction fails
the build."* It would have.

## Hypotheses tried

1. **Hypothesis:** the 10,384 `n_refs_broken` indicates broad doc rot, and the `high` is one
   of many.
   **Test:** read `$.findings[*].severity` from the buffered result; read the counter
   definitions at `src/librarian/tools/audit_doc_refs/mod.rs:953,1043`.
   **Verdict:** rejected. `n_refs_found` is `findings.len()` — a "finding" is *every ref
   examined*, resolved ones included — and exactly one finding was `high`.

2. **Hypothesis:** `overflow.total` (46692) is a denominator the tool did not count, the
   same defect as the grep `Showing N of M` family.
   **Test:** read `mod.rs:953` (`let total = findings.len()`).
   **Verdict:** rejected. `total` is the finding count and is correct.

3. **Hypothesis:** the scan is reaching files outside `DEFAULT_AUDIT_GLOBS` (Rust sources,
   `build.rs`, `contrib/pi/codescout-mode.ts` all appear as `md_file`).
   **Test:** read `mod.rs:341-354`.
   **Verdict:** rejected — by design. The default set is `DEFAULT_AUDIT_GLOBS` **chained
   with `DEFAULT_AUDIT_CODE_GLOBS`**; source files are scanned deliberately, to catch bug-file
   citations in code comments that were archived out from under them.

4. **Hypothesis:** the base directory used to resolve the ref is the markdown file's.
   **Test:** rewrite the ref as a repo-relative path, re-run.
   **Verdict:** confirmed. See *Root cause*.

## Fix

**Doc side — landed earlier.** `docs/architecture/augmented-artifacts.md:232` names the
repo-relative path instead of reproducing the macro argument, which is both resolvable
and more useful to a reader. Left as it is: the tool fix below makes the macro form
survivable, but naming the real path is still the better doc.

**Tool side — implemented,** as option (b) from the original sketch, though reached
through the dead zone rather than through macro detection.

One helper, `basename_candidate(raw_ref) -> Option<&str>`
(`src/librarian/tools/audit_doc_refs/resolver.rs`), strips a single leading `./`
before applying the existing no-slash eligibility rule. Both
`try_basename_fallback` and `unique_basename_path` now key off it, so the two cannot
disagree about which refs are basename-shaped — they previously carried the same
`contains('/')` test twice.

That is the whole fix for the reported case: `./render_template.j2` → key
`render_template.j2` → two hits in the index → `AmbiguousBasename` at **Med**,
below `--fail-on high`. A unique basename still resolves to a concrete file, so a
genuinely missing include target is still caught — which is why option (b) beat
skipping the ref outright.

Only one `./` is stripped, and `../` is not touched: those name a location relative to
a base `resolve_file_path` does not have, so they stay positional.

**Plus a guard the fix itself made necessary.** `resolve_link` also calls
`try_basename_fallback`, and for a *link* the `./` is real positional intent — it was
already resolved against `md_dir` a few lines above. With `./` now stripped for
fallback purposes, a broken `./` link would have been silently satisfied by a
same-basename file anywhere in the tree. `resolve_link` therefore only reaches the
fallback for **unprefixed** refs, which are the genuinely ambiguous ones (mdBook pages
are page-relative; the rest of `docs/` is repo-root-relative). This restates the old
behaviour explicitly rather than changing it — before `basename_candidate`, such a ref
fell out of the fallback on its own slash.

Fix SHA: this commit, on `experiments`. `master` is a strict ancestor at fix time, so
the promotion path is fast-forward and this SHA is already the master SHA.
## Tests added

Five, in `src/librarian/tools/audit_doc_refs/resolver.rs`. Written before the fix; the
first two failed with `Missing`, reproducing the CI verdict on a correct doc.

| Test | Mutation it catches |
|---|---|
| `resolver_dot_slash_ref_reaches_the_basename_fallback` | restoring `contains('/')` — the reported ref, asserting `AmbiguousBasename` at Med |
| `resolver_dot_slash_ref_resolves_when_the_basename_is_unique` | a fix that merely stops gating instead of resolving — a unique target must still resolve to a concrete file |
| `resolver_dot_slash_ref_with_real_structure_is_not_basename_resolved` | over-stripping: `./src/missing/b.j2` must not be satisfied by its last segment |
| `resolver_broken_dot_slash_link_still_gates_despite_a_basename_match` | dropping the `resolve_link` guard — the regression this fix would otherwise introduce |
| `resolver_unprefixed_link_still_uses_the_basename_fallback` | over-correcting that guard into switching the fallback off for links entirely |

The third and fifth exist because the first two would pass with a sloppier fix; the
fourth exists because the fix threatened a neighbour.

**Mutation-verified, both directions.** Removing the `resolve_link` guard turns
`resolver_broken_dot_slash_link_still_gates_despite_a_basename_match` red with
`ResolvedBasename` where `Missing` is required — a broken relative link silently
accepted because a same-named file exists elsewhere. Restoring `contains('/')` in
`basename_candidate` turns the first two red with `Missing`.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean,
`cargo test` 4022 passed / 0 failed / 45 ignored.
## Workarounds

Write the repo-relative path in the doc rather than reproducing the macro argument verbatim,
as above. `<!-- audit-doc-refs:ignore -->` also works (precedent:
`docs/manual/src/concepts/librarian-embedded.md:89`) but suppresses a check rather than
satisfying it.

## Resume

The two code paths this file previously flagged as **inferred** have been read, and
one of them said something different from what was inferred — see the correction at
the top of § Root cause. The mechanism is now cited to lines.

Remaining: confirm on the wire after `cargo rb` + `/mcp` by running the CI command
itself,

```
./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json --project .
```

and checking `exit_code=0`. Note this no longer reproduces the *original* finding — the
doc was rewritten before the tool fix — so the wire check is a regression check on the
corpus, not a replay. The replay lives in the unit tests.
## References

- `.github/workflows/ci.yml:345-370` — the `audit-doc-refs` job.
- `src/librarian/tools/audit_doc_refs/mod.rs:213-220` — `DEFAULT_AUDIT_GLOBS`.
- `src/librarian/tools/audit_doc_refs/mod.rs:341-354` — default glob/exclude selection.
- Sibling false-positive bugs, all fixed: JSON pointers
  (`docs/issues/archive/2026-07-28-audit-doc-refs-json-pointer-false-positive.md`),
  `Type/method` slugs
  (`docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`),
  comment markers
  (`docs/issues/archive/2026-08-15-audit-doc-refs-classifies-comment-markers-as-paths.md`).
