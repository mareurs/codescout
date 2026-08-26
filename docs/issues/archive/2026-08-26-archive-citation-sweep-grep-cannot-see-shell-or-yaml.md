---
status: fixed
opened: 2026-08-26
closed: 2026-08-26
severity: low
owner: marius
related: []
tags: [tracker-conventions, guide, doc-refs, archive-flow]
unverified: "The guide fix, step 1 (55 archive-move citations, 7884fc7b) and step 2 (7 comment-borne placeholders — 1 fixed in d7d4a5c7, 6 wontfix as deliberate illustrations) are all done and verified. The severity-promotion policy question is MEASURED and answered `not yet`: ~429 broken code-comment refs remain, still far too many to gate on. What is NOT characterised: what those ~429 actually are. A 50-of-430 sample suggests roughly 76% carry severity_reason=code_comment_capped and the rest are basename_ambiguous or gitignored_path — which would already be med or low on their own merits and so are NOT part of a promotion blast radius — but the audit caps its findings array at 50 with no flag to raise it, so that split is sampled, not counted. Step 3 (re-measure) is deliberately NOT done: the composition would have to change by an order of magnitude to move the answer off `not yet`."
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

## The policy question this raises — now measured

`scan_code_comments`' own comment says the forced `Med` should be promoted *"later if the
surface earns it; that is a policy change worth making on evidence rather than on the first
day."* This section is that evidence, gathered 2026-08-26.

### The numbers

| measure | value |
|---|---|
| broken refs, whole repo | **11,952** |
| findings originating in code comments | **1,781** of 51,941 (**3.4%**), across 219 files |
| **broken** refs originating in code comments | **484** (4.1% of all broken) → **430** after step 1 |
| of those, the `docs/issues/` archive-move class | **55**, in 30 files → **0** (fixed, `7884fc7b`) |

Method, and its two independent instruments — they agree, which is the only reason either
is quoted:

1. **The audit's own census.** `overflow.by_file` is a complete per-file map over *every*
   finding, not the 50-finding display window, so it can be bucketed by extension without
   the cap. Cross-check: it sums exactly to `overflow.total`.
2. **A scoped re-run** (`--paths` restricted to `DEFAULT_AUDIT_CODE_GLOBS`) whose top-level
   `n_refs_broken` is the broken count for code files alone. It found 1,784 refs where the
   census attributed 1,781 to code files — 0.2% apart, the delta being that an explicit
   `paths` argument drops `DEFAULT_AUDIT_EXCLUDES`.
3. **An independent grep + existence check** for the `docs/issues/` subset, which the audit
   cannot isolate (the display cap hides all but 50). Positive-controlled against the audit:
   `src/librarian/frontmatter.rs:669` cites an archived path from a `///` comment, and the
   audit independently reports it `verdict: missing, severity: med, severity_reason:
   code_comment_capped`.

