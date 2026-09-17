---
id: 99904f10f3084ee1
kind: bug
status: fixed
title: 'BUG: doc(update) section replace drops the blank line before the next heading, silently'
tags:
- cluster/accepted-parameter-silently-dropped
- librarian
- doc-tool
- markdown
topic: librarian document editing
claimed_at: 2026-09-17
claimed_by: a3bf229c-658b-42f9-8f4b-794fcf0d35c7
closed: 2026-09-17
opened: 2026-09-15
severity: low
---

# BUG: doc(update) section `replace` drops the blank line before the next heading

## Summary

`doc(action="update", patch={body_edits: [{action: "replace", …}]})` writes the new section
body with no blank line between its last line and the **next heading**, joining them. The
`content` may be passed *with* a trailing newline and it is still discarded, the call returns
`updated: true`, and nothing in the response mentions it. The same tool's `action: "edit"`
preserves the byte.

## Symptom (Effect)

After four `replace` edits to one bug file, four headings had lost their preceding blank line:

```
is not simply broken.
## Root cause
```

No error, no warning. The response was `{"id": …, "updated": true, "wrote_to": …}` each time.

## Reproduction

1. Take any catalog-managed markdown artifact whose sections are separated by blank lines.
2. `doc(action="update", id=…, patch={body_edits: [{heading: "## X", action: "replace",
   content: "…text…\n"}]})` — note the trailing newline in `content`.
3. Read the file: the line following the section's last line is `## <next heading>`, with no
   blank line between them.

Detector, which is also how this was found:

```bash
awk 'NR>1 && /^## / && prev != "" {print NR": "$0} {prev=$0}' <file>
```

## Environment

codescout MCP, `experiments` at `b59a035d`, this checkout. Observed 4/4 on `replace`; `edit`
did not reproduce it in 4 attempts.

## Root cause

**Read in the source 2026-09-17, and the boundary claim below was WRONG.**

`src/tools/markdown/edit_markdown.rs`, `plan_section_edit`. `compute_section_end` returns the
line index of the NEXT SIBLING HEADING, so a section's span runs up to that heading and the
blank line before it is *inside* the span. `replace` then plans
`span = line_start(heading_idx)..line_start(replace_end_idx)` and builds
`heading + separator + ensure_trailing_newline(new)` — which guarantees exactly ONE trailing
`\n`. The separator is emitted *before* the body and never *after* it, so the blank line the
span swallowed is not put back.

**It was never specific to `replace`.** Measured on 2026-09-16 and confirmed by unit test:
`insert_before` and `insert_after` + `at: "end-of-section"` exhibit it too, by a different
route — they splice AT the boundary, so the document's blank line ends up above the inserted
text and the inserted text butts against the heading. Nothing is destroyed there; the
separation is displaced. `remove` is the one arm whose author handled it
(`if lines[remove_end].trim().is_empty() { remove_end += 1 }`), and `edit` never exhibits it
because it routes through `perform_scoped_edit`, a substring swap inside an assembled body
that never touches the boundary — which is what the earlier reading correctly observed and
then over-generalised into "specific to `replace`".

The superseded paragraph's inference — *"`replace` appears to trim trailing whitespace from
`content`"* — was also wrong in a way worth one line, because it points at the caller's bytes:
nothing trims the caller's content. The separator loss is entirely a property of the span and
the single-newline guarantee, which is why the fix keys on the DOCUMENT and not on what the
caller passed.

### A second defect at the same site, and it is not cosmetic

The F-3 horizontal-rule boundary loses its blank line too, and there the damage inverts a
meaning rather than a layout. In CommonMark a `---` line directly beneath paragraph text is a
**setext heading underline**, so `new A\n---` renders the last line of the new body as an H2.
F-3 exists to preserve that separator and was preserving its BYTES while changing what they
mean.

**Five existing tests could not see it, by construction.** Every HR case asserts
`result.contains("---")` — an existence assertion, monotone under widening. The bytes are all
present in the damaged output, so they pass either way. `CLAUDE.md` § *Testing Discipline*'s
first law, holding inside the suite that guards F-3.

## Evidence

Measured on `docs/issues/2026-09-15-file-provenance-reads-mv-inside-a-filename-and-attributes-a-write.md`,
2026-09-15, across two `doc(update)` calls carrying four `replace` edits between them.

**The control is what makes this a measurement rather than a complaint about a file that may
always have been malformed** — the committed version was checked and was clean:

```bash
git show "HEAD:$F" | awk 'NR>1 && /^## / && prev != "" {print NR": "$0} {prev=$0}'
# (no output — every heading in the committed file had its blank line)
```

So the joins were introduced by the writes, not inherited. Repairing them with `action: "edit"`
and a trailing `\n` in `new_string` worked on all four, which is the second half of the same
discrimination: the byte is representable, and `replace` is what drops it.

### Independently reproduced 2026-09-16 — published because a confirmation is a DENOMINATOR

