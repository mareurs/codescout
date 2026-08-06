---
id: '21f6d21b3bf82c30'
kind: bug
status: open
title: audit_doc_refs flags JSON/config pointers as missing file paths with severity high
tags:
- librarian
- audit_doc_refs
- false-positive
- tooling
closed: ''
opened: 2026-07-28
owner: marius
related:
- docs/issues/archive/2026-05-17-audit-doc-refs-basename-false-positives.md
severity: low
---

# BUG: audit_doc_refs flags JSON/config pointers as missing file paths with severity high

> **2026-08-06 — wider than JSON pointers, and now failing CI.** The same extractor
> also misclassifies `Type/method` (codescout's own `name_path` symbol syntax), GitHub
> `org/repo` slugs, and ellipsis-elided external paths (`…/rocks/v492/LOCK`) as local
> file paths at severity `high`. All 18 high-severity findings on `experiments` are
> false positives of this class, and the `audit-doc-refs` CI job is red on them (it has
> no `continue-on-error`). Token inventory, per-ref classification and the proposed
> extractor + severity fix:
> `docs/issues/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`.
## Summary

`looks_like_path` classifies any relative multi-segment slash token as a filesystem
path unless it matches one of eight hardcoded rejections. A JSON pointer written in
prose — `mcpServers/codescout/env`, the standard way to name a key inside
`.claude.json` — passes every rejection, is classified `FilePath`, fails resolution,
and is reported **severity high**. The document is correct; the linter is wrong.

## Symptom (Effect)

Scanning a tracker that names a config key by its JSON path:

```
librarian(action="audit_doc_refs", paths=["docs/trackers/retrieval-benchmark.md"],
          emit_tracker=false)
```

```json
{
  "md_file": "docs/trackers/retrieval-benchmark.md",
  "md_line": 228,
  "raw_ref": "mcpServers/codescout/env",
  "ref_kind": "file_path",
  "verdict": "missing",
  "severity": "high",
  "severity_reason": "policy_default"
}
```

This was the **only** high-severity finding across 111 refs in that file. Every
genuine code reference in the same document resolved.

## Reproduction

```
git rev-parse HEAD    # 59f9a6b1cbbfb4fa2a59ebffe6b50eb99bad38cc, branch experiments
```

Any markdown containing an inline-code JSON pointer with two or more segments:

```markdown
The live config at `<profile>/.claude.json` -> `mcpServers/codescout/env` sets it.
```

Then `librarian(action="audit_doc_refs", paths=[that file])`. Expect one
`file_path` / `missing` / `high` finding on the pointer.

## Environment

Linux, codescout `experiments` @ `59f9a6b1`, MCP stdio transport, project codescout.
Not host- or LSP-dependent (`scan_meta.degraded: false`,
`lsp_languages_offline: []`) — the misclassification happens in the parser, before
resolution.

## Root cause

`looks_like_path` (`src/librarian/tools/audit_doc_refs/parser.rs:147-199`) is a
**denylist followed by an accept-by-default**. It rejects, in order: whitespace, URI
schemes, `~`-relative, `origin/`, `upstream/`, `path/to/`, `*` globs, `<>`
placeholders, `$` shell expressions. Then:

```rust
if s.contains('/') {
    let single_root_segment = s.starts_with('/') && !s[1..].contains('/');
    if single_root_segment {
        return has_known_ext(s);
    }
    return true;              // <-- any relative multi-segment slash token
}
has_known_ext(s)
```

The absolute single-segment case (`/foo`) must earn its classification via
`has_known_ext`. The relative multi-segment case earns nothing — `return true` is
unconditional. `mcpServers/codescout/env` has two slashes, does not start with `/`,
and matches none of the nine rejections, so it is a `FilePath` candidate.

The polarity is the defect, not the missing case. The rejection list is already
nine entries of accumulated special-casing (see the parser's own test names:
`parser_rejects_git_refs`, `parser_rejects_path_to_placeholder`,
`parser_rejects_home_relative`, `parser_rejects_shell_expression`), which is the
signature of a default that fires too often. Every new documentation idiom that
happens to contain a slash becomes a new special case.

Note the asymmetry with the already-fixed sibling bug
(`docs/issues/archive/2026-05-17-audit-doc-refs-basename-false-positives.md`): that
one covered the **no-slash** branch, which was tightened to require
`has_known_ext`. The slash branch was left permissive and is now the remaining hole.

## Evidence

### The classification is unconditional, not resolution-dependent

`ref_kind` is assigned by the parser before any filesystem lookup
(`parser.rs:81-83`), so `verdict: "missing"` is the *consequence* of the
misclassification, not independent evidence of a bad path.

### Severity is policy, not confidence

`severity_reason: "policy_default"` — the finding is high because unresolved
`FilePath` refs are high by policy, with no confidence weighting for
"extensionless token that has never existed on disk".

## Hypotheses tried

1. **Hypothesis:** covered by the archived basename false-positive fix.
   **Test:** read that bug plus the current `looks_like_path`.
   **Verdict:** rejected — that fix tightened the extensionless **no-slash** path
   (`has_known_ext` at the tail). The `s.contains('/')` branch still returns `true`
   unconditionally for relative multi-segment tokens.
2. **Hypothesis:** the ref only looks broken because the LSP was offline.
   **Test:** inspect `scan_meta` in the same response.
   **Verdict:** rejected — `degraded: false`, `lsp_languages_offline: []`.

## Fix

Not implemented. Preferred direction, in order of increasing ambition:

1. **Cheapest, consistent with the sibling fix:** require `has_known_ext(s)` **or**
   an on-disk hit for extensionless relative tokens, i.e. drop the bare
   `return true` and let extensionless multi-segment tokens resolve-or-be-ignored
   rather than resolve-or-be-high. A path that exists still resolves; one that
   never existed and has no extension stops being a high finding.
2. **Better:** downgrade severity for extensionless candidates —
   `severity_reason: "extensionless_candidate"` at low — so a real
   `docs/some-dir` reference is still surfaced without failing a `fail_on=high`
   gate.
3. **Root:** invert the polarity. Classify `FilePath` on positive evidence (known
   extension, on-disk hit, or a known project directory prefix) instead of
   accept-by-default-minus-denylist. This retires the special-case list.

Do not fix by adding `mcpServers/` to the denylist — that repeats the pattern this
bug is about.

## Tests added

None yet — no fix. When fixed, the regression test belongs beside the existing
rejection tests in `src/librarian/tools/audit_doc_refs/parser.rs`, asserting no
`RefKind::FilePath` candidate for extensionless config pointers. Cover at least
`mcpServers/codescout/env`, and a positive control that a real extensionless
directory reference still resolves.

## Workarounds

- `fail_on` defaults to `never`, so this does not fail the manual audit today. It
  *would* fail a `fail_on=high` gate on a fully correct document.
- In prose, write the pointer with a leading dot (`.mcpServers.codescout.env`) or
  as separate segments — but the linter should not be dictating prose style, which
  is why this is filed rather than papered over.

## Resume

Open `src/librarian/tools/audit_doc_refs/parser.rs:188-196` and replace the bare
`return true` with option 1 above (`has_known_ext(s) || resolves_on_disk`). Add the
rejection test next to `parser_rejects_git_refs`. Then re-run
`librarian(action="audit_doc_refs", paths=["docs/trackers/retrieval-benchmark.md"],
emit_tracker=false)` and confirm `n_refs_broken` drops by one and no high-severity
finding remains.

## References

- `src/librarian/tools/audit_doc_refs/parser.rs:147-199` — `looks_like_path`
- `src/librarian/tools/audit_doc_refs/parser.rs:81-83` — where `FilePath` is assigned
- `docs/issues/archive/2026-05-17-audit-doc-refs-basename-false-positives.md` — the
  no-slash sibling, fixed
- `docs/trackers/retrieval-benchmark.md:228` — the document that surfaced it