The grep needed a correction worth recording: its first pass returned **77**, which included
15 string literals (`docs/issues/2026-08-16-some-bug.md`, `docs/issues/foo.md` — test
fixtures the audit's tree-sitter pass correctly excludes and a regex does not) and 7 teaching
placeholders. Splitting on "does a file of this basename exist in `docs/issues/archive/`?"
separates the genuine archive-move rot cleanly, because a placeholder has no archived twin.

**That same error was then made a second time, in this file, on the residue** — the "22
refs" of step 2 below were also a regex count, and 15 of them were the same string
literals the audit never reads. **A regex over source and a tree-sitter pass over
documentation nodes are different instruments measuring different populations**, and the
regex over-counts every time by exactly the fixtures. Twice in one sitting is enough to
state it as a rule rather than an anecdote: when the question is "what does this structured
tool see?", the only instrument that answers it is that tool — a grep answers a similar-
sounding question about the bytes.

### Recommendation: do NOT promote, and here is the order to do it in

**484 is too many to switch on at once**, and it is the wrong 484. The pain that actually
occurred is the archive-move class — ~55 refs, each repaired by inserting one path segment.
The other ~429 are unclassified, and the 50-finding sample shows the population includes
**deliberate teaching placeholders inside the audit's own doc comments** — `docs/issues/foo.md`,
`docs/issues/2026-01-01-x.md`, and `docs/issues/….md` (a Unicode ellipsis) all appear as
`verdict: missing`. Promoting severity today would gate CI on the documentation of the very
tool doing the gating.

Sequence, cheapest and most certain first:

1. ~~Repair the 55 archive-move citations.~~ **DONE** — `7884fc7b`, patch-id
   `8ec3a2cf81a80b2283145f3527ed6df6e426dc5f`. Waited for a quiet tree (it is a
   30-file write; `bug-fix-session-log:F-67`), re-derived against HEAD immediately before
   applying because two peer archive moves had landed since the measurement — delta 0.
   Confirmed by both instruments: the archive-move class went 55 → **0**, and the audit's
   broken code-comment refs went 484 → **430** (−54, resolved +56). The −54/55 gap is not
   rounding: `n_refs_found` rose by 2 between runs from a peer commit, and the audit's
   tree-sitter extraction groups refs slightly differently from a line-based count.
2. Exclude the audit's own fixtures and teaching placeholders. **DONE, and it turned out to
   be almost nothing.**

   The "22 refs" figure recorded here on the first pass was **wrong, and wrong the same way
   the 77 was**: a regex over source counts string literals, and the audit's tree-sitter
   pass reads only *documentation nodes*. Splitting on that: **15 of the 22 are string
   literals inside test fixtures, which the audit never sees at all.** Only **7** are
   comment-borne, and only those 7 were ever in the report.

   Of the 7:

   - **1 was a genuine citation, not a placeholder**, and is fixed (`d7d4a5c7`):
     `src/library/auto_register.rs` cited
     `docs/issues/...-edit-code-write-path-ignores-workspace-pin.md` with the date elided,
     so it could never resolve for anyone. The real file is
     `docs/issues/archive/2026-07-09-edit-code-write-path-ignores-workspace-pin.md`. An
     ellipsis reads like a placeholder, which is precisely why it survived every sweep that
     skipped placeholders.
   - **6 are deliberate illustrations and are `wontfix`.** They live in the audit's own doc
     comments and their concreteness is the point. `mod.rs:30` says "`// see
     docs/issues/foo.md` is a pointer, and the backticks are a style habit rather than a
     signal of intent" — templating it to `<slug>` would make it read as a template and
     destroy the illustration. `resolver.rs:634` is sharper still: it explains that
     "`docs/issues/x.md` has a real `docs/`, so it is judged strictly and a miss there is
     genuine drift" — and the audit dutifully reports it as genuine drift. **The tool is
     flagging its own worked example of a correct flag.**

   Rewriting those six would falsify the documentation to satisfy a linter that, by design,
   is not gating on them — the exact move `get_guide("tracker-conventions")` warns against
   for archived documents. Recorded here so nobody re-opens it as cleanup.

   *(A templated form would work if it were ever wanted: measured 2026-08-26,
   `docs/issues/<slug>.md` in `tracker-conventions.md` is not extracted as a ref at all —
   the parser stops at the directory `docs/issues/`, which resolves. That is a principled
   escape, not a heuristic. It is simply the wrong trade here.)*
3. Re-measure. If the residue is small, promotion becomes a cheap change rather than a
   flag day.

**The recommendation is unchanged by step 1: still do not promote.** 430 remains far too
many to switch on, and clearing the archive-move class is what proves the point rather than
weakening it — the entire `docs/issues/` rot that motivated the question was 55 of 484, so
the other 89% is a different population that nobody has characterised.

The rationale for `Med` has not weakened and is not challenged by any of this: a contributor
archiving a bug file still should not break everyone's build via a comment they never
touched. What the measurement changes is that the cost of promotion is now known (484) and
the benefit is now bounded (55), which is enough to say "not yet" with a reason instead of a
shrug.
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