Not a second catch, and recorded as such. Session `a3bf229c` hit this while fixing the
mutation-probe non-cargo-runner bug: one `doc(action="update")` call carrying two
`body_edits` entries with `action: "replace"` (`## Fix`, `## Tests`), after which **both**
following headings — `## Tests` and `## References` — had lost their preceding blank line.
The replaced sections' own content was intact and the heading map read back correctly, so
neither the response nor a re-read named it; `cat -A` on the boundary did.

Two details this adds rather than restates. It fires **per replaced section**, not once per
call — two replaces produced two damaged boundaries in a single write. And `action: "edit"`
with `old_string`/`new_string` does **not** exhibit it: the repair was two `edit` calls
appending a newline, which makes the workaround this file already names a confirmed one
rather than an assumed one.

**Extended the same evening, and one half of the above was too narrow.** `action:
"insert_after"` with `at: "end-of-section"` exhibits it as well — two such inserts, into two
different bug files, each cost the blank line before the NEXT `##` heading. So the subject is
not `replace`: it is any body write that ends a section, and a reader who took "replace" as the
trigger would not check after an insert.

**And it accumulates unnoticed, which is the part worth acting on.** Scanning one of those files
before editing it found **two** boundaries already damaged — `## Fix` and `## Resume` — present
in the committed bytes at `HEAD`, from earlier writes by other sessions. Nothing had reported
them, and a later reader would have attributed them to whoever touched the file last. Both were
repaired in the same commit as the instance that found them.

The discriminator is one line, needs no tooling, and is worth running after any `doc` body write.
**It must be fence-aware, and the first version published here was not** — see below for why that
is this corpus's own parser law rather than a typo:

```
awk '/^```/{f=!f} !f && prev !~ /^$/ && /^#/ {print "MISSING BLANK BEFORE "NR": "$0} {prev=$0}' <file>
```

**The fence-blind version reported two findings on THIS FILE, and both were false.** Line 33 is
the `## Root cause` heading inside the fenced block above — this file quoting its own damage —
and line 77 is a `#` shell comment inside a fenced `bash` example. A document about a heading
defect cannot exhibit a damaged heading without a heading scanner reading the exhibit as real.
That is `CLAUDE.md` § *Parsers Over a Namespace* holding about the detector for the very bug
this file records, and the fence is the escape the corpus already has: honour it, or the
instrument is loudest exactly on the files that document the problem.

**Prevalence, measured with the corrected form at `HEAD` on 2026-09-16.** `CLAUDE.md` alone
carries **11** damaged `##` boundaries, none of them in a fence and none written by the session
that counted them. Reported as an observation, not a diagnosis: this detector reads a boundary
and cannot attribute a cause, so nothing here claims all 11 came from `doc`. They were left
unrepaired deliberately — an 11-hunk sweep of the hottest shared file on a checkout with live
peers costs more than the defect does.

The limit of the check, stated because it is an ABSENCE assertion: it is monotone under any
change that removes headings entirely, and it is a boundary check rather than a content one.


## Hypotheses tried

1. **The file was already malformed.** **Refuted** by the `git show HEAD` control above.
2. **Trailing newline in `content` was simply omitted by the caller.** **Refuted** — the
   `## Root cause` replace passed `content` ending in `\n` explicitly and still lost it.

## Fix

**Fixed in `976d8bac`** — patch-id `2e217cff164a3e316028625337719f4dc1fed8f3`. Gate green:
FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0.

**Shipped 2026-09-17.** One rule in `plan_section_edit`, applied at three sites: count the blank
lines immediately preceding the boundary in the ORIGINAL document (`blank_run_before`) and make
the planned replacement end with that many (`ensure_trailing_blank_lines`).

**PRESERVATION, not the normalisation this section originally recommended.** The earlier text
argued for emitting the separator unconditionally, on the grounds that it is a property of the
document rather than of the section's content. The first half of that is right and is exactly
what the fix keys on — but unconditional emission would also rewrite documents that never had a
separator, and the defect is DESTRUCTION, not absence. A formatting opinion does not belong in
an edit the caller did not ask for. A compact document yields `0` and is returned byte-identical,
which is why this repo's compact-markdown fixtures stayed green across the change: that they did
is a property of the fix, not a coincidence to be read as weak coverage.

Sites: `replace` (restores what the span destroyed), `insert_before` and `insert_after` +
`end-of-section` (re-establish separation the splice displaced). The `replace` arm covers the
heading boundary AND the F-3 horizontal-rule boundary — `replace_end_idx < lines.len()` rather
than a heading-only test, because of the setext reading above.

Not touched: `at: "after-heading-line"`, whose boundary is the first body line rather than a
heading, so `blank_run_before` returns 0 there by construction and the arm is a no-op. Whether a
blank line belongs immediately AFTER a heading is a different axis and is not addressed here.

## Tests added

Four, in `src/tools/markdown/tests.rs`, each written before the fix and each observed RED with
the exact missing `\n` in its diff:

