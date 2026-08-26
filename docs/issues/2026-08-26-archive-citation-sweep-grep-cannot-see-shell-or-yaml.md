---
status: fixed
opened: 2026-08-26
closed: 2026-08-26
severity: low
owner: marius
related: []
tags: [tracker-conventions, guide, doc-refs, archive-flow]
unverified: 'The guide fix is done and verified. What is NOT decided: whether code-comment citations should keep their forced `Med` severity. One datapoint is recorded below; the policy is deliberately left alone, because the reason for `Med` is still good and one incident is not evidence enough to overturn it.'
kind: bug
---

# BUG: the prescribed archive-citation sweep cannot see `.sh`, `.yml`, `.py` or `.toml`

## Summary

`get_guide("tracker-conventions")` tells you to re-point every citation in the same commit
as an `artifact(action="move")`, correctly warns that a green CI run does not prove you
did, and then hands you a `grep` whose `--include` list covers only `*.md`, `*.rs` and
`.env*`. Six live surfaces in this repo cite `docs/issues/` from file types that list
cannot see. One of them broke today.

## Symptom (Effect)

`7523f210` archived a bug file and ran the prescribed sweep, which reported nothing to fix
outside markdown. `scripts/build-windows.sh:48` was left citing the pre-archive path:

```
# (`docs/issues/2026-08-26-windows-lanes-still-red-on-four-remaining-causes.md` group A).
```

It surfaced hours later, by accident, while editing that file for an unrelated reason.

**The failure shape is the dangerous one:** a `--include` filter that misses a file type
returns a clean zero, which is indistinguishable from "no citations to fix". Nothing
errors, and the sweep looks like it ran.

## Reproduction

```
grep -rn 'docs/issues/' . --include='*.md' --include='*.rs' --include='.env*'   # misses it
grep -rn 'docs/issues/' scripts/build-windows.sh                               # finds it
```

## Environment

`experiments`, any host. Not environment-specific — it is a property of the documented
procedure.

## Root cause

`src/prompts/guides/tracker-conventions.md`, in the archive-flow section, specified:

```
grep -rn 'docs/issues/<slug>.md' . --include='*.md' --include='*.rs' --include='.env*'
```

The `--include` list is a hypothesis about where citations live, and it was wrong. Measured
2026-08-26 on this repo — files citing `docs/issues/` from outside `docs/issues/` itself,
by extension:

```
208 md   140 jsonl   133 rs   71 diff   3 json   2 yml   2 sh   2 py   1 toml   …
```

`.jsonl` and `.diff` are session artefacts under `.buddy/` and `.superpowers/` — not
surfaces. The **6 live ones the filter could not see**:

- `.github/workflows/ci.yml` — which this session alone added several bug-file citations to
- `docker-compose.yml`
- `scripts/build-windows.sh`
- `scripts/fetch-models.sh`
- `scripts/friction-probe.py`
- a `tests/fixtures` `.toml`

This is the R-3/R-79 law in its most ordinary clothing: *a search that finds nothing is
evidence about the search.* Whichever part of a query you wrote from memory rather than
verified against the data is the part that fails silently.

## Evidence

### The plausible explanation for the CI silence is the wrong one

The obvious reading — "`Audit Doc Refs` only scans markdown, so CI could not catch it
either" — is **false**, and was written into `d56f6145`'s commit message before being
checked. Verified afterwards:

- `DEFAULT_AUDIT_CODE_GLOBS` (`src/librarian/tools/audit_doc_refs/mod.rs`) lists `**/*.sh`
  and `**/*.bash`.
- `bash` is one of the ten languages `code_comments.rs` pins in its
  `every_supported_language_yields_its_doc_text` test.

So the audit **scanned the file and found the citation**. It reported it at `Med`, because
`scan_code_comments` forces that severity on purpose:

> *"A citation rotting inside a comment is real drift and should be visible, but it must
> not gate CI at `--fail-on high` — a contributor who archives a bug file should not break
> the build for everyone via a comment they never touched."*

`--fail-on high` therefore passed, correctly. The finding existed; the gate was silent by
design. That is a much better reason than the one I assumed, and it is the reason the guide
already gives for saying "the grep is the check".

## Hypotheses tried

1. **`audit_doc_refs` does not scan shell scripts.** **Test:** read
   `DEFAULT_AUDIT_CODE_GLOBS` and `code_comments.rs`'s language list. **Verdict:** rejected
   — `**/*.sh` is included and `bash` has a grammar.
2. **It scanned but the finding was suppressed below the gate threshold.** **Test:** read
   `scan_code_comments`' doc comment and severity assignment. **Verdict:** confirmed, and
   deliberate.

## Fix

`src/prompts/guides/tracker-conventions.md`:

- The sweep command now includes `*.sh`, `*.py`, `*.yml`, `*.toml` alongside the originals,
  and is line-wrapped so the list is readable rather than a wall.
- Added a paragraph naming the trap explicitly — that the `--include` list is the part that
  fails silently — with the measured histogram and the six surfaces, plus the advice to drop
  the filters entirely and read the extension histogram when unsure.
- Corrected the implied reason the gate stays quiet: it is forced `Med` severity on code
  comments, not a markdown-only scan. A reader who believed the wrong reason would "fix" it
  by widening the scan, which is already wide enough.
- The **live surfaces** list now names CI workflows and `scripts/` explicitly.

- **SHA:** `4c8fb479` (branch `experiments`)
- **patch-id:** `a9fc8e1ecee69fd652e49399c2ccb1b23b31de78`

## Tests added

None. The change is guide prose; the existing `guide_bodies_contain_no_deprecated_tool_names`
and `guide_topics_have_bodies` invariants cover it structurally (92 guide tests green after
the edit). A test asserting a particular `--include` list would pin the exact defect this
bug is about — a hard-coded guess at where citations live — one layer further down.

## The policy question this raises, and does NOT answer

`scan_code_comments`' own comment says the forced `Med` should be promoted *"later if the
surface earns it; that is a policy change worth making on evidence rather than on the first
day."*

**This is one datapoint toward that.** An archive move left a `Med` finding in a live,
load-bearing script — `scripts/build-windows.sh`'s header is the documentation for the
whole local Windows loop — and it went unnoticed until someone opened the file for an
unrelated reason.

It is deliberately not acted on. The reason for `Med` has not weakened: a contributor
archiving a bug file still should not break everyone's build via a comment they never
touched, and this repo carries **11,940** broken refs at the last full audit, so the blast
radius of a severity promotion is unmeasured and probably large. The right next step is to
count how many of those are code-comment citations before anyone proposes promoting them —
not to promote on one incident.

## Workarounds

Drop the `--include` filters and read the extension histogram, as the guide now suggests.

## Resume

N/A — guide fixed. Reopen only to act on the severity-policy question above, and measure
the code-comment share of broken refs first.

## References

- `src/prompts/guides/tracker-conventions.md` — the guide fixed here
- `src/librarian/tools/audit_doc_refs/mod.rs` — `DEFAULT_AUDIT_CODE_GLOBS`,
  `scan_code_comments` and the `Med` rationale
- `docs/issues/archive/2026-08-26-windows-lanes-still-red-on-four-remaining-causes.md` — the
  archive move whose sweep missed the `.sh` citation
