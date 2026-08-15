---
id: '772fff5739620581'
kind: bug
status: open
title: 'BUG: audit_doc_refs classifies bare comment markers // /// //! as file paths — the second-segment guard counts an empty segment as a segment'
tags:
- audit_doc_refs
- librarian
- parser
- false-positive
topic: audit_doc_refs ref classification
opened: 2026-08-15
owner: marius
related: []
severity: low
---

# BUG: audit_doc_refs classifies bare comment markers // /// //! as file paths — the second-segment guard counts an empty segment as a segment

## Summary

`looks_like_path` admits any anchored string with a second slash on the grounds
that a second path segment is positive evidence of pathness. It tests for that
segment with `!s[1..].contains('/')`, which an **empty** second segment also
satisfies. So bare Rust comment markers — `//`, `///`, `//!` — classify as
`RefKind::FilePath`, become findings, and each carries a remediation note
suggesting the reader widen the audit scope to resolve them.

The guard's own target case works correctly: `/mcp` is rejected. The defect is
that a string with *fewer* real segments passes a test that one with more real
segments fails.

## Symptom (Effect)

Every fenced code block or inline span containing a bare Rust comment marker
contributes a finding:

```json
{
  "raw_ref": "//",
  "ref_kind": "file_path",
  "verdict": "unknown",
  "severity": "low",
  "notes": "path outside active project; scope=umbrella required"
}
```

The `notes` field is the part that costs something. It is actionable-looking
advice — *rerun with a wider scope and this will resolve* — pointing at a token
that is not a path in any scope.

## Reproduction

At `3db6bae5` on `experiments`. Write this to a scratch markdown file anywhere
under the project (the path is arbitrary; delete it afterwards), then scan that
one file with `librarian(action="audit_doc_refs", paths=[<that file>],
write=false)`:

```markdown
Single root segment, no extension (guard SHOULD reject): `/mcp`

Two slashes: `//`

Three: `///`

Inner-doc marker: `//!`

Real anchored path: `/etc/hosts`
```

Observed — 4 refs found:

| written | classified? |
|---|---|
| `/mcp` | no — correctly rejected |
| `//` | **yes**, `file_path` |
| `///` | **yes**, `file_path` |
| `//!` | **yes**, `file_path` |
| `/etc/hosts` | yes, correct |

It also fires from inside fenced code blocks and inline code spans, which is how
it was noticed — quoting `src/librarian/tools/scope.rs`'s overlay comment in a
bug file produced a `//` finding.

## Environment

linux, MCP stdio, project `codescout`, branch `experiments`. **Note:** the
running server is a stale release binary predating the SD-1b prose branch —
confirmed in-session, a bare prose path is not extracted by it. The behaviour
above was observed on that binary; the mechanism below was traced in the source
at `3db6bae5`. Both agree, but the runtime confirmation is not from a
current build. Re-check after `cargo rb`.

## Root cause

`looks_like_path` (`src/librarian/tools/audit_doc_refs/parser.rs:428`), in the
anchored branch:

```rust
if s.starts_with('/') || s.starts_with("./") || s.starts_with("../") {
    // `/foo` with no further structure (no second segment, no extension)
    // is almost always a slash-command or shell shorthand in prose, not a
    // file path. Require either a second path segment or a known extension.
    let single_root_segment = s.starts_with('/') && !s[1..].contains('/');
    if single_root_segment {
        return has_known_ext(s);
    }
    return true;
}
```

Trace for `s = "//"`: no whitespace, no URI scheme, no `~` / `origin/` /
`path/to/` / `*` / `$` / placeholder. It contains `/` and starts with `/`, so
the anchored branch runs. `s[1..]` is `"/"`, which **does** contain `/`, so
`single_root_segment` is `false`, the extension requirement is skipped, and the
function returns `true`.

The comment states the intent exactly — *"require either a second path segment
or a known extension"* — and the expression under it tests for a second
**slash**, not a second **segment**. For `//` the second segment is empty, and
an empty segment is not evidence of anything. `///` and `//!` pass the same way.

Measured 2026-08-15: the repro table above, run against the live server;
mechanism read from the source at the cited line.

## Evidence

The discriminating pair is the whole finding. Both go through the same branch:

- `/mcp` — one real segment, no extension → `single_root_segment` true →
  `has_known_ext` → false → **rejected**.
- `//` — zero real segments → `single_root_segment` false → **accepted**.

A string with *fewer* real path segments is admitted by a guard that rejects one
with more.

## Hypotheses tried

1. **Hypothesis:** the markers are stripped upstream, as they are for prose refs.
   **Test:** the marker strip (`///`, `//!`, `//`, `#`, `*`) lives in
   `parse_prose_refs`, which handles plain prose only. Fenced blocks reach
   `parse_refs` via the `in_code_block` text branch and inline spans via
   `Event::Code`; neither strips markers.
   **Verdict:** confirmed as the reason the prose path is clean and the other
   two are not.

2. **Hypothesis:** `has_uri_scheme` already rejects it, since it exists to catch
   `doc://` and `file://`.
   **Verdict:** rejected — it matches a scheme *before* the slashes. `//` has no
   scheme.

## Fix

Not implemented. Two candidates, and the choice is not obvious:

- **Narrow:** require the second segment to be non-empty — compute
  `single_root_segment` over non-empty segments rather than over raw slash
  presence. Smallest change, fixes `//` and `///`, and `//!` falls out too since
  its only non-empty segment is `!`. Watch the interaction with the
  `s.ends_with('/')` acceptance in the unanchored branch, where a trailing empty
  segment *is* meaningful (`docs/` means a directory).
- **Broad:** reject any string whose characters are all `/` plus punctuation.
  Catches more marker shapes but risks Windows UNC paths (`//server/share`),
  which this repo does care about elsewhere.

Either way the fix belongs beside a test naming the discriminating pair
(`/mcp` rejected, `//` rejected, `/etc/hosts` accepted) — the current tests
cannot fail on this, since none of them feed a bare marker in.

## Tests added

None yet. See Fix.

## Workarounds

None needed — severity is low and the gate is unaffected: these land as
`unknown`/`low` and never drive a non-zero exit under `fail_on: "high"`. The
cost is noise in the findings array (which is capped at 50, so genuine
low-severity findings can be pushed out of view by markers) and the misleading
`scope=umbrella required` note.

## Resume

Pick narrow or broad from Fix, add the discriminating-pair test, then re-run the
Reproduction block against a rebuilt server (`cargo rb`, then `/mcp`) — the
observation above is from the stale binary and should be re-confirmed on a
current one before and after.

## References

- `src/librarian/tools/audit_doc_refs/parser.rs` — `looks_like_path`, `classify`.
- `src/librarian/tools/scope.rs` — the overlay comment whose quotation surfaced this.
- `docs/trackers/structural-debt-refactor.md` — SD-1b built the code-comment
  scanning surface this sits next to.