| test | site | pre-fix red |
|---|---|---|
| `replace_preserves_the_blank_line_before_the_next_heading` | `replace`, heading boundary | `new content\n## Usage` |
| `insert_before_keeps_the_new_section_separated_from_the_target` | `insert_before` | `words\n## Setup` |
| `insert_after_keeps_the_new_section_separated_from_the_following_heading` | `insert_after` | `more\n## Usage` |
| `replace_preserves_the_blank_line_before_a_trailing_hr_separator` | `replace`, F-3 HR boundary | `new A\n---` |

One per guarded SITE rather than one for the feature, and the reds are per-site evidence: each
assertion produced its own red and none has been edited since, so no red here is being credited
to an assertion that did not earn it.

**The fixture detail is load-bearing and annotated on the fixtures.** Every pre-existing fixture
in that file is compact markdown — `"# Title\n## Setup\nold content\n"` — with no blank line
anywhere, so the separator question CANNOT ARISE in them and all 193 passed throughout the
defect's life. That is not carelessness; it is a population selected, years of commits ago, so
that no member could falsify. The new tests exist to carry the blank lines, and tidying them out
would leave the tests green and no longer discriminating.

**The control this section originally asked for is now structural rather than a test.** It
proposed an `edit`-action case "so a fix that makes both paths equally wrong cannot pass". The
fix does not touch the `edit` path at all — `edit` routes through `perform_scoped_edit`, a
different function — so that failure mode is unreachable by construction here. The 193 compact
fixtures serve the same purpose in the other direction: they pin that the fix is preservation
rather than normalisation.

## Verified on the LIVE server, 2026-09-17

The fix was green in unit tests at archive time, and that is a claim about the
compiled-from-source path, not about the tool any session actually calls. The live MCP
binary is `target/release/codescout` behind a `~/.cargo/bin` symlink, so the fix reached
no running session until someone ran `cargo rb` and reconnected with `/mcp`. That happened
at 09:56:12 on 2026-09-17; this is the first measurement through the served surface.

Probe — a fixture in the discriminating shape, i.e. one carrying the blank lines the
pre-existing test fixtures did not have (compact markdown is why the defect survived a
passing suite in the first place):

```
# Title

## Setup

old content

## Next

tail
```

`edit_file(heading="## Setup", action="replace", body="rewritten content")` through the
live server, then `cat -A`:

```
rewritten content$
$
## Next$
```

**PASS** — the blank line before the next heading survives. `edit_file`'s heading grammar
and `doc(update, patch={body_edits})` both route through `perform_section_edit_ext` ->
`plan_section_edit`, which is the repaired function, so this exercises the reported surface
rather than a sibling of it.

**A note on the verdict, because it nearly went the other way.** The first check was
`grep -qz 'rewritten content\n\n## Next'`, which printed `FAIL`. grep does not interpret
`\n` in that pattern and said so — `warning: stray \ before n` — while the `cat -A` output
immediately above it showed the blank line plainly present. The instrument was broken, not
the fix, and the failing verdict line was the more legible of the two outputs. Recorded
here rather than dropped: this file's own defect was a silent write, and a probe for it
that reports a confident false FAIL is the same failure shape pointed the other way.

Binary freshness confirmed two ways before trusting any of this — `readlink /proc/<pid>/exe`
on the serving process (not a deleted inode), and `scripts/peer-sessions.sh` reporting this
session as `cs current`. Both were worth doing: the same run showed **15 of 19 live sessions
on a REPLACED binary**, so "the fix is in the tree" and "the fix is in the server you are
talking to" are different facts.

## Workarounds

Use `action: "edit"` with `old_string`/`new_string` when the section's tail matters, and run
the `awk` detector above after any batch of `replace` edits. Repairing after the fact works:
an `edit` whose `new_string` appends `\n` to the section's final line restores the separator.

## Severity note

**Low, and the reason is worth stating so nobody re-raises it as urgent.** CommonMark lets an
ATX heading interrupt a paragraph, so the rendered output is unchanged — this is a silent
mutation of stored bytes, not a rendering defect. It matters because it is silent and because
it accumulates: every `replace` on a document removes one more separator, and no reader of the
response has any signal that it happened.

## Classification

`cluster/accepted-parameter-silently-dropped` (`IC-15`). The caller's `content` — specifically
its trailing newline — is accepted at the boundary, the call succeeds, and the byte is
discarded downstream. `updated: true` is the only feedback, and it is identical whether the
newline was applied or dropped, which is that class's claim exactly.

## Resume

Read `replace`'s section-assembly path in the librarian body-edit code and confirm or refute
the inferred mechanism above before changing anything — the Root cause here is explicitly
marked inferred, and this repo has a standing rule that an unmeasured mechanism is a
hypothesis wearing a conclusion's clothes.

## References

- `docs/issues/archive/2026-09-15-file-provenance-reads-mv-inside-a-filename-and-attributes-a-write.md` (`76c83a43c2d1752f`) — the artifact this was found on, while editing it.
